// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkPicturePriv.h, src/core/SkPicture.cpp

//! `SkPicturePriv`: the private picture helpers (`MakePicture`, `ApproximateOpCount`, ...).
//!
//! skia-rust: the serialization half (`MakeFromBuffer`, `Flatten`, the version history) and the
//! drawable snapshot array (`SkSnapshotArray`) are not ported.

use std::sync::Arc;

use crate::bbh_factory::BBoxHierarchy;
use crate::canvas_priv::MAX_PICTURE_OPS_TO_UNROLL_INSTEAD_OF_REF;
use crate::picture::Picture;
use crate::record::Record;
use crate::records::Command;
use crate::rect::Rect;

/// The depth at which nested pictures stop being counted (`kDefaultRecursionLimit`).
// Port of: src/core/SkPicturePriv.h#L41-L41 (chrome/m156)
pub const DEFAULT_RECURSION_LIMIT: usize = 100;

/// The oldest picture format that can be read (`SkPicturePriv::kMin_Version`).
// Port of: src/core/SkPicturePriv.h#L201 (chrome/m156)
pub(crate) const MIN_VERSION: u32 = 82;

/// The version written by [`Picture::serialize`](crate::picture::Picture::serialize)
/// (`SkPicturePriv::kCurrent_Version`).
// Port of: src/core/SkPicturePriv.h#L202 (chrome/m156)
pub(crate) const CURRENT_VERSION: u32 = 110;

/// The version where paints gained blenders (`kSkBlenderInSkPaint`): older paints have a
/// draw looper and no blender.
// Port of: src/core/SkPicturePriv.h#L154 (chrome/m156)
pub(crate) const VERSION_SK_BLENDER_IN_SK_PAINT: u32 = 87;

/// The version where save layers gained a backdrop scale factor (`kBackdropScaleFactor`).
// Port of: src/core/SkPicturePriv.h#L157 (chrome/m156)
pub(crate) const VERSION_BACKDROP_SCALE_FACTOR: u32 = 90;

/// The version where save layers gained several image filters (`kMultipleFiltersOnSaveLayer`).
// Port of: src/core/SkPicturePriv.h#L171 (chrome/m156)
pub(crate) const VERSION_MULTIPLE_FILTERS_ON_SAVE_LAYER: u32 = 104;

/// The version where save layers gained a backdrop tile mode (`kSaveLayerBackdropTileMode`).
// Port of: src/core/SkPicturePriv.h#L173 (chrome/m156)
pub(crate) const VERSION_SAVE_LAYER_BACKDROP_TILE_MODE: u32 = 106;

/// The version where sampling options gained anisotropic filtering (`kAnisotropicFilter`).
// Port of: src/core/SkPicturePriv.h#L183 (chrome/m156)
pub(crate) const VERSION_ANISOTROPIC_FILTER: u32 = 92;

/// The version where the blend color filter became float, `kBlend4fColorFilter`.
// Port of: src/core/SkPicturePriv.h#L160 (chrome/m156)
pub(crate) const VERSION_BLEND_4F_COLOR_FILTER: u32 = 93;

/// The version where the matrix color filter could be unclamped, `kUnclampedMatrixColorFilter`.
// Port of: src/core/SkPicturePriv.h#L172 (chrome/m156)
pub(crate) const VERSION_UNCLAMPED_MATRIX_COLOR_FILTER: u32 = 105;

/// The version where the color shaders combined their color space, `kCombineColorShaders`.
// Port of: src/core/SkPicturePriv.h#L174 (chrome/m156)
pub(crate) const VERSION_COMBINE_COLOR_SHADERS: u32 = 107;

/// The version where `SkWorkingColorSpaceShader` gained its alpha type and output space
/// (`kWorkingColorSpaceOutput`).
// Port of: src/core/SkPicturePriv.h#L145-L176 (chrome/m156)
pub(crate) const VERSION_WORKING_COLOR_SPACE_OUTPUT: u32 = 109;

/// The shared ID of the resource cache entries made from picture `picture_id`
/// (`SkPicturePriv::MakeSharedID`).
// Port of: src/core/SkPicturePriv.h#L61-L64 (chrome/m156), `MakeSharedID`
#[doc(alias = "MakeSharedID")]
#[must_use]
pub fn make_shared_id(picture_id: u32) -> u64 {
    let shared_id = u64::from(crate::font_types::set_four_byte_tag(b'p', b'i', b'c', b't'));
    (shared_id << 32) | u64::from(picture_id)
}

/// Records that a cache entry was made from `pic` (`SkPicturePriv::AddedToCache`).
// Port of: src/core/SkPicturePriv.h#L66-L70 (chrome/m156), `AddedToCache`
#[doc(alias = "AddedToCache")]
pub fn added_to_cache(pic: &Picture) {
    pic.set_added_to_cache();
}

/// Makes a picture from its parts (`MakePicture`). A `None` record makes a placeholder.
// Port of: src/core/SkPicture.cpp#L355-L368 (chrome/m156)
#[doc(alias = "MakePicture")]
#[must_use]
pub fn make_picture(
    cull: Rect,
    record: Option<Arc<Record>>,
    bbh: Option<Arc<dyn BBoxHierarchy>>,
    approx_bytes_used_by_sub_pictures: usize,
) -> Picture {
    Picture::new(cull, record, bbh, approx_bytes_used_by_sub_pictures)
}

/// A picture with an empty cull rect and no commands (`MakeEmptyPicture`).
// Port of: src/core/SkPicture.cpp#L350-L353 (chrome/m156)
#[doc(alias = "MakeEmptyPicture")]
#[must_use]
pub fn make_empty_picture() -> Picture {
    make_picture(Rect::new_empty(), Some(Arc::new(Record::new())), None, 0)
}

/// The commands of `pic`; `None` for a placeholder (`GetRecord`).
#[doc(alias = "GetRecord")]
#[must_use]
pub fn get_record(pic: &Picture) -> Option<&Record> {
    pic.record().map(|r| &**r)
}

/// The approximate number of operations in `pic` (`ApproximateOpCount`); `depth` is how deep the
/// picture is nested, and `nested` whether to count the ops of the pictures it draws.
// Port of: src/core/SkPicture.cpp#L394-L426 (chrome/m156)
#[doc(alias = "ApproximateOpCount")]
#[must_use]
pub fn approximate_op_count(pic: &Picture, depth: usize, nested: bool) -> usize {
    if depth > DEFAULT_RECURSION_LIMIT {
        return 0;
    }

    let Some(record) = pic.record() else {
        // A placeholder.
        // approximateOpCount() needs to be greater than kMaxPictureOpsToUnrollInsteadOfRef
        // (SkCanvasPriv.h) to avoid unrolling this into a parent picture.
        return MAX_PICTURE_OPS_TO_UNROLL_INSTEAD_OF_REF + 1;
    };

    if !nested {
        return record.count();
    }

    // NestedApproxOpCounter
    let mut count = 0;
    for i in 0..record.count() {
        match record.get(i) {
            Command::DrawPicture(op) => {
                count += 1 + approximate_op_count(&op.picture, depth + 1, true);
            }
            _ => count += 1,
        }
    }
    count
}
