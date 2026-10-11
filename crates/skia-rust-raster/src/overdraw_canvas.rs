// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkOverdrawCanvas.h, src/core/SkOverdrawCanvas.cpp

//! [`OverdrawCanvas`]: a canvas that captures all drawing commands and, rather than drawing the
//! actual content, increments the alpha channel of each pixel every time it would have been
//! touched by a draw call (`SkOverdrawCanvas`). This is useful for detecting overdraw.
//!
//! skia-rust: `SkOverdrawCanvas` derives from `SkNWayCanvas`, so it is a [`NWayCanvas`] whose
//! hooks hold the n-way hooks and call them for the virtuals it does not override. The
//! protected `fList[0]->onDrawX(...)` calls are [`Canvas::on_draw_rect`] and friends.
//!
//! The glyph drawing of the C++ goes through a `TextDevice` that paints the masks on the overdraw
//! canvas. A hook cannot call the canvas that is calling it, so here the masks (and the glyph
//! paths) are drawn on the wrapped canvas directly, with the sequence of calls the overdraw
//! canvas forwards to it. Two cases are not drawn: a text run list with `RSXform`s, and a glyph
//! that is a drawable (the drawable would have to draw into the overdraw canvas).

use std::rc::Rc;

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blender::Blender;
use skia_rust_core::canvas::{
    Canvas, CanvasHooks, ImageSetEntry, Lattice, PointMode, QuadAAFlags, SaveLayerRec,
    SaveLayerStrategy, SrcRectConstraint,
};
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::device::{GlyphRunDrawCursor, PendingGlyphKind};
use skia_rust_core::glyph::Glyph;
use skia_rust_core::glyph_run::{GlyphRunBuilder, GlyphRunList};
use skia_rust_core::image::Image;
use skia_rust_core::lattice_iter::LatticeIter;
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::mesh::Mesh;
use skia_rust_core::mipmap::Mipmap;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::Region;
use skia_rust_core::rrect::RRect;
use skia_rust_core::rsxform::RSXform;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::slug::Slug;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_core::utils::n_way_canvas::{NWayCanvas, NWayHooks};
use skia_rust_core::utils::patch_utils;
use skia_rust_core::vertices::Vertices;
use std::ops::Deref;
use std::sync::Arc;

use crate::glyph_run_painter::{BitmapDevicePainter, GlyphRunListPainter};

/// The overrides of `SkOverdrawCanvas`.
struct OverdrawHooks {
    base: NWayHooks,
    // `fPaint`: draws one more layer of overdraw (alpha plus 1/255).
    paint: Paint,
}

impl OverdrawHooks {
    // `fList[0]`.
    fn first(&self) -> Option<Rc<Canvas>> {
        self.base.first_canvas()
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L275-L280 (chrome/m156)
    fn overdraw_paint(&self, paint: &Paint) -> Paint {
        let mut new_paint = self.paint.clone();
        new_paint.set_style(paint.style());
        new_paint.set_stroke_width(paint.stroke_width());
        new_paint
    }
}

/// `TextDevice::paintMasks`: the mask bounds, drawn as rects on the wrapped canvas.
// Port of: src/core/SkOverdrawCanvas.cpp#L63-L121 (chrome/m156), TextDevice
struct TextPainter<'a> {
    hooks: &'a OverdrawHooks,
    canvas: &'a Canvas,
}

impl BitmapDevicePainter for TextPainter<'_> {
    // Port of: src/core/SkOverdrawCanvas.cpp#L69-L79 (chrome/m156)
    fn paint_masks(&mut self, accepted: &[(&Glyph, Point)], _paint: &Paint) {
        for (glyph, pos) in accepted {
            let mask = glyph.mask_at(*pos);
            // We need to ignore any matrix on the overdraw canvas (it's already been baked into
            // our glyph positions). Otherwise, the CTM is double-applied. (skbug.com/40044818)
            // (`fOverdrawCanvas->save(); resetMatrix(); drawRect(...); restore()`, which the
            // overdraw canvas forwards to the wrapped canvas.)
            let save_count = self.canvas.save_count();
            self.canvas.save();
            self.canvas.set_matrix(&M44::new_identity());
            self.canvas.on_draw_rect(
                &Rect::from_irect(mask.bounds).sorted(),
                &self.hooks.overdraw_paint(&Paint::default()),
            );
            self.canvas.restore_to_count(save_count);
        }
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L81-L87 (chrome/m156)
    fn draw_bitmap(
        &mut self,
        _bitmap: &Bitmap,
        _matrix: &Matrix,
        _dst_or_null: Option<&Rect>,
        _sampling: &SamplingOptions,
        _paint: &Paint,
        _mips: Option<Arc<Mipmap>>,
    ) {
    }
}

impl OverdrawHooks {
    // Port of: src/core/SkOverdrawCanvas.cpp#L89-L96 (chrome/m156), TextDevice::onDrawGlyphRunList,
    // and SkDevice::drawGlyphRunList
    fn draw_glyph_run_list(&self, list: &GlyphRunList<'_>, paint: &Paint, total_matrix: &Matrix) {
        let Some(canvas) = self.first() else {
            return;
        };
        if list.has_rsxform() {
            return;
        }
        let props = SurfaceProps::default();
        let glyph_painter = GlyphRunListPainter::new(props, ColorType::N32, None);
        let mut text_painter = TextPainter {
            hooks: self,
            canvas: &canvas,
        };
        let mut cursor = GlyphRunDrawCursor::default();
        while let Some(pending) = glyph_painter.draw_for_bitmap_device(
            &mut text_painter,
            list,
            paint,
            total_matrix,
            &mut cursor,
        ) {
            let save_count = canvas.save_count();
            match pending.kind {
                PendingGlyphKind::Drawable(_) => {}
                PendingGlyphKind::Path(path) => {
                    canvas.save();
                    canvas.concat(&pending.matrix);
                    canvas.on_draw_path(&path, &self.paint);
                }
                PendingGlyphKind::DevicePath(path) => {
                    if pending.matrix.is_identity() {
                        canvas.on_draw_path(&path, &self.paint);
                    } else {
                        canvas.save();
                        canvas.concat(&pending.matrix);
                        canvas.on_draw_path(&path, &self.paint);
                    }
                }
            }
            canvas.restore_to_count(save_count);
        }
    }
}

impl CanvasHooks for OverdrawHooks {
    // The notifications are those of SkNWayCanvas.
    fn will_save(&mut self) {
        self.base.will_save();
    }

    fn get_save_layer_strategy(&mut self, rec: &SaveLayerRec<'_>) -> SaveLayerStrategy {
        self.base.get_save_layer_strategy(rec)
    }

    fn will_restore(&mut self) {
        self.base.will_restore();
    }

    fn did_concat44(&mut self, m: &M44) {
        self.base.did_concat44(m);
    }

    fn did_set_m44(&mut self, m: &M44) {
        self.base.did_set_m44(m);
    }

    fn did_scale(&mut self, sx: scalar, sy: scalar) {
        self.base.did_scale(sx, sy);
    }

    fn did_translate(&mut self, dx: scalar, dy: scalar) {
        self.base.did_translate(dx, dy);
    }

    fn on_clip_rect(&mut self, rect: &Rect, op: ClipOp, is_aa: bool) {
        self.base.on_clip_rect(rect, op, is_aa);
    }

    fn on_clip_rrect(&mut self, rrect: &RRect, op: ClipOp, is_aa: bool) {
        self.base.on_clip_rrect(rrect, op, is_aa);
    }

    fn on_clip_path(&mut self, path: &Path, op: ClipOp, is_aa: bool) {
        self.base.on_clip_path(path, op, is_aa);
    }

    fn on_clip_shader(&mut self, shader: &Shader, op: ClipOp) {
        self.base.on_clip_shader(shader, op);
    }

    fn on_clip_region(&mut self, device_rgn: &Region, op: ClipOp) {
        self.base.on_clip_region(device_rgn, op);
    }

    fn on_reset_clip(&mut self) {
        self.base.on_reset_clip();
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L107-L116 (chrome/m156)
    fn on_draw_text_blob(
        &mut self,
        blob: &TextBlob,
        x: scalar,
        y: scalar,
        paint: &Paint,
        total_matrix: &Matrix,
    ) -> bool {
        let mut builder = GlyphRunBuilder::new();
        let glyph_run_list = builder.blob_to_glyph_run_list(blob, Point::new(x, y));
        self.draw_glyph_run_list(&glyph_run_list, paint, total_matrix);
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L118-L127 (chrome/m156)
    fn on_draw_glyph_run_list(
        &mut self,
        list: &GlyphRunList<'_>,
        paint: &Paint,
        total_matrix: &Matrix,
    ) -> bool {
        self.draw_glyph_run_list(list, paint, total_matrix);
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L129-L133 (chrome/m156)
    fn on_draw_patch(
        &mut self,
        cubics: &[Point; patch_utils::NUM_CTRL_PTS],
        colors: Option<&[Color; patch_utils::NUM_CORNERS]>,
        tex_coords: Option<&[Point; patch_utils::NUM_CORNERS]>,
        mode: BlendMode,
        _paint: &Paint,
    ) -> bool {
        if let Some(c) = self.first() {
            c.on_draw_patch(cubics, colors, tex_coords, mode, &self.paint);
        }
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L135-L141 (chrome/m156)
    fn on_draw_paint(&mut self, paint: &Paint) -> bool {
        if paint.color() == Color::default()
            && paint.color_filter().is_none()
            && paint.shader().is_none()
        {
            // This is a clear, ignore it.
        } else if let Some(c) = self.first() {
            c.on_draw_paint(&self.overdraw_paint(paint));
        }
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L147-L149 (chrome/m156)
    fn on_draw_rect(&mut self, rect: &Rect, paint: &Paint) -> bool {
        if let Some(c) = self.first() {
            c.on_draw_rect(rect, &self.overdraw_paint(paint));
        }
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L151-L153 (chrome/m156)
    fn on_draw_region(&mut self, region: &Region, paint: &Paint) -> bool {
        if let Some(c) = self.first() {
            c.on_draw_region(region, &self.overdraw_paint(paint));
        }
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L155-L157 (chrome/m156)
    fn on_draw_oval(&mut self, oval: &Rect, paint: &Paint) -> bool {
        if let Some(c) = self.first() {
            c.on_draw_oval(oval, &self.overdraw_paint(paint));
        }
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L159-L162 (chrome/m156)
    fn on_draw_arc(
        &mut self,
        arc: &Rect,
        start_angle: scalar,
        sweep_angle: scalar,
        use_center: bool,
        paint: &Paint,
    ) -> bool {
        if let Some(c) = self.first() {
            c.on_draw_arc(
                arc,
                start_angle,
                sweep_angle,
                use_center,
                &self.overdraw_paint(paint),
            );
        }
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L164-L167 (chrome/m156)
    fn on_draw_drrect(&mut self, outer: &RRect, inner: &RRect, paint: &Paint) -> bool {
        if let Some(c) = self.first() {
            c.on_draw_drrect(outer, inner, &self.overdraw_paint(paint));
        }
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L169-L171 (chrome/m156)
    fn on_draw_rrect(&mut self, rrect: &RRect, paint: &Paint) -> bool {
        if let Some(c) = self.first() {
            c.on_draw_rrect(rrect, &self.overdraw_paint(paint));
        }
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L173-L176 (chrome/m156)
    fn on_draw_points(&mut self, mode: PointMode, pts: &[Point], paint: &Paint) -> bool {
        if let Some(c) = self.first() {
            c.on_draw_points(mode, pts, &self.overdraw_paint(paint));
        }
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L178-L181 (chrome/m156)
    fn on_draw_vertices_object(
        &mut self,
        vertices: &Vertices,
        mode: BlendMode,
        paint: &Paint,
    ) -> bool {
        if let Some(c) = self.first() {
            c.on_draw_vertices_object(vertices, mode, &self.overdraw_paint(paint));
        }
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L183-L196 (chrome/m156)
    fn on_draw_atlas2(
        &mut self,
        atlas_shader: Option<&Shader>,
        xform: &[RSXform],
        tex: &[Rect],
        colors: &[Color],
        mode: BlendMode,
        cull: Option<&Rect>,
        paint: Option<&Paint>,
    ) -> bool {
        let storage;
        let paint_ptr = if let Some(paint) = paint {
            storage = self.overdraw_paint(paint);
            &storage
        } else {
            &self.paint
        };

        if let (Some(c), Some(shader)) = (self.first(), atlas_shader) {
            c.draw_atlas_with_shader(
                shader,
                xform,
                tex,
                Some(colors).filter(|colors| !colors.is_empty()),
                mode,
                cull.copied(),
                paint_ptr,
            );
        }
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L198-L200 (chrome/m156)
    fn on_draw_path(&mut self, path: &Path, _paint: &Paint) -> bool {
        if let Some(c) = self.first() {
            c.on_draw_path(path, &self.paint);
        }
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L202-L206 (chrome/m156), onDrawImage2 is not
    // reachable (drawImage goes through drawImageRect)
    fn on_draw_image_rect2(
        &mut self,
        _image: &Image,
        _src: &Rect,
        dst: &Rect,
        _sampling: &SamplingOptions,
        _paint: Option<&Paint>,
        _constraint: SrcRectConstraint,
    ) -> bool {
        if let Some(c) = self.first() {
            c.on_draw_rect(dst, &self.paint);
        }
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L208-L232 (chrome/m156)
    fn on_draw_image_lattice2(
        &mut self,
        image: &Image,
        lattice: &Lattice<'_>,
        dst: &Rect,
        _filter: FilterMode,
        _paint: Option<&Paint>,
    ) -> bool {
        let Some(c) = self.first() else {
            return true;
        };
        let mut lattice_plus_bounds = Lattice {
            x_divs: lattice.x_divs,
            y_divs: lattice.y_divs,
            rect_types: lattice.rect_types,
            bounds: lattice.bounds,
            colors: lattice.colors,
        };
        if lattice_plus_bounds.bounds.is_none() {
            lattice_plus_bounds.bounds = Some(IRect::from_wh(image.width(), image.height()));
        }

        if LatticeIter::valid(image.width(), image.height(), &lattice_plus_bounds) {
            let mut iter = LatticeIter::new(&lattice_plus_bounds, dst);

            while let Some(patch) = iter.next_patch() {
                c.on_draw_rect(&patch.dst, &self.paint);
            }
        } else {
            c.on_draw_rect(dst, &self.paint);
        }
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L234-L236 (chrome/m156). The drawable draws into
    // this canvas (`drawable->draw(this, matrix)`), which is what the canvas does when the hook
    // does not handle the draw.
    fn on_draw_drawable(
        &mut self,
        _drawable: &skia_rust_core::drawable::Drawable,
        _matrix: Option<&Matrix>,
    ) -> bool {
        false
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L238-L240 (chrome/m156)
    fn on_draw_picture(
        &mut self,
        _picture: &skia_rust_core::picture::Picture,
        _matrix: Option<&Matrix>,
        _paint: Option<&Paint>,
    ) -> bool {
        // SkASSERT(false)
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L242-L242 (chrome/m156)
    fn on_draw_annotation(
        &mut self,
        _rect: &Rect,
        _key: &str,
        _value: Option<&skia_rust_core::data::Data>,
    ) -> bool {
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L251-L260 (chrome/m156)
    fn on_draw_edge_aa_quad(
        &mut self,
        rect: &Rect,
        clip: Option<&[Point; 4]>,
        _aa_flags: QuadAAFlags,
        _color: &Color4f,
        _mode: BlendMode,
    ) -> bool {
        if let Some(c) = self.first() {
            if let Some(clip) = clip {
                c.on_draw_path(&Path::polygon(clip, true, None, None), &self.paint);
            } else {
                c.on_draw_rect(rect, &self.paint);
            }
        }
        true
    }

    // Port of: src/core/SkOverdrawCanvas.cpp#L262-L289 (chrome/m156)
    fn on_draw_edge_aa_image_set2(
        &mut self,
        image_set: &[ImageSetEntry],
        dst_clips: &[Point],
        preview_matrices: &[Matrix],
        _sampling: &SamplingOptions,
        _paint: Option<&Paint>,
        _constraint: SrcRectConstraint,
    ) -> bool {
        let Some(c) = self.first() else {
            return true;
        };
        let mut clip_index = 0;
        for entry in image_set {
            if let Some(matrix_index) = entry.matrix_index {
                c.save();
                c.concat(&preview_matrices[matrix_index]);
            }
            if entry.has_clip {
                c.on_draw_path(
                    &Path::polygon(&dst_clips[clip_index..clip_index + 4], true, None, None),
                    &self.paint,
                );
                clip_index += 4;
            } else {
                c.on_draw_rect(&entry.dst_rect, &self.paint);
            }
            if entry.matrix_index.is_some() {
                c.restore();
            }
        }
        true
    }

    // Inherited from SkNWayCanvas.
    fn on_draw_mesh(&mut self, mesh: &Mesh, blender: &Blender, paint: &Paint) -> bool {
        self.base.on_draw_mesh(mesh, blender, paint)
    }

    // Inherited from SkNWayCanvas.
    fn on_draw_slug(&mut self, slug: &Slug, paint: &Paint) -> bool {
        self.base.on_draw_slug(slug, paint)
    }
}

/// Captures all drawing commands. Rather than draw the actual content, this canvas increments the
/// alpha channel of each pixel every time it would have been touched by a draw call. This is
/// useful for detecting overdraw (`SkOverdrawCanvas`).
// Port of: include/core/SkOverdrawCanvas.h#L35-L83 (chrome/m156)
#[doc(alias = "SkOverdrawCanvas")]
#[derive(Debug)]
pub struct OverdrawCanvas {
    n_way: NWayCanvas,
}

impl OverdrawCanvas {
    /// An overdraw canvas over `canvas`, which is shared, not owned (`SkOverdrawCanvas(SkCanvas*)`).
    // Port of: src/core/SkOverdrawCanvas.cpp#L38-L57 (chrome/m156)
    #[must_use]
    pub fn new(canvas: Rc<Canvas>) -> OverdrawCanvas {
        #[rustfmt::skip]
        static INCREMENT_ALPHA: [f32; 20] = [
            0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 1.0 / 255.0,
        ];

        let info = canvas.image_info();

        let mut paint = Paint::default();
        paint.set_anti_alias(false);
        paint.set_blend_mode(BlendMode::Plus);
        paint.set_color_filter(color_filters::matrix_row_major(
            &INCREMENT_ALPHA,
            Clamp::Yes,
        ));

        let n_way = NWayCanvas::with_hooks(info.width(), info.height(), |base| {
            Box::new(OverdrawHooks { base, paint })
        });
        // Non-drawing calls that SkOverdrawCanvas does not override (translate, save, etc.)
        // will pass through to the input canvas.
        n_way.add_canvas(canvas);
        OverdrawCanvas { n_way }
    }
}

impl Deref for OverdrawCanvas {
    type Target = Canvas;

    fn deref(&self) -> &Canvas {
        &self.n_way
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surfaces;
    use skia_rust_core::color_priv::get_packed_a32;

    #[test]
    fn counts_the_draws_that_touch_a_pixel() {
        let mut surface = surfaces::raster_n32_premul((10, 10)).unwrap();
        let canvas = OverdrawCanvas::new(surface.shared_canvas());
        let paint = Paint::default();
        canvas.draw_rect(Rect::new(0.0, 0.0, 6.0, 10.0), &paint);
        canvas.draw_rect(Rect::new(4.0, 0.0, 10.0, 10.0), &paint);
        canvas.draw_oval(Rect::new(0.0, 0.0, 10.0, 10.0), &paint);
        let image = surface.image_snapshot().unwrap();
        let bitmap = image.as_legacy_bitmap().unwrap();
        // x = 1 is in the first rect only, x = 5 in both; the corner pixel is outside the oval
        assert_eq!(get_packed_a32(bitmap.get_addr32(1, 5)), 2);
        assert_eq!(get_packed_a32(bitmap.get_addr32(5, 5)), 3);
        assert_eq!(get_packed_a32(bitmap.get_addr32(0, 0)), 1);
    }
}
