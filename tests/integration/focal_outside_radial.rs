//! Pixel-rendering tests for radial (two-point conical) gradients whose
//! focal point has been dragged *outside* the outer circle.
//!
//! Such a configuration is reachable from vector editing tools: the focal
//! handle can cross the outer circle's edge. With a zero focal radius the
//! shader has to solve the geometry of the family of circles expanding from
//! the focal point, and only paints where that family has a real solution.
//!
//! Every expectation below is derived from that geometric solution instead
//! of from a recorded bitmap, so the tests pin down *why* a pixel has a given
//! coverage/color and not just that some rendering changed.

use tiny_skia::*;

// Layer size.
const W: u32 = 120;
const H: u32 = 100;

// Focal circle: center F, radius 0.
// Outer circle: center C, radius R. The center distance d = 60 is clearly
// larger than R = 25, i.e. the focal point sits well outside the outer circle.
const FX: f64 = 15.0;
const FY: f64 = 50.0;
const CX: f64 = 75.0;
const CY: f64 = 50.0;
const R: f64 = 25.0;

// Pixels closer than this (in device pixels) to an analytic coverage edge are
// not asserted strictly. Anti-aliasing is disabled, but an f32/f64 ULP on a
// boundary that grazes a pixel center should not make the suite flaky.
const EDGE_BAND: f64 = 0.75;
// 8-bit quantization / SIMD lane ordering can shift a channel by one or two.
const LUMA_TOL: i32 = 2;
const COLOR_TOL: i32 = 3;
const MULTI_STOP_TOL: i32 = 4;
const COMPOSITE_TOL: i32 = 3;

/// Analytic solution of the expanding-circle family at one pixel center.
struct CircleModel {
    /// Whether a circle of the family actually reaches this pixel and the
    /// selected root is a valid (positive) gradient parameter.
    valid: bool,
    /// Greater root of the circle equation. This is the root tiny-skia's
    /// `XYTo2PtConicalGreater` stage selects for `d > R`, `r0 == 0`.
    t_greater: f64,
    /// Lesser root, kept around to prove that the greater one was chosen.
    t_lesser: f64,
    /// Distance in pixels to the two tangent rays that bound the cone.
    edge_distance: f64,
}

impl CircleModel {
    fn at(pixel_x: u32, pixel_y: u32) -> Self {
        // Scan conversion samples pixel centers.
        let px = pixel_x as f64 + 0.5;
        let py = pixel_y as f64 + 0.5;

        let dx = CX - FX;
        let dy = CY - FY;
        let d = dx.hypot(dy);
        let ex = dx / d;
        let ey = dy / d;

        // Coordinates in the gradient frame: origin at the focal point,
        // +X pointing from the focal point towards the outer center.
        let vx = px - FX;
        let vy = py - FY;
        let big_x = vx * ex + vy * ey;
        let big_y = -vx * ey + vy * ex;

        // A point P lies on the circle of the family at parameter t when
        // center(t) = F + t * (C - F) and radius(t) = t * R:
        //
        //     (X - t*d)^2 + Y^2 = (t*R)^2
        //
        // which expands to:
        //
        //     (d^2 - R^2) * t^2 - 2*d*X*t + (X^2 + Y^2) = 0.
        let a = d * d - R * R;
        let discriminant = R * R * big_x * big_x - a * big_y * big_y;
        let root = discriminant.max(0.0).sqrt();

        let t_greater = (d * big_x + root) / a;
        let t_lesser = (d * big_x - root) / a;

        // A negative discriminant means no real circle reaches the pixel
        // (outside the tangent cone). The pipeline additionally masks
        // non-positive t, which removes the backward cone behind the focus.
        let valid = discriminant >= 0.0 && t_greater > 0.0;

        // The cone boundaries are `R*X - sqrt(A)*|Y| = 0`; the normal of
        // either ray has length d.
        let edge_distance = (R * big_x - a.sqrt() * big_y.abs()).abs() / d;

        CircleModel {
            valid,
            t_greater,
            t_lesser,
            edge_distance,
        }
    }
}

fn two_color_stops(c0: Color, c1: Color) -> Vec<GradientStop> {
    vec![GradientStop::new(0.0, c0), GradientStop::new(1.0, c1)]
}

/// Fills the whole layer with the focal-outside radial gradient.
///
/// The filled path covers the entire pixmap so that every untouched pixel is
/// caused by the shader's own geometric mask, not by the path geometry.
fn render(stops: Vec<GradientStop>, mode: SpreadMode, background: Option<Color>) -> Pixmap {
    let mut pixmap = Pixmap::new(W, H).unwrap();
    if let Some(bg) = background {
        pixmap.fill(bg);
    }

    let mut paint = Paint::default();
    // Keep coverage binary so the assertions can be exact about the region.
    paint.anti_alias = false;
    paint.shader = RadialGradient::new(
        Point::from_xy(FX as f32, FY as f32),
        0.0,
        Point::from_xy(CX as f32, CY as f32),
        R as f32,
        stops,
        mode,
        Transform::identity(),
    )
    .expect("focal-outside radial gradient must be constructible");

    let layer = PathBuilder::from_rect(Rect::from_ltrb(0.0, 0.0, W as f32, H as f32).unwrap());
    pixmap.fill_path(
        &layer,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    pixmap
}

fn gray_stops() -> Vec<GradientStop> {
    two_color_stops(
        Color::from_rgba8(0, 0, 0, 255),
        Color::from_rgba8(255, 255, 255, 255),
    )
}

fn channel_max_diff(pixel: PremultipliedColorU8, r: i32, g: i32, b: i32) -> i32 {
    (pixel.red() as i32 - r)
        .abs()
        .max((pixel.green() as i32 - g).abs())
        .max((pixel.blue() as i32 - b).abs())
}

fn lerp_u8(a: u8, b: u8, t: f64) -> f64 {
    a as f64 + (b as f64 - a as f64) * t
}

/// Maps an unbounded gradient parameter to [0, 1] according to `mode`.
fn spread_parameter(t: f64, mode: SpreadMode) -> f64 {
    match mode {
        SpreadMode::Pad => t.clamp(0.0, 1.0),
        SpreadMode::Repeat => t - t.floor(),
        SpreadMode::Reflect => {
            let q = t.rem_euclid(2.0);
            if q <= 1.0 {
                q
            } else {
                2.0 - q
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Requirement 1: the painted region equals the geometric solution of the
// circle family expanding from the focal point.
// ---------------------------------------------------------------------------

#[test]
fn focal_outside_region_matches_expanding_circle_geometry() {
    let pixmap = render(gray_stops(), SpreadMode::Pad, None);

    let mut region_mismatches = 0usize;
    let mut first_mismatch = String::new();
    let mut checked_inside = 0usize;
    let mut checked_outside = 0usize;

    for y in 0..H {
        for x in 0..W {
            let model = CircleModel::at(x, y);
            let alpha = pixmap.pixel(x, y).unwrap().alpha();
            let actually_painted = alpha != 0;

            if model.edge_distance < EDGE_BAND {
                continue;
            }

            if model.valid {
                checked_inside += 1;
            } else {
                checked_outside += 1;
            }

            if actually_painted != model.valid {
                region_mismatches += 1;
                if first_mismatch.is_empty() {
                    first_mismatch = format!(
                        "pixel ({x}, {y}): rendered alpha={alpha}, expected valid={} \
                         (t+={:.3}, t-={:.3}, edge_dist={:.2})",
                        model.valid,
                        model.t_greater,
                        model.t_lesser,
                        model.edge_distance
                    );
                }
            }
        }
    }

    // Guard against a vacuous oracle that classifies everything the same.
    assert!(checked_inside > 1000, "too few inside pixels checked");
    assert!(checked_outside > 1000, "too few outside pixels checked");
    assert_eq!(
        region_mismatches, 0,
        "painted region diverges from the circle-family geometry: {first_mismatch}"
    );

    // Explicit geometric anchors.

    // Behind the focal point (away from the outer circle) the greater root is
    // negative, so that backward cone must stay fully transparent.
    assert_eq!(pixmap.pixel(4, 50).unwrap().alpha(), 0);

    // Points on the forward axis just past the focus are inside the solution.
    assert_eq!(pixmap.pixel(25, 50).unwrap().alpha(), 255);

    // Far above the cone there is no real solution either.
    assert_eq!(pixmap.pixel(60, 8).unwrap().alpha(), 0);

    // The shaded region is the tangent cone, not the outer disk: this point is
    // well outside the outer circle (distance ~35.5 > R = 25) yet inside the
    // cone and therefore painted.
    let beyond_outer = pixmap.pixel(110, 50).unwrap();
    assert_eq!(beyond_outer.alpha(), 255);
    assert_eq!(beyond_outer.red(), 255); // pad clamps t > 1 to the end stop
}

#[test]
fn focal_outside_shading_parameter_is_greater_root() {
    let pixmap = render(gray_stops(), SpreadMode::Pad, None);

    let mut checked = 0usize;
    let mut worst = 0i32;
    let mut first_bad = String::new();

    // Black -> white makes the gray level a direct reading of t (after pad).
    for y in 0..H {
        for x in 0..W {
            let model = CircleModel::at(x, y);
            if !model.valid
                || model.edge_distance < EDGE_BAND
                || model.t_greater <= 0.02
                || model.t_greater >= 0.98
            {
                continue;
            }

            let expected = (255.0 * model.t_greater).round() as i32;
            let pixel = pixmap.pixel(x, y).unwrap();
            let diff = (pixel.red() as i32 - expected)
                .abs()
                .max((pixel.green() as i32 - expected).abs())
                .max((pixel.blue() as i32 - expected).abs());

            checked += 1;
            worst = worst.max(diff);
            if diff > LUMA_TOL && first_bad.is_empty() {
                first_bad = format!(
                    "pixel ({x}, {y}): rgba=({},{},{},{}), expected gray {expected} for t+={:.4}",
                    pixel.red(),
                    pixel.green(),
                    pixel.blue(),
                    pixel.alpha(),
                    model.t_greater
                );
            }
        }
    }

    assert!(checked > 200, "expected a sizable t-in-(0,1) ramp region");
    assert!(
        first_bad.is_empty(),
        "shading parameter does not match the greater circle root (worst diff {worst}): \
         {first_bad}"
    );

    // Root-discrimination anchors: rendering must follow t+, not t-.
    let p = (35u32, 50u32);
    let model = CircleModel::at(p.0, p.1);
    assert!((0.55..0.63).contains(&model.t_greater), "t+ = {}", model.t_greater);
    assert!((0.20..0.28).contains(&model.t_lesser), "t- = {}", model.t_lesser);
    let pixel = pixmap.pixel(p.0, p.1).unwrap();
    assert!(
        (pixel.red() as i32 - (255.0 * model.t_greater).round() as i32).abs() <= LUMA_TOL
    );
    // And clearly not the lesser-root gray.
    assert!((pixel.red() as i32 - (255.0 * model.t_lesser).round() as i32).abs() > 60);

    // Further along the axis the roots straddle t = 1. Pad + greater root
    // yields the clamped end color (white); the lesser root would give gray.
    let p = (75u32, 50u32);
    let model = CircleModel::at(p.0, p.1);
    assert!(model.t_greater > 1.0);
    assert!(model.t_lesser < 1.0);
    assert_eq!(pixmap.pixel(p.0, p.1).unwrap().red(), 255);
}

// ---------------------------------------------------------------------------
// Requirement 2: colors follow the hit stop position.
// ---------------------------------------------------------------------------

#[test]
fn focal_outside_colors_follow_multi_stop_positions() {
    let stops = vec![
        GradientStop::new(0.00, Color::from_rgba8(230, 40, 40, 255)),
        GradientStop::new(0.50, Color::from_rgba8(40, 200, 70, 255)),
        GradientStop::new(1.00, Color::from_rgba8(40, 70, 230, 255)),
    ];
    let pixmap = render(stops, SpreadMode::Pad, None);

    let stop_colors: [(f64, (u8, u8, u8)); 3] = [
        (0.0, (230, 40, 40)),
        (0.5, (40, 200, 70)),
        (1.0, (40, 70, 230)),
    ];

    let expected_color = |t: f64| -> (f64, f64, f64) {
        let t = t.clamp(0.0, 1.0);
        let segment = stop_colors
            .windows(2)
            .find(|w| t >= w[0].0 && t <= w[1].0)
            .unwrap_or(&stop_colors[0..2]);
        let (t0, c0) = (segment[0].0, segment[0].1);
        let (t1, c1) = (segment[1].0, segment[1].1);
        let local = ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
        (
            lerp_u8(c0.0, c1.0, local),
            lerp_u8(c0.1, c1.1, local),
            lerp_u8(c0.2, c1.2, local),
        )
    };

    let mut checked = 0usize;
    let mut first_bad = String::new();

    for y in 0..H {
        for x in 0..W {
            let model = CircleModel::at(x, y);
            if !model.valid || model.edge_distance < EDGE_BAND {
                continue;
            }
            let (er, eg, eb) = expected_color(model.t_greater);
            let pixel = pixmap.pixel(x, y).unwrap();
            let diff = channel_max_diff(pixel, er.round() as i32, eg.round() as i32, eb.round() as i32);
            checked += 1;
            if diff > MULTI_STOP_TOL && first_bad.is_empty() {
                first_bad = format!(
                    "pixel ({x}, {y}) t={:.3}: rgba=({},{},{}), expected ~({},{},{}) diff={diff}",
                    model.t_greater,
                    pixel.red(),
                    pixel.green(),
                    pixel.blue(),
                    er.round(),
                    eg.round(),
                    eb.round()
                );
            }
        }
    }

    assert!(checked > 1000);
    assert!(first_bad.is_empty(), "stop interpolation mismatch: {first_bad}");

    // Stop-hit anchors along the axis.
    // t = 0.5 lands around pixel x = 32 -> the middle (green) stop.
    let mid = pixmap.pixel(32, 50).unwrap();
    assert!(
        mid.green() > mid.red() + 60 && mid.green() > mid.blue() + 60,
        "expected the middle green stop, got {:?}",
        mid
    );

    // Close to the focus the first (red) stop dominates. t+ ~= 0.13 here.
    let start = pixmap.pixel(19, 50).unwrap();
    assert!(
        start.red() > start.blue() + 60 && start.red() > start.green() + 60,
        "expected the first red stop, got {:?}",
        start
    );

    // At/beyond t = 1 the last (blue) stop is held by pad.
    let end = pixmap.pixel(52, 50).unwrap();
    assert!(
        end.blue() > end.red() + 60 && end.blue() > end.green() + 60,
        "expected the last blue stop, got {:?}",
        end
    );
}

// ---------------------------------------------------------------------------
// Requirement 2 (cont.): spread modes must show up in the pixels.
// ---------------------------------------------------------------------------

fn assert_spread_matches_analytic(mode: SpreadMode) {
    let c0 = Color::from_rgba8(200, 40, 40, 255);
    let c1 = Color::from_rgba8(40, 80, 220, 255);
    let pixmap = render(two_color_stops(c0, c1), mode, None);

    let mut checked = 0usize;
    let mut first_bad = String::new();

    for y in 0..H {
        for x in 0..W {
            let model = CircleModel::at(x, y);
            if !model.valid || model.edge_distance < EDGE_BAND {
                continue;
            }
            let t = spread_parameter(model.t_greater, mode);
            let er = lerp_u8(200, 40, t).round() as i32;
            let eg = lerp_u8(40, 80, t).round() as i32;
            let eb = lerp_u8(40, 220, t).round() as i32;

            let pixel = pixmap.pixel(x, y).unwrap();
            let diff = channel_max_diff(pixel, er, eg, eb);
            checked += 1;
            if diff > COLOR_TOL && first_bad.is_empty() {
                first_bad = format!(
                    "pixel ({x}, {y}) t+={:.3} spread={t:.3}: rgba=({},{},{}), \
                     expected ({er},{eg},{eb}) diff={diff}",
                    model.t_greater,
                    pixel.red(),
                    pixel.green(),
                    pixel.blue()
                );
            }
        }
    }

    assert!(checked > 1000);
    assert!(first_bad.is_empty(), "{mode:?} spread mismatch: {first_bad}");
}

#[test]
fn focal_outside_pad_spread_holds_end_color() {
    assert_spread_matches_analytic(SpreadMode::Pad);

    // Far along the cone t+ > 1 and pad holds the blue end stop constant
    // across neighbouring pixels.
    let a = pixmap_pub(SpreadMode::Pad, 95, 50);
    let b = pixmap_pub(SpreadMode::Pad, 110, 50);
    assert_eq!(a, b);
    assert!(b.blue() > b.red() + 80);
}

#[test]
fn focal_outside_repeat_spread_wraps_parameter() {
    assert_spread_matches_analytic(SpreadMode::Repeat);

    // At (75, 50), t+ > 1 but t- < 1. Repeat wraps t+ back into the ramp, so
    // the pixel must differ clearly from pad's held end color.
    let repeated = render(
        two_color_stops(
            Color::from_rgba8(200, 40, 40, 255),
            Color::from_rgba8(40, 80, 220, 255),
        ),
        SpreadMode::Repeat,
        None,
    )
    .pixel(75, 50)
    .unwrap();
    let padded = render(
        two_color_stops(
            Color::from_rgba8(200, 40, 40, 255),
            Color::from_rgba8(40, 80, 220, 255),
        ),
        SpreadMode::Pad,
        None,
    )
    .pixel(75, 50)
    .unwrap();
    assert_ne!(repeated, padded);

    let model = CircleModel::at(75, 50);
    let t = spread_parameter(model.t_greater, SpreadMode::Repeat);
    let expected_g = lerp_u8(40, 80, t).round() as i32;
    assert!((repeated.green() as i32 - expected_g).abs() <= COLOR_TOL);
}

#[test]
fn focal_outside_reflect_spread_mirrors_parameter() {
    assert_spread_matches_analytic(SpreadMode::Reflect);

    // Reflect and repeat disagree once t+ passes the first mirror point while
    // pad stays constant; sanity-check one such pixel against the oracle.
    let reflected = render(
        two_color_stops(
            Color::from_rgba8(200, 40, 40, 255),
            Color::from_rgba8(40, 80, 220, 255),
        ),
        SpreadMode::Reflect,
        None,
    )
    .pixel(95, 50)
    .unwrap();
    let model = CircleModel::at(95, 50);
    assert!(model.t_greater > 2.0, "test anchor t+ = {}", model.t_greater);
    let t = spread_parameter(model.t_greater, SpreadMode::Reflect);
    let er = lerp_u8(200, 40, t).round() as i32;
    let eg = lerp_u8(40, 80, t).round() as i32;
    let eb = lerp_u8(40, 220, t).round() as i32;
    assert!(channel_max_diff(reflected, er, eg, eb) <= COLOR_TOL);
}

/// Small helper for the pad constancy anchor.
fn pixmap_pub(mode: SpreadMode, x: u32, y: u32) -> PremultipliedColorU8 {
    render(
        two_color_stops(
            Color::from_rgba8(200, 40, 40, 255),
            Color::from_rgba8(40, 80, 220, 255),
        ),
        mode,
        None,
    )
    .pixel(x, y)
    .unwrap()
}

// ---------------------------------------------------------------------------
// Requirement 2 (cont.): the layer background survives in composited pixels.
// ---------------------------------------------------------------------------

#[test]
fn focal_outside_translucent_gradient_composites_over_background() {
    let background = Color::from_rgba8(30, 200, 90, 255);
    let mut pixmap = Pixmap::new(W, H).unwrap();
    pixmap.fill(background);
    let background_pixel = pixmap.pixel(0, 0).unwrap();

    let stops = vec![
        GradientStop::new(0.0, Color::from_rgba8(240, 50, 50, 128)),
        GradientStop::new(1.0, Color::from_rgba8(50, 50, 240, 64)),
    ];

    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = RadialGradient::new(
        Point::from_xy(FX as f32, FY as f32),
        0.0,
        Point::from_xy(CX as f32, CY as f32),
        R as f32,
        stops,
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap();
    let layer = PathBuilder::from_rect(Rect::from_ltrb(0.0, 0.0, W as f32, H as f32).unwrap());
    pixmap.fill_path(&layer, &paint, FillRule::Winding, Transform::identity(), None);

    let bg = (30.0, 200.0, 90.0);
    let mut checked_blended = 0usize;
    let mut outside_mismatches = 0usize;
    let mut first_bad = String::new();

    for y in 0..H {
        for x in 0..W {
            let model = CircleModel::at(x, y);
            let pixel = pixmap.pixel(x, y).unwrap();

            // Coverage edges are asserted by the geometry test; keep the
            // exact background/composite checks off their quantization band.
            if model.edge_distance < EDGE_BAND {
                continue;
            }

            if !model.valid {
                // The geometric mask must leave the layer background exactly
                // untouched outside the valid region.
                if pixel != background_pixel {
                    outside_mismatches += 1;
                    if first_bad.is_empty() {
                        first_bad = format!(
                            "outside region pixel ({x}, {y}) = {:?}, expected untouched background {:?}",
                            pixel, background_pixel
                        );
                    }
                }
                continue;
            }

            let t = model.t_greater.clamp(0.0, 1.0);
            // tiny-skia interpolates straight stop colors/alpha and applies
            // source-over compositing afterwards.
            let alpha = lerp_u8(128, 64, t) / 255.0;
            let sr = lerp_u8(240, 50, t);
            let sg = lerp_u8(50, 50, t);
            let sb = lerp_u8(50, 240, t);
            let er = (sr * alpha + bg.0 * (1.0 - alpha)).round() as i32;
            let eg = (sg * alpha + bg.1 * (1.0 - alpha)).round() as i32;
            let eb = (sb * alpha + bg.2 * (1.0 - alpha)).round() as i32;

            let diff = channel_max_diff(pixel, er, eg, eb);
            checked_blended += 1;
            if diff > COMPOSITE_TOL && first_bad.is_empty() {
                first_bad = format!(
                    "composite pixel ({x}, {y}) t={t:.3}: rgba=({},{},{},{}), \
                     expected ~({er},{eg},{eb},255) diff={diff}",
                    pixel.red(),
                    pixel.green(),
                    pixel.blue(),
                    pixel.alpha()
                );
            }
        }
    }

    assert!(checked_blended > 1000);
    assert_eq!(outside_mismatches, 0, "{first_bad}");
    assert!(first_bad.is_empty(), "compositing mismatch: {first_bad}");

    // The result is opaque because the layer background is opaque.
    assert_eq!(pixmap.pixel(60, 50).unwrap().alpha(), 255);
}

#[test]
fn focal_outside_background_changes_final_pixels() {
    // Same translucent gradient over two different backgrounds must composite
    // to visibly different pixels; over a transparent layer it stays
    // translucent. This directly demonstrates the background contribution.
    let stops = vec![
        GradientStop::new(0.0, Color::from_rgba8(255, 0, 0, 160)),
        GradientStop::new(1.0, Color::from_rgba8(0, 0, 255, 160)),
    ];

    let over_green = render(stops.clone(), SpreadMode::Pad, Some(Color::from_rgba8(0, 220, 0, 255)));
    let over_magenta = render(
        stops.clone(),
        SpreadMode::Pad,
        Some(Color::from_rgba8(220, 0, 220, 255)),
    );
    let over_nothing = render(stops, SpreadMode::Pad, None);

    let a = over_green.pixel(40, 50).unwrap();
    let b = over_magenta.pixel(40, 50).unwrap();
    let c = over_nothing.pixel(40, 50).unwrap();

    assert_eq!(a.alpha(), 255);
    assert_eq!(b.alpha(), 255);
    assert_ne!(a, b, "different backgrounds must change composited pixels");
    assert!(c.alpha() > 0 && c.alpha() < 255, "source stays translucent");

    // Green background pushes green channel above the magenta-background result.
    assert!(a.green() as i32 - b.green() as i32 > 60);
}
