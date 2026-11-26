// Integration tests for tiled (repeating) pattern sampling.
//
// Unlike `pattern.rs`, which compares against golden PNGs, these tests derive
// every expected color analytically from the sampling math, so the
// expectations stand on their own:
//
// * The shader seeds the pipeline with *pixel-center* coordinates
//   `(dx + 0.5, dy + 0.5)` and then applies the inverse pattern transform,
//   i.e. the local texture coordinate is `local = inverse(M) * (device + 0.5)`.
// * Nearest neighbor truncates the local coordinate; the address mode decides
//   which texel an out-of-range (wrapped / mirrored / clamped) coordinate maps to.
// * Bilinear filtering places the 2x2 footprint at `local +/- 0.5`, therefore a
//   sample whose local coordinate sits on an integer (a texel *boundary*) mixes
//   the two adjacent texels 50/50, while a coordinate on a half-integer (a texel
//   *center*) reproduces a single texel. Across a seam the footprint reaches into
//   the neighboring tile, and the address mode decides what is found there.
//
// The tile is a small 4x4 pixmap of fully opaque, uniquely colored texels:
//
//   R(x) = 40 + 50 * x          (40, 90, 140, 190)
//   G(y) = 30 + 60 * y          (30, 90, 150, 210)
//   B(x, y) = 10 + 12 * (4x + y)
//
// Each channel therefore encodes a different coordinate, which makes it possible
// to tell exactly which texel (or pair of texels) a pixel was read from.

use tiny_skia::*;

const TILE: u32 = 4;
const CANVAS: u32 = 8;

#[derive(Clone, Copy, PartialEq, Debug)]
struct Rgb(u8, u8, u8);

// --- Analytic description of the test tile --------------------------------

fn texel_red(x: u32) -> u8 {
    40 + 50 * x as u8
}

fn texel_green(y: u32) -> u8 {
    30 + 60 * y as u8
}

fn texel_blue(x: u32, y: u32) -> u8 {
    10 + 12 * (4 * x + y) as u8
}

fn texel_rgb(x: u32, y: u32) -> Rgb {
    Rgb(texel_red(x), texel_green(y), texel_blue(x, y))
}

fn make_tile() -> Pixmap {
    let mut pixmap = Pixmap::new(TILE, TILE).unwrap();
    for y in 0..TILE {
        for x in 0..TILE {
            let color = Color::from_rgba8(texel_red(x), texel_green(y), texel_blue(x, y), 255);
            pixmap.pixels_mut()[(y * TILE + x) as usize] =
                color.premultiply().to_color_u8();
        }
    }
    pixmap
}

// --- Rasterization helper --------------------------------------------------

fn render_tile(
    tile: &Pixmap,
    mode: SpreadMode,
    quality: FilterQuality,
    pattern_transform: Transform,
) -> Pixmap {
    let mut paint = Paint::default();
    // No anti-aliasing: every covered pixel must carry the raw shader output.
    paint.anti_alias = false;
    paint.shader = Pattern::new(
        tile.as_ref(),
        mode,
        quality,
        1.0,
        pattern_transform,
    );

    let mut pixmap = Pixmap::new(CANVAS, CANVAS).unwrap();
    pixmap.fill_rect(
        Rect::from_xywh(0.0, 0.0, CANVAS as f32, CANVAS as f32).unwrap(),
        &paint,
        Transform::identity(),
        None,
    );
    pixmap
}

fn pixel_rgb(pixmap: &Pixmap, x: u32, y: u32) -> Rgb {
    let p = pixmap.pixel(x, y).unwrap();
    assert_eq!(p.alpha(), 255, "the tile is opaque, every sample must be opaque");
    Rgb(p.red(), p.green(), p.blue())
}

// --- Independent scalar reference implementation ---------------------------
//
// This mirrors the pipeline stages (`SeedShader`, inverse transform,
// `exclusive_repeat` / `exclusive_reflect` / clamp, `Gather` and the 2x2
// bilinear `sampler_2x2`) using plain `f32` arithmetic. It is written from the
// math rather than copied from the pipeline, and serves as an independent
// oracle for whole-canvas comparisons.

fn tile_coord(v: f32, mode: SpreadMode) -> f32 {
    let limit = TILE as f32;
    match mode {
        SpreadMode::Pad => v,
        // v - floor(v / limit) * limit
        SpreadMode::Repeat => v - (v / limit).floor() * limit,
        // |(v - limit) - 2*limit*floor((v - limit) / (2*limit)) - limit|
        SpreadMode::Reflect => {
            let z = v - limit;
            (z - 2.0 * limit * (z / (2.0 * limit)).floor() - limit).abs()
        }
    }
}

// One ULP below `v`, mirroring the pipeline's exclusive->inclusive limit
// adjustment in `gather_ix`.
fn ulp_sub(v: f32) -> f32 {
    f32::from_bits(v.to_bits() - 1)
}

fn nearest_index(v: f32, mode: SpreadMode) -> usize {
    // `gather_ix` hard-clamps to [0, width - 1 ULP] *after* tiling (the Pad path
    // relies on this clamp for out-of-range coordinates).
    let upper = ulp_sub(TILE as f32);
    let v = tile_coord(v, mode).clamp(0.0, upper);
    v.trunc() as usize
}

fn reference_nearest(
    device_x: u32,
    device_y: u32,
    mode: SpreadMode,
    pattern_transform: Transform,
) -> Rgb {
    let inverse = pattern_transform.invert().unwrap();
    let dx = device_x as f32 + 0.5;
    let dy = device_y as f32 + 0.5;
    let local_x = inverse.sx * dx + inverse.kx * dy + inverse.tx;
    let local_y = inverse.ky * dx + inverse.sy * dy + inverse.ty;

    let x = nearest_index(local_x, mode);
    let y = nearest_index(local_y, mode);
    texel_rgb(x as u32, y as u32)
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    ((a as f32) + (b as f32 - a as f32) * t).round() as u8
}

fn mix_rgb(a: Rgb, b: Rgb, t: f32) -> Rgb {
    Rgb(
        lerp_u8(a.0, b.0, t),
        lerp_u8(a.1, b.1, t),
        lerp_u8(a.2, b.2, t),
    )
}

fn reference_bilinear(
    device_x: u32,
    device_y: u32,
    mode: SpreadMode,
    pattern_transform: Transform,
) -> Rgb {
    let inverse = pattern_transform.invert().unwrap();
    let dx = device_x as f32 + 0.5;
    let dy = device_y as f32 + 0.5;
    let cx = inverse.sx * dx + inverse.kx * dy + inverse.tx;
    let cy = inverse.ky * dx + inverse.sy * dy + inverse.ty;

    // Fractional weight between the left/top and right/bottom texel.
    let fx = (cx + 0.5).fract();
    let fy = (cy + 0.5).fract();
    let wx = [1.0 - fx, fx];
    let wy = [1.0 - fy, fy];

    // Same accumulation structure as the pipeline's `sampler_2x2`: each of the
    // four footprint corners contributes `weight * texel`, and rounding happens
    // once when the result is stored -- no intermediate per-row rounding.
    let mut r = 0.0f32;
    let mut g = 0.0f32;
    let mut b = 0.0f32;
    for j in 0..2 {
        let sample_y = cy - 0.5 + j as f32;
        let iy = nearest_index(sample_y, mode);
        for i in 0..2 {
            let sample_x = cx - 0.5 + i as f32;
            let ix = nearest_index(sample_x, mode);
            let corner = texel_rgb(ix as u32, iy as u32);
            let weight = wx[i] * wy[j];
            r += corner.0 as f32 * weight;
            g += corner.1 as f32 * weight;
            b += corner.2 as f32 * weight;
        }
    }

    Rgb(r.round() as u8, g.round() as u8, b.round() as u8)
}

fn assert_canvas_matches_reference<F>(pixmap: &Pixmap, mode: SpreadMode, transform: Transform, reference: F)
where
    F: Fn(u32, u32, SpreadMode, Transform) -> Rgb,
{
    for y in 0..CANVAS {
        for x in 0..CANVAS {
            let expected = reference(x, y, mode, transform);
            let actual = pixel_rgb(pixmap, x, y);
            // One unit of slack for differences in the order of f32 operations
            // between scalar code and the 8-lane SIMD pipeline.
            for (name, e, a) in [
                ("r", expected.0, actual.0),
                ("g", expected.1, actual.1),
                ("b", expected.2, actual.2),
            ] {
                assert!(
                    e.abs_diff(a) <= 1,
                    "pixel ({x}, {y}) channel {name}: expected ~{e}, got {a}"
                );
            }
        }
    }
}

// ===========================================================================
// Nearest-neighbor: expectations follow texel centers + the address mode.
// ===========================================================================

#[test]
fn nearest_repeat_wraps_across_origin() {
    let tile = make_tile();
    let transform = Transform::identity();
    let pixmap = render_tile(&tile, SpreadMode::Repeat, FilterQuality::Nearest, transform);

    // With an identity pattern transform the local coordinate of device pixel
    // (x, y) is (x + 0.5, y + 0.5); truncation selects texel (x % 4, y % 4).
    // The second tile period (x >= 4) must read exactly the same texels as the
    // first one, proving the brush samples past the texture origin.
    for y in 0..CANVAS {
        for x in 0..CANVAS {
            assert_eq!(
                pixel_rgb(&pixmap, x, y),
                texel_rgb(x % TILE, y % TILE),
                "pixel ({x}, {y}) broke the repeat period"
            );
        }
    }

    // Explicit seam check: the pixels on either side of a tile boundary read the
    // last texel of one tile and the first texel of the next.
    assert_eq!(pixel_rgb(&pixmap, 3, 0), texel_rgb(3, 0));
    assert_eq!(pixel_rgb(&pixmap, 4, 0), texel_rgb(0, 0));
    assert_eq!(pixel_rgb(&pixmap, 7, 0), texel_rgb(3, 0));

    assert_canvas_matches_reference(&pixmap, SpreadMode::Repeat, transform, reference_nearest);
}

#[test]
fn nearest_repeat_negative_coordinates_wrap_to_opposite_edge() {
    let tile = make_tile();
    // Pattern-space translation by (1, 2). The inverse is (-1, -2), so the local
    // coordinate of device pixel (x, y) is (x - 0.5, y - 1.5): the left column
    // and top rows genuinely sample *negative* local coordinates, i.e. the brush
    // reaches past the texture origin.
    let transform = Transform::from_translate(1.0, 2.0);
    let pixmap = render_tile(&tile, SpreadMode::Repeat, FilterQuality::Nearest, transform);

    // Device (0, 0): local (-0.5, -1.5). Repeat wraps -0.5 -> 3.5 and -1.5 ->
    // 2.5, so the pixel reads texel (3, 2) from the opposite edges of the tile.
    assert_eq!(pixel_rgb(&pixmap, 0, 0), texel_rgb(3, 2));
    // Device (1, 0): local (0.5, -1.5) -> x stays 0, y wraps to 2.
    assert_eq!(pixel_rgb(&pixmap, 1, 0), texel_rgb(0, 2));
    // Device (0, 1): local (-0.5, -0.5) -> wraps to texel (3, 3).
    assert_eq!(pixel_rgb(&pixmap, 0, 1), texel_rgb(3, 3));
    // One tile period later the pattern must repeat identically, including
    // pixels reached through the negative-coordinate wrap.
    assert_eq!(pixel_rgb(&pixmap, 4, 0), pixel_rgb(&pixmap, 0, 0));
    assert_eq!(pixel_rgb(&pixmap, 0, 4), pixel_rgb(&pixmap, 0, 0));

    assert_canvas_matches_reference(&pixmap, SpreadMode::Repeat, transform, reference_nearest);
}

#[test]
fn nearest_reflect_mirrors_at_the_seam() {
    let tile = make_tile();
    let transform = Transform::from_translate(1.0, 2.0);
    let pixmap = render_tile(&tile, SpreadMode::Reflect, FilterQuality::Nearest, transform);

    // Reflection of a negative coordinate bounces off the origin, whereas the
    // repeat test at the same device pixel (0, 0) read texel (3, 2): the two
    // address modes are distinguishable from the imaged color alone.
    // local -0.5 mirrors to 0.5 (texel 0), local -1.5 mirrors to 1.5 (texel 1).
    // Device (0, 0): local (-0.5, -1.5) -> texel (0, 1).
    assert_eq!(pixel_rgb(&pixmap, 0, 0), texel_rgb(0, 1));
    // Positive-side seam: device (5, 0) has local (4.5, -1.5); x = 4.5 mirrors
    // to 3.5 -> texel x = 3, y mirrors -1.5 -> 1.5 -> texel y = 1.
    assert_eq!(pixel_rgb(&pixmap, 5, 0), texel_rgb(3, 1));
    // Device (0, 5): local (-0.5, 3.5) -> x mirrors to 0, y stays 3.
    assert_eq!(pixel_rgb(&pixmap, 0, 5), texel_rgb(0, 3));

    assert_canvas_matches_reference(&pixmap, SpreadMode::Reflect, transform, reference_nearest);
}

#[test]
fn nearest_pad_clamps_outside_coordinates_to_edge_texels() {
    let tile = make_tile();
    let transform = Transform::from_translate(1.0, 2.0);
    let pixmap = render_tile(&tile, SpreadMode::Pad, FilterQuality::Nearest, transform);

    // Pad never wraps: negative local coordinates clamp to the origin edge
    // texels instead of wrapping. At (0, 0) repeat read texel (3, 2), proving
    // again that the address mode alone changes which color is imaged.
    // Device (0, 0): local (-0.5, -1.5) -> clamped to texel (0, 0).
    assert_eq!(pixel_rgb(&pixmap, 0, 0), texel_rgb(0, 0));
    // The whole left column samples a negative local x and must read the
    // left-edge column (red 40) regardless of row.
    for y in 0..CANVAS {
        assert_eq!(pixel_rgb(&pixmap, 0, y).0, texel_red(0), "y={y} must clamp to the left-edge column");
    }
    // The top two rows sample negative local y and must read the top-edge row
    // (green 30) regardless of column.
    for x in 0..CANVAS {
        assert_eq!(pixel_rgb(&pixmap, x, 0).1, texel_green(0), "x={x} must clamp to the top-edge row");
        assert_eq!(pixel_rgb(&pixmap, x, 1).1, texel_green(0), "x={x} must clamp to the top-edge row");
    }
    // Interior coordinates still sample normally: (3, 3) -> local (2.5, 1.5).
    assert_eq!(pixel_rgb(&pixmap, 3, 3), texel_rgb(2, 1));

    assert_canvas_matches_reference(&pixmap, SpreadMode::Pad, transform, reference_nearest);
}

// ===========================================================================
// Bilinear: fractional offsets mix the texels on both sides of a seam.
// ===========================================================================

// Pattern transform used by the bilinear tests:
//
//   M  = scale(2) then translate(-0.3, -0.7)
//   M^-1: local_x = 0.5 * (x + 0.5) + 0.15,  local_y = 0.5 * (y + 0.5) + 0.35
//
// It contains a scale, so the shader does *not* fall back to Nearest (which it
// does for identity / translate-only transforms).
fn bilinear_transform() -> Transform {
    Transform::from_row(2.0, 0.0, 0.0, 2.0, -0.3, -0.7)
}

#[test]
fn bilinear_repeat_halfway_across_seam_mixes_both_tiles() {
    let tile = make_tile();
    let transform = bilinear_transform();
    let pixmap = render_tile(&tile, SpreadMode::Repeat, FilterQuality::Bilinear, transform);

    // Device pixel (3, 1):
    //   local = (0.5*3.5 + 0.15, 0.5*1.5 + 0.35) = (1.9, 1.1)
    // The x footprint spans [1.4, 2.4], so fx = fract(2.4) = 0.4 and both x
    // neighbors are interior texels here. The interesting seam pixel is (7, 1):
    //   local_x = 0.5*7.5 + 0.15 = 3.9
    //   footprint x in [3.4, 4.4]: the right neighbor (4.4) wraps to texel 0 of
    //   the next tile, fx = fract(4.4) = 0.4.
    //   local_y = 1.1 -> fy = fract(1.6) = 0.6, neighbors y = 0 and y = 1.
    let left_x0 = texel_rgb(3, 0);
    let left_x1 = texel_rgb(3, 1);
    let right_x0 = texel_rgb(0, 0); // wrapped from the next tile
    let right_x1 = texel_rgb(0, 1);

    let row0 = mix_rgb(left_x0, right_x0, 0.4);
    let row1 = mix_rgb(left_x1, right_x1, 0.4);
    // Vertical weights: row 0 (y = 0) gets 1 - fy = 0.4, row 1 (y = 1) gets fy = 0.6.
    let expected = Rgb(
        (row0.0 as f32 * 0.4 + row1.0 as f32 * 0.6).round() as u8,
        (row0.1 as f32 * 0.4 + row1.1 as f32 * 0.6).round() as u8,
        (row0.2 as f32 * 0.4 + row1.2 as f32 * 0.6).round() as u8,
    );
    // The hand-derived value and the pipeline may differ by one unit due to f32
    // operation ordering; the exact equality is enforced via the full-canvas
    // scalar reference below.
    for (name, e, a) in [
        ("r", expected.0, pixel_rgb(&pixmap, 7, 1).0),
        ("g", expected.1, pixel_rgb(&pixmap, 7, 1).1),
        ("b", expected.2, pixel_rgb(&pixmap, 7, 1).2),
    ] {
        assert!(e.abs_diff(a) <= 1, "channel {name}: expected ~{e}, got {a}");
    }

    // The red channel alone proves both sides of the seam contributed: red only
    // depends on the texel column, so it is the 60/40 horizontal mix of edge
    // texel 190 and the wrapped opposite-edge texel 40, independently of the
    // vertical blend.
    let seam_red = pixel_rgb(&pixmap, 7, 1).0;
    assert_eq!(seam_red, (190.0f32 * 0.6 + 40.0f32 * 0.4).round() as u8);
    // And it differs from every source texel, i.e. it cannot be a nearest fetch.
    assert_ne!(seam_red, texel_red(0));
    assert_ne!(seam_red, texel_red(3));

    // Green depends only on the texel *row*, so it independently checks the
    // vertical 40/60 blend between rows y=0 (green 30) and y=1 (green 90),
    // unaffected by the horizontal seam.
    assert_eq!(pixel_rgb(&pixmap, 7, 1).1, (30.0f32 * 0.4 + 90.0f32 * 0.6).round() as u8);

    assert_canvas_matches_reference(&pixmap, SpreadMode::Repeat, transform, reference_bilinear);
}

#[test]
fn bilinear_texel_center_reproduces_a_single_texel() {
    let tile = make_tile();

    // Exact texel center under a scale-only pattern transform:
    //   M = scale(3), so M^-1 maps device (4, 4) to local (4.5/3, 4.5/3) = (1.5, 1.5).
    // A half-integer local coordinate is texel (1, 1)'s center: fx = fy = 0.5,
    // so all four weights collapse onto that single texel and the bilinear
    // result is the texel's exact color with no neighbor contribution.
    let center_transform = Transform::from_scale(3.0, 3.0);
    let center_pixmap =
        render_tile(&tile, SpreadMode::Repeat, FilterQuality::Bilinear, center_transform);
    assert_eq!(pixel_rgb(&center_pixmap, 4, 4), texel_rgb(1, 1));
    // The neighboring device pixel is a genuine blend, proving the center pixel
    // is not trivially identical to everything around it.
    assert_ne!(pixel_rgb(&center_pixmap, 4, 4), pixel_rgb(&center_pixmap, 5, 4));
    assert_canvas_matches_reference(
        &center_pixmap,
        SpreadMode::Repeat,
        center_transform,
        reference_bilinear,
    );

    // A regular off-center sample under the fractional translation transform:
    // device (1, 0) -> local (0.9, 0.6), fx = fract(1.4) = 0.4, fy = fract(1.1) = 0.1.
    let transform = bilinear_transform();
    let pixmap = render_tile(&tile, SpreadMode::Repeat, FilterQuality::Bilinear, transform);
    let expected = bilinear_combine(
        [texel_rgb(0, 0), texel_rgb(1, 0), texel_rgb(0, 1), texel_rgb(1, 1)],
        0.4,
        0.1,
    );
    let actual = pixel_rgb(&pixmap, 1, 0);
    assert!(
        expected.0.abs_diff(actual.0) <= 1
            && expected.1.abs_diff(actual.1) <= 1
            && expected.2.abs_diff(actual.2) <= 1,
        "expected ~{expected:?}, got {actual:?}"
    );
    assert_canvas_matches_reference(&pixmap, SpreadMode::Repeat, transform, reference_bilinear);
}

// Weighted combination of the four footprint texels using the pipeline's
// single-final-rounding accumulation: `fx` is the right-column weight,
// `fy` the bottom-row weight; corners are ordered (top-left, top-right,
// bottom-left, bottom-right).
fn bilinear_combine(corners: [Rgb; 4], fx: f32, fy: f32) -> Rgb {
    let [c00, c10, c01, c11] = corners;
    let wx = [1.0 - fx, fx];
    let wy = [1.0 - fy, fy];
    let rows = [[c00, c10], [c01, c11]];
    let mut r = 0.0f32;
    let mut g = 0.0f32;
    let mut b = 0.0f32;
    for j in 0..2 {
        for i in 0..2 {
            let color = rows[j][i];
            let weight = wx[i] * wy[j];
            r += color.0 as f32 * weight;
            g += color.1 as f32 * weight;
            b += color.2 as f32 * weight;
        }
    }
    Rgb(r.round() as u8, g.round() as u8, b.round() as u8)
}

#[test]
fn bilinear_reflect_seam_joins_edge_texel_with_its_mirror_neighbor() {
    let tile = make_tile();
    let transform = bilinear_transform();
    let pixmap = render_tile(&tile, SpreadMode::Reflect, FilterQuality::Bilinear, transform);

    // Device (7, 1): local (3.9, 1.1), fx = 0.4, fy = 0.6.
    // Under reflection the x footprint [3.4, 4.4] does not wrap to texel 0:
    // coordinate 4.4 mirrors to 3.6, so *both* x neighbors are edge texel x = 3.
    // The output must therefore equal the vertical interpolation of the two
    // texels in the mirrored edge column, with no contribution from column 0.
    let top = texel_rgb(3, 0);
    let bottom = texel_rgb(3, 1);
    // fy = 0.6: top row weight 1 - fy = 0.4, bottom row weight fy = 0.6.
    let expected = Rgb(
        (top.0 as f32 * 0.4 + bottom.0 as f32 * 0.6).round() as u8,
        (top.1 as f32 * 0.4 + bottom.1 as f32 * 0.6).round() as u8,
        (top.2 as f32 * 0.4 + bottom.2 as f32 * 0.6).round() as u8,
    );
    let actual = pixel_rgb(&pixmap, 7, 1);
    assert!(
        expected.0.abs_diff(actual.0) <= 1
            && expected.1.abs_diff(actual.1) <= 1
            && expected.2.abs_diff(actual.2) <= 1,
        "expected ~{expected:?}, got {actual:?}"
    );
    // Red stays on the edge texel's value 190; a repeat fetch would have bled
    // in column 0's red 40 instead.
    assert_eq!(actual.0, 190);

    assert_canvas_matches_reference(&pixmap, SpreadMode::Reflect, transform, reference_bilinear);
}

#[test]
fn bilinear_pad_seam_reuses_the_edge_texel() {
    let tile = make_tile();
    let transform = bilinear_transform();
    let pixmap = render_tile(&tile, SpreadMode::Pad, FilterQuality::Bilinear, transform);

    // Same local coordinate (3.9, 1.1) at device (7, 1). With Pad, the
    // out-of-range footprint corner 4.4 clamps to edge texel x = 3 as well, so
    // horizontally only column 3 contributes -- identical red behavior to
    // reflection here, while the vertical mix still depends on rows 0/1.
    let top = texel_rgb(3, 0);
    let bottom = texel_rgb(3, 1);
    // fy = 0.6: top row weight 0.4, bottom row weight 0.6.
    let expected = Rgb(
        (top.0 as f32 * 0.4 + bottom.0 as f32 * 0.6).round() as u8,
        (top.1 as f32 * 0.4 + bottom.1 as f32 * 0.6).round() as u8,
        (top.2 as f32 * 0.4 + bottom.2 as f32 * 0.6).round() as u8,
    );
    let actual = pixel_rgb(&pixmap, 7, 1);
    assert!(
        expected.0.abs_diff(actual.0) <= 1
            && expected.1.abs_diff(actual.1) <= 1
            && expected.2.abs_diff(actual.2) <= 1,
        "expected ~{expected:?}, got {actual:?}"
    );

    // Interior pixel (1, 0) is unaffected by the address mode and must match the
    // repeat rendering at the same position.
    let repeat = render_tile(&tile, SpreadMode::Repeat, FilterQuality::Bilinear, transform);
    assert_eq!(pixel_rgb(&pixmap, 1, 0), pixel_rgb(&repeat, 1, 0));

    assert_canvas_matches_reference(&pixmap, SpreadMode::Pad, transform, reference_bilinear);
}

#[test]
fn bilinear_fractional_translation_leaves_judgeable_contribution_from_each_side() {
    let tile = make_tile();
    // Scale plus a fractional translation, chosen so integer device columns land
    // either on a texel center or exactly on a texel *boundary*:
    //   M^-1: local = 0.5*(device + 0.5) + 0.75
    //   x = 1 -> local_x = 1.5 (center: fx = 0, single texel)
    //   x = 2 -> local_x = 2.0 (boundary: fx = 0.5, 50/50 split)
    let transform = Transform::from_row(2.0, 0.0, 0.0, 2.0, -1.5, -1.5);
    let pixmap = render_tile(&tile, SpreadMode::Repeat, FilterQuality::Bilinear, transform);

    // Device (2, 2): local = (2.0, 2.0), a four-texel corner where four tiles
    // meet (fx = fy = 0.5). The footprint texels are columns (3, wrapped 0) and
    // rows (3, wrapped 0).
    //
    // Red depends only on the column and is therefore the exact 50/50 midpoint
    // of edge texel red 190 and the wrapped opposite-edge texel red 40. Both
    // rows carry the same reds, so the vertical blend cannot disturb it.
    let corner = pixel_rgb(&pixmap, 2, 2);
    assert_eq!(corner.0, ((190u16 + 40u16) / 2) as u8);
    assert_ne!(corner.0, texel_red(0));
    assert_ne!(corner.0, texel_red(3));

    // Blue mixes all four corner texels with weight 0.25 each; it must be
    // strictly between the two seam-adjacent blue values, proving neither side
    // was dropped.
    let corner_blues = [
        texel_blue(3, 3),
        texel_blue(0, 3),
        texel_blue(3, 0),
        texel_blue(0, 0),
    ];
    let expected_blue =
        (corner_blues.iter().map(|&v| v as u32).sum::<u32>() / 4) as u8;
    assert_eq!(corner.2, expected_blue);
    assert!(corner.2 > texel_blue(0, 0) && corner.2 < texel_blue(3, 3));

    // One column earlier, device (1, 2) has local_x = 1.5, a texel *center*:
    // fx = 0 collapses both footprint columns onto texel column 1 (red 90), so
    // the imaged red is that texel's red regardless of the vertical blend.
    assert_eq!(pixel_rgb(&pixmap, 1, 2).0, texel_red(1));

    assert_canvas_matches_reference(&pixmap, SpreadMode::Repeat, transform, reference_bilinear);
}
