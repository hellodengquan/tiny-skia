//! Integration tests for exporting / drawing a single block ("tile" / "chunk")
//! of a much larger canvas.
//!
//! The big illustration is rendered once as a reference and every exported
//! block is then checked against the *same scene coordinates*:
//!
//! * coverage -> the pixel alpha must match (binary for the non-AA cases,
//!   exact for the anti-aliased gutter case);
//! * texture -> the colour must be the repeating-pattern texel sampled at the
//!   global scene phase, i.e. the pattern's own origin is never re-based onto
//!   a block corner.
//!
//! This guards the properties that matter for arbitrary region export:
//! a stroke crossing block borders keeps the original texture phase and
//! transparent holes of the selection stay at their original scene position.

use tiny_skia::*;

/// Side of the small repeating source tile.
const TILE: u32 = 4;

/// A fully opaque, deterministic colour for texel `(x, y)`.
///
/// The same function is used to build the source tile and to compute the
/// expected texture colour for a scene coordinate, so expectations do not
/// depend on a stored golden image.
fn texel_rgba(x: i32, y: i32) -> [u8; 4] {
    let ix = x.rem_euclid(TILE as i32) as u32;
    let iy = y.rem_euclid(TILE as i32) as u32;
    let i = iy * TILE + ix; // 0..16
    [
        ((i * 17 + 20) % 256) as u8,
        ((i * 29 + 90) % 256) as u8,
        ((i * 43 + 160) % 256) as u8,
        255,
    ]
}

/// The repeating source tile: every texel is opaque and uniquely coloured.
fn phase_tile() -> Pixmap {
    let mut pm = Pixmap::new(TILE, TILE).unwrap();
    for y in 0..TILE as i32 {
        for x in 0..TILE as i32 {
            let px = x as u32;
            let py = y as u32;
            pm.pixels_mut()[(py * TILE + px) as usize] = ColorU8::from_rgba(
                texel_rgba(x, y)[0],
                texel_rgba(x, y)[1],
                texel_rgba(x, y)[2],
                255,
            )
            .premultiply();
        }
    }
    pm
}

fn transparent() -> PremultipliedColorU8 {
    ColorU8::from_rgba(0, 0, 0, 0).premultiply()
}

/// Expected nearest/repeat texel for a global scene coordinate when the
/// pattern transform is an integer translation to `origin`.
///
/// The rasteriser seeds every pixel at its centre (`coord + 0.5`) and repeats
/// with modulo the tile size, which yields this exact index formula.
fn scene_texel(origin: (i32, i32), sx: i32, sy: i32) -> PremultipliedColorU8 {
    let ix = (sx - origin.0).rem_euclid(TILE as i32);
    let iy = (sy - origin.1).rem_euclid(TILE as i32);
    let c = texel_rgba(ix, iy);
    ColorU8::from_rgba(c[0], c[1], c[2], 255).premultiply()
}

/// Expected texel for an integer-scaled, integer-translated pattern.
///
/// `P = translate(origin) * scale(scale)` maps tile coords to scene coords.
fn scene_texel_scaled(origin: (i32, i32), scale: i32, sx: i32, sy: i32) -> PremultipliedColorU8 {
    let lx = (sx as f32 + 0.5 - origin.0 as f32) / scale as f32;
    let ly = (sy as f32 + 0.5 - origin.1 as f32) / scale as f32;
    let ix = lx.floor() as i32;
    let iy = ly.floor() as i32;
    let c = texel_rgba(ix, iy);
    ColorU8::from_rgba(c[0], c[1], c[2], 255).premultiply()
}

fn repeat_paint(tile: &Pixmap, transform: Transform, anti_alias: bool) -> Paint<'_> {
    let mut paint = Paint::default();
    paint.anti_alias = anti_alias;
    paint.shader = Pattern::new(
        tile.as_ref(),
        SpreadMode::Repeat,
        FilterQuality::Nearest,
        1.0,
        transform,
    );
    paint
}

fn draw_scene(
    target: &mut Pixmap,
    path: &Path,
    paint: &Paint,
    stroke: Option<&Stroke>,
    ts: Transform,
) {
    match stroke {
        Some(stroke) => target.stroke_path(path, paint, stroke, ts, None),
        None => target.fill_path(path, paint, FillRule::Winding, ts, None),
    }
}

/// Renders the whole scene with scene origin at the pixmap origin.
fn render_full(
    w: u32,
    h: u32,
    path: &Path,
    paint: &Paint,
    stroke: Option<&Stroke>,
    scene_ts: Transform,
) -> Pixmap {
    let mut pm = Pixmap::new(w, h).unwrap();
    draw_scene(&mut pm, path, paint, stroke, scene_ts);
    pm
}

/// Exports the block covering scene rect `(ox, oy, cw, ch)`.
///
/// The block offset is composed onto the scene transform as
/// `T(-ox,-oy) * scene_ts`, mapping scene coordinates to block-local pixels.
/// tiny-skia applies that draw transform to both geometry and shader, so the
/// global pattern phase is carried into the block unchanged.
fn render_chunk(
    cw: u32,
    ch: u32,
    ox: i32,
    oy: i32,
    path: &Path,
    paint: &Paint,
    stroke: Option<&Stroke>,
    scene_ts: Transform,
) -> Pixmap {
    let mut chunk = Pixmap::new(cw, ch).unwrap();
    let offset = Transform::from_translate(-(ox as f32), -(oy as f32));
    let ts = offset.pre_concat(scene_ts); // T(-ox,-oy) * scene_ts
    draw_scene(&mut chunk, path, paint, stroke, ts);
    chunk
}

/// Asserts every block pixel equals the full-canvas pixel at the same scene
/// coordinate `(ox + x, oy + y)`.
fn assert_matches_scene(full: &Pixmap, chunk: &Pixmap, ox: i32, oy: i32) {
    assert!(
        ox >= 0 && oy >= 0,
        "test helper expects non-negative export origin"
    );
    assert!(ox + chunk.width() as i32 <= full.width() as i32);
    assert!(oy + chunk.height() as i32 <= full.height() as i32);

    let mut problems = Vec::new();
    'outer: for y in 0..chunk.height() as i32 {
        for x in 0..chunk.width() as i32 {
            let actual = chunk.pixel(x as u32, y as u32).unwrap();
            let expected = full.pixel((ox + x) as u32, (oy + y) as u32).unwrap();
            if actual != expected {
                problems.push(format!(
                    "block({x},{y}) = scene({},{}) got RGBA {:?}, expected {:?}",
                    ox + x,
                    oy + y,
                    actual.demultiply(),
                    expected.demultiply()
                ));
                if problems.len() >= 12 {
                    break 'outer;
                }
            }
        }
    }

    assert!(
        problems.is_empty(),
        "block export at ({ox},{oy}) {}x{} differs from the original canvas:\n{}",
        chunk.width(),
        chunk.height(),
        problems.join("\n")
    );
}

/// Partitions the whole canvas into an odd-sized block grid, exports every
/// block and pastes them back at their scene offset. The reconstruction must
/// be pixel identical to a single full-canvas render.
fn assert_grid_roundtrip(
    w: u32,
    h: u32,
    block_w: u32,
    block_h: u32,
    path: &Path,
    paint: &Paint,
    stroke: Option<&Stroke>,
    scene_ts: Transform,
    full: &Pixmap,
) {
    let mut reconstructed = Pixmap::new(w, h).unwrap();
    let pixmap_paint = PixmapPaint::default();

    let mut oy = 0i32;
    while oy < h as i32 {
        let ch = (block_h as i32).min(h as i32 - oy) as u32;
        let mut ox = 0i32;
        while ox < w as i32 {
            let cw = (block_w as i32).min(w as i32 - ox) as u32;
            let chunk = render_chunk(cw, ch, ox, oy, path, paint, stroke, scene_ts);
            assert_matches_scene(full, &chunk, ox, oy);
            reconstructed.draw_pixmap(
                ox,
                oy,
                chunk.as_ref(),
                &pixmap_paint,
                Transform::identity(),
                None,
            );
            ox += block_w as i32;
        }
        oy += block_h as i32;
    }

    assert_eq!(
        &reconstructed, full,
        "pasting exported blocks back must reconstruct the original canvas"
    );
}

// ---------------------------------------------------------------------------
// Texture phase oracle
// ---------------------------------------------------------------------------

#[test]
fn pattern_origin_formula_matches_renderer() {
    let tile = phase_tile();
    let origin = (3, 2);
    let paint = repeat_paint(&tile, Transform::from_translate(3.0, 2.0), false);

    let mut pm = Pixmap::new(40, 32).unwrap();
    pm.fill_rect(
        Rect::from_ltrb(0.0, 0.0, 40.0, 32.0).unwrap(),
        &paint,
        Transform::identity(),
        None,
    );

    for y in 0..32i32 {
        for x in 0..40i32 {
            assert_eq!(
                pm.pixel(x as u32, y as u32).unwrap(),
                scene_texel(origin, x, y),
                "texture phase at scene ({x},{y})"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Flagship: expectations derived from per-coordinate coverage + texture
// ---------------------------------------------------------------------------

#[test]
fn exported_chunk_uses_scene_coverage_and_texture() {
    const W: u32 = 48;
    const H: u32 = 40;
    // Hard, integer-aligned rectangle -> binary coverage `L <= x < R`.
    let (l, t, r, b) = (6i32, 5i32, 42i32, 35i32);

    let tile = phase_tile();
    let origin = (3, 2);
    let paint = repeat_paint(&tile, Transform::from_translate(3.0, 2.0), false);
    let path =
        PathBuilder::from_rect(Rect::from_ltrb(l as f32, t as f32, r as f32, b as f32).unwrap());

    let full = render_full(W, H, &path, &paint, None, Transform::identity());

    // An export origin that is deliberately not a multiple of the 4px period
    // and that straddles the top-left corner of the rectangle, so the block
    // contains a genuine blank margin next to covered texels.
    let (ox, oy, cw, ch) = (2i32, 1i32, 14u32, 12u32);
    let chunk = render_chunk(cw, ch, ox, oy, &path, &paint, None, Transform::identity());

    // Independent coordinate-based expectation.
    for ly in 0..ch as i32 {
        for lx in 0..cw as i32 {
            let sx = ox + lx;
            let sy = oy + ly;
            let covered = l <= sx && sx < r && t <= sy && sy < b;
            let expected = if covered {
                scene_texel(origin, sx, sy)
            } else {
                transparent()
            };
            assert_eq!(
                chunk.pixel(lx as u32, ly as u32).unwrap(),
                expected,
                "block({lx},{ly}) / scene({sx},{sy}) coverage={covered}"
            );
        }
    }

    // And it must agree with the corresponding original-canvas region.
    assert_matches_scene(&full, &chunk, ox, oy);

    // Explicit blank anchors. These block-local pixels map to scene points
    // outside the rectangle's top/left edges and must stay transparent at
    // those scene positions rather than collapsing onto the block origin.
    assert_eq!(chunk.pixel(0, 0).unwrap().alpha(), 0); // scene (2,1)
    assert_eq!(chunk.pixel(3, 3).unwrap().alpha(), 0); // scene (5,4)
    assert_eq!(chunk.pixel(4, 0).unwrap().alpha(), 0); // scene (6,1), above top
    assert_eq!(chunk.pixel(0, 4).unwrap().alpha(), 0); // scene (2,5), left of left edge

    // First fully covered texel: scene (6,5) == block (4,4).
    assert_eq!(chunk.pixel(4, 4).unwrap(), scene_texel(origin, 6, 5));
    assert_eq!(chunk.pixel(4, 4).unwrap().alpha(), 255);
}

// ---------------------------------------------------------------------------
// Block grid export + reassembly
// ---------------------------------------------------------------------------

#[test]
fn block_grid_matches_full_scene_and_reassembles() {
    const W: u32 = 52;
    const H: u32 = 44;

    let tile = phase_tile();
    // Scaled pattern with its own non-grid origin.
    let paint = repeat_paint(
        &tile,
        Transform::from_row(2.0, 0.0, 0.0, 2.0, 4.0, 1.0),
        false,
    );
    let path = PathBuilder::from_circle(26.0, 22.0, 18.0).unwrap();

    let full = render_full(W, H, &path, &paint, None, Transform::identity());

    // Odd block sizes leave partial blocks on the right and bottom edges.
    assert_grid_roundtrip(
        W,
        H,
        15,
        13,
        &path,
        &paint,
        None,
        Transform::identity(),
        &full,
    );
}

// ---------------------------------------------------------------------------
// Cross-block stroke: texture phase must continue across block borders
// ---------------------------------------------------------------------------

#[test]
fn cross_block_stroke_continues_texture_phase() {
    const W: u32 = 48;
    const H: u32 = 48;
    let origin = (5, 7);

    let tile = phase_tile();
    let paint = repeat_paint(&tile, Transform::from_translate(5.0, 7.0), false);

    // One horizontal and one vertical thick stroke crossing several borders.
    let mut pb = PathBuilder::new();
    pb.move_to(-6.0, 24.0);
    pb.line_to(54.0, 26.0);
    pb.move_to(24.0, -6.0);
    pb.line_to(26.0, 54.0);
    let path = pb.finish().unwrap();
    let stroke = Stroke {
        width: 6.0,
        ..Stroke::default()
    };

    let full = render_full(W, H, &path, &paint, Some(&stroke), Transform::identity());

    // Full 3x3 block grid round-trip.
    assert_grid_roundtrip(
        W,
        H,
        16,
        16,
        &path,
        &paint,
        Some(&stroke),
        Transform::identity(),
        &full,
    );

    // An unaligned export straddling the (16,16) cross border.
    let (ox, oy, cw, ch) = (9i32, 9i32, 18u32, 18u32);
    let chunk = render_chunk(
        cw,
        ch,
        ox,
        oy,
        &path,
        &paint,
        Some(&stroke),
        Transform::identity(),
    );
    assert_matches_scene(&full, &chunk, ox, oy);

    // Along the stroke every fully covered block pixel must carry the texel
    // belonging to its *global* scene coordinate, proving the phase was not
    // restarted at the block corner. Non-AA coverage is binary (0 or 255).
    let mut opaque_checked = 0u32;
    for ly in 0..ch as i32 {
        for lx in 0..cw as i32 {
            let sx = ox + lx;
            let sy = oy + ly;
            let scene_pixel = full.pixel(sx as u32, sy as u32).unwrap();
            let block_pixel = chunk.pixel(lx as u32, ly as u32).unwrap();
            if scene_pixel.alpha() == 255 {
                opaque_checked += 1;
                assert_eq!(
                    block_pixel,
                    scene_texel(origin, sx, sy),
                    "stroke texel phase at block({lx},{ly}) / scene({sx},{sy})"
                );
            } else {
                assert_eq!(block_pixel.alpha(), 0);
            }
        }
    }
    assert!(
        opaque_checked > 40,
        "stroke should cover a substantial number of block pixels, got {opaque_checked}"
    );

    // Find an opaque adjacent pair straddling the block border x=16 and
    // confirm both sides use their global phases (continuous tiling).
    let mut cross_pair_checked = false;
    for sy in 18..32i32 {
        let before = full.pixel(15, sy as u32).unwrap();
        let after = full.pixel(16, sy as u32).unwrap();
        if before.alpha() == 255 && after.alpha() == 255 {
            assert_eq!(before, scene_texel(origin, 15, sy));
            assert_eq!(after, scene_texel(origin, 16, sy));
            cross_pair_checked = true;
            break;
        }
    }
    assert!(
        cross_pair_checked,
        "expected an opaque stroke pair across x=16"
    );
}

// ---------------------------------------------------------------------------
// Primitive transform differs from the pattern transform
// ---------------------------------------------------------------------------

#[test]
fn chunk_matches_scene_with_distinct_geometry_and_pattern_transforms() {
    const W: u32 = 56;
    const H: u32 = 48;

    let tile = phase_tile();
    // Pattern carries its own scale + origin, unrelated to the geometry.
    let p0 = Transform::from_row(2.0, 0.0, 0.0, 2.0, 5.0, 3.0);
    let paint = repeat_paint(&tile, p0, false);

    // Primitive is positioned by a different transform.
    let geometry_ts = Transform::from_translate(4.0, 2.0);
    let path = PathBuilder::from_rect(Rect::from_ltrb(6.0, 6.0, 44.0, 36.0).unwrap());

    let full = render_full(W, H, &path, &paint, None, geometry_ts);

    for (ox, oy, cw, ch) in [
        (0i32, 0i32, 18u32, 16u32),
        (11, 7, 20, 18),
        (30, 22, 22, 20),
    ] {
        let chunk = render_chunk(cw, ch, ox, oy, &path, &paint, None, geometry_ts);
        assert_matches_scene(&full, &chunk, ox, oy);
    }

    assert_grid_roundtrip(W, H, 17, 15, &path, &paint, None, geometry_ts, &full);
}

#[test]
fn global_pattern_origin_survives_independent_primitive_transform() {
    // The pattern is anchored in *scene* space (`p_global`), while the
    // primitive is placed with a separate transform `geometry_ts`.
    // Compensating the pattern constructor by `G^-1` makes tiny-skia compose
    // `G` back to `p_global`; block export must preserve that global phase.
    const W: u32 = 52;
    const H: u32 = 44;

    let tile = phase_tile();
    let p_global = Transform::from_row(2.0, 0.0, 0.0, 2.0, 3.0, 2.0);
    let geometry_ts = Transform::from_translate(6.0, 4.0);
    let p_constructor = geometry_ts.invert().unwrap().pre_concat(p_global);

    let paint = repeat_paint(&tile, p_constructor, false);
    // Path-local rect; after G it occupies scene [14,46) x [12,36).
    let path = PathBuilder::from_rect(Rect::from_ltrb(8.0, 8.0, 40.0, 32.0).unwrap());

    let full = render_full(W, H, &path, &paint, None, geometry_ts);

    let (ox, oy, cw, ch) = (10i32, 9i32, 18u32, 14u32);
    let chunk = render_chunk(cw, ch, ox, oy, &path, &paint, None, geometry_ts);
    assert_matches_scene(&full, &chunk, ox, oy);

    // Coordinate-based expectation using the *global* scaled pattern origin.
    for ly in 0..ch as i32 {
        for lx in 0..cw as i32 {
            let sx = ox + lx;
            let sy = oy + ly;
            let covered = 14 <= sx && sx < 46 && 12 <= sy && sy < 36;
            let pixel = chunk.pixel(lx as u32, ly as u32).unwrap();
            if covered {
                assert_eq!(
                    pixel,
                    scene_texel_scaled((3, 2), 2, sx, sy),
                    "global phase at block({lx},{ly}) / scene({sx},{sy})"
                );
            } else {
                assert_eq!(pixel.alpha(), 0);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Blank / empty selection regions stay at their original scene position
// ---------------------------------------------------------------------------

#[test]
fn blank_selection_pixels_stay_at_scene_position() {
    const W: u32 = 60;
    const H: u32 = 48;

    let tile = phase_tile();
    let paint = repeat_paint(&tile, Transform::from_translate(2.0, 6.0), false);
    // A single small square at scene [30,34) x [20,24).
    let path = PathBuilder::from_rect(Rect::from_ltrb(30.0, 20.0, 34.0, 24.0).unwrap());

    let full = render_full(W, H, &path, &paint, None, Transform::identity());

    // Export [21,41) x [13,29). The origin is deliberately not a multiple of
    // the 4px pattern period, so re-basing phase onto the block corner is also
    // caught. The square lands at block-local [9,13)x[7,11).
    let (ox, oy, cw, ch) = (21i32, 13i32, 20u32, 16u32);
    let chunk = render_chunk(cw, ch, ox, oy, &path, &paint, None, Transform::identity());
    assert_matches_scene(&full, &chunk, ox, oy);

    // Far corners of the selection are empty.
    assert_eq!(chunk.pixel(0, 0).unwrap().alpha(), 0); // scene (21,13)
    assert_eq!(chunk.pixel(19, 15).unwrap().alpha(), 0); // scene (40,28)

    // The mark is at its *offset* block position, not pulled to (0,0).
    assert_eq!(chunk.pixel(9, 7).unwrap().alpha(), 255); // scene (30,20)
    assert_eq!(chunk.pixel(9, 7).unwrap(), scene_texel((2, 6), 30, 20));
    assert_eq!(chunk.pixel(8, 7).unwrap().alpha(), 0); // immediately left
    assert_eq!(chunk.pixel(9, 6).unwrap().alpha(), 0); // immediately above
    assert_eq!(chunk.pixel(12, 10).unwrap().alpha(), 255); // scene (33,23)
    assert_eq!(chunk.pixel(13, 11).unwrap().alpha(), 0); // just past bottom-right
}

// ---------------------------------------------------------------------------
// Anti-aliased export via a gutter
// ---------------------------------------------------------------------------

/// Renders a block with `gutter` pixels of overdraw on every side and crops
/// the requested region. This keeps anti-aliased scan conversion away from the
/// temporary pixmap's clip edge, yielding bit-identical scene coverage.
fn render_chunk_with_gutter(
    cw: u32,
    ch: u32,
    ox: i32,
    oy: i32,
    gutter: i32,
    path: &Path,
    paint: &Paint,
) -> Pixmap {
    let gw = cw + 2 * gutter as u32;
    let gh = ch + 2 * gutter as u32;
    let mut big = Pixmap::new(gw, gh).unwrap();
    let ts = Transform::from_translate(gutter as f32 - ox as f32, gutter as f32 - oy as f32);
    big.fill_path(path, paint, FillRule::Winding, ts, None);
    big.as_ref()
        .clone_rect(IntRect::from_xywh(gutter, gutter, cw, ch).unwrap())
        .unwrap()
}

#[test]
fn anti_aliased_chunk_matches_scene_coverage() {
    let tile = phase_tile();
    // Integer translate keeps nearest sampling exactly phase-stable.
    let paint = repeat_paint(&tile, Transform::from_translate(4.0, 1.0), true);
    let path = PathBuilder::from_circle(30.0, 24.0, 19.0).unwrap();

    let full = render_full(60, 48, &path, &paint, None, Transform::identity());

    // Includes the 17x13 block whose bottom/right edge cuts through an
    // anti-aliased arc.
    for (ox, oy, cw, ch) in [
        (0i32, 0i32, 17u32, 13u32),
        (13, 9, 22, 18),
        (25, 18, 20, 16),
        (40, 30, 18, 16),
    ] {
        let chunk = render_chunk_with_gutter(cw, ch, ox, oy, 8, &path, &paint);
        assert_matches_scene(&full, &chunk, ox, oy);
    }
}

// ---------------------------------------------------------------------------
// Internal DrawTiler boundary (>8191px) phase + blank preservation
// ---------------------------------------------------------------------------

#[test]
fn internal_tiler_keeps_horizontal_phase_across_blocks() {
    // Three internal blocks: 8191 + 8191 + 5.
    let w = 8191u32 * 2 + 5;
    let h = 24u32;
    let origin = (5, 3);

    let tile = phase_tile();
    let paint = repeat_paint(&tile, Transform::from_translate(5.0, 3.0), false);

    let mut pixmap = Pixmap::new(w, h).unwrap();
    // Full-width band y in [5,19); blank rows above/below.
    pixmap.fill_rect(
        Rect::from_ltrb(0.0, 5.0, w as f32, 19.0).unwrap(),
        &paint,
        Transform::identity(),
        None,
    );

    // Columns straddling both internal 8191-block borders.
    for sx in [
        8189i32, 8190, 8191, 8192, 8193, 16380, 16381, 16382, 16383, 16384,
    ] {
        for sy in [6i32, 12, 18] {
            assert_eq!(
                pixmap.pixel(sx as u32, sy as u32).unwrap(),
                scene_texel(origin, sx, sy),
                "horizontal tiler phase at ({sx},{sy})"
            );
        }
        for sy in [0i32, 3, 20, 23] {
            assert_eq!(
                pixmap.pixel(sx as u32, sy as u32).unwrap().alpha(),
                0,
                "blank row not preserved at ({sx},{sy})"
            );
        }
    }
}

#[test]
fn internal_tiler_keeps_vertical_phase_across_blocks() {
    let w = 24u32;
    let h = 8191u32 * 2 + 5;
    let origin = (2, 4);

    let tile = phase_tile();
    let paint = repeat_paint(&tile, Transform::from_translate(2.0, 4.0), false);

    let mut pixmap = Pixmap::new(w, h).unwrap();
    // Full-height band x in [5,19); blank columns left/right.
    pixmap.fill_rect(
        Rect::from_ltrb(5.0, 0.0, 19.0, h as f32).unwrap(),
        &paint,
        Transform::identity(),
        None,
    );

    // Rows straddling both internal borders, sampled inside the band.
    for sy in [
        8189i32, 8190, 8191, 8192, 8193, 16380, 16381, 16382, 16383, 16384,
    ] {
        for sx in [6i32, 11, 18] {
            assert_eq!(
                pixmap.pixel(sx as u32, sy as u32).unwrap(),
                scene_texel(origin, sx, sy),
                "vertical tiler phase at ({sx},{sy})"
            );
        }
        for sx in [0i32, 2, 20, 23] {
            assert_eq!(
                pixmap.pixel(sx as u32, sy as u32).unwrap().alpha(),
                0,
                "blank column not preserved at ({sx},{sy})"
            );
        }
    }
}
