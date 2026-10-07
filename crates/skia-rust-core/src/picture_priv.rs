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
