// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crop_imagefilter.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::canvas::{Canvas as CoreCanvas, SaveLayerRec, SrcRectConstraint};
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::Color;
use skia_rust_core::image::{Image, RequiredProperties};
use skia_rust_core::image_filter_types::{round_in, round_out};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::size::ISize;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::dash_path_effect;
use skia_rust_effects::image_filters::{blur, crop};
use skia_rust_raster::surfaces;

const K_OUTPUT_BOUNDS_COLOR: Color = Color::RED;
const K_CROP_RECT_COLOR: Color = Color::GREEN;
const K_CONTENT_BOUNDS_COLOR: Color = Color::BLUE;

/// The example bounds (`kExampleBounds`).
// Port of: gm/crop_imagefilter.cpp#L23 (chrome/m156)
fn example_bounds() -> Rect {
    Rect::new(0.0, 0.0, 100.0, 100.0)
}

/// "Crop" refers to the rect passed to the crop image filter, "Rect" refers to some other rect
/// from context, likely the output bounds or the content bounds.
// Port of: gm/crop_imagefilter.cpp#L25-L31 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CropRelation {
    /// Intersect but doesn't fully contain one way or the other
    CropOverlapsRect,
    CropContainsRect,
    RectContainsCrop,
    CropRectDisjoint,
}

// Port of: gm/crop_imagefilter.cpp#L33-L35 (chrome/m156)
fn make_overlap(r: Rect, amount_x: f32, amount_y: f32) -> Rect {
    r.with_offset((r.width() * amount_x, r.height() * amount_y))
}

// Port of: gm/crop_imagefilter.cpp#L37-L39 (chrome/m156)
fn make_inset(r: Rect, amount_x: f32, amount_y: f32) -> Rect {
    r.with_inset((r.width() * amount_x, r.height() * amount_y))
}

// Port of: gm/crop_imagefilter.cpp#L41-L43 (chrome/m156)
fn make_outset(r: Rect, amount_x: f32, amount_y: f32) -> Rect {
    r.with_outset((r.width() * amount_x, r.height() * amount_y))
}

// Port of: gm/crop_imagefilter.cpp#L45-L53 (chrome/m156)
fn make_disjoint(r: Rect, amount_x: f32, amount_y: f32) -> Rect {
    let x_offset = if amount_x > 0.0 {
        r.width() + r.width() * amount_x
    } else if amount_x < 0.0 {
        -r.width() + r.width() * amount_x
    } else {
        0.0
    };
    let y_offset = if amount_y > 0.0 {
        r.height() + r.height() * amount_y
    } else if amount_y < 0.0 {
        -r.height() + r.height() * amount_y
    } else {
        0.0
    };
    r.with_offset((x_offset, y_offset))
}

/// `get_example_rects`: the output bounds, crop rect and content bounds for the given relations.
// Port of: gm/crop_imagefilter.cpp#L55-L108 (chrome/m156)
fn get_example_rects(
    output_relation: CropRelation,
    input_relation: CropRelation,
    hint_content: bool,
) -> (Rect, Rect, Rect) {
    let output_bounds = example_bounds().with_inset((20.0, 20.0));
    let mut crop_rect = match output_relation {
        CropRelation::CropOverlapsRect => make_overlap(output_bounds, -0.15, 0.15),
        CropRelation::CropContainsRect => make_outset(output_bounds, 0.15, 0.15),
        CropRelation::RectContainsCrop => make_inset(output_bounds, 0.15, 0.15),
        CropRelation::CropRectDisjoint => make_disjoint(output_bounds, 0.15, 0.0),
    };
    crop_rect.intersect(example_bounds());

    // Determine content bounds for example based on computed crop rect and input relation
    let content_bounds = if hint_content {
        let mut content = match input_relation {
            CropRelation::CropOverlapsRect => make_overlap(crop_rect, 0.075, -0.75),
            CropRelation::CropContainsRect => make_inset(crop_rect, 0.075, 0.075),
            CropRelation::RectContainsCrop => make_outset(crop_rect, 0.1, 0.1),
            CropRelation::CropRectDisjoint => make_disjoint(crop_rect, 0.0, 0.075),
        };
        content.intersect(example_bounds());
        content
    } else {
        example_bounds()
    };
    (output_bounds, crop_rect, content_bounds)
}

/// The test pattern (`make_image`): grid lines on dark gray, everything outside the content
/// bounds is red.
// Port of: gm/crop_imagefilter.cpp#L110-L146 (chrome/m156)
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors SkScalarCeilToInt
fn make_image(content_bounds: Option<Rect>) -> Option<Image> {
    let example = example_bounds();
    let w = example.width();
    let h = example.height();
    let info = ImageInfo::new(
        ISize::new(w.ceil() as i32, h.ceil() as i32),
        skia_rust_core::color_type::ColorType::N32,
        skia_rust_core::alpha_type::AlphaType::Premul,
        None,
    );
    let mut surf = surfaces::raster(&info, None, None)?;
    let canvas = surf.canvas();
    canvas.draw_color(Color::DARK_GRAY, None);

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(Style::Stroke);

    // Draw four horizontal lines at 1/4, 3/8, 5/8, 3/4.
    paint.set_stroke_width(h / 16.0);
    paint.set_color(Color::RED);
    canvas.draw_line((0.0, 1.0 * h / 4.0), (w, 1.0 * h / 4.0), &paint);
    paint.set_color(Color::new(0xFF71_EEB8)); // sea foam
    canvas.draw_line((0.0, 3.0 * h / 8.0), (w, 3.0 * h / 8.0), &paint);
    paint.set_color(Color::YELLOW);
    canvas.draw_line((0.0, 5.0 * h / 8.0), (w, 5.0 * h / 8.0), &paint);
    paint.set_color(Color::CYAN);
    canvas.draw_line((0.0, 3.0 * h / 4.0), (w, 3.0 * h / 4.0), &paint);

    // Draw four vertical lines at 1/4, 3/8, 5/8, 3/4.
    paint.set_stroke_width(w / 16.0);
    paint.set_color(Color::new(0xFFFF_A500)); // orange
    canvas.draw_line((1.0 * w / 4.0, 0.0), (1.0 * h / 4.0, h), &paint);
    paint.set_color(Color::BLUE);
    canvas.draw_line((3.0 * w / 8.0, 0.0), (3.0 * h / 8.0, h), &paint);
    paint.set_color(Color::MAGENTA);
    canvas.draw_line((5.0 * w / 8.0, 0.0), (5.0 * h / 8.0, h), &paint);
    paint.set_color(Color::GREEN);
    canvas.draw_line((3.0 * w / 4.0, 0.0), (3.0 * h / 4.0, h), &paint);

    // Fill everything outside of the content bounds with red since it shouldn't be sampled from.
    if let Some(content) = content_bounds {
        let buffer = content.with_outset((1.0, 1.0));
        canvas.clip_rect(buffer, ClipOp::Difference, false);
        canvas.clear(Color::RED);
    }
    surf.image_snapshot()
}

/// Subsets `image` to `content_bounds`, and tiles it with `content_tile` to fill a `crop_rect`
/// sized image (`make_cropped_image`).
// Port of: gm/crop_imagefilter.cpp#L148-L168 (chrome/m156)
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors SkScalarCeilToInt
fn make_cropped_image(
    image: &Image,
    content_bounds: Rect,
    content_tile: TileMode,
    crop_rect: Rect,
) -> Option<Image> {
    let info = image.image_info().with_wh(
        crop_rect.width().ceil() as i32,
        crop_rect.height().ceil() as i32,
    );
    let mut surface = surfaces::raster(&info, None, None)?;
    let subset_bounds: IRect = if content_tile == TileMode::Decal {
        round_out(&content_bounds)
    } else {
        round_in(&content_bounds)
    };
    let content = image.make_subset(subset_bounds, RequiredProperties::default())?;
    let shader_matrix = Matrix::translate((content_bounds.left, content_bounds.top));
    let mut tiled_content = Paint::default();
    tiled_content.set_shader(content.to_shader(
        Some((content_tile, content_tile)),
        SamplingOptions::from(FilterMode::Nearest),
        &shader_matrix,
    ));
    {
        let canvas = surface.canvas();
        canvas.translate((-crop_rect.left, -crop_rect.top));
        canvas.draw_paint(&tiled_content);
    }
    surface.image_snapshot()
}

/// `draw_example_tile`: one example of the input and output tile modes for one relation.
// Port of: gm/crop_imagefilter.cpp#L170-L244 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors draw_example_tile
fn draw_example_tile(
    canvas: &CoreCanvas,
    input_mode: TileMode,
    input_relation: CropRelation,
    hint_content: bool,
    output_mode: TileMode,
    output_relation: CropRelation,
) {
    // Determine crop rect for example based on output relation
    let (output_bounds, crop_rect, content_bounds) =
        get_example_rects(output_relation, input_relation, hint_content);

    let image = make_image(if hint_content {
        Some(content_bounds)
    } else {
        None
    });
    let Some(image) = image else {
        return;
    };

    let guard = AutoCanvasRestore::guard(canvas, true);

    // Visualize the image tiled on the content bounds (blue border) and then tiled on the crop
    // rect (green) border, semi-transparent
    if let Some(crop_image) = make_cropped_image(&image, content_bounds, input_mode, crop_rect) {
        let mut tiled_paint = Paint::default();
        tiled_paint.set_shader(crop_image.to_shader(
            Some((output_mode, output_mode)),
            SamplingOptions::from(FilterMode::Nearest),
            &Matrix::translate((crop_rect.left, crop_rect.top)),
        ));
        tiled_paint.set_alpha_f(0.25);
        {
            let inner = AutoCanvasRestore::guard(&guard, true);
            inner.clip_rect(example_bounds(), ClipOp::Intersect, false);
            inner.draw_paint(&tiled_paint);
        }
    }

    // Build filter, clip, save layer, draw, restore - the interesting part is in the tile modes
    // and how the various bounds intersect each other.
    {
        let filter = crop(&content_bounds, input_mode, None);
        let filter = blur(4.0, 4.0, TileMode::Decal, filter, None);
        let filter = crop(&crop_rect, output_mode, filter);
        let mut layer_paint = Paint::default();
        layer_paint.set_image_filter(filter);
        let inner = AutoCanvasRestore::guard(&guard, true);
        inner.clip_rect(output_bounds, ClipOp::Intersect, false);
        let rec = SaveLayerRec::default().paint(&layer_paint);
        let rec = if hint_content {
            rec.bounds(&content_bounds)
        } else {
            rec
        };
        inner.save_layer(&rec);
        inner.draw_image_rect_with_sampling_options(
            &image,
            Some((&content_bounds, SrcRectConstraint::Strict)),
            content_bounds,
            SamplingOptions::from(FilterMode::Nearest),
            &Paint::default(),
        );
        inner.restore();
    }

    // Visualize bounds after the actual rendering.
    {
        let mut border = Paint::default();
        border.set_style(Style::Stroke);
        border.set_color(K_OUTPUT_BOUNDS_COLOR);
        guard.draw_rect(output_bounds, &border);
        border.set_color(K_CROP_RECT_COLOR);
        guard.draw_rect(crop_rect, &border);
        if hint_content {
            border.set_color(K_CONTENT_BOUNDS_COLOR);
            guard.draw_rect(content_bounds, &border);
        }
    }
}

/// `draw_example_column`: five examples for the relations of the content bounds and crop rect.
// Port of: gm/crop_imagefilter.cpp#L246-L262 (chrome/m156)
fn draw_example_column(
    canvas: &CoreCanvas,
    input_mode: TileMode,
    output_mode: TileMode,
    output_relation: CropRelation,
) {
    let input_relations = [
        (CropRelation::CropOverlapsRect, false),
        (CropRelation::CropOverlapsRect, true),
        (CropRelation::CropContainsRect, true),
        (CropRelation::RectContainsCrop, true),
        (CropRelation::CropRectDisjoint, true),
    ];
    let guard = AutoCanvasRestore::guard(canvas, true);
    for (input_relation, hint_content) in input_relations {
        draw_example_tile(
            &guard,
            input_mode,
            input_relation,
            hint_content,
            output_mode,
            output_relation,
        );
        guard.translate((0.0, example_bounds().bottom + 1.0));
    }
}

const K_NUM_ROWS: i32 = 5;
const K_NUM_COLS: i32 = 4;

/// `draw_example_grid`: the 5x4 grid of examples, with dashed lines between them.
// Port of: gm/crop_imagefilter.cpp#L264-L305 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // grid indices are small
fn draw_example_grid(canvas: &CoreCanvas, input_mode: TileMode, output_mode: TileMode) {
    let example = example_bounds();
    let grid_width = K_NUM_COLS as f32 * (example.right + 1.0) - 1.0;
    let grid_height = K_NUM_ROWS as f32 * (example.bottom + 1.0) - 1.0;
    {
        let guard = AutoCanvasRestore::guard(canvas, true);
        for output_relation in [
            CropRelation::CropOverlapsRect,
            CropRelation::CropContainsRect,
            CropRelation::RectContainsCrop,
            CropRelation::CropRectDisjoint,
        ] {
            draw_example_column(&guard, input_mode, output_mode, output_relation);
            guard.translate((example.right + 1.0, 0.0));
        }
    }

    // Draw dashed lines between rows and columns
    let mut dashed_line = Paint::default();
    dashed_line.set_color(Color::GRAY);
    dashed_line.set_style(Style::Stroke);
    dashed_line.set_stroke_cap(skia_rust_core::paint::Cap::Square);
    let dashes = [5.0f32, 15.0];
    dashed_line.set_path_effect(dash_path_effect::new(&dashes, 0.0));
    for y in 1..K_NUM_ROWS {
        let line_y = y as f32 * (example.bottom + 1.0) - 0.5;
        canvas.draw_line((0.5, line_y), (grid_width - 0.5, line_y), &dashed_line);
    }
    for x in 1..K_NUM_COLS {
        let line_x = x as f32 * (example.right + 1.0) - 0.5;
        canvas.draw_line((line_x, 0.5), (line_x, grid_height - 0.5), &dashed_line);
    }
}

/// `CropImageFilterGM(inputMode, outputMode)`.
// Port of: gm/crop_imagefilter.cpp#L307-L349 (chrome/m156)
struct CropImageFilterGm {
    input_mode: TileMode,
    output_mode: TileMode,
}

impl CropImageFilterGm {
    fn new(input_mode: TileMode, output_mode: TileMode) -> Self {
        CropImageFilterGm {
            input_mode,
            output_mode,
        }
    }
}

/// The name of a tile mode in the golden names (`crop_imagefilter_<in>-in_<out>-out`).
fn tile_mode_name(mode: TileMode) -> &'static str {
    match mode {
        TileMode::Decal => "decal",
        TileMode::Clamp => "clamp",
        TileMode::Repeat => "repeat",
        TileMode::Mirror => "mirror",
    }
}

impl GM for CropImageFilterGm {
    fn name(&self) -> String {
        format!(
            "crop_imagefilter_{}-in_{}-out",
            tile_mode_name(self.input_mode),
            tile_mode_name(self.output_mode)
        )
    }

    // Port of: gm/crop_imagefilter.cpp#L317-L321 (chrome/m156)
    #[allow(clippy::cast_possible_truncation)] // SkScalarRoundToInt of a small grid size
    fn size(&mut self) -> ISize {
        let example = example_bounds();
        ISize::new(
            (4.0 * (example.right + 1.0) - 1.0).round() as i32,
            (5.0 * (example.bottom + 1.0) - 1.0).round() as i32,
        )
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        draw_example_grid(canvas, self.input_mode, self.output_mode);
    }
}

// Port of: gm/crop_imagefilter.cpp#L351-L366 (chrome/m156)
crate::def_gm!(
    CropImageFilterGM_decal_decal = "CropImageFilterGM(SkTileMode::kDecal, SkTileMode::kDecal)",
    CropImageFilterGm::new(TileMode::Decal, TileMode::Decal)
);
crate::def_gm!(
    CropImageFilterGM_decal_clamp = "CropImageFilterGM(SkTileMode::kDecal, SkTileMode::kClamp)",
    CropImageFilterGm::new(TileMode::Decal, TileMode::Clamp)
);
crate::def_gm!(
    CropImageFilterGM_decal_repeat = "CropImageFilterGM(SkTileMode::kDecal, SkTileMode::kRepeat)",
    CropImageFilterGm::new(TileMode::Decal, TileMode::Repeat)
);
crate::def_gm!(
    CropImageFilterGM_decal_mirror = "CropImageFilterGM(SkTileMode::kDecal, SkTileMode::kMirror)",
    CropImageFilterGm::new(TileMode::Decal, TileMode::Mirror)
);
crate::def_gm!(
    CropImageFilterGM_clamp_decal = "CropImageFilterGM(SkTileMode::kClamp, SkTileMode::kDecal)",
    CropImageFilterGm::new(TileMode::Clamp, TileMode::Decal)
);
crate::def_gm!(
    CropImageFilterGM_clamp_clamp = "CropImageFilterGM(SkTileMode::kClamp, SkTileMode::kClamp)",
    CropImageFilterGm::new(TileMode::Clamp, TileMode::Clamp)
);
crate::def_gm!(
    CropImageFilterGM_clamp_repeat = "CropImageFilterGM(SkTileMode::kClamp, SkTileMode::kRepeat)",
    CropImageFilterGm::new(TileMode::Clamp, TileMode::Repeat)
);
crate::def_gm!(
    CropImageFilterGM_clamp_mirror = "CropImageFilterGM(SkTileMode::kClamp, SkTileMode::kMirror)",
    CropImageFilterGm::new(TileMode::Clamp, TileMode::Mirror)
);
crate::def_gm!(
    CropImageFilterGM_repeat_decal = "CropImageFilterGM(SkTileMode::kRepeat, SkTileMode::kDecal)",
    CropImageFilterGm::new(TileMode::Repeat, TileMode::Decal)
);
crate::def_gm!(
    CropImageFilterGM_repeat_clamp = "CropImageFilterGM(SkTileMode::kRepeat, SkTileMode::kClamp)",
    CropImageFilterGm::new(TileMode::Repeat, TileMode::Clamp)
);
crate::def_gm!(
    CropImageFilterGM_repeat_repeat = "CropImageFilterGM(SkTileMode::kRepeat, SkTileMode::kRepeat)",
    CropImageFilterGm::new(TileMode::Repeat, TileMode::Repeat)
);
crate::def_gm!(
    CropImageFilterGM_repeat_mirror = "CropImageFilterGM(SkTileMode::kRepeat, SkTileMode::kMirror)",
    CropImageFilterGm::new(TileMode::Repeat, TileMode::Mirror)
);
crate::def_gm!(
    CropImageFilterGM_mirror_decal = "CropImageFilterGM(SkTileMode::kMirror, SkTileMode::kDecal)",
    CropImageFilterGm::new(TileMode::Mirror, TileMode::Decal)
);
crate::def_gm!(
    CropImageFilterGM_mirror_clamp = "CropImageFilterGM(SkTileMode::kMirror, SkTileMode::kClamp)",
    CropImageFilterGm::new(TileMode::Mirror, TileMode::Clamp)
);
crate::def_gm!(
    CropImageFilterGM_mirror_repeat = "CropImageFilterGM(SkTileMode::kMirror, SkTileMode::kRepeat)",
    CropImageFilterGm::new(TileMode::Mirror, TileMode::Repeat)
);
crate::def_gm!(
    CropImageFilterGM_mirror_mirror = "CropImageFilterGM(SkTileMode::kMirror, SkTileMode::kMirror)",
    CropImageFilterGm::new(TileMode::Mirror, TileMode::Mirror)
);
