// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/filterfastbounds.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::PointMode;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::{
    blur, drop_shadow, drop_shadow_only, image, matrix_transform, offset, picture, tile,
};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;

const TILE_WIDTH: i32 = 100;
const TILE_HEIGHT: i32 = 100;
const NUM_VERT_TILES: i32 = 7;
const NUM_XTRA_COLS: i32 = 2;

// Each method of this type must draw its geometry inside 'r' using 'p'
type DrawMth = fn(&Canvas, &Rect, &Paint);

// Port of: gm/filterfastbounds.cpp#L41-L43 (chrome/m156), draw_rect
fn draw_rect(canvas: &Canvas, r: &Rect, p: &Paint) {
    canvas.draw_rect(r, p);
}

// Port of: gm/filterfastbounds.cpp#L45-L47 (chrome/m156), draw_oval
fn draw_oval(canvas: &Canvas, r: &Rect, p: &Paint) {
    canvas.draw_oval(r, p);
}

// Port of: gm/filterfastbounds.cpp#L49-L56 (chrome/m156), draw_rrect
fn draw_rrect(canvas: &Canvas, r: &Rect, p: &Paint) {
    let x_rad = r.width() / 4.0;
    let y_rad = r.height() / 4.0;
    let rr = RRect::new_rect_xy(r, x_rad, y_rad);
    canvas.draw_rrect(rr, p);
}

// Port of: gm/filterfastbounds.cpp#L58-L67 (chrome/m156), draw_drrect
fn draw_drrect(canvas: &Canvas, r: &Rect, p: &Paint) {
    let x_rad = r.width() / 4.0;
    let y_rad = r.height() / 4.0;
    let outer = RRect::new_rect_xy(r, x_rad, y_rad);
    let mut inner = outer;
    inner.inset((x_rad, y_rad));
    canvas.draw_drrect(outer, inner, p);
}

// Port of: gm/filterfastbounds.cpp#L69-L75 (chrome/m156), draw_path
fn draw_path(canvas: &Canvas, r: &Rect, p: &Paint) {
    let pts = [
        Point::new(r.left, r.top),
        Point::new(r.left, r.bottom),
        Point::new(r.right, r.bottom),
    ];
    let path = Path::polygon(&pts, true, PathFillType::Winding, false);
    canvas.draw_path(&path, p);
}

// Port of: gm/filterfastbounds.cpp#L77-L83 (chrome/m156), draw_points
fn draw_points(canvas: &Canvas, r: &Rect, p: &Paint) {
    let pts0 = [Point::new(r.left, r.top), Point::new(r.right, r.bottom)];
    let pts1 = [Point::new(r.left, r.bottom), Point::new(r.right, r.top)];
    canvas.draw_points(PointMode::Lines, &pts0, p);
    canvas.draw_points(PointMode::Lines, &pts1, p);
}

// Port of: gm/filterfastbounds.cpp#L85-L93 (chrome/m156), draw_bitmap
fn draw_bitmap(canvas: &Canvas, r: &Rect, p: &Paint) {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((64, 64), None);
    if let Some(temp) = Canvas::from_bitmap(&mut bm, None) {
        temp.clear(Color::MAGENTA);
    }
    if let Some(image) = bm.as_image() {
        canvas.draw_image_rect_with_sampling_options(
            &image,
            None,
            r,
            SamplingOptions::default(),
            p,
        );
    }
}

// Port of: gm/filterfastbounds.cpp#L95-L103 (chrome/m156), gDrawMthds
const DRAW_MTHDS: [DrawMth; 7] = [
    draw_rect,
    draw_oval,
    draw_rrect,
    draw_drrect,
    draw_path,
    draw_points,
    draw_bitmap,
];

// Port of: gm/filterfastbounds.cpp#L105-L110 (chrome/m156), add_paint
fn add_paint(paints: &mut Vec<Paint>, filter: Option<ImageFilter>) {
    let mut p = Paint::default();
    p.set_image_filter(filter);
    debug_assert!(p.can_compute_fast_bounds());
    paints.push(p);
}

// Create a selection of imagefilter-based paints to test
// Port of: gm/filterfastbounds.cpp#L112-L160 (chrome/m156), create_paints
fn create_paints(paints: &mut Vec<Paint>, source: Option<ImageFilter>) {
    {
        let scale = Matrix::scale((2.0, 2.0));
        let scale_mif = matrix_transform(
            &scale,
            SamplingOptions::from(FilterMode::Linear),
            source.clone(),
        );
        add_paint(paints, scale_mif);
    }
    {
        let rot = Matrix::rotate_deg(-33.3);
        let rot_mif = matrix_transform(
            &rot,
            SamplingOptions::from(FilterMode::Linear),
            source.clone(),
        );
        add_paint(paints, rot_mif);
    }
    {
        let src = Rect::from_xywh(20.0, 20.0, 10.0, 10.0);
        let dst = Rect::from_xywh(30.0, 30.0, 30.0, 30.0);
        let tile_if = tile(&src, &dst, None);
        add_paint(paints, tile_if);
    }
    {
        let dsif = drop_shadow(
            (10.0, 10.0),
            (3.0, 3.0),
            Color4f::from_color(Color::RED),
            None,
            source.clone(),
            None,
        );
        add_paint(paints, dsif);
    }
    {
        let dsif = drop_shadow_only(
            (27.0, 27.0),
            (3.0, 3.0),
            Color4f::from_color(Color::RED),
            None,
            source.clone(),
            None,
        );
        add_paint(paints, dsif);
    }
    add_paint(
        paints,
        blur(3.0, 3.0, TileMode::Decal, source.clone(), None),
    );
    add_paint(paints, offset((15.0, 15.0), source, None));
}

// Port of: gm/filterfastbounds.cpp#L181-L208 (chrome/m156), draw_geom_with_paint
fn draw_geom_with_paint(draw: DrawMth, off: (i32, i32), canvas: &Canvas, p: &Paint) {
    let mut red_stroked = Paint::default();
    red_stroked.set_color(Color::RED);
    red_stroked.set_style(Style::Stroke);

    let mut blue_stroked = Paint::default();
    blue_stroked.set_color(Color::BLUE);
    blue_stroked.set_style(Style::Stroke);

    let r = Rect::from_ltrb(20.0, 20.0, 30.0, 30.0);
    canvas.save();
    canvas.translate((int_to_scalar(off.0), int_to_scalar(off.1)));
    canvas.scale((1.5, 1.5));
    let fast_bound = p.compute_fast_bounds(&r);
    canvas.save();
    canvas.clip_rect(fast_bound, ClipOp::Intersect, false);
    draw(canvas, &r, p);
    canvas.restore();
    canvas.draw_rect(r, &red_stroked);
    canvas.draw_rect(fast_bound, &blue_stroked);
    canvas.restore();
}

// Port of: gm/filterfastbounds.cpp#L210-L237 (chrome/m156), draw_savelayer_with_paint
fn draw_savelayer_with_paint(off: (i32, i32), canvas: &Canvas, p: &Paint) {
    let mut red_stroked = Paint::default();
    red_stroked.set_color(Color::RED);
    red_stroked.set_style(Style::Stroke);

    let mut blue_stroked = Paint::default();
    blue_stroked.set_color(Color::BLUE);
    blue_stroked.set_style(Style::Stroke);

    let bounds = Rect::from_wh(10.0, 10.0);
    canvas.save();
    canvas.translate((30.0, 30.0));
    canvas.translate((int_to_scalar(off.0), int_to_scalar(off.1)));
    canvas.scale((1.5, 1.5));
    let fast_bound = p.compute_fast_bounds(&bounds);
    let rec = skia_rust_core::canvas::SaveLayerRec::default()
        .bounds(&fast_bound)
        .paint(p);
    canvas.save_layer(&rec);
    canvas.restore();
    canvas.draw_rect(bounds, &red_stroked);
    canvas.draw_rect(fast_bound, &blue_stroked);
    canvas.restore();
}

// This GM visualizes the fast bounds for various combinations of geometry and image filter
// Port of: gm/filterfastbounds.cpp#L162-L334 (chrome/m156), ImageFilterFastBoundGM
struct ImageFilterFastBoundGm;

impl GM for ImageFilterFastBoundGm {
    fn name(&self) -> String {
        "filterfastbounds".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(
            (i32::try_from(DRAW_MTHDS.len()).unwrap_or(0) + NUM_XTRA_COLS) * TILE_WIDTH,
            NUM_VERT_TILES * TILE_HEIGHT,
        )
    }

    // `setBGColor(0xFFCCCCCC)` in the constructor.
    fn bg_color(&self) -> Color {
        Color::new(0xFFCC_CCCC)
    }

    // Port of: gm/filterfastbounds.cpp#L239-L326 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let black_fill = Paint::default();

        //-----------
        // Normal paints (no source)
        let mut paints = Vec::new();
        create_paints(&mut paints, None);

        //-----------
        // Paints with a PictureImageFilter as a source
        let pic: Option<Picture> = {
            let mut rec = PictureRecorder::new();
            let c = rec.begin_recording(Rect::from_wh(10.0, 10.0), false);
            c.draw_rect(Rect::from_wh(10.0, 10.0), &black_fill);
            rec.finish_recording_as_picture(None)
        };
        let mut pif_paints = Vec::new();
        create_paints(&mut pif_paints, picture(pic, None));

        //-----------
        // Paints with a SkImageSource as a source
        let image_source: Option<ImageFilter> = match surfaces::raster_n32_premul((10, 10)) {
            Some(mut surface) => {
                {
                    let temp = surface.shared_canvas();
                    let mut p = Paint::default();
                    temp.clear(Color::YELLOW);
                    p.set_color(Color::BLUE);
                    temp.draw_rect(Rect::from_ltrb(5.0, 5.0, 10.0, 10.0), &p);
                    p.set_color(Color::GREEN);
                    temp.draw_rect(Rect::from_ltrb(5.0, 0.0, 10.0, 5.0), &p);
                }
                let image: Option<Image> = surface.image_snapshot();
                image.map(|image| {
                    let bounds =
                        Rect::from_wh(int_to_scalar(image.width()), int_to_scalar(image.height()));
                    image_filter_from(image, bounds)
                })
            }
            None => None,
        };
        let mut bms_paints = Vec::new();
        create_paints(&mut bms_paints, image_source);

        debug_assert_eq!(paints.len(), usize::try_from(NUM_VERT_TILES).unwrap_or(0));
        debug_assert_eq!(paints.len(), pif_paints.len());
        debug_assert_eq!(paints.len(), bms_paints.len());

        let num_cols = i32::try_from(DRAW_MTHDS.len()).unwrap_or(0) + NUM_XTRA_COLS;
        let num_rows = i32::try_from(paints.len()).unwrap_or(0);

        // horizontal separators
        for i in 1..num_rows {
            canvas.draw_line(
                (0.0, int_to_scalar(i) * int_to_scalar(TILE_HEIGHT)),
                (
                    int_to_scalar(num_cols) * int_to_scalar(TILE_WIDTH),
                    int_to_scalar(i) * int_to_scalar(TILE_HEIGHT),
                ),
                &black_fill,
            );
        }

        // vertical separators
        for i in 0..num_cols {
            canvas.draw_line(
                (int_to_scalar(i * TILE_WIDTH), 0.0),
                (
                    int_to_scalar(i * TILE_WIDTH),
                    int_to_scalar(num_rows * TILE_WIDTH),
                ),
                &black_fill,
            );
        }

        // A column of saveLayers with PictureImageFilters
        for (i, p) in pif_paints.iter().enumerate() {
            draw_savelayer_with_paint((0, i32::try_from(i).unwrap_or(0) * TILE_HEIGHT), canvas, p);
        }

        // A column of saveLayers with BitmapSources
        for (i, p) in bms_paints.iter().enumerate() {
            draw_savelayer_with_paint(
                (TILE_WIDTH, i32::try_from(i).unwrap_or(0) * TILE_HEIGHT),
                canvas,
                p,
            );
        }

        // Multiple columns with different geometry
        for (i, draw) in DRAW_MTHDS.iter().enumerate() {
            let col = i32::try_from(i).unwrap_or(0) + NUM_XTRA_COLS;
            for (j, p) in paints.iter().enumerate() {
                draw_geom_with_paint(
                    *draw,
                    (
                        col * TILE_WIDTH,
                        i32::try_from(j).unwrap_or(0) * TILE_HEIGHT,
                    ),
                    canvas,
                    p,
                );
            }
        }
    }
}

// `SkImageFilters::Image(image, SkFilterMode::kLinear)`: the image's own bounds for both rects.
// Port of: include/effects/SkImageFilters.h#L272-L279 (chrome/m156), the Image(image, sampling)
// overload, with the bounds of the image.
fn image_filter_from(img: Image, bounds: Rect) -> ImageFilter {
    image(
        Some(img),
        bounds,
        bounds,
        SamplingOptions::from(FilterMode::Linear),
    )
}

crate::def_gm!(ImageFilterFastBoundGM, ImageFilterFastBoundGm);
