// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/utils/SkNWayCanvas.h, src/utils/SkNWayCanvas.cpp,
// include/utils/SkNullCanvas.h, src/utils/SkNullCanvas.cpp

//! [`NWayCanvas`]: a canvas that forwards every call to a list of other canvases
//! (`SkNWayCanvas`), and [`make_null_canvas`] (`SkMakeNullCanvas`).
//!
//! skia-rust: `SkNWayCanvas` subclasses `SkNoDrawCanvas` and overrides the virtual hooks of
//! `SkCanvas`. Here it is a [`Canvas`] over a no-pixels device that carries a [`CanvasHooks`]
//! object forwarding to the list, and [`NWayCanvas`] owns that canvas and derefs to it (as
//! [`RecordCanvas`](crate::record_canvas::RecordCanvas) does). The list holds `Rc<Canvas>`
//! instead of `SkCanvas*`: the n-way canvas never owns the lifetime of its canvases in Skia, and
//! here it only keeps them alive while it refers to them, so dropping it does not drop a canvas
//! somebody else still holds. `SkCanvas::onDoSaveBehind`, `onDrawBehind` and `onDrawShadowRec`
//! are not forwarded: those draws are not ported on `Canvas`.

use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

use crate::blend_mode::BlendMode;
use crate::blender::Blender;
use crate::canvas::{
    Canvas, CanvasHooks, ImageSetEntry, Lattice, PointMode, QuadAAFlags, SaveLayerRec,
    SaveLayerStrategy, SrcRectConstraint,
};
use crate::clip_op::ClipOp;
use crate::color::{Color, Color4f};
use crate::data::Data;
use crate::drawable::Drawable;
use crate::glyph_run::GlyphRunList;
use crate::image::Image;
use crate::m44::M44;
use crate::matrix::Matrix;
use crate::mesh::Mesh;
use crate::paint::Paint;
use crate::path::Path;
use crate::picture::Picture;
use crate::point::{Point, Vector};
use crate::rect::{IRect, Rect};
use crate::region::Region;
use crate::rrect::RRect;
use crate::rsxform::RSXform;
use crate::sampling_options::{FilterMode, SamplingOptions};
use crate::scalar::scalar;
use crate::shader::Shader;
use crate::slug::Slug;
use crate::text_blob::TextBlob;
use crate::utils::canvas_stack::CanvasData;
use crate::utils::patch_utils;
use crate::vertices::Vertices;

/// The canvases an n-way canvas forwards to (`SkNWayCanvas::fList`).
pub(crate) type CanvasList = Rc<RefCell<Vec<Rc<Canvas>>>>;

/// The overrides of `SkNWayCanvas`. When `stack` is set, they are those of `SkCanvasStack`,
/// which derives from `SkNWayCanvas` and changes the matrix and clip calls.
struct NWayHooks {
    list: CanvasList,
    stack: Option<Rc<RefCell<Vec<CanvasData>>>>,
}

impl NWayHooks {
    // `SkNWayCanvas::Iter`: visits the canvases in the order they were added.
    // Port of: src/utils/SkNWayCanvas.cpp#L65-L84 (chrome/m156)
    fn each(&self, mut f: impl FnMut(&Canvas)) {
        let mut index = 0;
        loop {
            let Some(canvas) = self.list.borrow().get(index).map(Rc::clone) else {
                break;
            };
            index += 1;
            f(&canvas);
        }
    }

    // Port of: src/utils/SkCanvasStack.cpp#L60-L64 (chrome/m156), `clipToZOrderedBounds`; a plain
    // n-way canvas does nothing
    fn clip_to_z_ordered_bounds(&self) {
        if let Some(stack) = &self.stack {
            crate::utils::canvas_stack::clip_to_z_ordered_bounds(&self.list, stack);
        }
    }
}

impl CanvasHooks for NWayHooks {
    // Port of: src/utils/SkNWayCanvas.cpp#L86-L93 (chrome/m156)
    fn will_save(&mut self) {
        self.each(|c| {
            c.save();
        });
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L95-L104 (chrome/m156)
    fn get_save_layer_strategy(&mut self, rec: &SaveLayerRec<'_>) -> SaveLayerStrategy {
        self.each(|c| {
            c.save_layer(rec);
        });
        // No need for a layer.
        SaveLayerStrategy::NoLayer
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L115-L121 (chrome/m156)
    fn will_restore(&mut self) {
        self.each(|c| {
            c.restore();
        });
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L123-L128 (chrome/m156)
    fn did_concat44(&mut self, m: &M44) {
        self.each(|c| {
            c.concat_44(m);
        });
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L130-L135 (chrome/m156), and
    // src/utils/SkCanvasStack.cpp#L66-L73 (chrome/m156) for a stack
    fn did_set_m44(&mut self, matrix: &M44) {
        if let Some(stack) = &self.stack {
            crate::utils::canvas_stack::did_set_m44(&self.list, stack, matrix);
            return;
        }
        self.each(|c| {
            c.set_matrix(matrix);
        });
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L144-L149 (chrome/m156)
    fn did_scale(&mut self, x: scalar, y: scalar) {
        self.each(|c| {
            c.scale((x, y));
        });
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L137-L142 (chrome/m156)
    fn did_translate(&mut self, x: scalar, y: scalar) {
        self.each(|c| {
            c.translate(Vector::new(x, y));
        });
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L151-L157 (chrome/m156), and
    // src/utils/SkCanvasStack.cpp#L75-L78 (chrome/m156) for a stack
    fn on_clip_rect(&mut self, rect: &Rect, op: ClipOp, is_aa: bool) {
        self.each(|c| {
            c.clip_rect(rect, op, is_aa);
        });
        self.clip_to_z_ordered_bounds();
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L159-L165 (chrome/m156), and
    // src/utils/SkCanvasStack.cpp#L80-L83 (chrome/m156) for a stack
    fn on_clip_rrect(&mut self, rrect: &RRect, op: ClipOp, is_aa: bool) {
        self.each(|c| {
            c.clip_rrect(rrect, op, is_aa);
        });
        self.clip_to_z_ordered_bounds();
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L167-L173 (chrome/m156), and
    // src/utils/SkCanvasStack.cpp#L85-L88 (chrome/m156) for a stack
    fn on_clip_path(&mut self, path: &Path, op: ClipOp, is_aa: bool) {
        self.each(|c| {
            c.clip_path(path, op, is_aa);
        });
        self.clip_to_z_ordered_bounds();
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L175-L181 (chrome/m156), and
    // src/utils/SkCanvasStack.cpp#L90-L93 (chrome/m156) for a stack
    fn on_clip_shader(&mut self, shader: &Shader, op: ClipOp) {
        self.each(|c| {
            c.clip_shader(shader.clone(), op);
        });
        // (a stack does not change the "bounds" of the clip, so it needs no z-order update)
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L183-L189 (chrome/m156), and
    // src/utils/SkCanvasStack.cpp#L95-L105 (chrome/m156) for a stack
    fn on_clip_region(&mut self, device_rgn: &Region, op: ClipOp) {
        if let Some(stack) = &self.stack {
            crate::utils::canvas_stack::on_clip_region(&self.list, stack, device_rgn, op);
            return;
        }
        self.each(|c| {
            c.clip_region(device_rgn, op);
        });
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L191-L197 (chrome/m156)
    fn on_reset_clip(&mut self) {
        self.each(|c| {
            // SkCanvasPriv::ResetClip
            c.reset_clip();
        });
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L199-L204 (chrome/m156)
    fn on_draw_paint(&mut self, paint: &Paint) -> bool {
        self.each(|c| {
            c.draw_paint(paint);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L213-L219 (chrome/m156)
    fn on_draw_points(&mut self, mode: PointMode, pts: &[Point], paint: &Paint) -> bool {
        self.each(|c| {
            c.draw_points(mode, pts, paint);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L221-L226 (chrome/m156)
    fn on_draw_rect(&mut self, rect: &Rect, paint: &Paint) -> bool {
        self.each(|c| {
            c.draw_rect(rect, paint);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L228-L233 (chrome/m156)
    fn on_draw_region(&mut self, region: &Region, paint: &Paint) -> bool {
        self.each(|c| {
            c.draw_region(region, paint);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L235-L240 (chrome/m156)
    fn on_draw_oval(&mut self, rect: &Rect, paint: &Paint) -> bool {
        self.each(|c| {
            c.draw_oval(rect, paint);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L242-L248 (chrome/m156)
    fn on_draw_arc(
        &mut self,
        rect: &Rect,
        start_angle: scalar,
        sweep_angle: scalar,
        use_center: bool,
        paint: &Paint,
    ) -> bool {
        self.each(|c| {
            c.draw_arc(rect, start_angle, sweep_angle, use_center, paint);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L250-L255 (chrome/m156)
    fn on_draw_rrect(&mut self, rrect: &RRect, paint: &Paint) -> bool {
        self.each(|c| {
            c.draw_rrect(rrect, paint);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L257-L262 (chrome/m156)
    fn on_draw_drrect(&mut self, outer: &RRect, inner: &RRect, paint: &Paint) -> bool {
        self.each(|c| {
            c.draw_drrect(outer, inner, paint);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L264-L269 (chrome/m156)
    fn on_draw_path(&mut self, path: &Path, paint: &Paint) -> bool {
        self.each(|c| {
            c.draw_path(path, paint);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L309-L314 (chrome/m156)
    fn on_draw_glyph_run_list(&mut self, list: &GlyphRunList<'_>, paint: &Paint) -> bool {
        self.each(|c| c.draw_glyph_run_list(list, paint));
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L316-L322 (chrome/m156)
    fn on_draw_text_blob(&mut self, blob: &TextBlob, x: scalar, y: scalar, paint: &Paint) -> bool {
        self.each(|c| {
            c.draw_text_blob(blob, (x, y), paint);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L324-L329 (chrome/m156)
    fn on_draw_slug(&mut self, slug: &Slug, paint: &Paint) -> bool {
        self.each(|c| {
            c.draw_slug(Some(slug), paint);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L345-L351 (chrome/m156)
    fn on_draw_vertices_object(
        &mut self,
        vertices: &Vertices,
        mode: BlendMode,
        paint: &Paint,
    ) -> bool {
        self.each(|c| {
            c.draw_vertices(vertices, mode, paint);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L353-L360 (chrome/m156)
    fn on_draw_patch(
        &mut self,
        cubics: &[Point; patch_utils::NUM_CTRL_PTS],
        colors: Option<&[Color; patch_utils::NUM_CORNERS]>,
        tex_coords: Option<&[Point; patch_utils::NUM_CORNERS]>,
        mode: BlendMode,
        paint: &Paint,
    ) -> bool {
        self.each(|c| {
            c.draw_patch(cubics, colors, tex_coords, mode, paint);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L286-L299 (chrome/m156). The atlas is forwarded as the
    // shader that `drawAtlas` made from it.
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
        let Some(atlas_shader) = atlas_shader else {
            return true;
        };
        self.each(|c| {
            c.draw_atlas_with_shader(
                atlas_shader,
                xform,
                tex,
                Some(colors).filter(|colors| !colors.is_empty()),
                mode,
                cull.copied(),
                paint,
            );
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L331-L336 (chrome/m156)
    fn on_draw_picture(
        &mut self,
        picture: &Picture,
        matrix: Option<&Matrix>,
        paint: Option<&Paint>,
    ) -> bool {
        self.each(|c| {
            c.draw_picture(picture, matrix, paint);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L338-L343 (chrome/m156)
    fn on_draw_drawable(&mut self, drawable: &Drawable, matrix: Option<&Matrix>) -> bool {
        self.each(|c| c.draw_drawable(drawable, matrix));
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L368-L373 (chrome/m156)
    fn on_draw_annotation(&mut self, rect: &Rect, key: &str, value: Option<&Data>) -> bool {
        self.each(|c| {
            c.draw_annotation(rect, key, value);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L282-L287 (chrome/m156)
    fn on_draw_mesh(&mut self, mesh: &Mesh, blender: &Blender, paint: &Paint) -> bool {
        self.each(|c| {
            c.draw_mesh(mesh, Some(blender.clone()), paint);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L375-L381 (chrome/m156)
    fn on_draw_edge_aa_quad(
        &mut self,
        rect: &Rect,
        clip: Option<&[Point; 4]>,
        aa_flags: QuadAAFlags,
        color: &Color4f,
        mode: BlendMode,
    ) -> bool {
        self.each(|c| {
            c.experimental_draw_edge_aa_quad(rect, clip, aa_flags, *color, mode);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L383-L392 (chrome/m156)
    fn on_draw_edge_aa_image_set2(
        &mut self,
        image_set: &[ImageSetEntry],
        dst_clips: &[Point],
        preview_matrices: &[Matrix],
        sampling: &SamplingOptions,
        paint: Option<&Paint>,
        constraint: SrcRectConstraint,
    ) -> bool {
        self.each(|c| {
            c.experimental_draw_edge_aa_image_set(
                image_set,
                dst_clips,
                preview_matrices,
                *sampling,
                paint,
                constraint,
            );
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L271-L282 (chrome/m156), onDrawImage2 (unreachable in
    // Skia, so the image is forwarded as its rect draw by `onDrawImageRect2`)
    fn on_draw_image_rect2(
        &mut self,
        image: &Image,
        src: &Rect,
        dst: &Rect,
        sampling: &SamplingOptions,
        paint: Option<&Paint>,
        constraint: SrcRectConstraint,
    ) -> bool {
        self.each(|c| {
            c.draw_image_rect_nullable_paint(image, src, dst, sampling, paint, constraint);
        });
        true
    }

    // Port of: src/utils/SkNWayCanvas.cpp#L279-L290 (chrome/m156)
    fn on_draw_image_lattice2(
        &mut self,
        image: &Image,
        lattice: &Lattice<'_>,
        dst: &Rect,
        filter: FilterMode,
        paint: Option<&Paint>,
    ) -> bool {
        self.each(|c| {
            c.draw_image_lattice(image, lattice, dst, filter, paint);
        });
        true
    }
}

/// A canvas that forwards every call to the canvases added to it (`SkNWayCanvas`).
// Port of: include/utils/SkNWayCanvas.h#L47-L123 (chrome/m156)
#[doc(alias = "SkNWayCanvas")]
pub struct NWayCanvas {
    canvas: Canvas,
    list: CanvasList,
}

impl std::fmt::Debug for NWayCanvas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NWayCanvas")
            .field("canvas_count", &self.list.borrow().len())
            .finish_non_exhaustive()
    }
}

impl NWayCanvas {
    // Port of: src/utils/SkNWayCanvas.cpp#L42-L42 (chrome/m156), over SkNoDrawCanvas(width,
    // height) (include/utils/SkNoDrawCanvas.h)
    pub(crate) fn make_canvas(
        width: i32,
        height: i32,
        stack: Option<Rc<RefCell<Vec<CanvasData>>>>,
    ) -> (Canvas, CanvasList) {
        let canvas = Canvas::new_no_pixels_irect(&IRect::from_wh(width, height), None);
        let list: CanvasList = Rc::new(RefCell::new(Vec::new()));
        canvas.set_hooks(Some(Box::new(NWayHooks {
            list: Rc::clone(&list),
            stack,
        })));
        (canvas, list)
    }

    pub(crate) fn from_parts(canvas: Canvas, list: CanvasList) -> NWayCanvas {
        NWayCanvas { canvas, list }
    }

    /// An n-way canvas of `width` by `height` with no canvases (`SkNWayCanvas(int, int)`).
    #[must_use]
    pub fn new(width: i32, height: i32) -> NWayCanvas {
        let (canvas, list) = Self::make_canvas(width, height, None);
        NWayCanvas { canvas, list }
    }

    /// Adds a canvas to forward to (`addCanvas`). The n-way canvas shares the canvas; it does not
    /// manage its lifetime beyond that.
    // Port of: src/utils/SkNWayCanvas.cpp#L46-L56 (chrome/m156)
    #[doc(alias = "addCanvas")]
    pub fn add_canvas(&self, canvas: Rc<Canvas>) {
        self.list.borrow_mut().push(canvas);
    }

    /// Removes a canvas (`removeCanvas`). The order of the others may change.
    // Port of: src/utils/SkNWayCanvas.cpp#L58-L63 (chrome/m156)
    #[doc(alias = "removeCanvas")]
    pub fn remove_canvas(&self, canvas: &Rc<Canvas>) {
        let mut list = self.list.borrow_mut();
        if let Some(found) = list.iter().position(|c| Rc::ptr_eq(c, canvas)) {
            list.swap_remove(found); // removeShuffle
        }
    }

    /// Removes every canvas (`removeAll`).
    // Port of: src/utils/SkNWayCanvas.cpp#L65-L67 (chrome/m156)
    #[doc(alias = "removeAll")]
    pub fn remove_all(&self) {
        self.list.borrow_mut().clear();
    }
}

impl Deref for NWayCanvas {
    type Target = Canvas;

    fn deref(&self) -> &Canvas {
        &self.canvas
    }
}

impl Drop for NWayCanvas {
    // Port of: src/utils/SkNWayCanvas.cpp#L44-L44 (chrome/m156)
    fn drop(&mut self) {
        self.remove_all();
    }
}

/// A canvas that draws nothing (`SkMakeNullCanvas`): an n-way canvas with no canvases.
// Port of: src/utils/SkNullCanvas.cpp#L13-L16 (chrome/m156)
#[doc(alias = "SkMakeNullCanvas")]
#[must_use]
pub fn make_null_canvas() -> NWayCanvas {
    // An N-Way canvas forwards calls to N canvas's. When N == 0 it's
    // effectively a null canvas.
    NWayCanvas::new(0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip_op::ClipOp;

    #[test]
    fn forwards_state_calls_to_every_canvas() {
        let a = Rc::new(Canvas::new_no_pixels((20, 20), None).unwrap());
        let b = Rc::new(Canvas::new_no_pixels((20, 20), None).unwrap());
        let nway = NWayCanvas::new(20, 20);
        nway.add_canvas(Rc::clone(&a));
        nway.add_canvas(Rc::clone(&b));

        nway.save();
        nway.translate((3.0, 4.0));
        nway.clip_rect(Rect::new(0.0, 0.0, 5.0, 5.0), ClipOp::Intersect, false);
        for c in [&a, &b] {
            assert_eq!(c.save_count(), 2);
            assert_eq!(c.total_matrix(), Matrix::translate(Vector::new(3.0, 4.0)));
            assert_eq!(
                c.device_clip_bounds(),
                Some(IRect::new(3, 4, 8, 9))
            );
        }
        nway.restore();
        assert_eq!(a.save_count(), 1);
        assert_eq!(b.save_count(), 1);

        nway.remove_canvas(&a);
        nway.save();
        nway.translate((1.0, 1.0)); // a save is deferred until something needs it
        assert_eq!(a.save_count(), 1);
        assert_eq!(b.save_count(), 2);
    }
}
