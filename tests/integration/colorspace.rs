//! Pixel-level tests for drawing a semi-transparent gradient over a colored
//! background with an explicit working color space (`Paint::colorspace`).
//!
//! The working color space must drive *both* halves of this drawing path:
//!
//! 1. interpolation: gradient stops are mapped through the space's per-channel
//!    transfer (expansion) curve *before* they are interpolated;
//! 2. compositing: the destination is expanded, source-over blending happens
//!    in the working (linear) space and the result is compressed back.
//!
//! Expected pixels are derived analytically from the per-channel transfer
//! curves and the shader/layer opacity, mirroring the high-quality raster
//! pipeline stage by stage:
//!
//!   expand(stops) -> interpolate -> premultiply
//!     -> expand(dst) -> source-over blend -> compress -> store
//!
//! All draws go through `fill_rect` with a fully opaque clip mask. The mask
//! routes every pixel (not just anti-aliased edges) through the
//! `LoadDestination -> expand -> blend -> compress` pipeline, which is what
//! makes the compositing step colorspace-aware. `force_hq_pipeline` keeps the
//! math in f32 for every color space, including `Linear`.

use tiny_skia::*;

const WIDTH: u32 = 256;
const HEIGHT: u32 = 4;

/// Rendering tolerance, in u8 units, between the analytic model and the
/// renderer. Absorbs f32 rounding-order differences and the round-to-nearest
/// behavior of the final `store` stage.
const TOLERANCE: i32 = 1;

const ALL_SPACES: [ColorSpace; 4] = [
    ColorSpace::Linear,
    ColorSpace::Gamma2,
    ColorSpace::SimpleSRGB,
    ColorSpace::FullSRGBGamma,
];

/// Gradient positions sampled by the tests: both ends, the middle and
/// a few points in between.
const XS: [u32; 9] = [0, 32, 64, 96, 128, 160, 192, 224, 255];

// ---------------------------------------------------------------------------
// Analytic model of the drawing path
// ---------------------------------------------------------------------------

/// A bit-exact copy of `src/math.rs::approx_powf`, which backs both the scalar
/// (`ColorSpace::expand_channel`) and the SIMD (`f32x8::powf`) transfer curves.
/// Replicating it keeps the expected values within `TOLERANCE` of the renderer
/// even for the pow-based spaces.
fn approx_powf(x: f32, y: f32) -> f32 {
    if x == 0.0 || x == 1.0 {
        return x;
    }

    let e = x.to_bits() as f32 * (1.0f32 / ((1 << 23) as f32));
    let m = f32::from_bits((x.to_bits() & 0x007fffff) | 0x3f000000);

    let log2_x =
        e - 124.225514990f32 - 1.498030302f32 * m - 1.725879990f32 / (0.3520887068f32 + m);

    let x = log2_x * y;

    let f = x - x.floor();

    let mut a = x + 121.274057500f32;
    a -= f * 1.490129070f32;
    a += 27.728023300f32 / (4.84252568f32 - f);
    a *= (1 << 23) as f32;

    if a < f32::INFINITY.to_bits() as f32 {
        if a > 0.0 {
            f32::from_bits(a.round() as u32)
        } else {
            0.0
        }
    } else {
        f32::INFINITY
    }
}

/// Encoded -> working space, one channel. Mirrors `ColorSpace::expand_channel`.
fn expand_channel(cs: ColorSpace, x: f32) -> f32 {
    match cs {
        ColorSpace::Linear => x,
        ColorSpace::Gamma2 => x * x,
        ColorSpace::SimpleSRGB => approx_powf(x, 2.2).clamp(0.0, 1.0),
        ColorSpace::FullSRGBGamma => {
            let x = if x <= 0.04045 {
                x / 12.92
            } else {
                approx_powf((x + 0.055) / 1.055, 2.4)
            };
            x.clamp(0.0, 1.0)
        }
    }
}

/// Working space -> encoded, one channel. Mirrors `ColorSpace::compress_channel`.
fn compress_channel(cs: ColorSpace, x: f32) -> f32 {
    match cs {
        ColorSpace::Linear => x,
        ColorSpace::Gamma2 => x.sqrt().clamp(0.0, 1.0),
        ColorSpace::SimpleSRGB => approx_powf(x, 0.45454545).clamp(0.0, 1.0),
        ColorSpace::FullSRGBGamma => {
            let x = if x <= 0.0031308 {
                x * 12.92
            } else {
                approx_powf(x, 0.416666666) * 1.055 - 0.055
            };
            x.clamp(0.0, 1.0)
        }
    }
}

/// The pipeline's f32 -> u8 conversion (`unnorm` in the `store` stage):
/// clamp to 0..=1, scale by 255, round to nearest.
fn to_u8(x: f32) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Mirrors the compositing tail of the high-quality pipeline:
/// the destination is loaded as premultiplied u8, expanded per channel,
/// blended source-over in the working space and compressed back.
/// `src` is a premultiplied color in the working space.
fn composite_in_working_space(cs: ColorSpace, src: [f32; 4], dst: [u8; 4]) -> [u8; 4] {
    // `load_8888` multiplies by 1/255 rather than dividing by 255.
    let d = [
        dst[0] as f32 * (1.0 / 255.0),
        dst[1] as f32 * (1.0 / 255.0),
        dst[2] as f32 * (1.0 / 255.0),
        dst[3] as f32 * (1.0 / 255.0),
    ];
    // `GammaExpandDestination*` touches RGB only; alpha is passed through.
    let d = [
        expand_channel(cs, d[0]),
        expand_channel(cs, d[1]),
        expand_channel(cs, d[2]),
        d[3],
    ];

    // `source_over`: s + d * (1 - sa), per channel, alpha included.
    let inv_sa = 1.0 - src[3];
    let out = [
        d[0] * inv_sa + src[0],
        d[1] * inv_sa + src[1],
        d[2] * inv_sa + src[2],
        d[3] * inv_sa + src[3],
    ];

    // `GammaCompress*` touches RGB only; alpha is stored as is.
    [
        to_u8(compress_channel(cs, out[0])),
        to_u8(compress_channel(cs, out[1])),
        to_u8(compress_channel(cs, out[2])),
        to_u8(out[3]),
    ]
}

/// Expected pixel of a two-stop gradient draw at gradient position `t`.
///
/// `c0`/`c1` are the stop colors (unpremultiplied, 0..=1, layer opacity
/// already applied to their alpha). Mirrors:
/// expand stops -> `EvenlySpaced2StopGradient` -> `Premultiply` -> composite.
fn expected_gradient_pixel(
    cs: ColorSpace,
    c0: [f32; 4],
    c1: [f32; 4],
    t: f32,
    dst: [u8; 4],
) -> [u8; 4] {
    // Stops are expanded per channel *before* interpolation; alpha is not expanded.
    let e0 = [
        expand_channel(cs, c0[0]),
        expand_channel(cs, c0[1]),
        expand_channel(cs, c0[2]),
        c0[3],
    ];
    let e1 = [
        expand_channel(cs, c1[0]),
        expand_channel(cs, c1[1]),
        expand_channel(cs, c1[2]),
        c1[3],
    ];

    // Interpolation happens in the working space: bias + factor * t.
    let mut c = [0.0; 4];
    for i in 0..4 {
        c[i] = t * (e1[i] - e0[i]) + e0[i];
    }

    // `Premultiply` runs after the interpolation, in the working space.
    c[0] *= c[3];
    c[1] *= c[3];
    c[2] *= c[3];

    composite_in_working_space(cs, c, dst)
}

/// Expected pixel of a semi-transparent solid color draw.
/// Mirrors the `SolidColor` shader stage: expand -> premultiply -> composite.
fn expected_solid_pixel(cs: ColorSpace, color: [f32; 4], dst: [u8; 4]) -> [u8; 4] {
    let e = [
        expand_channel(cs, color[0]),
        expand_channel(cs, color[1]),
        expand_channel(cs, color[2]),
        color[3],
    ];
    let src = [e[0] * e[3], e[1] * e[3], e[2] * e[3], e[3]];
    composite_in_working_space(cs, src, dst)
}

// ---------------------------------------------------------------------------
// Drawing helpers
// ---------------------------------------------------------------------------

fn rgba8(c: [u8; 4]) -> Color {
    Color::from_rgba8(c[0], c[1], c[2], c[3])
}

fn normalized(c: [u8; 4]) -> [f32; 4] {
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        c[3] as f32 / 255.0,
    ]
}

fn pixel_rgba(pixmap: &Pixmap, x: u32) -> [u8; 4] {
    let px = pixmap.pixel(x, HEIGHT / 2).unwrap();
    [px.red(), px.green(), px.blue(), px.alpha()]
}

fn max_channel_diff(a: [u8; 4], b: [u8; 4]) -> i32 {
    (0..4)
        .map(|i| (a[i] as i32 - b[i] as i32).abs())
        .max()
        .unwrap()
}

/// Draws a full-canvas rect with `shader` over a solid background, composited
/// through the working color space (fully opaque clip mask, hq pipeline).
fn draw(cs: ColorSpace, shader: Shader, bg: [u8; 4]) -> Pixmap {
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).unwrap();
    pixmap.fill(rgba8(bg));

    let paint = Paint {
        shader,
        blend_mode: BlendMode::SourceOver,
        anti_alias: false,
        colorspace: cs,
        force_hq_pipeline: true,
    };

    let mut mask = Mask::new(WIDTH, HEIGHT).unwrap();
    mask.data_mut().fill(255);

    let rect = Rect::from_xywh(0.0, 0.0, WIDTH as f32, HEIGHT as f32).unwrap();
    pixmap.fill_rect(rect, &paint, Transform::identity(), Some(&mask));
    pixmap
}

/// A horizontal two-stop gradient spanning the whole canvas, with an optional
/// layer opacity applied on top of the stop colors.
fn horizontal_gradient(c0: [u8; 4], c1: [u8; 4], opacity: f32) -> Shader<'static> {
    let mut shader = LinearGradient::new(
        Point::from_xy(0.0, 0.0),
        Point::from_xy(WIDTH as f32, 0.0),
        vec![
            GradientStop::new(0.0, rgba8(c0)),
            GradientStop::new(1.0, rgba8(c1)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap();
    shader.apply_opacity(opacity);
    shader
}

/// Gradient position sampled at the center of pixel `x` (`SeedShader` emits
/// pixel centers and the gradient maps 0..WIDTH to 0..1).
fn t_at(x: u32) -> f32 {
    (x as f32 + 0.5) / WIDTH as f32
}

fn assert_pixel_close(pixmap: &Pixmap, x: u32, expected: [u8; 4], context: &str) {
    let actual = pixel_rgba(pixmap, x);
    assert!(
        max_channel_diff(actual, expected) <= TOLERANCE,
        "{}: pixel at x={} differs: actual {:?}, expected {:?}",
        context,
        x,
        actual,
        expected
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Colors between the stops must be interpolated in the working color space:
/// the same semi-transparent gradient over the same colored background must
/// produce the pixels dictated by each space's per-channel transfer curves.
#[test]
fn gradient_interpolation_follows_working_colorspace() {
    const BG: [u8; 4] = [30, 120, 200, 255];
    const C0: [u8; 4] = [255, 80, 0, 200];
    const C1: [u8; 4] = [0, 60, 255, 100];

    for &cs in &ALL_SPACES {
        let pixmap = draw(cs, horizontal_gradient(C0, C1, 1.0), BG);
        for &x in &XS {
            let expected =
                expected_gradient_pixel(cs, normalized(C0), normalized(C1), t_at(x), BG);
            assert_pixel_close(&pixmap, x, expected, &format!("{:?}", cs));
        }
    }

    // The expectations of any two distinct spaces must disagree somewhere by
    // more than the rendering tolerance, otherwise this test could not detect
    // interpolation happening in the wrong space.
    for (i, &cs_a) in ALL_SPACES.iter().enumerate() {
        for &cs_b in &ALL_SPACES[i + 1..] {
            let max_diff = XS
                .iter()
                .map(|&x| {
                    let a = expected_gradient_pixel(cs_a, normalized(C0), normalized(C1), t_at(x), BG);
                    let b = expected_gradient_pixel(cs_b, normalized(C0), normalized(C1), t_at(x), BG);
                    max_channel_diff(a, b)
                })
                .max()
                .unwrap();
            assert!(
                max_diff > TOLERANCE,
                "{:?} and {:?} predictions are indistinguishable; \
                 the test cannot detect a wrong-space regression",
                cs_a,
                cs_b
            );
        }
    }
}

/// The compositing step itself must happen in the working color space:
/// a constant semi-transparent source (no interpolation involved) blended
/// over a colored background must match expand(dst) -> blend -> compress.
#[test]
fn compositing_follows_working_colorspace() {
    const BG: [u8; 4] = [30, 120, 200, 255];
    const COLOR: [u8; 4] = [250, 200, 60, 160];

    for &cs in &ALL_SPACES {
        let pixmap = draw(cs, Shader::SolidColor(rgba8(COLOR)), BG);
        let expected = expected_solid_pixel(cs, normalized(COLOR), BG);
        // A constant source must produce the same pixel everywhere.
        for x in 0..WIDTH {
            assert_pixel_close(&pixmap, x, expected, &format!("{:?}", cs));
        }

        // Compositing in the encoded space (i.e. what the `Linear` model
        // describes) is the regression this test guards against; the
        // non-linear spaces must land clearly away from it.
        if cs != ColorSpace::Linear {
            let linear = expected_solid_pixel(ColorSpace::Linear, normalized(COLOR), BG);
            assert!(
                max_channel_diff(expected, linear) > 4,
                "{:?}: compositing prediction is too close to the encoded-space one",
                cs
            );
        }
    }
}

/// A gradient whose one end fades to transparent (alpha varies along the
/// gradient) is not the same as a uniformly faded layer (constant color at
/// half layer opacity): they only coincide at the gradient midpoint, where
/// both have an effective alpha of 0.5.
#[test]
fn fading_end_differs_from_layer_opacity() {
    const BG: [u8; 4] = [40, 90, 160, 255];
    const C: [u8; 4] = [250, 160, 40, 255];

    let c_opaque = normalized(C);
    let c_transparent = [c_opaque[0], c_opaque[1], c_opaque[2], 0.0];
    let c_half = [c_opaque[0], c_opaque[1], c_opaque[2], c_opaque[3] * 0.5];

    for &cs in &ALL_SPACES {
        // One end gradually transparent: alpha goes 1 -> 0 along the gradient.
        let fade = draw(
            cs,
            horizontal_gradient(C, [C[0], C[1], C[2], 0], 1.0),
            BG,
        );
        // The whole layer faded: constant opaque color at 0.5 layer opacity.
        let layer = draw(cs, horizontal_gradient(C, C, 0.5), BG);

        // Both must follow the analytic model built from the transfer curves
        // and the layer opacity.
        for &x in &XS {
            let t = t_at(x);
            assert_pixel_close(
                &fade,
                x,
                expected_gradient_pixel(cs, c_opaque, c_transparent, t, BG),
                &format!("{:?} fade", cs),
            );
            assert_pixel_close(
                &layer,
                x,
                expected_gradient_pixel(cs, c_half, c_half, t, BG),
                &format!("{:?} layer", cs),
            );
        }

        // A uniformly faded layer is constant across the whole gradient.
        assert_eq!(pixel_rgba(&layer, 0), pixel_rgba(&layer, WIDTH - 1));

        // A fading end is not: its red channel drops monotonically towards
        // the background as the source becomes transparent.
        let r_start = pixel_rgba(&fade, 0)[0] as i32;
        let r_mid = pixel_rgba(&fade, WIDTH / 2)[0] as i32;
        let r_end = pixel_rgba(&fade, WIDTH - 1)[0] as i32;
        assert!(
            r_start > r_mid + 4 && r_mid > r_end + 4,
            "{:?}: fading gradient must vary along its length: {} -> {} -> {}",
            cs,
            r_start,
            r_mid,
            r_end
        );

        // The two effects coincide at the midpoint (effective alpha 0.5 in
        // both cases) but diverge towards the ends.
        assert!(
            max_channel_diff(pixel_rgba(&fade, WIDTH / 2), pixel_rgba(&layer, WIDTH / 2))
                <= TOLERANCE,
            "{:?}: fade and layer opacity must coincide at the gradient midpoint",
            cs
        );
        for &x in &[32, 224] {
            assert!(
                max_channel_diff(pixel_rgba(&fade, x), pixel_rgba(&layer, x)) > 8,
                "{:?}: fading end and layer opacity must differ at x={}",
                cs,
                x
            );
        }
    }
}
