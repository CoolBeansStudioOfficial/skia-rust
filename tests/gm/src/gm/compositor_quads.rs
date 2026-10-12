// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/compositor_quads.cpp (chrome/m156)
//
// skia-rust: the "image" grid's YUV renderer draws nothing on raster (there is no GPU for its
// `LazyYUVImage`), as DM's raster config does; see `YuvTextureSetRenderer`.

// Casts and names mirror the C++ arithmetic and loop counters this file ports, so they stay
// as written in the source.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::similar_names,
    clippy::too_many_lines
)]

use std::cell::RefCell;

use crate::prelude::*;
use crate::tool_utils::{get_resource_as_data, get_resource_as_image};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{ImageSetEntry, QuadAAFlags, SrcRectConstraint};
use skia_rust_core::color::{Color4f, colors};
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::floating_point::float_round2int;
use skia_rust_core::font::Font;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::line_clipper::intersect_line;
use skia_rust_core::m44::M44;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::point::Point;
use skia_rust_core::rect::IRect;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::FilterMode;
use skia_rust_core::scalar::{Scalar, scalar};
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_effects::image_filters::dilate;
use skia_rust_effects::shader_mask_filter;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

const TILE_WIDTH: scalar = 40.0;
const TILE_HEIGHT: scalar = 30.0;
const ROW_COUNT: i32 = 4;
const COL_COUNT: i32 = 3;

// Point indices of the clipping quad (`kTL`, `kTR`, `kBR`, `kBL`, `kS0`, `kS1`).
const K_TL: usize = 0;
const K_TR: usize = 1;
const K_BR: usize = 2;
const K_BL: usize = 3;
const K_S0: usize = 4;
const K_S1: usize = 5;

/// The three points of the BSP clipping triangle (`kClipP1..kClipP3`).
// To mimic Chromium's BSP clipping strategy, a set of three lines formed by triangle edges of
// the below points are used to clip against the regular tile grid.
fn clip_points() -> [Point; 3] {
    [
        Point::new(1.75 * TILE_WIDTH, 0.8 * TILE_HEIGHT),
        Point::new(0.6 * TILE_WIDTH, 2.0 * TILE_HEIGHT),
        Point::new(2.9 * TILE_WIDTH, 3.5 * TILE_HEIGHT),
    ]
}

// Port of: gm/compositor_quads.cpp#L78-L86 (chrome/m156)
fn clipping_line_segment(p0: Point, p1: Point) -> [Point; 2] {
    let v = p1 - p0;
    // 10f was chosen as a balance between large enough to scale the currently set clip points
    // outside of the tile grid, but small enough to preserve precision.
    [p0 - v * 10.0, p1 + v * 10.0]
}

// Port of: gm/compositor_quads.cpp#L88-L137 (chrome/m156)
// Returns the intersection point if line segment (p0-p1) intersects with line segment (l0-l1).
fn intersect_line_segments(p0: Point, p1: Point, l0: Point, l1: Point) -> Option<Point> {
    const HORIZONTAL_TOLERANCE: scalar = 0.01; // Pretty conservative

    // Use doubles for accuracy, since the clipping strategy used below can create T junctions,
    // and lower precision could artificially create gaps
    let p_y = f64::from(p1.y) - f64::from(p0.y);
    let p_x = f64::from(p1.x) - f64::from(p0.x);
    let l_y = f64::from(l1.y) - f64::from(l0.y);
    let l_x = f64::from(l1.x) - f64::from(l0.x);
    let pl_y = f64::from(p0.y) - f64::from(l0.y);
    let pl_x = f64::from(p0.x) - f64::from(l0.x);

    // (SkScalarNearlyZero takes a float, so the doubles are narrowed first.)
    if (p_y as scalar).nearly_zero(HORIZONTAL_TOLERANCE) {
        if (l_y as scalar).nearly_zero(HORIZONTAL_TOLERANCE) {
            // Two horizontal lines
            return None;
        }
        // Recalculate but swap p and l
        return intersect_line_segments(l0, l1, p0, p1);
    }

    // Up to now, the line segments do not form an invalid intersection
    let l_numerator = pl_x * p_y - pl_y * p_x;
    let l_denom = l_x * p_y - l_y * p_x;
    if (l_denom as scalar).nearly_zero(None) {
        // Parallel or identical
        return None;
    }

    // Calculate alphaL that provides the intersection point along (l0-l1), e.g. l0+alphaL*(l1-l0)
    let alpha_l = l_numerator / l_denom;
    if !(0.0..=1.0).contains(&alpha_l) {
        // Outside of the l segment
        return None;
    }

    // Calculate alphaP from the valid alphaL (since it could be outside p segment)
    let alpha_p = (alpha_l * l_y - pl_y) / p_y;
    if !(0.0..=1.0).contains(&alpha_p) {
        // Outside of p segment
        return None;
    }

    // Is valid, so calculate the actual intersection point
    Some(l1 * (alpha_l as scalar) + l0 * ((1.0 - alpha_l) as scalar))
}

// Draw a line through the two points, outset by a fixed length in screen space
// Port of: gm/compositor_quads.cpp#L139-L148 (chrome/m156)
fn draw_outset_line(canvas: &Canvas, local: &Matrix, pts: &[Point; 2], paint: &Paint) {
    const LINE_OUTSET: scalar = 10.0;
    let mut mapped = [Point::default(); 2];
    local.map_points(&mut mapped, pts);
    let mut v = mapped[1] - mapped[0];
    v.set_length(v.length() + LINE_OUTSET);
    canvas.draw_line(mapped[1] - v, mapped[0] + v, paint);
}

// Draw grid of red lines at interior tile boundaries.
// Port of: gm/compositor_quads.cpp#L150-L165 (chrome/m156)
fn draw_tile_boundaries(canvas: &Canvas, local: &Matrix) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::RED);
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(0.0);
    for x in 1..COL_COUNT {
        let pts = [
            Point::new(x as scalar * TILE_WIDTH, 0.0),
            Point::new(x as scalar * TILE_WIDTH, ROW_COUNT as scalar * TILE_HEIGHT),
        ];
        draw_outset_line(canvas, local, &pts, &paint);
    }
    for y in 1..ROW_COUNT {
        let pts = [
            Point::new(0.0, y as scalar * TILE_HEIGHT),
            Point::new(TILE_WIDTH * COL_COUNT as scalar, y as scalar * TILE_HEIGHT),
        ];
        draw_outset_line(canvas, local, &pts, &paint);
    }
}

// Draw the arbitrary clipping/split boundaries that intersect the tile grid as green lines
// Port of: gm/compositor_quads.cpp#L167-L193 (chrome/m156)
fn draw_clipping_boundaries(canvas: &Canvas, local: &Matrix) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::GREEN);
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(0.0);

    // Clip the "infinite" line segments to a rectangular region outside the tile grid
    let border = Rect::from_wh(
        TILE_WIDTH * COL_COUNT as scalar,
        TILE_HEIGHT * ROW_COUNT as scalar,
    );
    let [p1, p2, p3] = clip_points();

    // Draw p1 to p2, p2 to p3, and p3 to p1
    for (a, b) in [(p1, p2), (p2, p3), (p3, p1)] {
        let line = clipping_line_segment(a, b);
        let clipped = intersect_line(&line, &border).expect("SkAssertResult(IntersectLine)");
        draw_outset_line(canvas, local, &clipped, &paint);
    }
}

// Port of: gm/compositor_quads.cpp#L195-L198 (chrome/m156)
fn draw_text(canvas: &Canvas, text: &str) {
    let font = Font::from_size(default_portable_typeface(), 12.0);
    canvas.draw_str(text, (0.0, 0.0), &font, &Paint::default());
}

/// A tile renderer: draws the base rect, possibly clipped by a quad (`ClipTileRenderer`).
// Port of: gm/compositor_quads.cpp#L205-L226 (chrome/m156)
trait ClipTileRenderer {
    /// Draws the base rect, possibly clipped by `clip`. The edges to anti-alias are in `edge_aa`
    /// (top, right, bottom, left), in the same order as `clip`. Returns the draw count.
    fn draw_tile(
        &self,
        canvas: &Canvas,
        rect: &Rect,
        clip: Option<&[Point; 4]>,
        edge_aa: [bool; 4],
        tile_id: i32,
        quad_id: i32,
    ) -> i32;

    fn draw_banner(&self, canvas: &Canvas);

    /// Draws the tile grid, split by the three clipping lines. Returns the draw count.
    // Port of: gm/compositor_quads.cpp#L220-L252 (chrome/m156)
    fn draw_tiles(&self, canvas: &Canvas) -> i32 {
        draw_tile_grid(self, canvas)
    }
}

/// The grid loop of `ClipTileRenderer::drawTiles` (`INHERITED::drawTiles`): every tile of the
/// grid, split by the three clipping lines, drawn with `draw_tile`. A renderer that overrides
/// `draw_tiles` calls it for the grid and then flushes what it accumulated.
// Port of: gm/compositor_quads.cpp#L220-L252 (chrome/m156)
fn draw_tile_grid(renderer: &(impl ClipTileRenderer + ?Sized), canvas: &Canvas) -> i32 {
    // All three lines in a list
    let [p1, p2, p3] = clip_points();
    let mut lines = Vec::with_capacity(6);
    lines.extend(clipping_line_segment(p1, p2));
    lines.extend(clipping_line_segment(p2, p3));
    lines.extend(clipping_line_segment(p3, p1));

    let mut tile_id = 0;
    let mut draw_count = 0;
    for i in 0..ROW_COUNT {
        for j in 0..COL_COUNT {
            // The unclipped tile geometry
            let tile = Rect::from_xywh(
                j as scalar * TILE_WIDTH,
                i as scalar * TILE_HEIGHT,
                TILE_WIDTH,
                TILE_HEIGHT,
            );
            // Base edge AA flags if there are no clips; clipped lines will only turn off edges
            let edge_aa = [
                i == 0,             // Top
                j == COL_COUNT - 1, // Right
                i == ROW_COUNT - 1, // Bottom
                j == 0,             // Left
            ];

            // Now clip against the 3 lines formed by kClipPx and split into general purpose
            // quads as needed.
            let mut quad_count = 0;
            draw_count += clip_tile(
                renderer,
                canvas,
                tile_id,
                &tile,
                None,
                edge_aa,
                &lines,
                &mut quad_count,
            );
            tile_id += 1;
        }
    }
    draw_count
}

/// `maskToFlags`: the AA flags for the edges in `edge_aa` (top, right, bottom, left).
// Port of: gm/compositor_quads.cpp#L254-L262 (chrome/m156)
fn mask_to_flags(edge_aa: [bool; 4]) -> QuadAAFlags {
    let mut flags = QuadAAFlags::NONE;
    if edge_aa[0] {
        flags |= QuadAAFlags::TOP;
    }
    if edge_aa[1] {
        flags |= QuadAAFlags::RIGHT;
    }
    if edge_aa[2] {
        flags |= QuadAAFlags::BOTTOM;
    }
    if edge_aa[3] {
        flags |= QuadAAFlags::LEFT;
    }
    flags
}

/// `ClipTileRenderer::clipTile`: recursively splits `quad` (or `base_rect` if it is `None`)
/// against the segments in `lines` (two points per line), and draws the leaves with `draw_tile`.
// Port of: gm/compositor_quads.cpp#L265-L416 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn clip_tile(
    renderer: &(impl ClipTileRenderer + ?Sized),
    canvas: &Canvas,
    tile_id: i32,
    base_rect: &Rect,
    quad: Option<&[Point; 4]>,
    edge_aa: [bool; 4],
    lines: &[Point],
    quad_count: &mut i32,
) -> i32 {
    if lines.is_empty() {
        // No lines, so end recursion by drawing the tile. If the tile was never split then
        // 'quad' remains null so that drawTile() can differentiate how it should draw.
        let draws = renderer.draw_tile(canvas, base_rect, quad, edge_aa, tile_id, *quad_count);
        *quad_count += 1;
        return draws;
    }

    let mut points = [Point::default(); 6];
    if let Some(quad) = quad {
        // Copy the original 4 points into set of points to consider
        points[..4].copy_from_slice(quad);
    } else {
        // Haven't been split yet, so fill in based on the rect
        base_rect.copy_to_quad(&mut points, PathDirection::CW);
    }

    // Consider the first line against the 4 quad edges in tile, which should have 0,1, or 2
    // intersection points since the tile is convex.
    let mut split_indices = [0_usize; 2]; // Edge that was intersected
    let mut intersection_count = 0_usize;
    for i in 0..4 {
        let next = if i == 3 { 0 } else { i + 1 };
        if let Some(intersect) =
            intersect_line_segments(points[i], points[next], lines[0], lines[1])
        {
            // If the intersected point is the same as the last found intersection, the line runs
            // through a vertex, so don't double count it
            let duplicate = (0..intersection_count)
                .any(|j| (intersect - points[K_S0 + j]).length().nearly_zero(None));
            if !duplicate {
                points[K_S0 + intersection_count] = intersect;
                split_indices[intersection_count] = i;
                intersection_count += 1;
            }
        }
    }

    if intersection_count < 2 {
        // Either the first line never intersected the quad (count == 0), or it intersected at a
        // single vertex without going through quad area (count == 1), so check next line
        return clip_tile(
            renderer,
            canvas,
            tile_id,
            base_rect,
            quad,
            edge_aa,
            &lines[2..],
            quad_count,
        );
    }
    debug_assert_eq!(intersection_count, 2);

    // Split the tile points into 2+ sub quads and recurse to the next lines, which may or may
    // not further split the tile. Since the configurations are relatively simple, the possible
    // splits are hardcoded below; subtile quad orderings are such that the sub tiles remain in
    // clockwise order and match expected edges for QuadAAFlags. subtile indices refer to the
    // 6-element 'points' array.
    let mut subtiles: Vec<[usize; 4]> = Vec::with_capacity(3);
    let mut s2: Option<usize> = None; // Index of an original vertex chosen for a artificial split
    if split_indices[1] - split_indices[0] == 2 {
        // Opposite edges, so the split trivially forms 2 sub quads
        if split_indices[0] == 0 {
            subtiles.push([K_TL, K_S0, K_S1, K_BL]);
            subtiles.push([K_S0, K_TR, K_BR, K_S1]);
        } else {
            subtiles.push([K_TL, K_TR, K_S0, K_S1]);
            subtiles.push([K_S1, K_S0, K_BR, K_BL]);
        }
    } else {
        // Adjacent edges, which makes for a more complicated split, since it forms a degenerate
        // quad (triangle) and a pentagon that must be artificially split. The pentagon is split
        // using one of the original vertices (remembered in 's2'), which adds an additional
        // degenerate quad, but ensures there are no T-junctions.
        match split_indices[0] {
            0 => {
                // Could be connected to edge 1 or edge 3
                if split_indices[1] == 1 {
                    s2 = Some(K_BL);
                    subtiles.push([K_S0, K_TR, K_S1, K_S0]); // degenerate
                    let c = if edge_aa[0] { K_S0 } else { K_BL };
                    subtiles.push([K_TL, K_S0, c, K_BL]); // degenerate
                    subtiles.push([K_S0, K_S1, K_BR, K_BL]);
                } else {
                    debug_assert_eq!(split_indices[1], 3);
                    s2 = Some(K_BR);
                    subtiles.push([K_TL, K_S0, K_S1, K_S1]); // degenerate
                    let c = if edge_aa[3] { K_S1 } else { K_BR };
                    subtiles.push([K_S1, c, K_BR, K_BL]); // degenerate
                    subtiles.push([K_S0, K_TR, K_BR, K_S1]);
                }
            }
            1 => {
                // Edge 0 handled above, should only be connected to edge 2
                debug_assert_eq!(split_indices[1], 2);
                s2 = Some(K_TL);
                subtiles.push([K_S0, K_S0, K_BR, K_S1]); // degenerate
                let c = if edge_aa[1] { K_S0 } else { K_TL };
                subtiles.push([K_TL, K_TR, K_S0, c]); // degenerate
                subtiles.push([K_TL, K_S0, K_S1, K_BL]);
            }
            2 => {
                // Edge 1 handled above, should only be connected to edge 3
                debug_assert_eq!(split_indices[1], 3);
                s2 = Some(K_TR);
                subtiles.push([K_S1, K_S0, K_S0, K_BL]); // degenerate
                let c = if edge_aa[2] { K_S0 } else { K_TR };
                subtiles.push([c, K_TR, K_BR, K_S0]); // degenerate
                subtiles.push([K_TL, K_TR, K_S0, K_S1]);
            }
            _ => {
                // Fall through, an adjacent edge split that hits edge 3 should have first found
                // been found with edge 0 or edge 2 for the other end
                debug_assert!(false, "adjacent split on edge 3");
                return 0;
            }
        }
    }

    let mut draws = 0;
    for subtile in &subtiles {
        // Fill in the quad points and update edge AA rules for new interior edges
        let mut sub = [Point::default(); 4];
        let mut sub_aa = [false; 4];
        for j in 0..4 {
            let p = subtile[j];
            sub[j] = points[p];
            let np = if j == 3 { subtile[0] } else { subtile[j + 1] };
            // The "new" edges are the edges that connect between the two split points or between
            // a split point and the chosen s2 point. Otherwise the edge remains aligned with the
            // original shape, so should preserve the AA setting.
            sub_aa[j] = if (p >= K_S0 && (s2 == Some(np) || np >= K_S0))
                || (np >= K_S0 && (s2 == Some(p) || p >= K_S0))
            {
                // New edge
                false
            } else {
                edge_aa[j]
            };
        }

        // Split the sub quad with the next line
        draws += clip_tile(
            renderer,
            canvas,
            tile_id,
            base_rect,
            Some(&sub),
            sub_aa,
            &lines[2..],
            quad_count,
        );
    }
    draws
}

/// The grid of tiles drawn by one renderer, with its banner (`CompositorGM` renders one row per
/// renderer).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DebugTileRenderer {
    aa_override: QuadAAFlags,
    enable_aa_override: bool,
}

impl DebugTileRenderer {
    // Port of: gm/compositor_quads.cpp#L554-L570 (chrome/m156)
    /// Since the AA override is disabled, the quad flags argument does not matter.
    fn make() -> Self {
        Self {
            aa_override: QuadAAFlags::ALL,
            enable_aa_override: false,
        }
    }

    fn make_aa() -> Self {
        Self {
            aa_override: QuadAAFlags::ALL,
            enable_aa_override: true,
        }
    }

    fn make_non_aa() -> Self {
        Self {
            aa_override: QuadAAFlags::NONE,
            enable_aa_override: true,
        }
    }
}

impl ClipTileRenderer for DebugTileRenderer {
    // Port of: gm/compositor_quads.cpp#L570-L587 (chrome/m156)
    fn draw_tile(
        &self,
        canvas: &Canvas,
        rect: &Rect,
        clip: Option<&[Point; 4]>,
        edge_aa: [bool; 4],
        tile_id: i32,
        quad_id: i32,
    ) -> i32 {
        // Colorize the tile based on its grid position and quad ID
        let i = tile_id / COL_COUNT;
        let j = tile_id % COL_COUNT;

        let mut c = Color4f::new(
            (i as f32 + 1.0) / ROW_COUNT as f32,
            (j as f32 + 1.0) / COL_COUNT as f32,
            0.4,
            1.0,
        );
        let alpha = quad_id as f32 / 10.0;
        c.r = c.r * (1.0 - alpha) + alpha;
        c.g = c.g * (1.0 - alpha) + alpha;
        c.b = c.b * (1.0 - alpha) + alpha;
        c.a = c.a * (1.0 - alpha) + alpha;

        let aa_flags = if self.enable_aa_override {
            self.aa_override
        } else {
            mask_to_flags(edge_aa)
        };
        canvas.experimental_draw_edge_aa_quad(
            rect,
            clip,
            aa_flags,
            c.to_color(),
            BlendMode::SrcOver,
        );
        1
    }

    // Port of: gm/compositor_quads.cpp#L589-L619 (chrome/m156)
    fn draw_banner(&self, canvas: &Canvas) {
        draw_text(canvas, "Edge AA");
        canvas.translate((0.0, 15.0));

        let config = if self.enable_aa_override {
            assert!(
                self.aa_override == QuadAAFlags::ALL || self.aa_override == QuadAAFlags::NONE,
                "the override is all or none"
            );
            if self.aa_override == QuadAAFlags::ALL {
                "Ext(yes) - Int(yes)"
            } else {
                "Ext(no) - Int(no)"
            }
        } else {
            "Ext(yes) - Int(no)"
        };
        draw_text(canvas, config);
    }
}

/// A single solid color (`SolidColorRenderer`): tests `experimental_DrawEdgeAAQuad`.
// Port of: gm/compositor_quads.cpp#L621-L645 (chrome/m156)
struct SolidColorRenderer {
    color: Color4f,
}

impl ClipTileRenderer for SolidColorRenderer {
    fn draw_tile(
        &self,
        canvas: &Canvas,
        rect: &Rect,
        clip: Option<&[Point; 4]>,
        edge_aa: [bool; 4],
        _tile_id: i32,
        _quad_id: i32,
    ) -> i32 {
        canvas.experimental_draw_edge_aa_quad(
            rect,
            clip,
            mask_to_flags(edge_aa),
            self.color.to_color(),
            BlendMode::SrcOver,
        );
        1
    }

    fn draw_banner(&self, canvas: &Canvas) {
        draw_text(canvas, "Solid Color");
    }
}

type RendererFactory = fn() -> Vec<Box<dyn ClipTileRenderer>>;

/// `TextureSetRenderer` (compositor_quads.cpp#L648-L892): "tests `drawEdgeAAImageSet()`, but can
/// batch the entries together in different ways". The tiles are accumulated as image-set entries
/// and drawn with `experimental_DrawEdgeAAImageSet`, flushed after each quad or after each batch of
/// `transform_batch_count` transforms.
// Port of: gm/compositor_quads.cpp#L648-L892 (chrome/m156)
struct TextureSetRenderer {
    top_banner: String,
    bottom_banner: String,
    image: Image,
    shader: Option<Shader>,
    color_filter: Option<ColorFilter>,
    image_filter: Option<ImageFilter>,
    mask_filter: Option<MaskFilter>,
    paint_alpha: f32,
    reset_each_quad: bool,
    transform_batch_count: i32,
    // The mutable accumulators of the C++ class (`drawTile` and `drawAndReset` change them).
    state: RefCell<TextureSetState>,
}

/// The accumulated set of `TextureSetRenderer` (`fDstClips`, `fPreViewMatrices`, `fSetEntries`,
/// `fBaseCTM`, `fBatchCount`).
#[derive(Default)]
struct TextureSetState {
    dst_clips: Vec<Point>,
    pre_view_matrices: Vec<Matrix>,
    set_entries: Vec<ImageSetEntry>,
    base_ctm: Matrix,
    batch_count: i32,
}

impl TextureSetRenderer {
    // Port of: TextureSetRenderer::Make (compositor_quads.cpp#L683-L700)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    fn make(
        top_banner: &str,
        bottom_banner: &str,
        image: Image,
        shader: Option<Shader>,
        color_filter: Option<ColorFilter>,
        image_filter: Option<ImageFilter>,
        mask_filter: Option<MaskFilter>,
        paint_alpha: f32,
        reset_after_each_quad: bool,
        transform_count: i32,
    ) -> Box<dyn ClipTileRenderer> {
        Box::new(Self {
            top_banner: top_banner.to_owned(),
            bottom_banner: bottom_banner.to_owned(),
            image,
            shader,
            color_filter,
            image_filter,
            mask_filter,
            paint_alpha,
            reset_each_quad: reset_after_each_quad,
            transform_batch_count: transform_count,
            state: RefCell::new(TextureSetState::default()),
        })
    }

    // Port of: TextureSetRenderer::MakeUnbatched (compositor_quads.cpp#L655-L658)
    fn make_unbatched(image: Image) -> Box<dyn ClipTileRenderer> {
        Self::make("Texture", "", image, None, None, None, None, 1.0, true, 0)
    }

    // Port of: TextureSetRenderer::MakeBatched (compositor_quads.cpp#L660-L665)
    fn make_batched(image: Image, transform_count: i32) -> Box<dyn ClipTileRenderer> {
        let subtitle = if transform_count == 0 {
            ""
        } else {
            "w/ xforms"
        };
        Self::make(
            "Texture Set",
            subtitle,
            image,
            None,
            None,
            None,
            None,
            1.0,
            false,
            transform_count,
        )
    }

    // Port of: TextureSetRenderer::MakeShader (compositor_quads.cpp#L667-L672)
    fn make_shader(
        name: &str,
        image: Image,
        shader: Shader,
        local: bool,
    ) -> Box<dyn ClipTileRenderer> {
        Self::make(
            "Shader",
            name,
            image,
            Some(shader),
            None,
            None,
            None,
            1.0,
            local,
            0,
        )
    }

    // Port of: TextureSetRenderer::MakeColorFilter (compositor_quads.cpp#L674-L678)
    fn make_color_filter(
        name: &str,
        image: Image,
        filter: ColorFilter,
    ) -> Box<dyn ClipTileRenderer> {
        Self::make(
            "Color Filter",
            name,
            image,
            None,
            Some(filter),
            None,
            None,
            1.0,
            false,
            0,
        )
    }

    // Port of: TextureSetRenderer::MakeImageFilter (compositor_quads.cpp#L680-L684)
    fn make_image_filter(
        name: &str,
        image: Image,
        filter: ImageFilter,
    ) -> Box<dyn ClipTileRenderer> {
        Self::make(
            "Image Filter",
            name,
            image,
            None,
            None,
            Some(filter),
            None,
            1.0,
            false,
            0,
        )
    }

    // Port of: TextureSetRenderer::MakeMaskFilter (compositor_quads.cpp#L686-L690)
    fn make_mask_filter(name: &str, image: Image, filter: MaskFilter) -> Box<dyn ClipTileRenderer> {
        Self::make(
            "Mask Filter",
            name,
            image,
            None,
            None,
            None,
            Some(filter),
            1.0,
            false,
            0,
        )
    }

    // Port of: TextureSetRenderer::MakeAlpha (compositor_quads.cpp#L692-L696)
    fn make_alpha(image: Image, alpha: f32) -> Box<dyn ClipTileRenderer> {
        Self::make(
            "Alpha",
            &format!("a = {alpha:.2}"),
            image,
            None,
            None,
            None,
            None,
            alpha,
            false,
            0,
        )
    }

    // Port of: TextureSetRenderer::configureTilePaint (compositor_quads.cpp#L782-L811)
    fn configure_tile_paint(&self, rect: &Rect) -> Paint {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_blend_mode(BlendMode::SrcOver);

        // Send non-white RGB, that should be ignored
        paint.set_color4f(Color4f::new(1.0, 0.4, 0.25, self.paint_alpha), None);

        if let Some(shader) = &self.shader {
            if self.reset_each_quad {
                // Apply a local transform in the shader to map from the tile rectangle to (0,0,w,h)
                let target = Rect::from_wh(TILE_WIDTH, TILE_HEIGHT);
                let local = Matrix::rect_to_rect_or_identity(target, rect, None);
                paint.set_shader(shader.with_local_matrix(&local));
            } else {
                paint.set_shader(shader.clone());
            }
        }

        paint.set_color_filter(self.color_filter.clone());
        paint.set_image_filter(self.image_filter.clone());
        paint.set_mask_filter(self.mask_filter.clone());
        paint
    }

    // Port of: TextureSetRenderer::drawAndReset (compositor_quads.cpp#L813-L860)
    fn draw_and_reset(&self, canvas: &Canvas) -> i32 {
        let mut state = self.state.borrow_mut();
        // Early out if there's nothing to draw
        if state.set_entries.is_empty() {
            return 0;
        }

        if !self.reset_each_quad && self.transform_batch_count > 0 {
            // A batch is completed
            state.batch_count += 1;
            if state.batch_count < self.transform_batch_count {
                // Haven't hit the point to submit yet, but end the current tile
                return 0;
            }

            // Submitting all tiles back to where fBaseCTM was the canvas' matrix, while the canvas
            // currently has the CTM of the last tile batch, so reset it.
            canvas.set_matrix(&M44::from(&state.base_ctm));
        }

        let last_tile_rect = state.set_entries[state.set_entries.len() - 1].dst_rect;
        let paint = self.configure_tile_paint(&last_tile_rect);

        canvas.experimental_draw_edge_aa_image_set(
            &state.set_entries,
            &state.dst_clips,
            &state.pre_view_matrices,
            FilterMode::Linear,
            Some(&paint),
            SrcRectConstraint::Fast,
        );

        // Reset for next tile
        state.dst_clips.clear();
        state.pre_view_matrices.clear();
        state.set_entries.clear();
        state.batch_count = 0;

        1
    }
}

impl ClipTileRenderer for TextureSetRenderer {
    // Port of: TextureSetRenderer::drawTiles (compositor_quads.cpp#L727-L732)
    fn draw_tiles(&self, canvas: &Canvas) -> i32 {
        let mut draws = draw_tile_grid(self, canvas);
        // Push the last tile set
        draws += self.draw_and_reset(canvas);
        draws
    }

    // Port of: TextureSetRenderer::drawTile (compositor_quads.cpp#L734-L781)
    fn draw_tile(
        &self,
        canvas: &Canvas,
        rect: &Rect,
        clip: Option<&[Point; 4]>,
        edge_aa: [bool; 4],
        _tile_id: i32,
        _quad_id: i32,
    ) -> i32 {
        let mut state = self.state.borrow_mut();
        // Now don't actually draw the tile, accumulate it in the growing entry set
        let mut has_clip = false;
        if let Some(clip) = clip {
            // Record the four points into fDstClips
            state.dst_clips.extend_from_slice(clip);
            has_clip = true;
        }

        let mut matrix_idx: Option<usize> = None;
        if !self.reset_each_quad && self.transform_batch_count > 0 {
            // Handle transform batching. This works by capturing the CTM of the first tile draw,
            // and then calculate the difference between that and future CTMs for later tiles.
            if state.pre_view_matrices.is_empty() {
                state.base_ctm = canvas.total_matrix();
                state.pre_view_matrices.push(Matrix::new_identity());
                matrix_idx = Some(0);
            } else {
                // Calculate matrix s.t. getTotalMatrix() = fBaseCTM * M
                match state.base_ctm.invert() {
                    None => {
                        // SkDebugf("Cannot invert CTM, transform batching will not be correct.\n")
                        eprintln!("Cannot invert CTM, transform batching will not be correct.");
                    }
                    Some(inv_base) => {
                        let pre_view = Matrix::concat(&inv_base, &canvas.total_matrix());
                        if state.pre_view_matrices.last() != Some(&pre_view) {
                            // Add the new matrix
                            state.pre_view_matrices.push(pre_view);
                        } // else re-use the last matrix
                        matrix_idx = Some(state.pre_view_matrices.len() - 1);
                    }
                }
            }
        }

        // This acts like the whole image is rendered over the entire tile grid, so derive local
        // coordinates from 'rect', based on the grid to image transform.
        let grid = Rect::from_wh(
            COL_COUNT as scalar * TILE_WIDTH,
            ROW_COUNT as scalar * TILE_HEIGHT,
        );
        let image_rect = Rect::from(IRect::from_wh(self.image.width(), self.image.height()));
        let grid_to_image = Matrix::rect_to_rect_or_identity(grid, image_rect, None);
        let (local_rect, _) = grid_to_image.map_rect(rect);

        // drawTextureSet automatically derives appropriate local quad from localRect if clipPtr
        // is not null.
        state.set_entries.push(ImageSetEntry::new(
            self.image.clone(),
            local_rect,
            *rect,
            matrix_idx,
            1.0,
            mask_to_flags(edge_aa),
            has_clip,
        ));
        drop(state);

        if self.reset_each_quad {
            // Only ever draw one entry at a time
            self.draw_and_reset(canvas)
        } else {
            0
        }
    }

    // Port of: TextureSetRenderer::drawBanner (compositor_quads.cpp#L862-L869)
    fn draw_banner(&self, canvas: &Canvas) {
        if !self.top_banner.is_empty() {
            draw_text(canvas, &self.top_banner);
        }
        canvas.translate((0.0, 15.0));
        if !self.bottom_banner.is_empty() {
            draw_text(canvas, &self.bottom_banner);
        }
    }
}

/// `YUVTextureSetRenderer` (compositor_quads.cpp#L894-L995): a JPEG decoded to YUV planes, drawn
/// as an image set.
///
/// skia-rust: on raster there is no GPU, so `LazyYUVImage::refImage(nullptr, kFromPixmaps)` is
/// always null (`tools/gpu/YUVUtils.cpp`, `ensureYUVImage`). `drawTiles` returns before drawing the
/// grid, so this renderer draws nothing and reports zero draws, as DM's raster config does. Its
/// banner is still drawn. The JPEG is kept only so the renderer is built as in the C++ code.
// Port of: gm/compositor_quads.cpp#L894-L995 (chrome/m156)
struct YuvTextureSetRenderer {
    _jpeg_data: Option<Vec<u8>>,
}

impl ClipTileRenderer for YuvTextureSetRenderer {
    // Port of: YUVTextureSetRenderer::drawTiles (compositor_quads.cpp#L901-L916), the raster case
    // where refImage is null.
    fn draw_tiles(&self, _canvas: &Canvas) -> i32 {
        0
    }

    // Port of: YUVTextureSetRenderer::drawTile (compositor_quads.cpp#L918-L943). Not reached on
    // raster: `draw_tiles` returns first.
    fn draw_tile(
        &self,
        _canvas: &Canvas,
        _rect: &Rect,
        _clip: Option<&[Point; 4]>,
        _edge_aa: [bool; 4],
        _tile_id: i32,
        _quad_id: i32,
    ) -> i32 {
        0
    }

    // Port of: YUVTextureSetRenderer::drawBanner (compositor_quads.cpp#L944-L949)
    fn draw_banner(&self, canvas: &Canvas) {
        draw_text(canvas, "Texture");
        canvas.translate((0.0, 15.0));
        draw_text(canvas, "YUV + alpha - GPU Only");
    }
}

// Port of: gm/compositor_quads.cpp#L1006-L1020 (chrome/m156)
fn make_shader_renderers() -> Vec<Box<dyn ClipTileRenderer>> {
    let pts = [
        Point::new(0.0, 0.0),
        Point::new(0.25 * TILE_WIDTH, 0.25 * TILE_HEIGHT),
    ];
    let colors = [colors::BLUE, colors::WHITE];
    let gradient = Gradient::new(
        Colors::new(&colors, None, TileMode::Mirror, None),
        Interpolation::default(),
    );
    let shader = gradient_shaders::linear_gradient((pts[0], pts[1]), &gradient, None)
        .expect("a linear gradient");

    let info = ImageInfo::new((1, 1), ColorType::Alpha8, AlphaType::Opaque, None);
    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(&info, None);
    bm.erase_color(Color::WHITE);
    let image = bm.as_image().expect("a 1x1 image");

    vec![
        TextureSetRenderer::make_shader("Gradient", image.clone(), shader.clone(), false),
        TextureSetRenderer::make_shader("Local Gradient", image, shader, true),
    ]
}

// Port of: gm/compositor_quads.cpp#L1022-L1029 (chrome/m156)
#[allow(clippy::cast_possible_wrap)] // kMatrixCount is 5
fn make_image_renderers() -> Vec<Box<dyn ClipTileRenderer>> {
    let mandrill = get_resource_as_image("images/mandrill_512.png").expect("mandrill_512.png");
    let mandrill_jpeg = get_resource_as_data("images/mandrill_h1v1.jpg");
    vec![
        TextureSetRenderer::make_unbatched(mandrill.clone()),
        TextureSetRenderer::make_batched(mandrill.clone(), 0),
        TextureSetRenderer::make_batched(mandrill, MATRIX_COUNT as i32),
        Box::new(YuvTextureSetRenderer {
            _jpeg_data: mandrill_jpeg,
        }),
    ]
}

// Port of: gm/compositor_quads.cpp#L1031-L1054 (chrome/m156)
fn make_filtered_renderers() -> Vec<Box<dyn ClipTileRenderer>> {
    let mandrill = get_resource_as_image("images/mandrill_512.png").expect("mandrill_512.png");

    let mut cm = ColorMatrix::default();
    cm.set_saturation(10.0);
    let color_filter = color_filters::matrix(&cm, Clamp::Yes).expect("a saturation color filter");
    let image_filter = dilate((8.0, 8.0), None, None).expect("a dilate image filter");

    let alphas = [colors::TRANSPARENT, colors::BLACK];
    let alpha_gradient = Gradient::new(
        Colors::new(&alphas, None, TileMode::Clamp, None),
        Interpolation::default(),
    );
    let alpha_shader = gradient_shaders::radial_gradient(
        (
            Point::new(
                0.5 * TILE_WIDTH * COL_COUNT as scalar,
                0.5 * TILE_HEIGHT * ROW_COUNT as scalar,
            ),
            0.25 * TILE_WIDTH * COL_COUNT as scalar,
        ),
        &alpha_gradient,
        None,
    )
    .expect("a radial gradient");
    let mask_filter = shader_mask_filter::new(alpha_shader);

    vec![
        TextureSetRenderer::make_alpha(mandrill.clone(), 0.5),
        TextureSetRenderer::make_color_filter("Saturation", mandrill.clone(), color_filter),
        // NOTE: won't draw correctly until SkCanvas' AutoLoopers are used to handle image filters
        TextureSetRenderer::make_image_filter("Dilate", mandrill.clone(), image_filter),
        // NOTE: blur mask filters do work (tested locally), but visually they don't make much
        // sense, since each quad is blurred independently
        TextureSetRenderer::make_mask_filter("Shader", mandrill, mask_filter),
    ]
}

// Port of: gm/compositor_quads.cpp#L996-L1000 (chrome/m156)
fn make_debug_renderers() -> Vec<Box<dyn ClipTileRenderer>> {
    vec![
        Box::new(DebugTileRenderer::make()),
        Box::new(DebugTileRenderer::make_aa()),
        Box::new(DebugTileRenderer::make_non_aa()),
    ]
}

// Port of: gm/compositor_quads.cpp#L1002-L1004 (chrome/m156)
fn make_solid_color_renderers() -> Vec<Box<dyn ClipTileRenderer>> {
    vec![Box::new(SolidColorRenderer {
        color: Color4f::new(0.2, 0.8, 0.3, 1.0),
    })]
}

const MATRIX_COUNT: usize = 5;

/// The GM: a grid of renderers (rows) by transforms (columns) (`CompositorGM`).
struct CompositorGm {
    name: &'static str,
    make_renderer_fn: RendererFactory,
    renderers: Vec<Box<dyn ClipTileRenderer>>,
    matrices: Vec<Matrix>,
    matrix_names: Vec<&'static str>,
}

impl CompositorGm {
    // Port of: gm/compositor_quads.cpp#L424-L427 (chrome/m156)
    fn new(name: &'static str, make_renderer_fn: RendererFactory) -> Self {
        Self {
            name,
            make_renderer_fn,
            renderers: Vec::new(),
            matrices: Vec::new(),
            matrix_names: Vec::new(),
        }
    }

    // Port of: gm/compositor_quads.cpp#L450-L453 (chrome/m156)
    fn once_before_draw(&mut self) {
        if !self.renderers.is_empty() {
            return;
        }
        self.renderers = (self.make_renderer_fn)();
        self.configure_matrices();
    }

    // Port of: gm/compositor_quads.cpp#L513-L553 (chrome/m156)
    fn configure_matrices(&mut self) {
        self.matrices.clear();
        self.matrix_names.clear();
        self.matrices
            .extend((0..MATRIX_COUNT).map(|_| Matrix::new_identity()));

        // Identity
        self.matrices[0].set_identity();
        self.matrix_names.push("Identity");

        // Translate/scale
        self.matrices[1].set_translate((5.5, 20.25));
        self.matrices[1].post_scale((0.9, 0.7), None);
        self.matrix_names.push("T+S");

        // Rotation
        self.matrices[2].set_rotate(20.0, None);
        self.matrices[2].pre_translate((15.0, -20.0));
        self.matrix_names.push("Rotate");

        // Skew
        self.matrices[3].set_skew((0.5, 0.25), None);
        self.matrices[3].pre_translate((-30.0, 0.0));
        self.matrix_names.push("Skew");

        // Perspective
        let src = Rect::from_wh(
            COL_COUNT as scalar * TILE_WIDTH,
            ROW_COUNT as scalar * TILE_HEIGHT,
        )
        .to_quad(PathDirection::CW);
        let dst = [
            Point::new(0.0, 0.0),
            Point::new(COL_COUNT as scalar * TILE_WIDTH + 10.0, 15.0),
            Point::new(
                COL_COUNT as scalar * TILE_WIDTH - 28.0,
                ROW_COUNT as scalar * TILE_HEIGHT + 40.0,
            ),
            Point::new(25.0, ROW_COUNT as scalar * TILE_HEIGHT - 15.0),
        ];
        assert!(self.matrices[4].set_poly_to_poly(&src, &dst));
        self.matrices[4].pre_translate((0.0, 10.0));
        self.matrix_names.push("Perspective");
    }
}

impl GM for CompositorGm {
    fn name(&self) -> String {
        format!("compositor_quads_{}", self.name)
    }

    // Port of: gm/compositor_quads.cpp#L429-L443 (chrome/m156)
    fn size(&mut self) -> ISize {
        // Initialize the array of renderers.
        self.once_before_draw();
        // The GM draws a grid of renderers (rows) x transforms (col). Within each cell, the
        // renderer draws the transformed tile grid, which is approximately
        // (kColCount*kTileWidth, kRowCount*kTileHeight), although it has additional line
        // visualizations and can be transformed outside of those rectangular bounds (i.e.
        // persp), so pad the cell dimensions to be conservative. Must also account for the
        // banner text.
        let cell_width = 1.3_f32 * COL_COUNT as f32 * TILE_WIDTH;
        let cell_height = 1.3_f32 * ROW_COUNT as f32 * TILE_HEIGHT;
        ISize::new(
            float_round2int(cell_width * MATRIX_COUNT as f32 + 175.0),
            float_round2int(cell_height * self.renderers.len() as f32 + 75.0),
        )
    }

    // Port of: gm/compositor_quads.cpp#L455-L493 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        const GAP: scalar = 40.0;
        const BANNER_WIDTH: scalar = 120.0;
        const OFFSET: scalar = 15.0;

        self.once_before_draw();
        let mut draw_counts = vec![0_i32; self.renderers.len()];

        canvas.save();
        canvas.translate((OFFSET + BANNER_WIDTH, OFFSET));
        for i in 0..self.matrices.len() {
            let matrix = &self.matrices[i];
            canvas.save();
            draw_text(canvas, self.matrix_names[i]);

            canvas.translate((0.0, GAP));
            for (j, renderer) in self.renderers.iter().enumerate() {
                canvas.save();
                draw_tile_boundaries(canvas, matrix);
                draw_clipping_boundaries(canvas, matrix);

                canvas.concat(matrix);
                draw_counts[j] += renderer.draw_tiles(canvas);

                canvas.restore();
                // And advance to the next row
                canvas.translate((0.0, GAP + ROW_COUNT as scalar * TILE_HEIGHT));
            }
            // Reset back to the left edge
            canvas.restore();
            // And advance to the next column
            canvas.translate((GAP + COL_COUNT as scalar * TILE_WIDTH, 0.0));
        }
        canvas.restore();

        // Print a row header, with total draw counts
        canvas.save();
        canvas.translate((OFFSET, GAP + 0.5 * ROW_COUNT as scalar * TILE_HEIGHT));
        for (j, renderer) in self.renderers.iter().enumerate() {
            renderer.draw_banner(canvas);
            canvas.translate((0.0, 15.0));
            draw_text(canvas, &format!("Draws = {}", draw_counts[j]));
            canvas.translate((0.0, GAP + ROW_COUNT as scalar * TILE_HEIGHT));
        }
        canvas.restore();
    }
}

// Port of: gm/compositor_quads.cpp#L1057-L1058 (chrome/m156)
crate::def_gm!(
    CompositorGM_debug = "CompositorGM(\"debug\", make_debug_renderers)",
    CompositorGm::new("debug", make_debug_renderers)
);
// Port of: gm/compositor_quads.cpp#L1058-L1058 (chrome/m156)
crate::def_gm!(
    CompositorGM_color = "CompositorGM(\"color\", make_solid_color_renderers)",
    CompositorGm::new("color", make_solid_color_renderers)
);
// Port of: gm/compositor_quads.cpp#L1059-L1059 (chrome/m156)
crate::def_gm!(
    CompositorGM_shader = "CompositorGM(\"shader\", make_shader_renderers)",
    CompositorGm::new("shader", make_shader_renderers)
);
// Port of: gm/compositor_quads.cpp#L1060-L1060 (chrome/m156)
crate::def_gm!(
    CompositorGM_image = "CompositorGM(\"image\", make_image_renderers)",
    CompositorGm::new("image", make_image_renderers)
);
// Port of: gm/compositor_quads.cpp#L1061-L1061 (chrome/m156)
crate::def_gm!(
    CompositorGM_filter = "CompositorGM(\"filter\", make_filtered_renderers)",
    CompositorGm::new("filter", make_filtered_renderers)
);
