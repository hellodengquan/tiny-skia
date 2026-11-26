use tiny_skia::*;

#[test]
fn two_stops_linear_pad_lq() {
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = LinearGradient::new(
        Point::from_xy(10.0, 10.0),
        Point::from_xy(190.0, 190.0),
        vec![
            GradientStop::new(0.0, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(1.0, Color::from_rgba8(220, 140, 75, 180)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/two-stops-linear-pad-lq.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn two_stops_linear_repeat_lq() {
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = LinearGradient::new(
        Point::from_xy(10.0, 10.0),
        Point::from_xy(100.0, 100.0),
        vec![
            GradientStop::new(0.0, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(1.0, Color::from_rgba8(220, 140, 75, 180)),
        ],
        SpreadMode::Repeat,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/two-stops-linear-repeat-lq.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn two_stops_linear_reflect_lq() {
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = LinearGradient::new(
        Point::from_xy(10.0, 10.0),
        Point::from_xy(100.0, 100.0),
        vec![
            GradientStop::new(0.0, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(1.0, Color::from_rgba8(220, 140, 75, 180)),
        ],
        SpreadMode::Reflect,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/two-stops-linear-reflect-lq.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn three_stops_evenly_spaced_lq() {
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = LinearGradient::new(
        Point::from_xy(10.0, 10.0),
        Point::from_xy(190.0, 190.0),
        vec![
            GradientStop::new(0.25, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(0.50, Color::from_rgba8(220, 140, 75, 180)),
            GradientStop::new(0.75, Color::from_rgba8(40, 180, 55, 160)),
        ],
        // No need to check other modes. "Two stops" tests will cover them.
        SpreadMode::Pad,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/three-stops-evenly-spaced-lq.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn two_stops_unevenly_spaced_lq() {
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = LinearGradient::new(
        Point::from_xy(10.0, 10.0),
        Point::from_xy(190.0, 190.0),
        vec![
            GradientStop::new(0.25, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(0.75, Color::from_rgba8(220, 140, 75, 180)),
        ],
        // No need to check other modes. "Two stops" tests will cover them.
        SpreadMode::Pad,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/two-stops-unevenly-spaced-lq.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn two_stops_linear_pad_hq() {
    let mut paint = Paint::default();
    paint.force_hq_pipeline = true;
    paint.anti_alias = false;
    paint.shader = LinearGradient::new(
        Point::from_xy(10.0, 10.0),
        Point::from_xy(190.0, 190.0),
        vec![
            GradientStop::new(0.0, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(1.0, Color::from_rgba8(220, 140, 75, 180)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/two-stops-linear-pad-hq.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn two_stops_linear_repeat_hq() {
    let mut paint = Paint::default();
    paint.force_hq_pipeline = true;
    paint.anti_alias = false;
    paint.shader = LinearGradient::new(
        Point::from_xy(10.0, 10.0),
        Point::from_xy(100.0, 100.0),
        vec![
            GradientStop::new(0.0, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(1.0, Color::from_rgba8(220, 140, 75, 180)),
        ],
        SpreadMode::Repeat,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/two-stops-linear-repeat-hq.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn two_stops_linear_reflect_hq() {
    let mut paint = Paint::default();
    paint.force_hq_pipeline = true;
    paint.anti_alias = false;
    paint.shader = LinearGradient::new(
        Point::from_xy(10.0, 10.0),
        Point::from_xy(100.0, 100.0),
        vec![
            GradientStop::new(0.0, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(1.0, Color::from_rgba8(220, 140, 75, 180)),
        ],
        SpreadMode::Reflect,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/two-stops-linear-reflect-hq.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn three_stops_evenly_spaced_hq() {
    let mut paint = Paint::default();
    paint.force_hq_pipeline = true;
    paint.anti_alias = false;
    paint.shader = LinearGradient::new(
        Point::from_xy(10.0, 10.0),
        Point::from_xy(190.0, 190.0),
        vec![
            GradientStop::new(0.25, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(0.50, Color::from_rgba8(220, 140, 75, 180)),
            GradientStop::new(0.75, Color::from_rgba8(40, 180, 55, 160)),
        ],
        // No need to check other modes. "Two stops" tests will cover them.
        SpreadMode::Pad,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/three-stops-evenly-spaced-hq.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn two_stops_unevenly_spaced_hq() {
    let mut paint = Paint::default();
    paint.force_hq_pipeline = true;
    paint.anti_alias = false;
    paint.shader = LinearGradient::new(
        Point::from_xy(10.0, 10.0),
        Point::from_xy(190.0, 190.0),
        vec![
            GradientStop::new(0.25, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(0.75, Color::from_rgba8(220, 140, 75, 180)),
        ],
        // No need to check other modes. "Two stops" tests will cover them.
        SpreadMode::Pad,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/two-stops-unevenly-spaced-hq.png").unwrap();
    assert_eq!(pixmap, expected);
}

// The radial gradient is only supported by the high quality pipeline.
// Therefore we do not have a lq/hq split.

#[test]
fn well_behaved_radial() {
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = RadialGradient::new(
        Point::from_xy(100.0, 100.0),
        0.0,
        Point::from_xy(120.0, 80.0),
        100.0,
        vec![
            GradientStop::new(0.25, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(0.75, Color::from_rgba8(220, 140, 75, 180)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/well-behaved-radial.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn focal_on_circle_radial() {
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = RadialGradient::new(
        Point::from_xy(100.0, 100.0),
        0.0,
        Point::from_xy(120.0, 80.0),
        28.29, // This radius forces the required pipeline stage.
        vec![
            GradientStop::new(0.25, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(0.75, Color::from_rgba8(220, 140, 75, 180)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/focal-on-circle-radial.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn conical_greater_radial() {
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = RadialGradient::new(
        Point::from_xy(100.0, 100.0),
        0.0,
        Point::from_xy(120.0, 80.0),
        10.0, // This radius forces the required pipeline stage.
        vec![
            GradientStop::new(0.25, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(0.75, Color::from_rgba8(220, 140, 75, 180)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/conical-greater-radial.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn simple_radial_lq() {
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = RadialGradient::new(
        Point::from_xy(100.0, 100.0),
        0.0,
        Point::from_xy(100.0, 100.0),
        100.0,
        vec![
            GradientStop::new(0.25, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(1.00, Color::from_rgba8(220, 140, 75, 180)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/simple-radial-lq.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn simple_radial_hq() {
    let mut paint = Paint::default();
    paint.force_hq_pipeline = true;
    paint.anti_alias = false;
    paint.shader = RadialGradient::new(
        Point::from_xy(100.0, 100.0),
        0.0,
        Point::from_xy(100.0, 100.0),
        100.0,
        vec![
            GradientStop::new(0.25, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(1.00, Color::from_rgba8(220, 140, 75, 180)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/simple-radial-hq.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn simple_radial_with_ts_hq() {
    let mut paint = Paint::default();
    paint.force_hq_pipeline = true;
    paint.anti_alias = false;
    paint.shader = RadialGradient::new(
        Point::from_xy(100.0, 100.0),
        0.0,
        Point::from_xy(100.0, 100.0),
        100.0,
        vec![
            GradientStop::new(0.25, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(1.00, Color::from_rgba8(220, 140, 75, 180)),
        ],
        SpreadMode::Pad,
        Transform::from_row(2.0, 0.3, -0.7, 1.2, 10.5, -12.3),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/simple-radial-with-ts-hq.png").unwrap();
    assert_eq!(pixmap, expected);
}

// Gradient doesn't add the Premultiply stage when all stops are opaque.
// But it checks colors only on creation, so we have to recheck them after calling `apply_opacity`.
#[test]
fn global_opacity() {
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = RadialGradient::new(
        Point::from_xy(100.0, 100.0),
        0.0,
        Point::from_xy(100.0, 100.0),
        100.0,
        vec![
            GradientStop::new(0.25, Color::from_rgba8(50, 127, 150, 255)), // no opacity here
            GradientStop::new(1.00, Color::from_rgba8(220, 140, 75, 255)), // no opacity here
        ],
        SpreadMode::Pad,
        Transform::identity(),
    ).unwrap();
    paint.shader.apply_opacity(0.5);

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/global-opacity.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn strip_gradient() {
    // Equal radii, different centers creates a Strip gradient
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = RadialGradient::new(
        Point::from_xy(50.0, 100.0),
        50.0,
        Point::from_xy(150.0, 100.0),
        50.0,
        vec![
            GradientStop::new(0.0, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(1.0, Color::from_rgba8(220, 140, 75, 180)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/strip-gradient.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn concentric_radial() {
    // Same center, non-zero start radius (concentric gradient)
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = RadialGradient::new(
        Point::from_xy(100.0, 100.0),
        30.0,
        Point::from_xy(100.0, 100.0),
        90.0,
        vec![
            GradientStop::new(0.0, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(1.0, Color::from_rgba8(220, 140, 75, 180)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/concentric-radial.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn conical_smaller_radial() {
    // Configuration that triggers XYTo2PtConicalSmaller stage
    // r0=60, r1=30, distance=50
    // r0_norm=1.2, r1_norm=0.6, focal_x = 1.2/0.6 = 2.0 > 1.0
    // self.r1 = 0.6 / |1.0 - 2.0| = 0.6, so is_well_behaved() = false
    // (1.0 - focal_x) < 0.0 is true, triggering XYTo2PtConicalSmaller
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = RadialGradient::new(
        Point::from_xy(100.0, 100.0),
        60.0,
        Point::from_xy(150.0, 100.0),
        30.0,
        vec![
            GradientStop::new(0.0, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(1.0, Color::from_rgba8(220, 140, 75, 180)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    ).unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

    let expected = Pixmap::load_png("tests/images/gradients/conical-smaller-radial.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn sweep_gradient() {
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = SweepGradient::new(
        Point::from_xy(100.0, 100.0),
        135.0,
        225.0,
        vec![
            GradientStop::new(0.0, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(1.0, Color::from_rgba8(220, 140, 75, 180)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
        .unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    let expected = Pixmap::load_png("tests/images/gradients/sweep-gradient.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn sweep_gradient_full() {
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = SweepGradient::new(
        Point::from_xy(100.0, 100.0),
        0.0,
        360.0,
        vec![
            GradientStop::new(0.0, Color::from_rgba8(50, 127, 150, 200)),
            GradientStop::new(1.0, Color::from_rgba8(220, 140, 75, 180)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(10.0, 10.0, 190.0, 190.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    let expected = Pixmap::load_png("tests/images/gradients/sweep-gradient-full.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn sweep_gradient_cross_zero_pad() {
    // A clockwise arc from 315deg to 45deg wraps across the zero-degree seam.
    // The stops span the 90-degree arc continuously (red -> blue through 0deg),
    // while the Pad spread mode fills the rest of the wheel with edge colors.
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = SweepGradient::new(
        Point::from_xy(100.0, 100.0),
        315.0,
        45.0,
        vec![
            GradientStop::new(0.0, Color::from_rgba8(255, 0, 0, 255)),
            GradientStop::new(1.0, Color::from_rgba8(0, 0, 255, 255)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(0.0, 0.0, 200.0, 200.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    let expected =
        Pixmap::load_png("tests/images/gradients/sweep-gradient-cross-zero-pad.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn sweep_gradient_cross_zero_repeat() {
    // The same wrap-around arc with the Repeat spread mode. The 90-degree
    // gradient is tiled around the whole wheel, so colors outside the arc are
    // taken from repeated copies instead of being clamped to an edge color.
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = SweepGradient::new(
        Point::from_xy(100.0, 100.0),
        315.0,
        45.0,
        vec![
            GradientStop::new(0.0, Color::from_rgba8(255, 0, 0, 255)),
            GradientStop::new(1.0, Color::from_rgba8(0, 0, 255, 255)),
        ],
        SpreadMode::Repeat,
        Transform::identity(),
    )
    .unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(0.0, 0.0, 200.0, 200.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    let expected =
        Pixmap::load_png("tests/images/gradients/sweep-gradient-cross-zero-repeat.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn sweep_gradient_cross_zero_reflect() {
    let mut paint = Paint::default();
    paint.anti_alias = false;
    paint.shader = SweepGradient::new(
        Point::from_xy(100.0, 100.0),
        315.0,
        45.0,
        vec![
            GradientStop::new(0.0, Color::from_rgba8(255, 0, 0, 255)),
            GradientStop::new(1.0, Color::from_rgba8(0, 0, 255, 255)),
        ],
        SpreadMode::Reflect,
        Transform::identity(),
    )
    .unwrap();

    let path = PathBuilder::from_rect(Rect::from_ltrb(0.0, 0.0, 200.0, 200.0).unwrap());

    let mut pixmap = Pixmap::new(200, 200).unwrap();
    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    let expected =
        Pixmap::load_png("tests/images/gradients/sweep-gradient-cross-zero-reflect.png").unwrap();
    assert_eq!(pixmap, expected);
}

#[test]
fn sweep_gradient_cross_zero_directions() {
    // Probes the color at fixed directions to verify the stops run
    // continuously across the zero-degree seam and that the spread mode gives
    // every direction a definite color.
    fn pixel_at(pixmap: &Pixmap, angle_deg: f32, radius: f32) -> (u8, u8, u8) {
        let rad = angle_deg.to_radians();
        let x = (100.0 + radius * rad.cos()).round() as u32;
        let y = (100.0 + radius * rad.sin()).round() as u32;
        let c = pixmap.pixel(x, y).unwrap();
        (c.red(), c.green(), c.blue())
    }

    fn render(mode: SpreadMode) -> Pixmap {
        let mut paint = Paint::default();
        paint.anti_alias = false;
        paint.shader = SweepGradient::new(
            Point::from_xy(100.0, 100.0),
            315.0,
            45.0,
            vec![
                GradientStop::new(0.0, Color::from_rgba8(255, 0, 0, 255)),
                GradientStop::new(1.0, Color::from_rgba8(0, 0, 255, 255)),
            ],
            mode,
            Transform::identity(),
        )
        .unwrap();
        let path = PathBuilder::from_rect(Rect::from_ltrb(0.0, 0.0, 200.0, 200.0).unwrap());
        let mut pixmap = Pixmap::new(200, 200).unwrap();
        pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
        pixmap
    }

    let pad = render(SpreadMode::Pad);

    // Start of the arc (315deg) is the first stop: red.
    let (r, _g, b) = pixel_at(&pad, 315.0, 60.0);
    assert!(r > 245 && b < 10);

    // The middle of the arc sits exactly on 0deg, proving the interpolation
    // continues across the seam rather than restarting.
    let (r, _g, b) = pixel_at(&pad, 0.0, 60.0);
    assert!((r as i32 - b as i32).abs() <= 5, "seam midpoint r={r} b={b}");

    // End of the arc (45deg) is the last stop: blue.
    let (r, _g, b) = pixel_at(&pad, 45.0, 60.0);
    assert!(b > 245 && r < 10);

    // Pad gives the complement a definite edge color rather than leaving it
    // undefined. The whole wheel must render as fully opaque colors.
    assert!(pad.pixels().iter().all(|p| p.alpha() == 255));

    // Repeat and Reflect also cover every direction, and vary inside the
    // complement instead of collapsing to a single edge color.
    for mode in [SpreadMode::Repeat, SpreadMode::Reflect] {
        let pm = render(mode);
        assert!(pm.pixels().iter().all(|p| p.alpha() == 255));
        // A direction in the middle of the complement (180deg) is neither the
        // pure first nor the pure last stop color.
        let (r, _g, b) = pixel_at(&pm, 180.0, 60.0);
        assert!(!(r > 245 && b < 10), "complement must not be flat first color");
        assert!(!(b > 245 && r < 10), "complement must not be flat last color");
    }
}

#[test]
fn sweep_gradient_cross_zero_normalizes_wide_angles() {
    // Endpoints outside [0, 360] are reduced to turns, so 675deg == 315deg and
    // -315deg == 45deg. This must describe the same arc as 315..45.
    let mut paint_a = Paint::default();
    paint_a.anti_alias = false;
    paint_a.shader = SweepGradient::new(
        Point::from_xy(100.0, 100.0),
        675.0,
        -315.0,
        vec![
            GradientStop::new(0.0, Color::from_rgba8(255, 0, 0, 255)),
            GradientStop::new(1.0, Color::from_rgba8(0, 0, 255, 255)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap();

    let mut paint_b = Paint::default();
    paint_b.anti_alias = false;
    paint_b.shader = SweepGradient::new(
        Point::from_xy(100.0, 100.0),
        315.0,
        45.0,
        vec![
            GradientStop::new(0.0, Color::from_rgba8(255, 0, 0, 255)),
            GradientStop::new(1.0, Color::from_rgba8(0, 0, 255, 255)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap();

    let rect = Rect::from_ltrb(0.0, 0.0, 200.0, 200.0).unwrap();
    let path = PathBuilder::from_rect(rect);

    let mut a = Pixmap::new(200, 200).unwrap();
    a.fill_path(&path, &paint_a, FillRule::Winding, Transform::identity(), None);
    let mut b = Pixmap::new(200, 200).unwrap();
    b.fill_path(&path, &paint_b, FillRule::Winding, Transform::identity(), None);

    assert_eq!(a, b);
}

#[test]
fn sweep_gradient_cross_zero_rejects_invalid() {
    let stops = vec![
        GradientStop::new(0.0, Color::from_rgba8(255, 0, 0, 255)),
        GradientStop::new(1.0, Color::from_rgba8(0, 0, 255, 255)),
    ];

    // Non-finite angles are rejected.
    assert!(SweepGradient::new(
        Point::from_xy(0.0, 0.0),
        f32::NAN,
        45.0,
        stops.clone(),
        SpreadMode::Pad,
        Transform::identity(),
    )
    .is_none());
    assert!(SweepGradient::new(
        Point::from_xy(0.0, 0.0),
        315.0,
        f32::INFINITY,
        stops.clone(),
        SpreadMode::Pad,
        Transform::identity(),
    )
    .is_none());

    // Empty stops are rejected.
    assert!(SweepGradient::new(
        Point::from_xy(0.0, 0.0),
        315.0,
        45.0,
        vec![],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .is_none());

    // A non-invertible (rank-deficient) transform is rejected.
    let singular = Transform::from_row(1.0, 2.0, 2.0, 4.0, 0.0, 0.0);
    assert!(singular.invert().is_none());
    assert!(SweepGradient::new(
        Point::from_xy(0.0, 0.0),
        315.0,
        45.0,
        stops,
        SpreadMode::Pad,
        singular,
    )
    .is_none());
}
