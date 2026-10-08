// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkPicturePlayback.h, src/core/SkPicturePlayback.cpp (the subset
// below), and src/core/SkPicture.cpp (Forwardport)

//! `SkPicturePlayback`: reads the op stream of a picture and plays it into a canvas.
//!
//! Reading a picture back is "forward porting" it: the ops are played into a recorder, which
//! makes a new picture from them (`SkPicture::Forwardport`).
//!
//! skia-rust: the ops that are not read yet (clip shaders and regions, region draws, text blobs,
//! nested pictures, images, lattices, and the obsolete save layer fields) make the stream
//! unreadable, and so does any invalid op. Skia would return the part it read; here the
//! picture is `None`.

use crate::canvas::{Canvas, PointMode, SaveLayerFlags, SaveLayerRec};
use crate::clip_op::ClipOp;
use crate::m44::M44;
use crate::paint::Paint;
use crate::path::Path;
use crate::picture::Picture;
use crate::picture_data::{PictInfo, PictureData};
use crate::picture_flat::{MASK_24, clip_params_unpack_do_aa, draw_type, save_layer_rec};
use crate::picture_priv::{
    VERSION_BACKDROP_SCALE_FACTOR, VERSION_MULTIPLE_FILTERS_ON_SAVE_LAYER,
    VERSION_SAVE_LAYER_BACKDROP_TILE_MODE,
};
use crate::picture_recorder::PictureRecorder;
use crate::point::Point;
use crate::read_buffer::ReadBuffer;
use crate::text_blob::TextBlob;
use crate::tile_mode::TileMode;

/// `SkPicturePlayback::draw` for the ops of `data`, which were written at `version`. Returns
/// false if an op is invalid, or not read yet. The canvas is restored to its save count at the
/// start afterwards.
// Port of: src/core/SkPicturePlayback.cpp#L61-L103 (chrome/m156), draw
fn play(data: &PictureData, version: u32, canvas: &Canvas) -> bool {
    let Some(op_data) = data.op_data() else {
        return false;
    };
    let mut reader = ReadBuffer::new(op_data);
    reader.set_version(version);

    // Record this, so we can concat with it if we encounter a setMatrix().
    let initial_matrix = canvas.local_to_device();
    // SkAutoCanvasRestore acr(canvas, false)
    let save_count = canvas.save_count();

    let mut ok = true;
    while reader.available() > 0 && reader.is_valid() {
        let bits = reader.read_uint();
        let op = u8::try_from(bits >> 24).expect("8 bits");
        let mut size = bits & MASK_24;
        if size == MASK_24 {
            size = reader.read_uint();
        }
        if !reader.validate(size > 0 && op > 0 && op <= draw_type::LAST_DRAWTYPE_ENUM) {
            ok = false;
            break;
        }
        if !handle_op(&mut reader, op, canvas, &initial_matrix, data, version) {
            ok = false;
            break;
        }
    }

    canvas.restore_to_count(save_count);
    ok && reader.is_valid()
}

/// The paint an op uses: its 1-based index in the paints, which must be there (`requiredPaint`).
// Port of: src/core/SkPictureData.cpp#L617-L624 (chrome/m156), requiredPaint
fn required_paint<'a>(reader: &mut ReadBuffer<'_>, data: &'a PictureData) -> Option<&'a Paint> {
    let index = reader.read_int();
    let index = usize::try_from(index).ok().filter(|&i| i >= 1);
    let paint = index.and_then(|i| data.paints().get(i - 1));
    if reader.validate(paint.is_some()) {
        paint
    } else {
        None
    }
}

/// The path an op uses: its 1-based index in the paths, which must be there (`getPath`).
// Port of: src/core/SkPictureData.h#L121-L125 (chrome/m156), getPath
fn get_path<'a>(reader: &mut ReadBuffer<'_>, data: &'a PictureData) -> Option<&'a Path> {
    let index = reader.read_int();
    let index = usize::try_from(index).ok().filter(|&i| i >= 1);
    let path = index.and_then(|i| data.paths().get(i - 1));
    if reader.validate(path.is_some()) {
        path
    } else {
        None
    }
}

/// The text blob an op uses: its 1-based index in the blobs, which must be there
/// (`getTextBlob`).
// Port of: src/core/SkPictureData.h#L142-L144 (chrome/m156), getTextBlob
fn get_text_blob<'a>(reader: &mut ReadBuffer<'_>, data: &'a PictureData) -> Option<&'a TextBlob> {
    let index = reader.read_int();
    let index = usize::try_from(index).ok().filter(|&i| i >= 1);
    let blob = index.and_then(|i| data.text_blobs().get(i - 1));
    if reader.validate(blob.is_some()) {
        blob
    } else {
        None
    }
}

/// The offset of the restore that a clip op skips to, when the clip is empty. It is an absolute
/// offset in the op stream, and it must be after the offset word (`validate_offsetToRestore`).
// Port of: src/core/SkPicturePlayback.cpp#L105-L109 (chrome/m156), validate_offsetToRestore
fn read_offset_to_restore(reader: &mut ReadBuffer<'_>) -> Option<usize> {
    let offset = reader.read_int();
    let Ok(offset) = usize::try_from(offset) else {
        reader.validate(false);
        return None;
    };
    if offset != 0 {
        reader.validate(offset.is_multiple_of(4) && offset >= reader.offset());
    }
    reader.is_valid().then_some(offset)
}

/// The clip op of packed clip parameters: the region op in the low four bits, which must be
/// difference or intersect (`ClipParams_unpackRegionOp`, for the ops that are not old ones).
// Port of: src/core/SkPictureFlat.h#L175-L183 (chrome/m156), ClipParams_unpackRegionOp
fn clip_op_of(reader: &mut ReadBuffer<'_>, packed: u32) -> Option<ClipOp> {
    match packed & 0xF {
        0 => Some(ClipOp::Difference),
        1 => Some(ClipOp::Intersect),
        _ => {
            reader.validate(false);
            None
        }
    }
}

/// Reads the clip parameters and the restore offset that follow a clip's shape, and plays the
/// clip with `clip` if they are valid. An empty clip then skips to the restore it names.
// Port of: src/core/SkPicturePlayback.cpp#L146-L165 (chrome/m156), the tail of the clip ops
fn finish_clip(
    reader: &mut ReadBuffer<'_>,
    canvas: &Canvas,
    packed: u32,
    clip: impl FnOnce(&Canvas, ClipOp, bool),
) -> bool {
    let do_aa = clip_params_unpack_do_aa(packed);
    let offset_to_restore = read_offset_to_restore(reader);
    let Some(offset_to_restore) = offset_to_restore else {
        return false;
    };
    if !reader.is_valid() {
        return false;
    }
    if let Some(op) = clip_op_of(reader, packed) {
        clip(canvas, op, do_aa);
    }
    if canvas.is_clip_empty() && offset_to_restore != 0 {
        let to_skip = offset_to_restore - reader.offset();
        if reader.skip(to_skip).is_none() {
            return false;
        }
    }
    reader.is_valid()
}

/// Reads an op's arguments and plays it into `canvas` (`handleOp`). Returns false for an op that
/// is invalid or not read yet.
// Port of: src/core/SkPicturePlayback.cpp#L132-L762 (chrome/m156), handleOp, for the ops below
fn handle_op(
    reader: &mut ReadBuffer<'_>,
    op: u8,
    canvas: &Canvas,
    initial_matrix: &M44,
    data: &PictureData,
    version: u32,
) -> bool {
    match op {
        draw_type::CLIP_RECT => {
            let rect = reader.read_rect();
            let packed = reader.read_uint();
            finish_clip(reader, canvas, packed, |canvas, op, do_aa| {
                canvas.clip_rect(rect, Some(op), Some(do_aa));
            })
        }
        draw_type::CLIP_RRECT => {
            let rrect = reader.read_rrect();
            let packed = reader.read_uint();
            finish_clip(reader, canvas, packed, |canvas, op, do_aa| {
                canvas.clip_rrect(rrect, Some(op), Some(do_aa));
            })
        }
        draw_type::CLIP_PATH => {
            let Some(path) = get_path(reader, data) else {
                return false;
            };
            let packed = reader.read_uint();
            finish_clip(reader, canvas, packed, |canvas, op, do_aa| {
                canvas.clip_path(path, Some(op), Some(do_aa));
            })
        }
        draw_type::RESET_CLIP => {
            canvas.reset_clip();
            true
        }
        draw_type::CONCAT44 => {
            let m = read_m44(reader);
            if !reader.is_valid() {
                return false;
            }
            canvas.concat_44(&m);
            true
        }
        draw_type::SET_M44 => {
            let m = read_m44(reader);
            canvas.set_matrix(&M44::concat(initial_matrix, &m));
            reader.is_valid()
        }
        draw_type::SAVE => {
            canvas.save();
            true
        }
        draw_type::RESTORE => {
            canvas.restore();
            true
        }
        draw_type::SAVE_LAYER_SAVELAYERREC => play_save_layer(reader, canvas, data, version),
        _ => handle_draw_op(reader, op, canvas, data),
    }
}

/// Reads the arguments of a draw op and draws with it into `canvas` (the draw part of
/// `handleOp`). Returns false for an op that is invalid or not read yet.
// Port of: src/core/SkPicturePlayback.cpp#L526-L596 (chrome/m156), the draw arms of handleOp
/// `DRAW_TEXT_BLOB`: the paint, the blob, and the origin of the blob.
// Port of: src/core/SkPicturePlayback.cpp#L633-L641 (chrome/m156), DRAW_TEXT_BLOB
fn play_draw_text_blob(reader: &mut ReadBuffer<'_>, data: &PictureData, canvas: &Canvas) -> bool {
    let Some(paint) = required_paint(reader, data) else {
        return false;
    };
    let Some(blob) = get_text_blob(reader, data) else {
        return false;
    };
    let x = reader.read_scalar();
    let y = reader.read_scalar();
    if !reader.is_valid() {
        return false;
    }
    canvas.draw_text_blob(blob, (x, y), paint);
    true
}

// One arm per draw op, as the switch of `SkPicturePlayback::playbackDrawOp` has.
#[allow(clippy::too_many_lines)]
fn handle_draw_op(
    reader: &mut ReadBuffer<'_>,
    op: u8,
    canvas: &Canvas,
    data: &PictureData,
) -> bool {
    match op {
        draw_type::DRAW_PAINT => {
            let Some(paint) = required_paint(reader, data) else {
                return false;
            };
            canvas.draw_paint(paint);
            reader.is_valid()
        }
        draw_type::DRAW_RECT | draw_type::DRAW_OVAL => {
            let Some(paint) = required_paint(reader, data) else {
                return false;
            };
            let rect = reader.read_rect();
            if !reader.is_valid() {
                return false;
            }
            if op == draw_type::DRAW_RECT {
                canvas.draw_rect(rect, paint);
            } else {
                canvas.draw_oval(rect, paint);
            }
            true
        }
        draw_type::DRAW_ARC => {
            let Some(paint) = required_paint(reader, data) else {
                return false;
            };
            let oval = reader.read_rect();
            let start_angle = reader.read_scalar();
            let sweep_angle = reader.read_scalar();
            let use_center = reader.read_int() != 0;
            if !reader.is_valid() {
                return false;
            }
            canvas.draw_arc(oval, start_angle, sweep_angle, use_center, paint);
            true
        }
        draw_type::DRAW_RRECT => {
            let Some(paint) = required_paint(reader, data) else {
                return false;
            };
            let rrect = reader.read_rrect();
            if !reader.is_valid() {
                return false;
            }
            canvas.draw_rrect(rrect, paint);
            true
        }
        draw_type::DRAW_DRRECT => {
            let Some(paint) = required_paint(reader, data) else {
                return false;
            };
            let outer = reader.read_rrect();
            let inner = reader.read_rrect();
            if !reader.is_valid() {
                return false;
            }
            canvas.draw_drrect(outer, inner, paint);
            true
        }
        draw_type::DRAW_PATH => {
            let Some(paint) = required_paint(reader, data) else {
                return false;
            };
            let Some(path) = get_path(reader, data) else {
                return false;
            };
            canvas.draw_path(path, paint);
            reader.is_valid()
        }
        draw_type::DRAW_POINTS => {
            let Some(paint) = required_paint(reader, data) else {
                return false;
            };
            let Some(mode) = point_mode_of(reader) else {
                return false;
            };
            let Ok(count) = usize::try_from(reader.read_int()) else {
                reader.validate(false);
                return false;
            };
            let Some(bytes) = reader.skip(count * 2 * size_of::<f32>()) else {
                return false;
            };
            let pts: Vec<Point> = bytes
                .as_chunks::<{ 2 * size_of::<f32>() }>()
                .0
                .iter()
                .map(|pt| {
                    Point::new(
                        f32::from_ne_bytes(pt[..4].try_into().expect("four bytes")),
                        f32::from_ne_bytes(pt[4..].try_into().expect("four bytes")),
                    )
                })
                .collect();
            canvas.draw_points(mode, &pts, paint);
            reader.is_valid()
        }
        draw_type::DRAW_TEXT_BLOB => play_draw_text_blob(reader, data, canvas),
        _ => false,
    }
}

/// Reads a 4x4 matrix in column-major order (`SkReadBuffer::read(SkM44*)`); the identity if the
/// buffer cannot give it.
// Port of: src/core/SkReadBuffer.cpp#L184-L193 (chrome/m156), read(SkM44*)
fn read_m44(reader: &mut ReadBuffer<'_>) -> M44 {
    let mut values = [0.0; 16];
    for value in &mut values {
        *value = reader.read_scalar();
    }
    if reader.is_valid() {
        M44::col_major(&values)
    } else {
        M44::new_identity()
    }
}

/// The point mode of an op: one of the three modes (`checkRange`).
// Port of: src/core/SkPicturePlayback.cpp#L586-L589 (chrome/m156), the mode of DRAW_POINTS
fn point_mode_of(reader: &mut ReadBuffer<'_>) -> Option<PointMode> {
    match reader.read_int() {
        0 => Some(PointMode::Points),
        1 => Some(PointMode::Lines),
        2 => Some(PointMode::Polygon),
        _ => {
            reader.validate(false);
            None
        }
    }
}

/// The tile mode of a save layer, which is one of the four (`read32LE(kLastTileMode)`).
fn tile_mode_of(reader: &mut ReadBuffer<'_>) -> Option<TileMode> {
    match reader.read_uint() {
        0 => Some(TileMode::Clamp),
        1 => Some(TileMode::Repeat),
        2 => Some(TileMode::Mirror),
        3 => Some(TileMode::Decal),
        _ => {
            reader.validate(false);
            None
        }
    }
}

/// Reads a save layer and saves it (`SAVE_LAYER_SAVELAYERREC`). The fields that are not ported
/// (the backdrop, the backdrop scale, the several filters and the obsolete clip mask and matrix)
/// make the op unreadable when they are present in a version that has them.
// Port of: src/core/SkPicturePlayback.cpp#L681-L734 (chrome/m156), SAVE_LAYER_SAVELAYERREC
fn play_save_layer(
    reader: &mut ReadBuffer<'_>,
    canvas: &Canvas,
    data: &PictureData,
    version: u32,
) -> bool {
    let flat_flags = reader.read_uint();
    let mut bounds = None;
    if flat_flags & save_layer_rec::HAS_BOUNDS != 0 {
        bounds = Some(reader.read_rect());
    }
    let mut paint = None;
    if flat_flags & save_layer_rec::HAS_PAINT != 0 {
        paint = required_paint(reader, data);
        if paint.is_none() {
            return false;
        }
    }
    // Not ported: a backdrop (a paint with an image filter), the obsolete clip mask and matrix.
    let not_ported = save_layer_rec::HAS_BACKDROP
        | save_layer_rec::HAS_CLIPMASK_OBSOLETE
        | save_layer_rec::HAS_CLIPMATRIX_OBSOLETE;
    if flat_flags & not_ported != 0 {
        return false;
    }
    let mut flags = 0;
    if flat_flags & save_layer_rec::HAS_FLAGS != 0 {
        flags = reader.read_uint();
    }
    // Not ported: the backdrop scale factor, and the several filters.
    if flat_flags & save_layer_rec::HAS_BACKDROP_SCALE != 0
        && version >= VERSION_BACKDROP_SCALE_FACTOR
    {
        return false;
    }
    if flat_flags & save_layer_rec::HAS_MULTIPLE_FILTERS != 0
        && version >= VERSION_MULTIPLE_FILTERS_ON_SAVE_LAYER
    {
        return false;
    }
    let mut backdrop_tile_mode = TileMode::Clamp;
    if flat_flags & save_layer_rec::HAS_BACKDROP_TILEMODE != 0
        && version >= VERSION_SAVE_LAYER_BACKDROP_TILE_MODE
    {
        let Some(mode) = tile_mode_of(reader) else {
            return false;
        };
        backdrop_tile_mode = mode;
    }
    if !reader.is_valid() {
        return false;
    }

    let mut rec = SaveLayerRec::default();
    if let Some(bounds) = bounds.as_ref() {
        rec = rec.bounds(bounds);
    }
    if let Some(paint) = paint {
        rec = rec.paint(paint);
    }
    rec = rec
        .backdrop_tile_mode(backdrop_tile_mode)
        .flags(SaveLayerFlags::from_bits_retain(flags));
    canvas.save_layer(&rec);
    true
}

/// Forward ports a picture from its data: plays its ops into a recorder with the cull rect of
/// `info`, and returns the picture it records (`SkPicture::Forwardport`). `None` if the ops are
/// not all readable.
// Port of: src/core/SkPicture.cpp#L147-L160 (chrome/m156), Forwardport
#[doc(alias = "Forwardport")]
pub(crate) fn forward_port(info: &PictInfo, data: &PictureData) -> Option<Picture> {
    data.op_data()?;
    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(info.cull_rect, false);
    if !play(data, info.version, canvas) {
        return None;
    }
    recorder.finish_recording_as_picture(None)
}
