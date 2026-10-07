// Copyright 2007 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPicture.h, src/core/SkPicture.cpp

//! `SkPicture`: a recorded sequence of canvas drawing commands that can be played back later.
//!
//! A picture is made by a [`PictureRecorder`](crate::picture_recorder::PictureRecorder). It has a
//! cull rect, which is used as a bounding box hint. To limit picture bounds, use a canvas clip
//! when recording or drawing the picture.
//!
//! skia-rust: serialization (`serialize`, `MakeFromStream`, `MakeFromData`, `SkPictureData`) and
//! `makeShader` (the picture shader is Phase 3) are not ported, and a picture holds no drawable
//! snapshots (`SkDrawable` is not ported).

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::bbh_factory::BBoxHierarchy;
use crate::canvas::Canvas;
use crate::picture_priv;
use crate::record::Record;
use crate::record_draw::record_draw;
use crate::rect::{Contains, Rect};

/// May be passed to [`Picture::playback_with_callback`] to stop it before all drawing commands
/// have been processed (`SkPicture::AbortCallback`).
///
/// If [`abort`](Self::abort) returns true, playback is interrupted. The part of the picture drawn
/// when aborted is undefined. If the abort happens inside one or more calls to
/// [`Canvas::save`], the stack of canvas matrix and clip values is restored to its state before
/// playback was called.
// Port of: include/core/SkPicture.h#L83-L103 (chrome/m156)
#[doc(alias = "SkPicture::AbortCallback")]
pub trait AbortCallback {
    /// Provides an override that can stop playback (`abort`). Returns true to stop playback.
    fn abort(&mut self) -> bool;
}

/* This handles generating unique IDs */
// Port of: src/core/SkPicture.cpp#L47-L54 (chrome/m156)
fn next_picture_id() -> u32 {
    static NEXT_ID: AtomicU32 = AtomicU32::new(1);
    loop {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        if id != 0 {
            return id;
        }
    }
}

#[derive(Debug)]
pub(crate) struct PictureInner {
    unique_id: u32,
    cull_rect: Rect,
    approx_bytes_used_by_sub_pictures: usize,
    // When the record is None, then we treat the picture as a "placeholder"
    // placeholders provide the bare minimum to operate on a picture
    record: Option<Arc<Record>>,
    bbh: Option<Arc<dyn BBoxHierarchy>>,
}

impl fmt::Debug for dyn BBoxHierarchy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BBoxHierarchy").finish_non_exhaustive()
    }
}

/// A recorded sequence of canvas drawing commands (`SkPicture`).
///
/// A picture is immutable and cheap to clone (it shares its commands).
// Port of: include/core/SkPicture.h#L35-L196 (chrome/m156)
#[doc(alias = "SkPicture")]
#[derive(Clone)]
pub struct Picture {
    inner: Arc<PictureInner>,
}

impl fmt::Debug for Picture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Picture")
            .field("cull_rect", &self.cull_rect())
            .field("unique_id", &self.unique_id())
            .field("approximate_op_count", &self.approximate_op_count())
            .field("approximate_bytes_used", &self.approximate_bytes_used())
            .finish()
    }
}

impl AsRef<Picture> for Picture {
    fn as_ref(&self) -> &Picture {
        self
    }
}

impl Picture {
    // Port of: src/core/SkPicture.cpp#L56-L67 (chrome/m156)
    pub(crate) fn new(
        cull: Rect,
        record: Option<Arc<Record>>,
        bbh: Option<Arc<dyn BBoxHierarchy>>,
        approx_bytes_used_by_sub_pictures: usize,
    ) -> Picture {
        Picture {
            inner: Arc::new(PictureInner {
                unique_id: next_picture_id(),
                cull_rect: cull,
                approx_bytes_used_by_sub_pictures,
                record,
                bbh,
            }),
        }
    }

    /// Replays the drawing commands on the specified canvas. In the case that the commands are
    /// recorded, each command in the picture is sent separately to `canvas`. To add a single
    /// command to draw the picture to a recording canvas, call [`Canvas::draw_picture`] instead.
    pub fn playback(&self, canvas: &Canvas) {
        self.playback_with_callback(canvas, None);
    }

    /// Like [`playback`](Self::playback), with a callback that can stop it early.
    // Port of: src/core/SkPicture.cpp#L379-L392 (chrome/m156)
    pub fn playback_with_callback(
        &self,
        canvas: &Canvas,
        callback: Option<&mut dyn AbortCallback>,
    ) {
        let Some(record) = &self.inner.record else {
            // A placeholder.
            return;
        };
        let local_clip_bounds = canvas.local_clip_bounds().unwrap_or_else(Rect::new_empty);
        let use_bbh = !local_clip_bounds.contains(self.cull_rect());
        record_draw(
            record,
            canvas,
            if use_bbh {
                self.inner.bbh.as_deref()
            } else {
                None
            },
            callback,
        );
    }

    /// Returns the cull rect: a hint of the picture's bounds. It does not specify the clipping
    /// rect for the picture; the picture is free to discard recorded drawing commands that fall
    /// outside it (`cullRect`).
    #[doc(alias = "cullRect")]
    #[must_use]
    pub fn cull_rect(&self) -> Rect {
        self.inner.cull_rect
    }

    /// Returns a non-zero value unique among pictures in Skia's process (`uniqueID`).
    #[doc(alias = "uniqueID")]
    #[must_use]
    pub fn unique_id(&self) -> u32 {
        self.inner.unique_id
    }

    /// Returns a placeholder picture with the given cull rect (`MakePlaceholder`).
    ///
    /// Result is immutable and its identifier is unique. The placeholder can be intercepted
    /// during playback to insert other commands into the canvas draw stream.
    // Port of: src/core/SkPicture.cpp#L449-L451 (chrome/m156)
    #[doc(alias = "MakePlaceholder")]
    #[must_use]
    pub fn new_placeholder(cull: impl AsRef<Rect>) -> Picture {
        picture_priv::make_picture(*cull.as_ref(), None, None, 0)
    }

    /// Returns the approximate number of operations in the picture, not counting nested ones
    /// (`approximateOpCount`).
    #[doc(alias = "approximateOpCount")]
    #[must_use]
    pub fn approximate_op_count(&self) -> usize {
        self.approximate_op_count_nested(false)
    }

    /// Returns the approximate number of operations in the picture. The number may be greater or
    /// less than the number of canvas calls recorded: some calls may be recorded as more than one
    /// operation, other calls may be optimized away. If `nested` is true, includes the op-counts
    /// of nested pictures as well, else just counts the ops in the top-level picture
    /// (`approximateOpCount(bool)`).
    // Port of: src/core/SkPicture.cpp#L428-L430 (chrome/m156)
    #[doc(alias = "approximateOpCount")]
    #[must_use]
    pub fn approximate_op_count_nested(&self, nested: impl Into<Option<bool>>) -> usize {
        picture_priv::approximate_op_count(self, 0, nested.into().unwrap_or(false))
    }

    /// Returns the approximate byte size of the picture, including the memory referenced by it
    /// (`approximateBytesUsed`).
    // Port of: src/core/SkPicture.cpp#L432-L443 (chrome/m156)
    #[doc(alias = "approximateBytesUsed")]
    #[must_use]
    pub fn approximate_bytes_used(&self) -> usize {
        let Some(record) = &self.inner.record else {
            // A placeholder.
            return std::mem::size_of::<PictureInner>();
        };
        let mut bytes = std::mem::size_of::<PictureInner>()
            + record.bytes_used()
            + self.inner.approx_bytes_used_by_sub_pictures;
        if let Some(bbh) = &self.inner.bbh {
            bytes += bbh.bytes_used();
        }
        bytes
    }

    /// The commands of the picture, `None` for a placeholder.
    pub(crate) fn record(&self) -> Option<&Arc<Record>> {
        self.inner.record.as_ref()
    }
}
