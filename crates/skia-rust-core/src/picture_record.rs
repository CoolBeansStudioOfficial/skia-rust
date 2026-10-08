// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkPictureRecord.h, src/core/SkPictureRecord.cpp (the subset below)

//! `SkPictureRecord`: the encoder that turns a picture's commands into its op stream.
//!
//! Skia's `SkPictureRecord` is a canvas: [`Picture::serialize`](crate::picture::Picture::serialize)
//! plays the picture into it, and each canvas call is written as an op (op code, size and
//! arguments). Paints and paths go to side tables, which the op stream indexes.
//!
//! skia-rust: the hooks of a no-pixels canvas stand for the overrides of `SkPictureRecord`, and
//! [`backport`] plays the picture into such a canvas. The ops that are not encoded yet (clip
//! shaders and regions, region draws, text blobs, nested pictures, images, lattices) make the
//! picture unserializable: [`backport`] returns `None` for them.

use std::cell::RefCell;
use std::rc::Rc;

use crate::blend_mode::BlendMode;
use crate::canvas::{
    Canvas, CanvasHooks, Lattice, PointMode, SaveLayerRec, SaveLayerStrategy, SrcRectConstraint,
};
use crate::clip_op::ClipOp;
use crate::color::Color;
use crate::image::Image;
use crate::m44::M44;
use crate::matrix::Matrix;
use crate::paint::Paint;
use crate::path::Path;
use crate::picture::Picture;
use crate::picture_data::PictureData;
use crate::picture_flat::{MASK_24, clip_params_pack, draw_type, pack_8_24, save_layer_rec};
use crate::point::Point;
use crate::rect::{IRect, Rect, RoundOut};
use crate::region::Region;
use crate::rrect::RRect;
use crate::rsxform::RSXform;
use crate::sampling_options::{FilterMode, SamplingOptions};
use crate::scalar::scalar;
use crate::shader::Shader;
use crate::text_blob::TextBlob;
use crate::tile_mode::TileMode;
use crate::utils::patch_utils;
use crate::vertices::Vertices;
use crate::write_buffer::Writer32;

/// The state of an encoding: the op stream, and the paints and paths that its ops index
/// (`SkPictureRecord`'s `fWriter`, `fPaints` and `fPaths`).
// Port of: src/core/SkPictureRecord.h#L273-L292 (chrome/m156), the members used so far
#[derive(Debug, Default)]
struct PictureRecordState {
    /// `fWriter`: the op stream.
    writer: Writer32,
    /// `fRestoreOffsetStack`: for each save level, the offset of the last restore placeholder
    /// of that level, or the negated offset of the save op itself.
    restore_offset_stack: Vec<i32>,
    /// `fPaints`: the paints, in the order of their 1-based indices.
    paints: Vec<Paint>,
    /// `fPaths`: the distinct paths, in the order of their 1-based indices.
    paths: Vec<Path>,
    /// `fTextBlobs`: the distinct text blobs, in the order of their 1-based indices.
    text_blobs: Vec<TextBlob>,
    /// Set when the picture has a command that is not encoded yet.
    unsupported: bool,
}

/// The word of an unsigned count or index, as `SkToU32` gives it.
fn word(value: usize) -> u32 {
    u32::try_from(value).expect("SkToU32")
}

/// The bits of an `int` as the unsigned word that holds them.
fn word_of_int(value: i32) -> u32 {
    u32::from_ne_bytes(value.to_ne_bytes())
}

impl PictureRecordState {
    /// Writes the header of an op with `size` bytes (`addDraw`): the op word, with a size word
    /// after it when the size does not fit in the op word. The size counts the header.
    // Port of: src/core/SkPictureRecord.h#L141-L158 (chrome/m156), addDraw
    fn add_draw(&mut self, op: u8, size: usize) {
        let mask = usize::try_from(MASK_24).expect("24 bits fit in usize");
        if size & !mask != 0 || size == mask {
            self.writer.write_u32(pack_8_24(op, MASK_24));
            // Skia adds 1 to the size here, not the 4 bytes of the size word; the reader does
            // not use the size to find the next op, so this is kept as it is.
            self.writer.write_u32(word(size + 1));
        } else {
            self.writer.write_u32(pack_8_24(op, word(size)));
        }
    }

    /// Writes an `int` (`addInt`).
    fn add_int(&mut self, value: i32) {
        self.writer.write_u32(word_of_int(value));
    }

    /// Writes an unsigned word (a count, index, flag or enum).
    fn add_word(&mut self, value: usize) {
        self.writer.write_u32(word(value));
    }

    /// Writes a scalar (`addScalar`).
    fn add_scalar(&mut self, value: scalar) {
        self.writer.write_scalar(value);
    }

    /// Writes a rectangle (`addRect`).
    fn add_rect(&mut self, rect: &Rect) {
        self.writer.write_scalar(rect.left);
        self.writer.write_scalar(rect.top);
        self.writer.write_scalar(rect.right);
        self.writer.write_scalar(rect.bottom);
    }

    /// Writes a round rectangle (`addRRect`): its bounds and radii.
    fn add_rrect(&mut self, rrect: &RRect) {
        let mut bytes = Vec::new();
        rrect.write_to_memory(&mut bytes);
        self.writer
            .reserve(RRect::SIZE_IN_MEMORY)
            .copy_from_slice(&bytes);
    }

    /// Writes points, as they are in memory (`addPoints`).
    fn add_points(&mut self, pts: &[Point]) {
        for pt in pts {
            self.writer.write_scalar(pt.x);
            self.writer.write_scalar(pt.y);
        }
    }

    /// Writes a 4x4 matrix in column-major order (`SkMatrixPriv::M44ColMajor`).
    fn add_m44(&mut self, m: &M44) {
        let mut values = [0.0; 16];
        m.get_col_major(&mut values);
        for value in values {
            self.writer.write_scalar(value);
        }
    }

    /// Writes a paint's index: the paint is appended to the table, and its 1-based index is
    /// written, or zero for no paint (`addPaintPtr`).
    fn add_paint_ptr(&mut self, paint: Option<&Paint>) {
        if let Some(paint) = paint {
            self.paints.push(paint.clone());
            self.add_word(self.paints.len());
        } else {
            self.add_word(0);
        }
    }

    /// Writes a paint (`addPaint`).
    fn add_paint(&mut self, paint: &Paint) {
        self.add_paint_ptr(Some(paint));
    }

    /// The 1-based index of `path` in the table, adding it if it is new (`addPathToHeap`).
    fn add_path_to_heap(&mut self, path: &Path) -> usize {
        if let Some(position) = self.paths.iter().position(|known| known == path) {
            return position + 1;
        }
        self.paths.push(path.clone());
        self.paths.len()
    }

    /// Writes a path's 1-based index (`addPath`).
    fn add_path(&mut self, path: &Path) {
        let index = self.add_path_to_heap(path);
        self.add_word(index);
    }

    /// Writes a text blob's 1-based index, adding the blob if it is new (`addTextBlob`). Blobs
    /// are the same when they have the same unique id (`equals`).
    // Port of: src/core/SkPictureRecord.cpp#L958-L961 (chrome/m156), addTextBlob, with
    // find_or_append from src/core/SkPictureRecord.cpp#L847-L857 (chrome/m156)
    fn add_text_blob(&mut self, blob: &TextBlob) {
        let position = self
            .text_blobs
            .iter()
            .position(|known| known.unique_id() == blob.unique_id())
            .unwrap_or_else(|| {
                self.text_blobs.push(blob.clone());
                self.text_blobs.len() - 1
            });
        // follow the convention of recording a 1-based index
        self.add_int(i32::try_from(position + 1).expect("SkToS32"));
    }

    /// Writes an offset placeholder that the restore will fill in, linked to the previous
    /// placeholder of this save level (`recordRestoreOffsetPlaceholder`). Nothing is written at
    /// the top level, where no save is open.
    // Port of: src/core/SkPictureRecord.cpp#L314-L330 (chrome/m156)
    fn record_restore_offset_placeholder(&mut self) {
        let Some(&prev_offset) = self.restore_offset_stack.last() else {
            return;
        };
        let offset = self.writer.bytes_written();
        self.add_int(prev_offset);
        // The placeholder is the offset of the word just written, so the next one links to it.
        *self
            .restore_offset_stack
            .last_mut()
            .expect("checked non-empty") = i32::try_from(offset).expect("SkToS32");
    }

    /// Makes every placeholder of the current save level hold `restore_offset`
    /// (`fillRestoreOffsetPlaceholdersForCurrentStackLevel`).
    // Port of: src/core/SkPictureRecord.cpp#L283-L300 (chrome/m156)
    fn fill_restore_offset_placeholders_for_current_stack_level(&mut self, restore_offset: u32) {
        let mut offset = *self
            .restore_offset_stack
            .last()
            .expect("the initial save is on the stack");
        while offset > 0 {
            let at = usize::try_from(offset).expect("positive offset");
            let peek = self.writer.read32_at(at);
            self.writer.overwrite32(at, restore_offset);
            offset = i32::from_ne_bytes(peek.to_ne_bytes());
        }
    }

    // Port of: src/core/SkPictureRecord.cpp#L66-L72 (chrome/m156), recordSave
    fn record_save(&mut self) {
        self.add_draw(draw_type::SAVE, 4);
    }

    // Port of: src/core/SkPictureRecord.cpp#L214-L221 (chrome/m156), recordRestore
    fn record_restore(&mut self, fill_in_skips: bool) {
        if fill_in_skips {
            let offset = word(self.writer.bytes_written());
            self.fill_restore_offset_placeholders_for_current_stack_level(offset);
        }
        self.add_draw(draw_type::RESTORE, 4);
    }

    // Port of: src/core/SkPictureRecord.cpp#L109-L178 (chrome/m156), recordSaveLayer, for the
    // layers without a backdrop filter (a backdrop is a paint with an image filter, not ported)
    fn record_save_layer(&mut self, rec: &SaveLayerRec<'_>) {
        if rec.backdrop.is_some() {
            self.unsupported = true;
            return;
        }
        // op + flatFlags
        let mut size = 2 * 4;
        let mut flat_flags = 0;
        if rec.bounds.is_some() {
            flat_flags |= save_layer_rec::HAS_BOUNDS;
            size += 16;
        }
        if rec.paint.is_some() {
            flat_flags |= save_layer_rec::HAS_PAINT;
            size += 4;
        }
        if rec.flags.bits() != 0 {
            flat_flags |= save_layer_rec::HAS_FLAGS;
            size += 4;
        }
        if rec.backdrop_tile_mode != TileMode::Clamp {
            flat_flags |= save_layer_rec::HAS_BACKDROP_TILEMODE;
            size += 4;
        }
        self.add_draw(draw_type::SAVE_LAYER_SAVELAYERREC, size);
        self.add_word(flat_flags as usize);
        if let Some(bounds) = rec.bounds {
            self.add_rect(bounds);
        }
        if flat_flags & save_layer_rec::HAS_PAINT != 0 {
            self.add_paint_ptr(rec.paint);
        }
        if flat_flags & save_layer_rec::HAS_FLAGS != 0 {
            self.add_word(rec.flags.bits() as usize);
        }
        if flat_flags & save_layer_rec::HAS_BACKDROP_TILEMODE != 0 {
            self.add_int(rec.backdrop_tile_mode as i32);
        }
    }

    // Port of: src/core/SkPictureRecord.cpp#L337-L352 (chrome/m156), recordClipRect
    fn record_clip_rect(&mut self, rect: &Rect, op: ClipOp, do_aa: bool) {
        // op + rect + clip params, and the restore offset when a save is open
        let mut size = 4 + 16 + 4;
        if !self.restore_offset_stack.is_empty() {
            size += 4;
        }
        self.add_draw(draw_type::CLIP_RECT, size);
        self.add_rect(rect);
        self.add_word(clip_params_pack(op as u32, do_aa) as usize);
        self.record_restore_offset_placeholder();
    }

    // Port of: src/core/SkPictureRecord.cpp#L359-L373 (chrome/m156), recordClipRRect
    fn record_clip_rrect(&mut self, rrect: &RRect, op: ClipOp, do_aa: bool) {
        let mut size = 4 + RRect::SIZE_IN_MEMORY + 4;
        if !self.restore_offset_stack.is_empty() {
            size += 4;
        }
        self.add_draw(draw_type::CLIP_RRECT, size);
        self.add_rrect(rrect);
        self.add_word(clip_params_pack(op as u32, do_aa) as usize);
        self.record_restore_offset_placeholder();
    }

    // Port of: src/core/SkPictureRecord.cpp#L381-L395 (chrome/m156), recordClipPath
    fn record_clip_path(&mut self, path_id: usize, op: ClipOp, do_aa: bool) {
        let mut size = 3 * 4;
        if !self.restore_offset_stack.is_empty() {
            size += 4;
        }
        self.add_draw(draw_type::CLIP_PATH, size);
        self.add_word(path_id);
        self.add_word(clip_params_pack(op as u32, do_aa) as usize);
        self.record_restore_offset_placeholder();
    }

    // Port of: src/core/SkPictureRecord.cpp#L501-L508 (chrome/m156), onDrawRect (the op with a
    // rect and a paint, of `size` bytes after the paint index)
    fn record_rect_op(&mut self, op: u8, rect: &Rect, paint: &Paint) {
        self.add_draw(op, 2 * 4 + 16);
        self.add_paint(paint);
        self.add_rect(rect);
    }

    // Port of: src/core/SkPictureRecord.cpp#L520-L527 (chrome/m156), onDrawRRect (the op with
    // rrects after the paint index, `count` of them)
    fn record_rrect_op(&mut self, op: u8, rrects: &[&RRect], paint: &Paint) {
        let size = 2 * 4 + RRect::SIZE_IN_MEMORY * rrects.len();
        self.add_draw(op, size);
        self.add_paint(paint);
        for rrect in rrects {
            self.add_rrect(rrect);
        }
    }

    // Port of: src/core/SkPictureRecord.cpp#L245-L254 (chrome/m156), the matrix ops:
    // didConcat44 writes the column-major matrix after the op word.
    fn record_concat44(&mut self, m: &M44) {
        self.add_draw(draw_type::CONCAT44, 4 + 16 * 4);
        self.add_m44(m);
    }

    // Port of: src/core/SkPictureRecord.cpp#L256-L264 (chrome/m156), didSetM44
    fn record_set_m44(&mut self, m: &M44) {
        self.add_draw(draw_type::SET_M44, 4 + 16 * 4);
        self.add_m44(m);
    }
}

/// The hooks that write the calls of a canvas into a [`PictureRecordState`] (the overrides of
/// `SkPictureRecord`).
struct PictureRecordHooks {
    state: Rc<RefCell<PictureRecordState>>,
}

impl PictureRecordHooks {
    fn with<R>(&self, f: impl FnOnce(&mut PictureRecordState) -> R) -> R {
        f(&mut self.state.borrow_mut())
    }

    /// Marks the picture as not encodable: its op is not written yet.
    fn unsupported(&self) -> bool {
        self.with(|state| state.unsupported = true);
        true
    }
}

impl CanvasHooks for PictureRecordHooks {
    // Port of: src/core/SkPictureRecord.cpp#L57-L64 (chrome/m156), willSave: the offset of the
    // save op is pushed (negated, so it is not a placeholder), then the op is recorded.
    fn will_save(&mut self) {
        self.with(|state| {
            let offset = i32::try_from(state.writer.bytes_written()).expect("SkToS32");
            state.restore_offset_stack.push(-offset);
            state.record_save();
        });
    }

    // Port of: src/core/SkPictureRecord.cpp#L74-L87 (chrome/m156), getSaveLayerStrategy: the
    // layer is never allocated while recording.
    fn get_save_layer_strategy(&mut self, rec: &SaveLayerRec<'_>) -> SaveLayerStrategy {
        self.with(|state| {
            let offset = i32::try_from(state.writer.bytes_written()).expect("SkToS32");
            state.restore_offset_stack.push(-offset);
            state.record_save_layer(rec);
        });
        SaveLayerStrategy::NoLayer
    }

    // Port of: src/core/SkPictureRecord.cpp#L197-L212 (chrome/m156), willRestore: an
    // unmatched restore is ignored.
    fn will_restore(&mut self) {
        self.with(|state| {
            if state.restore_offset_stack.is_empty() {
                return;
            }
            state.record_restore(true);
            state.restore_offset_stack.pop();
        });
    }

    // Port of: src/core/SkPictureRecord.cpp#L245-L254 (chrome/m156), didConcat44
    fn did_concat44(&mut self, m: &M44) {
        self.with(|state| state.record_concat44(m));
    }

    // Port of: src/core/SkPictureRecord.cpp#L256-L264 (chrome/m156), didSetM44
    fn did_set_m44(&mut self, m: &M44) {
        self.with(|state| state.record_set_m44(m));
    }

    // Port of: src/core/SkPictureRecord.cpp#L266-L268 (chrome/m156), didScale: a scale is
    // recorded as a concat of the scale matrix.
    fn did_scale(&mut self, sx: scalar, sy: scalar) {
        self.did_concat44(&M44::scale(sx, sy, 1.0));
    }

    // Port of: src/core/SkPictureRecord.cpp#L270-L272 (chrome/m156), didTranslate
    fn did_translate(&mut self, dx: scalar, dy: scalar) {
        self.did_concat44(&M44::translate(dx, dy, 0.0));
    }

    // Port of: src/core/SkPictureRecord.cpp#L332-L335 (chrome/m156), onClipRect
    fn on_clip_rect(&mut self, rect: &Rect, op: ClipOp, is_aa: bool) {
        self.with(|state| state.record_clip_rect(rect, op, is_aa));
    }

    // Port of: src/core/SkPictureRecord.cpp#L354-L357 (chrome/m156), onClipRRect
    fn on_clip_rrect(&mut self, rrect: &RRect, op: ClipOp, is_aa: bool) {
        self.with(|state| state.record_clip_rrect(rrect, op, is_aa));
    }

    // Port of: src/core/SkPictureRecord.cpp#L375-L379 (chrome/m156), onClipPath
    fn on_clip_path(&mut self, path: &Path, op: ClipOp, is_aa: bool) {
        self.with(|state| {
            let path_id = state.add_path_to_heap(path);
            state.record_clip_path(path_id, op, is_aa);
        });
    }

    // Clip shaders are in a paint (a shader is not ported).
    fn on_clip_shader(&mut self, _shader: &Shader, _op: ClipOp) {
        self.unsupported();
    }

    // Clip regions are not ported.
    fn on_clip_region(&mut self, _device_rgn: &Region, _op: ClipOp) {
        self.unsupported();
    }

    // Port of: src/core/SkPictureRecord.cpp#L435-L446 (chrome/m156), onResetClip
    fn on_reset_clip(&mut self) {
        self.with(|state| {
            if !state.restore_offset_stack.is_empty() {
                // The earlier clips may not jump to a restore, as they could hide this reset.
                state.fill_restore_offset_placeholders_for_current_stack_level(0);
            }
            state.add_draw(draw_type::RESET_CLIP, 4);
        });
    }

    // Port of: src/core/SkPictureRecord.cpp#L448-L454 (chrome/m156), onDrawPaint
    fn on_draw_paint(&mut self, paint: &Paint) -> bool {
        self.with(|state| {
            state.add_draw(draw_type::DRAW_PAINT, 2 * 4);
            state.add_paint(paint);
        });
        true
    }

    // Port of: src/core/SkPictureRecord.cpp#L465-L476 (chrome/m156), onDrawPoints
    fn on_draw_points(&mut self, mode: PointMode, pts: &[Point], paint: &Paint) -> bool {
        self.with(|state| {
            state.add_draw(draw_type::DRAW_POINTS, 4 * 4 + pts.len() * 8);
            state.add_paint(paint);
            state.add_word(mode as usize);
            state.add_word(pts.len());
            state.add_points(pts);
        });
        true
    }

    // Port of: src/core/SkPictureRecord.cpp#L478-L485 (chrome/m156), onDrawOval
    fn on_draw_oval(&mut self, oval: &Rect, paint: &Paint) -> bool {
        self.with(|state| state.record_rect_op(draw_type::DRAW_OVAL, oval, paint));
        true
    }

    // Port of: src/core/SkPictureRecord.cpp#L487-L499 (chrome/m156), onDrawArc
    fn on_draw_arc(
        &mut self,
        oval: &Rect,
        start_angle: scalar,
        sweep_angle: scalar,
        use_center: bool,
        paint: &Paint,
    ) -> bool {
        self.with(|state| {
            // op + paint index + rect + start + sweep + bool (as int)
            state.add_draw(draw_type::DRAW_ARC, 2 * 4 + 16 + 4 + 4 + 4);
            state.add_paint(paint);
            state.add_rect(oval);
            state.add_scalar(start_angle);
            state.add_scalar(sweep_angle);
            state.add_int(i32::from(use_center));
        });
        true
    }

    // Port of: src/core/SkPictureRecord.cpp#L501-L508 (chrome/m156), onDrawRect
    fn on_draw_rect(&mut self, rect: &Rect, paint: &Paint) -> bool {
        self.with(|state| state.record_rect_op(draw_type::DRAW_RECT, rect, paint));
        true
    }

    // Regions are not ported.
    fn on_draw_region(&mut self, _region: &Region, _paint: &Paint) -> bool {
        self.unsupported()
    }

    // Port of: src/core/SkPictureRecord.cpp#L520-L527 (chrome/m156), onDrawRRect
    fn on_draw_rrect(&mut self, rrect: &RRect, paint: &Paint) -> bool {
        self.with(|state| state.record_rrect_op(draw_type::DRAW_RRECT, &[rrect], paint));
        true
    }

    // Port of: src/core/SkPictureRecord.cpp#L529-L538 (chrome/m156), onDrawDRRect
    fn on_draw_drrect(&mut self, outer: &RRect, inner: &RRect, paint: &Paint) -> bool {
        self.with(|state| {
            state.record_rrect_op(draw_type::DRAW_DRRECT, &[outer, inner], paint);
        });
        true
    }

    // Port of: src/core/SkPictureRecord.cpp#L540-L547 (chrome/m156), onDrawPath
    fn on_draw_path(&mut self, path: &Path, paint: &Paint) -> bool {
        self.with(|state| {
            // op + paint index + path index
            state.add_draw(draw_type::DRAW_PATH, 3 * 4);
            state.add_paint(paint);
            state.add_path(path);
        });
        true
    }

    // Port of: src/core/SkPictureRecord.cpp#L594-L607 (chrome/m156), onDrawTextBlob
    fn on_draw_text_blob(&mut self, blob: &TextBlob, x: scalar, y: scalar, paint: &Paint) -> bool {
        self.with(|state| {
            // op + paint index + blob index + x/y
            state.add_draw(draw_type::DRAW_TEXT_BLOB, 3 * 4 + 2 * 4);
            state.add_paint(paint);
            state.add_text_blob(blob);
            state.add_scalar(x);
            state.add_scalar(y);
        });
        true
    }

    // Vertices are not encoded yet.
    fn on_draw_vertices_object(
        &mut self,
        _vertices: &Vertices,
        _mode: BlendMode,
        _paint: &Paint,
    ) -> bool {
        self.unsupported()
    }

    // Patches are not encoded yet.
    fn on_draw_patch(
        &mut self,
        _cubics: &[Point; patch_utils::NUM_CTRL_PTS],
        _colors: Option<&[Color; patch_utils::NUM_CORNERS]>,
        _tex_coords: Option<&[Point; patch_utils::NUM_CORNERS]>,
        _mode: BlendMode,
        _paint: &Paint,
    ) -> bool {
        self.unsupported()
    }

    // Atlases are not encoded yet.
    fn on_draw_atlas2(
        &mut self,
        _atlas_shader: Option<&Shader>,
        _xform: &[RSXform],
        _tex: &[Rect],
        _colors: &[Color],
        _mode: BlendMode,
        _cull: Option<&Rect>,
        _paint: Option<&Paint>,
    ) -> bool {
        self.unsupported()
    }

    // Nested pictures are not encoded yet (their section is not ported).
    fn on_draw_picture(
        &mut self,
        _picture: &Picture,
        _matrix: Option<&Matrix>,
        _paint: Option<&Paint>,
    ) -> bool {
        self.unsupported()
    }

    // Images are not encoded yet (the image section is not ported).
    fn on_draw_image_rect2(
        &mut self,
        _image: &Image,
        _src: &Rect,
        _dst: &Rect,
        _sampling: &SamplingOptions,
        _paint: Option<&Paint>,
        _constraint: SrcRectConstraint,
    ) -> bool {
        self.unsupported()
    }

    // Lattices are not encoded yet.
    fn on_draw_image_lattice2(
        &mut self,
        _image: &Image,
        _lattice: &Lattice<'_>,
        _dst: &Rect,
        _filter: FilterMode,
        _paint: Option<&Paint>,
    ) -> bool {
        self.unsupported()
    }
}

/// Plays `picture` into a canvas whose hooks encode the calls, and returns the data of the
/// picture: its op stream, paints and paths (`SkPicture::backport`, with the `SkPictureRecord`
/// of the picture's cull rect). `None` if the picture has a command that is not encoded yet.
// Port of: src/core/SkPicture.cpp#L254-L261 (chrome/m156), backport
#[must_use]
pub(crate) fn backport(picture: &Picture) -> Option<PictureData> {
    let state = Rc::new(RefCell::new(PictureRecordState::default()));
    // SkPictureRecord rec(info.fCullRect.roundOut(), 0)
    let dimensions: IRect = picture.cull_rect().round_out();
    let canvas = Canvas::new_no_pixels_irect(&dimensions, None);
    canvas.set_hooks(Some(Box::new(PictureRecordHooks {
        state: Rc::clone(&state),
    })));
    // rec.beginRecording(): the initial save, balanced by endRecording below.
    let initial_save_count = canvas.save();
    picture.playback(&canvas);
    // rec.endRecording()
    canvas.restore_to_count(initial_save_count);
    drop(canvas);

    let state = Rc::try_unwrap(state)
        .expect("the canvas is dropped")
        .into_inner();
    if state.unsupported {
        return None;
    }
    Some(PictureData::new(
        state.writer.into_bytes(),
        state.paints,
        state.paths,
        state.text_blobs,
    ))
}
