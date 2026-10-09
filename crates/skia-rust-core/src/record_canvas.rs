// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRecordCanvas.h, src/core/SkRecordCanvas.cpp

//! `SkRecordCanvas`: a canvas that records its calls into a [`Record`].
//!
//! skia-rust: Skia's `SkRecordCanvas` subclasses `SkNoDrawCanvas` and overrides the virtual
//! hooks of `SkCanvas`. Here it is a [`Canvas`] over a no-pixels device that carries a
//! [`CanvasHooks`] object appending to the record; [`RecordCanvas`] owns that canvas and derefs
//! to it. The record is shared (`Rc<RefCell<Record>>`) between the hooks and the owner, as
//! `SkRecord*` is in Skia. The `SkDrawableList` and the draws of record types that are not ported
//! (text, vertices, ...) are not here. `onDrawImage2` is unreachable in Skia (`drawImage` goes
//! through `drawImageRect`), so `DrawImage` is never recorded.

use std::cell::{Cell, RefCell};
use std::ops::Deref;
use std::rc::Rc;

use crate::canvas::{
    Canvas, CanvasHooks, Lattice, PointMode, SaveLayerRec, SaveLayerStrategy, SrcRectConstraint,
};
use crate::clip_op::ClipOp;
use crate::glyph_run::GlyphRunList;
use crate::image::Image;
use crate::m44::M44;
use crate::matrix::Matrix;
use crate::paint::Paint;
use crate::path::Path;
use crate::picture::Picture;
use crate::point::Point;
use crate::record::Record;
use crate::records::{
    ClipOpAndAA, ClipPath, ClipRRect, ClipRect, ClipRegion, ClipShader, Concat44, DrawArc,
    DrawDRRect, DrawImageLattice, DrawImageRect, DrawOval, DrawPaint, DrawPath, DrawPicture,
    DrawPoints, DrawRRect, DrawRect, DrawRegion, DrawTextBlob, ResetClip, Restore, Save, SaveLayer,
    Scale, SetM44, Translate,
};
use crate::rect::{IRect, Rect, RoundOut};
use crate::region::Region;
use crate::rrect::RRect;
use crate::sampling_options::{FilterMode, SamplingOptions};
use crate::scalar::scalar;
use crate::shader::Shader;
use crate::text_blob::TextBlob;

/// A record shared between the recording canvas and its owner (`SkRecord*`).
pub type SharedRecord = Rc<RefCell<Record>>;

// SK_MaxS32FitsInFloat: the largest float below 2^31.
const MAX_S32_FITS_IN_FLOAT: i32 = 2_147_483_520;

// Port of: src/core/SkRecordCanvas.cpp#L88-L101 (chrome/m156)
fn safe_picture_bounds(bounds: &Rect) -> IRect {
    const SAFE_EDGE: i32 = MAX_S32_FITS_IN_FLOAT / 2 - 1;
    const SAFE_BOUNDS: IRect = IRect {
        left: -SAFE_EDGE,
        top: -SAFE_EDGE,
        right: SAFE_EDGE,
        bottom: SAFE_EDGE,
    };
    let mut pic_bounds = bounds.round_out();
    // roundOut() saturates the float edges to +/-SK_MaxS32FitsInFloat (~2billion), but this is
    // large enough that width/height calculations will overflow, leading to negative dimensions.
    match IRect::intersect(&pic_bounds, &SAFE_BOUNDS) {
        Some(r) => pic_bounds = r,
        None => pic_bounds.set_empty(),
    }
    pic_bounds
}

/// The hooks that append the calls of a canvas to a record (the overrides of `SkRecordCanvas`).
struct RecordHooks {
    record: SharedRecord,
    approx_bytes_used_by_sub_pictures: Rc<Cell<usize>>,
}

impl RecordHooks {
    // To make appending to the record a little less verbose.
    // Port of: src/core/SkRecordCanvas.cpp#L121-L124 (chrome/m156)
    fn append<T: crate::records::RecordKind>(&self, record: T) {
        self.record.borrow_mut().append(record);
    }
}

impl CanvasHooks for RecordHooks {
    // Port of: src/core/SkRecordCanvas.cpp#L342-L342 (chrome/m156)
    fn will_save(&mut self) {
        self.append(Save);
    }

    // Port of: src/core/SkRecordCanvas.cpp#L344-L359 (chrome/m156)
    fn get_save_layer_strategy(&mut self, rec: &SaveLayerRec<'_>) -> SaveLayerStrategy {
        self.append(SaveLayer {
            bounds: rec.bounds.copied(),
            paint: rec.paint.cloned(),
            backdrop: rec.backdrop.cloned(),
            save_layer_flags: rec.flags,
            backdrop_scale: 1.0, // SkCanvasPriv::GetBackdropScaleFactor(rec)
            backdrop_tile_mode: rec.backdrop_tile_mode,
            filters: Vec::new(),
        });
        SaveLayerStrategy::NoLayer
    }

    // Port of: src/core/SkRecordCanvas.cpp#L366-L366 (chrome/m156)
    fn did_restore(&mut self, total_matrix: &Matrix) {
        self.append(Restore {
            matrix: total_matrix.clone(),
        });
    }

    // Port of: src/core/SkRecordCanvas.cpp#L368-L368 (chrome/m156)
    fn did_concat44(&mut self, m: &M44) {
        self.append(Concat44 { matrix: *m });
    }

    // Port of: src/core/SkRecordCanvas.cpp#L370-L370 (chrome/m156)
    fn did_set_m44(&mut self, m: &M44) {
        self.append(SetM44 { matrix: *m });
    }

    // Port of: src/core/SkRecordCanvas.cpp#L372-L372 (chrome/m156)
    fn did_scale(&mut self, sx: scalar, sy: scalar) {
        self.append(Scale { sx, sy });
    }

    // Port of: src/core/SkRecordCanvas.cpp#L374-L376 (chrome/m156)
    fn did_translate(&mut self, dx: scalar, dy: scalar) {
        self.append(Translate { dx, dy });
    }

    // Port of: src/core/SkRecordCanvas.cpp#L378-L382 (chrome/m156)
    fn on_clip_rect(&mut self, rect: &Rect, op: ClipOp, is_aa: bool) {
        self.append(ClipRect {
            rect: *rect,
            op_aa: ClipOpAndAA::new(op, is_aa),
        });
    }

    // Port of: src/core/SkRecordCanvas.cpp#L384-L388 (chrome/m156)
    fn on_clip_rrect(&mut self, rrect: &RRect, op: ClipOp, is_aa: bool) {
        self.append(ClipRRect {
            rrect: *rrect,
            op_aa: ClipOpAndAA::new(op, is_aa),
        });
    }

    // Port of: src/core/SkRecordCanvas.cpp#L390-L394 (chrome/m156)
    fn on_clip_path(&mut self, path: &Path, op: ClipOp, is_aa: bool) {
        self.append(ClipPath {
            path: path.clone(),
            op_aa: ClipOpAndAA::new(op, is_aa),
        });
    }

    // Port of: src/core/SkRecordCanvas.cpp#L396-L399 (chrome/m156)
    fn on_clip_shader(&mut self, shader: &Shader, op: ClipOp) {
        self.append(ClipShader {
            shader: shader.clone(),
            op,
        });
    }

    // Port of: src/core/SkRecordCanvas.cpp#L401-L404 (chrome/m156)
    fn on_clip_region(&mut self, device_rgn: &Region, op: ClipOp) {
        self.append(ClipRegion {
            region: device_rgn.clone(),
            op,
        });
    }

    // Port of: src/core/SkRecordCanvas.cpp#L406-L409 (chrome/m156)
    fn on_reset_clip(&mut self) {
        self.append(ResetClip);
    }

    // Port of: src/core/SkRecordCanvas.cpp#L125-L127 (chrome/m156)
    fn on_draw_paint(&mut self, paint: &Paint) -> bool {
        self.append(DrawPaint {
            paint: paint.clone(),
        });
        true
    }

    // Port of: src/core/SkRecordCanvas.cpp#L133-L139 (chrome/m156)
    fn on_draw_points(&mut self, mode: PointMode, pts: &[Point], paint: &Paint) -> bool {
        self.append(DrawPoints {
            paint: paint.clone(),
            mode,
            pts: pts.to_vec(),
        });
        true
    }

    // Port of: src/core/SkRecordCanvas.cpp#L141-L143 (chrome/m156)
    fn on_draw_rect(&mut self, rect: &Rect, paint: &Paint) -> bool {
        self.append(DrawRect {
            paint: paint.clone(),
            rect: *rect,
        });
        true
    }

    // Port of: src/core/SkRecordCanvas.cpp#L145-L147 (chrome/m156)
    fn on_draw_region(&mut self, region: &Region, paint: &Paint) -> bool {
        self.append(DrawRegion {
            paint: paint.clone(),
            region: region.clone(),
        });
        true
    }

    // Port of: src/core/SkRecordCanvas.cpp#L149-L151 (chrome/m156)
    fn on_draw_oval(&mut self, oval: &Rect, paint: &Paint) -> bool {
        self.append(DrawOval {
            paint: paint.clone(),
            oval: *oval,
        });
        true
    }

    // Port of: src/core/SkRecordCanvas.cpp#L153-L160 (chrome/m156)
    fn on_draw_arc(
        &mut self,
        oval: &Rect,
        start_angle: scalar,
        sweep_angle: scalar,
        use_center: bool,
        paint: &Paint,
    ) -> bool {
        self.append(DrawArc {
            paint: paint.clone(),
            oval: *oval,
            start_angle,
            sweep_angle,
            use_center,
        });
        true
    }

    // Port of: src/core/SkRecordCanvas.cpp#L162-L164 (chrome/m156)
    fn on_draw_rrect(&mut self, rrect: &RRect, paint: &Paint) -> bool {
        self.append(DrawRRect {
            paint: paint.clone(),
            rrect: *rrect,
        });
        true
    }

    // Port of: src/core/SkRecordCanvas.cpp#L166-L170 (chrome/m156)
    fn on_draw_drrect(&mut self, outer: &RRect, inner: &RRect, paint: &Paint) -> bool {
        self.append(DrawDRRect {
            paint: paint.clone(),
            outer: *outer,
            inner: *inner,
        });
        true
    }

    // Port of: src/core/SkRecordCanvas.cpp#L182-L184 (chrome/m156)
    fn on_draw_path(&mut self, path: &Path, paint: &Paint) -> bool {
        self.append(DrawPath {
            paint: paint.clone(),
            path: path.clone(),
        });
        true
    }

    // Port of: src/core/SkRecordCanvas.cpp#L277-L285 (chrome/m156), onDrawGlyphRunList
    fn on_draw_glyph_run_list(&mut self, list: &GlyphRunList<'_>, paint: &Paint) -> bool {
        // (the list's own blob is not kept, so one is made from its runs)
        if let Some(blob) = list.make_blob() {
            let origin = list.origin();
            self.on_draw_text_blob(&blob, origin.x, origin.y, paint);
        }
        true
    }

    // Port of: src/core/SkRecordCanvas.cpp#L266-L271 (chrome/m156), onDrawTextBlob
    fn on_draw_text_blob(&mut self, blob: &TextBlob, x: scalar, y: scalar, paint: &Paint) -> bool {
        self.append(DrawTextBlob {
            paint: paint.clone(),
            blob: blob.clone(),
            x,
            y,
        });
        true
    }

    // Port of: src/core/SkRecordCanvas.cpp#L235-L243 (chrome/m156)
    fn on_draw_image_rect2(
        &mut self,
        image: &Image,
        src: &Rect,
        dst: &Rect,
        sampling: &SamplingOptions,
        paint: Option<&Paint>,
        constraint: SrcRectConstraint,
    ) -> bool {
        self.append(DrawImageRect {
            paint: paint.cloned(),
            image: image.clone(),
            src: *src,
            dst: *dst,
            sampling: *sampling,
            constraint,
        });
        true
    }

    // Port of: src/core/SkRecordCanvas.cpp#L245-L265 (chrome/m156)
    fn on_draw_image_lattice2(
        &mut self,
        image: &Image,
        lattice: &Lattice<'_>,
        dst: &Rect,
        filter: FilterMode,
        paint: Option<&Paint>,
    ) -> bool {
        let flag_count = if lattice.rect_types.is_some() {
            (lattice.x_divs.len() + 1) * (lattice.y_divs.len() + 1)
        } else {
            0
        };
        // `this->copy(lattice.fRectTypes, flagCount)` and `copy(lattice.fColors, flagCount)`.
        let flags = lattice
            .rect_types
            .map_or_else(Vec::new, |f| f[..flag_count].to_vec());
        let colors = lattice
            .colors
            .map_or_else(Vec::new, |c| c[..flag_count].to_vec());
        self.append(DrawImageLattice {
            paint: paint.cloned(),
            image: image.clone(),
            x_divs: lattice.x_divs.to_vec(),
            y_divs: lattice.y_divs.to_vec(),
            flag_count,
            flags,
            colors,
            src: lattice.bounds.expect("the lattice has bounds"), // SkASSERT(lattice.fBounds)
            dst: *dst,
            filter,
        });
        true
    }

    // Port of: src/core/SkRecordCanvas.cpp#L255-L261 (chrome/m156)
    fn on_draw_picture(
        &mut self,
        picture: &Picture,
        matrix: Option<&Matrix>,
        paint: Option<&Paint>,
    ) -> bool {
        self.approx_bytes_used_by_sub_pictures
            .set(self.approx_bytes_used_by_sub_pictures.get() + picture.approximate_bytes_used());
        self.append(DrawPicture {
            paint: paint.cloned(),
            picture: picture.clone(),
            matrix: matrix.cloned().unwrap_or_else(Matrix::new_identity),
        });
        true
    }
}

/// A canvas that records its calls into a [`Record`] (`SkRecordCanvas`).
// Port of: src/core/SkRecordCanvas.h#L80-L195 (chrome/m156)
#[doc(alias = "SkRecordCanvas")]
#[derive(Debug)]
pub struct RecordCanvas {
    canvas: Canvas,
    approx_bytes_used_by_sub_pictures: Rc<Cell<usize>>,
}

impl Deref for RecordCanvas {
    type Target = Canvas;

    fn deref(&self) -> &Canvas {
        &self.canvas
    }
}

impl RecordCanvas {
    fn make_canvas(
        record: Option<&SharedRecord>,
        bounds: &IRect,
        approx_bytes: &Rc<Cell<usize>>,
    ) -> Canvas {
        let canvas = Canvas::new_no_pixels_irect(bounds, None);
        // With no record, calls are not recorded (a record that nobody reads stands in).
        let record = record.map_or_else(|| Rc::new(RefCell::new(Record::new())), Rc::clone);
        canvas.set_hooks(Some(Box::new(RecordHooks {
            record,
            approx_bytes_used_by_sub_pictures: Rc::clone(approx_bytes),
        })));
        canvas
    }

    /// A recording canvas of `width` by `height` appending to `record`
    /// (`SkRecordCanvas(SkRecord*, int, int)`).
    // Port of: src/core/SkRecordCanvas.cpp#L103-L108 (chrome/m156)
    #[must_use]
    pub fn new(record: &SharedRecord, width: i32, height: i32) -> RecordCanvas {
        debug_assert!(width >= 0 && height >= 0);
        let approx_bytes = Rc::new(Cell::new(0));
        RecordCanvas {
            canvas: Self::make_canvas(Some(record), &IRect::from_wh(width, height), &approx_bytes),
            approx_bytes_used_by_sub_pictures: approx_bytes,
        }
    }

    /// A recording canvas covering `bounds` appending to `record`
    /// (`SkRecordCanvas(SkRecord*, const SkRect&)`).
    // Port of: src/core/SkRecordCanvas.cpp#L110-L115 (chrome/m156)
    #[must_use]
    pub fn new_with_bounds(record: Option<&SharedRecord>, bounds: &Rect) -> RecordCanvas {
        let approx_bytes = Rc::new(Cell::new(0));
        let canvas = Self::make_canvas(record, &safe_picture_bounds(bounds), &approx_bytes);
        debug_assert!(canvas.image_info().width() >= 0 && canvas.image_info().height() >= 0);
        RecordCanvas {
            canvas,
            approx_bytes_used_by_sub_pictures: approx_bytes,
        }
    }

    /// Starts recording into `record`, with a canvas covering `bounds` (`reset`).
    // Port of: src/core/SkRecordCanvas.cpp#L117-L122 (chrome/m156)
    pub fn reset(&mut self, record: &SharedRecord, bounds: &Rect) {
        self.forget_record();
        // The canvas is reset (restoring to its first save, which records the restores into the
        // new record) and told to append to the new record.
        let hooks = RecordHooks {
            record: Rc::clone(record),
            approx_bytes_used_by_sub_pictures: Rc::clone(&self.approx_bytes_used_by_sub_pictures),
        };
        self.canvas.set_hooks(Some(Box::new(hooks)));
        self.canvas
            .reset_for_next_picture(&safe_picture_bounds(bounds));
        debug_assert!(
            self.canvas.image_info().width() >= 0 && self.canvas.image_info().height() >= 0
        );
    }

    /// The approximate bytes used by the pictures drawn into this canvas
    /// (`approxBytesUsedBySubPictures`).
    #[doc(alias = "approxBytesUsedBySubPictures")]
    #[must_use]
    pub fn approx_bytes_used_by_sub_pictures(&self) -> usize {
        self.approx_bytes_used_by_sub_pictures.get()
    }

    /// Makes the canvas forget entirely about its record; calls to the canvas then record
    /// nothing (`forgetRecord`).
    // Port of: src/core/SkRecordCanvas.cpp#L117-L122 (chrome/m156)
    #[doc(alias = "forgetRecord")]
    pub fn forget_record(&mut self) {
        self.approx_bytes_used_by_sub_pictures.set(0);
        self.canvas.set_hooks(Some(Box::new(RecordHooks {
            record: Rc::new(RefCell::new(Record::new())),
            approx_bytes_used_by_sub_pictures: Rc::clone(&self.approx_bytes_used_by_sub_pictures),
        })));
    }
}
