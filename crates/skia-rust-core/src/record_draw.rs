// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRecordDraw.h, src/core/SkRecordDraw.cpp

//! `SkRecordDraw`: plays a [`Record`] back into a [`Canvas`], and computes the bounds of each op
//! for a bounding box hierarchy.
//!
//! skia-rust: Skia's `drawablePicts`/`drawables` parameters (for `DrawDrawable`) are not here;
//! `SkDrawable` is not ported. The draws of the record types that are not ported
//! (`DrawTextBlob`, ...) have no arm.

use crate::bbh_factory::{BBoxHierarchy, Metadata};
use crate::blend_mode::BlendMode;
use crate::canvas::{AutoCanvasRestore, Canvas, Lattice, SaveLayerRec};
use crate::m44::M44;
use crate::matrix::Matrix;
use crate::paint::Paint;
use crate::picture::AbortCallback;
use crate::point::Point;
use crate::record::Record;
use crate::records::Command;
use crate::rect::Rect;
use crate::scalar::scalar;

/// Draws a record into a canvas (`SkRecordDraw`).
///
/// With a `bbh`, only the ops that affect the pixels in the canvas's current clip are drawn.
// Port of: src/core/SkRecordDraw.cpp#L48-L87 (chrome/m156)
#[doc(alias = "SkRecordDraw")]
pub fn record_draw(
    record: &Record,
    canvas: &Canvas,
    bbh: Option<&dyn BBoxHierarchy>,
    mut callback: Option<&mut dyn AbortCallback>,
) {
    let _save_restore = AutoCanvasRestore::guard(canvas, true /*save now, restore at exit*/);

    if let Some(bbh) = bbh {
        // Draw only ops that affect pixels in the canvas's current clip.
        // The record and BBH were recorded in identity space.  This canvas
        // is not necessarily in that same space.  getLocalClipBounds() returns us
        // this canvas' clip bounds transformed back into identity space, which
        // lets us query the BBH.
        let query = canvas.local_clip_bounds().unwrap_or_else(Rect::new_empty);

        let mut ops = Vec::new();
        bbh.search(&query, &mut ops);

        let draw = Draw::new(canvas, None);
        for &op in &ops {
            if let Some(callback) = callback.as_deref_mut()
                && callback.abort()
            {
                return;
            }
            // This visit call uses Draw::draw to call methods on the |canvas|.
            draw.draw(record.get(op));
        }
    } else {
        // Draw all ops.
        let draw = Draw::new(canvas, None);
        for i in 0..record.count() {
            if let Some(callback) = callback.as_deref_mut()
                && callback.abort()
            {
                return;
            }
            // This visit call uses Draw::draw to call methods on the |canvas|.
            draw.draw(record.get(i));
        }
    }
}

/// The visitor that draws a record to a canvas (`SkRecords::Draw`).
// Port of: src/core/SkRecordDraw.h#L31-L62 (chrome/m156)
#[doc(alias = "SkRecords::Draw")]
#[derive(Debug)]
pub struct Draw<'a> {
    initial_ctm: M44,
    canvas: &'a Canvas,
}

impl<'a> Draw<'a> {
    /// A visitor drawing into `canvas`; `initial_ctm` defaults to the canvas's current matrix.
    pub fn new(canvas: &'a Canvas, initial_ctm: Option<&M44>) -> Draw<'a> {
        Draw {
            initial_ctm: initial_ctm
                .copied()
                .unwrap_or_else(|| canvas.local_to_device()),
            canvas,
        }
    }

    /// Calls the canvas method for `command` (the `DRAW()` wrappers).
    // Port of: src/core/SkRecordDraw.cpp#L89-L176 (chrome/m156)
    #[allow(clippy::too_many_lines)] // one arm per record type, as the DRAW() list
    pub fn draw(&self, command: &Command) {
        let canvas = self.canvas;
        match command {
            // NoOps draw nothing.
            Command::NoOp(_) => {}
            Command::Restore(_) => {
                canvas.restore();
            }
            Command::Save(_) => {
                canvas.save();
            }
            Command::SaveLayer(r) => {
                // (The backdrop scale and the filters list of the record are not part of
                // `SaveLayerRec` yet: image filters are Phase 3.)
                let mut rec = SaveLayerRec::default();
                if let Some(bounds) = &r.bounds {
                    rec = rec.bounds(bounds);
                }
                if let Some(paint) = &r.paint {
                    rec = rec.paint(paint);
                }
                if let Some(backdrop) = &r.backdrop {
                    rec = rec.backdrop(backdrop);
                }
                canvas.save_layer(
                    &rec.backdrop_tile_mode(r.backdrop_tile_mode)
                        .flags(r.save_layer_flags),
                );
            }
            Command::SetMatrix(r) => {
                let m = Matrix::concat(&self.initial_ctm.to_m33(), &r.matrix);
                canvas.set_matrix(&M44::from(&m));
            }
            Command::SetM44(r) => {
                canvas.set_matrix(&M44::concat(&self.initial_ctm, &r.matrix));
            }
            Command::Concat44(r) => {
                canvas.concat_44(&r.matrix);
            }
            Command::Concat(r) => {
                canvas.concat(&r.matrix);
            }
            Command::Translate(r) => {
                canvas.translate((r.dx, r.dy));
            }
            Command::Scale(r) => {
                canvas.scale((r.sx, r.sy));
            }

            Command::ClipPath(r) => {
                canvas.clip_path(&r.path, r.op_aa.op(), r.op_aa.aa());
            }
            Command::ClipRRect(r) => {
                canvas.clip_rrect(r.rrect, r.op_aa.op(), r.op_aa.aa());
            }
            Command::ClipRect(r) => {
                canvas.clip_rect(r.rect, r.op_aa.op(), r.op_aa.aa());
            }
            Command::ClipRegion(r) => {
                canvas.clip_region(&r.region, r.op);
            }
            Command::ClipShader(r) => {
                canvas.clip_shader(r.shader.clone(), r.op);
            }
            Command::ResetClip(_) => {
                canvas.reset_clip();
            }

            Command::DrawArc(r) => {
                canvas.draw_arc(r.oval, r.start_angle, r.sweep_angle, r.use_center, &r.paint);
            }
            Command::DrawDRRect(r) => {
                canvas.draw_drrect(r.outer, r.inner, &r.paint);
            }
            Command::DrawImage(r) => {
                canvas.draw_image_with_sampling_options(
                    &r.image,
                    (r.left, r.top),
                    r.sampling,
                    r.paint.as_ref(),
                );
            }
            // Port of: src/core/SkRecordDraw.cpp#L141-L151 (chrome/m156)
            Command::DrawImageLattice(r) => {
                let lattice = Lattice {
                    x_divs: &r.x_divs,
                    y_divs: &r.y_divs,
                    rect_types: if r.flag_count == 0 {
                        None
                    } else {
                        Some(&r.flags)
                    },
                    colors: if r.flag_count == 0 || r.colors.is_empty() {
                        None
                    } else {
                        Some(&r.colors)
                    },
                    bounds: Some(r.src),
                };
                canvas.draw_image_lattice(&r.image, &lattice, r.dst, r.filter, r.paint.as_ref());
            }
            Command::DrawImageRect(r) => {
                canvas.draw_image_rect_nullable_paint(
                    &r.image,
                    &r.src,
                    &r.dst,
                    &r.sampling,
                    r.paint.as_ref(),
                    r.constraint,
                );
            }
            Command::DrawOval(r) => {
                canvas.draw_oval(r.oval, &r.paint);
            }
            Command::DrawPaint(r) => {
                canvas.draw_paint(&r.paint);
            }
            Command::DrawPath(r) => {
                canvas.draw_path(&r.path, &r.paint);
            }
            Command::DrawPicture(r) => {
                canvas.draw_picture(&r.picture, Some(&r.matrix), r.paint.as_ref());
            }
            Command::DrawPoints(r) => {
                canvas.draw_points(r.mode, &r.pts, &r.paint);
            }
            Command::DrawRRect(r) => {
                canvas.draw_rrect(r.rrect, &r.paint);
            }
            Command::DrawRect(r) => {
                canvas.draw_rect(r.rect, &r.paint);
            }
            Command::DrawRegion(r) => {
                canvas.draw_region(&r.region, &r.paint);
            }
            Command::DrawTextBlob(r) => {
                canvas.draw_text_blob(&r.blob, (r.x, r.y), &r.paint);
            }
        }
    }
}

// This is a record visitor that fills a bounding box hierarchy.
//
// The interesting part here is how to calculate bounds for ops which don't
// have intrinsic bounds.  What is the bounds of a Save or a Translate?
//
// We answer this by thinking about a particular definition of bounds: if I
// don't execute this op, pixels in this rectangle might draw incorrectly.  So
// the bounds of a Save, a Translate, a Restore, etc. are the union of the
// bounds of Draw* ops that they might have an effect on.  For any given
// Save/Restore block, the bounds of the Save, the Restore, and any other
// non-drawing ("control") ops inside are exactly the union of the bounds of
// the drawing ops inside that block.
//
// To implement this, we keep a stack of active Save blocks.  As we consume ops
// inside the Save/Restore block, drawing ops are unioned with the bounds of
// the block, and control ops are stashed away for later.  When we finish the
// block with a Restore, our bounds are complete, and we go back and fill them
// in for all the control ops we stashed away.
// Port of: src/core/SkRecordDraw.cpp#L178-L576 (chrome/m156)
struct FillBounds<'r, 'o> {
    // We do not guarantee anything for operations outside of the cull rect
    cull_rect: Rect,

    // Conservative identity-space bounds for each op in the record.
    bounds: &'o mut [Rect],

    // Parallel array to bounds, holding metadata for each bounds rect.
    meta: &'o mut [Metadata],

    // We walk current_op through the record,
    // as we go using update_ctm() to maintain the exact CTM (ctm).
    current_op: usize,
    ctm: Matrix,

    // Used to track the bounds of Save/Restore blocks and the control ops inside them.
    save_stack: Vec<SaveBounds<'r>>,
    control_indices: Vec<usize>,
}

// In this file, Rect are in local coordinates, Bounds are translated back to identity space.
type Bounds = Rect;

struct SaveBounds<'r> {
    control_ops: usize, // Number of control ops in this Save block, including the Save.
    bounds: Bounds,     // Bounds of everything in the block.
    paint: Option<&'r Paint>, // If set, adjusts the bounds of all ops in this block.
    ctm: Matrix,
}

impl<'r, 'o> FillBounds<'r, 'o> {
    // Port of: src/core/SkRecordDraw.cpp#L198-L211 (chrome/m156)
    fn new(cull_rect: &Rect, bounds: &'o mut [Rect], meta: &'o mut [Metadata]) -> Self {
        let mut fill = FillBounds {
            cull_rect: *cull_rect,
            bounds,
            meta,
            current_op: 0,
            ctm: Matrix::new_identity(),
            save_stack: Vec::new(),
            control_indices: Vec::new(),
        };
        // We push an extra save block to track the bounds of any top-level control operations.
        fill.save_stack.push(SaveBounds {
            control_ops: 0,
            bounds: Bounds::new_empty(),
            paint: None,
            ctm: Matrix::new_identity(),
        });
        fill
    }

    // The destructor of SkRecords::FillBounds.
    // Port of: src/core/SkRecordDraw.cpp#L213-L224 (chrome/m156)
    fn finish(mut self) {
        // If we have any lingering unpaired Saves, simulate restores to make
        // sure all ops in those Save blocks have their bounds calculated.
        while !self.save_stack.is_empty() {
            self.pop_save_block();
        }

        // Any control ops not part of any Save/Restore block draw everywhere.
        while !self.control_indices.is_empty() {
            let cull = self.cull_rect;
            self.pop_control(&cull);
        }
    }

    fn set_current_op(&mut self, current_op: usize) {
        self.current_op = current_op;
    }

    // Port of: src/core/SkRecordDraw.cpp#L228-L231 (chrome/m156)
    fn visit(&mut self, op: &'r Command) {
        self.update_ctm(op);
        self.track_bounds(op);
    }

    // Adjust rect for all paints that may affect its geometry, then map it to identity space.
    // Port of: src/core/SkRecordDraw.cpp#L236-L260 (chrome/m156)
    fn adjust_and_map(&self, mut rect: Rect, paint: Option<&Paint>) -> Bounds {
        // Inverted rectangles really confuse our BBHs.
        rect.sort();

        // Adjust the rect for its own paint.
        if !Self::adjust_for_paint(paint, &mut rect) {
            // The paint could do anything to our bounds.  The only safe answer is the cull.
            return self.cull_rect;
        }

        // Adjust rect for all the paints from the SaveLayers we're inside.
        if !self.adjust_for_save_layer_paints(&mut rect, 0) {
            // Same deal as above.
            return self.cull_rect;
        }

        // Map the rect back to identity space.
        rect = self.ctm.map_rect(rect).0;

        // Nothing can draw outside the cull rect.
        if !rect.intersect(self.cull_rect) {
            return Bounds::new_empty();
        }

        rect
    }

    // Only Restore, SetMatrix, Concat, and Translate change the CTM.
    // Port of: src/core/SkRecordDraw.cpp#L270-L277 (chrome/m156)
    fn update_ctm(&mut self, op: &Command) {
        match op {
            Command::Restore(op) => self.ctm = op.matrix.clone(),
            Command::SetMatrix(op) => self.ctm = op.matrix.clone(),
            Command::SetM44(op) => self.ctm = op.matrix.to_m33(),
            Command::Concat44(op) => {
                self.ctm.pre_concat(&op.matrix.to_m33());
            }
            Command::Concat(op) => {
                self.ctm.pre_concat(&op.matrix);
            }
            Command::Scale(op) => {
                self.ctm.pre_scale((op.sx, op.sy), None);
            }
            Command::Translate(op) => {
                self.ctm.pre_translate((op.dx, op.dy));
            }
            _ => {}
        }
    }

    // The bounds of these ops must be calculated when we hit the Restore
    // from the bounds of the ops in the same Save block.
    // Port of: src/core/SkRecordDraw.cpp#L279-L318 (chrome/m156)
    fn track_bounds(&mut self, op: &'r Command) {
        match op {
            Command::Save(_) => {
                self.push_save_block(None, /*has_backdrop_filter=*/ false);
            }
            Command::SaveLayer(op) => {
                self.push_save_block(
                    op.paint.as_ref(),
                    /*has_backdrop_filter=*/ op.backdrop.is_some(),
                );
            }
            Command::Restore(_) => {
                let is_save_layer = self
                    .save_stack
                    .last()
                    .expect("a restore has a save block")
                    .paint
                    .is_some();
                let bounds = self.pop_save_block();
                self.bounds[self.current_op] = bounds;
                self.meta[self.current_op].is_draw = is_save_layer;
            }

            Command::SetMatrix(_)
            | Command::SetM44(_)
            | Command::Concat(_)
            | Command::Concat44(_)
            | Command::Scale(_)
            | Command::Translate(_)
            | Command::ClipRect(_)
            | Command::ClipRRect(_)
            | Command::ClipPath(_)
            | Command::ClipRegion(_)
            | Command::ClipShader(_)
            | Command::ResetClip(_) => self.push_control(),

            // For all other ops, we can calculate and store the bounds directly now.
            _ => {
                self.bounds[self.current_op] = self.bounds_of(op);
                self.meta[self.current_op].is_draw = true;
                let bounds = self.bounds[self.current_op];
                self.update_save_bounds(&bounds);
            }
        }
    }

    // Port of: src/core/SkRecordDraw.cpp#L320-L338 (chrome/m156)
    fn push_save_block(&mut self, paint: Option<&'r Paint>, has_backdrop_filter: bool) {
        // Starting a new Save block.  Push a new entry to represent that.
        let mut sb = SaveBounds {
            control_ops: 0,
            bounds: Bounds::new_empty(),
            paint,
            ctm: self.ctm.clone(),
        };

        // If the paint affects transparent black, or we have a backdrop filter,
        // the bound shouldn't be smaller than the cull.
        let affects_full_cull_rect =
            has_backdrop_filter || Self::paint_may_affect_transparent_black(paint);
        sb.bounds = if affects_full_cull_rect {
            self.cull_rect
        } else {
            Bounds::new_empty()
        };

        self.save_stack.push(sb);
        self.push_control();
    }

    // Port of: src/core/SkRecordDraw.cpp#L340-L378 (chrome/m156)
    fn paint_may_affect_transparent_black(paint: Option<&Paint>) -> bool {
        if let Some(paint) = paint {
            // FIXME: this is very conservative
            if paint
                .image_filter()
                .is_some_and(|f| f.as_base().affects_transparent_black())
                || paint
                    .color_filter()
                    .is_some_and(|f| f.as_base().affects_transparent_black())
            {
                return true;
            }
            let Some(bm) = paint.as_blend_mode() else {
                return true; // can we query other blenders for this?
            };

            // Unusual blendmodes require us to process a saved layer
            // even with operations outisde the clip.
            // For example, DstIn is used by masking layers.
            // https://code.google.com/p/skia/issues/detail?id=1291
            // https://crbug.com/401593
            match bm {
                // For each of the following transfer modes, if the source
                // alpha is zero (our transparent black), the resulting
                // blended alpha is not necessarily equal to the original
                // destination alpha.
                BlendMode::Clear
                | BlendMode::Src
                | BlendMode::SrcIn
                | BlendMode::DstIn
                | BlendMode::SrcOut
                | BlendMode::DstATop
                | BlendMode::Modulate => return true,
                _ => {}
            }
        }
        false
    }

    // Port of: src/core/SkRecordDraw.cpp#L380-L395 (chrome/m156)
    fn pop_save_block(&mut self) -> Bounds {
        // We're done the Save block.  Apply the block's bounds to all control ops inside it.
        let mut sb = self.save_stack.pop().expect("a save block to pop");

        while sb.control_ops > 0 {
            sb.control_ops -= 1;
            self.pop_control(&sb.bounds);
        }

        // This whole Save block may be part another Save block.
        self.update_save_bounds(&sb.bounds);

        // If called from a real Restore (not a phony one for balance), it'll need the bounds.
        sb.bounds
    }

    // Port of: src/core/SkRecordDraw.cpp#L397-L402 (chrome/m156)
    fn push_control(&mut self) {
        self.control_indices.push(self.current_op);
        if let Some(back) = self.save_stack.last_mut() {
            back.control_ops += 1;
        }
    }

    // Port of: src/core/SkRecordDraw.cpp#L404-L408 (chrome/m156)
    fn pop_control(&mut self, bounds: &Bounds) {
        let index = *self.control_indices.last().expect("a control op to pop");
        self.bounds[index] = *bounds;
        self.meta[index].is_draw = false;
        self.control_indices.pop();
    }

    // Port of: src/core/SkRecordDraw.cpp#L410-L415 (chrome/m156)
    fn update_save_bounds(&mut self, bounds: &Bounds) {
        // If we're in a Save block, expand its bounds to cover these bounds too.
        if let Some(back) = self.save_stack.last_mut() {
            back.bounds.join(bounds);
        }
    }

    // The bounds of a draw (the overloads of `bounds()`).
    // Port of: src/core/SkRecordDraw.cpp#L417-L535 (chrome/m156)
    fn bounds_of(&self, op: &Command) -> Bounds {
        match op {
            Command::DrawPaint(_) => self.cull_rect,
            Command::DrawImage(op) => {
                #[allow(clippy::cast_precision_loss)] // mirrors SkRect::MakeXYWH(int args)
                let rect = Rect::from_xywh(
                    op.left,
                    op.top,
                    op.image.width() as scalar,
                    op.image.height() as scalar,
                );
                self.adjust_and_map(rect, op.paint.as_ref())
            }
            Command::DrawImageLattice(op) => self.adjust_and_map(op.dst, op.paint.as_ref()),
            Command::DrawImageRect(op) => self.adjust_and_map(op.dst, op.paint.as_ref()),
            Command::NoOp(_) => Bounds::new_empty(), // NoOps don't draw.

            Command::DrawRect(op) => self.adjust_and_map(op.rect, Some(&op.paint)),
            Command::DrawRegion(op) => {
                let rect = Rect::from_irect(op.region.bounds());
                self.adjust_and_map(rect, Some(&op.paint))
            }
            Command::DrawTextBlob(op) => {
                // `op.blob->bounds()` offset by the origin, then the paint's adjustments.
                let dst = op.blob.bounds().with_offset(Point::new(op.x, op.y));
                self.adjust_and_map(dst, Some(&op.paint))
            }
            Command::DrawOval(op) => self.adjust_and_map(op.oval, Some(&op.paint)),
            // Tighter arc bounds?
            Command::DrawArc(op) => self.adjust_and_map(op.oval, Some(&op.paint)),
            Command::DrawRRect(op) => self.adjust_and_map(*op.rrect.rect(), Some(&op.paint)),
            Command::DrawDRRect(op) => self.adjust_and_map(*op.outer.rect(), Some(&op.paint)),
            Command::DrawPath(op) => {
                if op.path.is_inverse_fill_type() {
                    self.cull_rect
                } else {
                    self.adjust_and_map(*op.path.bounds(), Some(&op.paint))
                }
            }
            Command::DrawPoints(op) => {
                let mut dst = Rect::bounds_or_empty(&op.pts);

                // Pad the bounding box a little to make sure hairline points' bounds aren't
                // empty.
                let stroke_width = op.paint.stroke_width();
                // std::max(a, b)
                let stroke = if stroke_width < 0.01 {
                    0.01
                } else {
                    stroke_width
                };
                dst.outset((stroke / 2.0, stroke / 2.0));

                self.adjust_and_map(dst, Some(&op.paint))
            }
            Command::DrawPicture(op) => {
                let mut dst = op.picture.cull_rect();
                dst = op.matrix.map_rect(dst).0;
                self.adjust_and_map(dst, op.paint.as_ref())
            }

            // The control ops never get here.
            Command::Restore(_)
            | Command::Save(_)
            | Command::SaveLayer(_)
            | Command::SetMatrix(_)
            | Command::SetM44(_)
            | Command::Translate(_)
            | Command::Scale(_)
            | Command::Concat(_)
            | Command::Concat44(_)
            | Command::ClipPath(_)
            | Command::ClipRRect(_)
            | Command::ClipRect(_)
            | Command::ClipRegion(_)
            | Command::ClipShader(_)
            | Command::ResetClip(_) => unreachable!("control ops have no intrinsic bounds"),
        }
    }

    // Returns true if rect was meaningfully adjusted for the effects of paint,
    // false if the paint could affect the rect in unknown ways.
    // Port of: src/core/SkRecordDraw.cpp#L537-L546 (chrome/m156)
    fn adjust_for_paint(paint: Option<&Paint>, rect: &mut Rect) -> bool {
        if let Some(paint) = paint {
            if paint.can_compute_fast_bounds() {
                *rect = paint.compute_fast_bounds(rect);
                return true;
            }
            return false;
        }
        true
    }

    // Port of: src/core/SkRecordDraw.cpp#L548-L560 (chrome/m156)
    fn adjust_for_save_layer_paints(&self, rect: &mut Rect, saves_to_ignore: usize) -> bool {
        for i in (0..(self.save_stack.len() - saves_to_ignore)).rev() {
            let Some(inverse) = self.save_stack[i].ctm.invert() else {
                return false;
            };
            *rect = inverse.map_rect(*rect).0;
            if !Self::adjust_for_paint(self.save_stack[i].paint, rect) {
                return false;
            }
            *rect = self.save_stack[i].ctm.map_rect(*rect).0;
        }
        true
    }
}

/// Calculates conservative identity space bounds for each op in the record
/// (`SkRecordFillBounds`).
///
/// `bounds` and `meta` must have a slot for each op of the record.
// Port of: src/core/SkRecordDraw.cpp#L580-L592 (chrome/m156)
#[doc(alias = "SkRecordFillBounds")]
pub fn record_fill_bounds(
    cull_rect: &Rect,
    record: &Record,
    bounds: &mut [Rect],
    meta: &mut [Metadata],
) {
    let mut visitor = FillBounds::new(cull_rect, bounds, meta);
    for i in 0..record.count() {
        visitor.set_current_op(i);
        visitor.visit(record.get(i));
    }
    visitor.finish();
}
