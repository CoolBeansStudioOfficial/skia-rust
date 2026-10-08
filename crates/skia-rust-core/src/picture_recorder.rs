// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPictureRecorder.h, src/core/SkPictureRecorder.cpp

//! `SkPictureRecorder`: records drawing commands into a [`Picture`] through a [`Canvas`].
//!
//! skia-rust: `finishRecordingAsDrawable` is not ported (`SkDrawable` is not).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::bbh_factory::{BBHFactory, BBoxHierarchy, Metadata, RTreeFactory};
use crate::canvas::Canvas;
use crate::picture::Picture;
use crate::picture_priv;
use crate::record::Record;
use crate::record_canvas::{RecordCanvas, SharedRecord};
use crate::record_draw::{record_draw, record_fill_bounds};
use crate::record_opts::record_optimize;
use crate::rect::{Contains, Rect};

/// Records drawing commands made to a [`Canvas`] into a [`Picture`] (`SkPictureRecorder`).
// Port of: include/core/SkPictureRecorder.h#L28-L112 (chrome/m156)
#[doc(alias = "SkPictureRecorder")]
#[derive(Debug)]
pub struct PictureRecorder {
    bbh: Option<Arc<dyn BBoxHierarchy>>,
    recorder: RecordCanvas,
    record: SharedRecord,
    cull_rect: Rect,
    actively_recording: bool,
}

impl Default for PictureRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl PictureRecorder {
    /// A recorder that is not recording yet.
    // Port of: src/core/SkPictureRecorder.cpp#L25-L28 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        PictureRecorder {
            bbh: None,
            recorder: RecordCanvas::new_with_bounds(None, &Rect::new_empty()),
            record: Rc::new(RefCell::new(Record::new())),
            cull_rect: Rect::new_empty(),
            actively_recording: false,
        }
    }

    /// Returns the canvas that records the drawing commands.
    ///
    /// - `bounds` the cull rect used when recording this picture. Any drawing that falls outside
    ///   of this rect is undefined, and may be drawn or it may not
    /// - `use_bbh` whether to use a bounding box hierarchy (an R-tree)
    #[doc(alias = "beginRecording")]
    pub fn begin_recording(&mut self, bounds: impl AsRef<Rect>, use_bbh: bool) -> &Canvas {
        if use_bbh {
            self.begin_recording_with_factory(bounds, Some(&RTreeFactory))
        } else {
            self.begin_recording_with_bbh(bounds, None)
        }
    }

    /// Like [`begin_recording`](Self::begin_recording), with the hierarchy made by `factory`
    /// (`beginRecording(const SkRect&, SkBBHFactory*)`).
    // Port of: src/core/SkPictureRecorder.cpp#L50-L52 (chrome/m156)
    #[doc(alias = "beginRecording")]
    pub fn begin_recording_with_factory(
        &mut self,
        bounds: impl AsRef<Rect>,
        factory: Option<&dyn BBHFactory>,
    ) -> &Canvas {
        self.begin_recording_with_bbh(bounds, factory.and_then(BBHFactory::make))
    }

    /// Like [`begin_recording`](Self::begin_recording), with the given hierarchy
    /// (`beginRecording(const SkRect&, sk_sp<SkBBoxHierarchy>)`).
    ///
    /// # Panics
    /// Never: the recording canvas exists once recording has started.
    // Port of: src/core/SkPictureRecorder.cpp#L30-L48 (chrome/m156)
    #[doc(alias = "beginRecording")]
    pub fn begin_recording_with_bbh(
        &mut self,
        bounds: impl AsRef<Rect>,
        bbh: Option<Arc<dyn BBoxHierarchy>>,
    ) -> &Canvas {
        let user_cull_rect = bounds.as_ref();
        let cull_rect = if user_cull_rect.is_empty() {
            Rect::new_empty()
        } else {
            *user_cull_rect
        };

        self.cull_rect = cull_rect;
        self.bbh = bbh;

        self.recorder.reset(&self.record, &cull_rect);
        self.actively_recording = true;
        self.recording_canvas().expect("recording just started")
    }

    /// Returns the recording canvas if one is active, or `None` if recording is not active
    /// (`getRecordingCanvas`).
    // Port of: src/core/SkPictureRecorder.cpp#L54-L56 (chrome/m156)
    #[doc(alias = "getRecordingCanvas")]
    #[must_use]
    pub fn recording_canvas(&self) -> Option<&Canvas> {
        if self.actively_recording {
            Some(&self.recorder)
        } else {
            None
        }
    }

    /// Signals that the caller is done recording. This invalidates the canvas returned by
    /// [`begin_recording`](Self::begin_recording) or
    /// [`recording_canvas`](Self::recording_canvas).
    ///
    /// The returned picture is immutable. `cull_rect` updates the cull rect to use for bounding
    /// box hierarchy (BBH) generation (`finishRecordingAsPictureWithCull`); with `None` the one
    /// passed to `begin_recording` is used. Returns `None` if recording was not active.
    // Port of: src/core/SkPictureRecorder.cpp#L58-L96 (chrome/m156)
    #[doc(alias = "finishRecordingAsPicture")]
    #[doc(alias = "finishRecordingAsPictureWithCull")]
    pub fn finish_recording_as_picture(&mut self, cull_rect: Option<&Rect>) -> Option<Picture> {
        self.recording_canvas()?;
        if let Some(cull_rect) = cull_rect {
            self.cull_rect = *cull_rect;
        }
        Some(self.finish_recording())
    }

    fn finish_recording(&mut self) -> Picture {
        self.actively_recording = false;
        self.recorder.restore_to_count(1); // If we were missing any restores, add them now.

        if self.record.borrow().count() == 0 {
            return picture_priv::make_empty_picture();
        }

        // TODO: delay as much of this work until just before first playback?
        record_optimize(&mut self.record.borrow_mut());

        if let Some(bbh) = &self.bbh {
            let record = self.record.borrow();
            let count = record.count();
            let mut bounds = vec![Rect::new_empty(); count];
            let mut meta = vec![Metadata::default(); count];
            record_fill_bounds(&self.cull_rect, &record, &mut bounds, &mut meta);

            bbh.insert_with_metadata(&bounds, &meta);

            // Now that we've calculated content bounds, we can update fCullRect, often trimming
            // it.
            let mut bbh_bound = Rect::new_empty();
            for b in &bounds {
                bbh_bound.join(b);
            }
            debug_assert!(
                (bbh_bound.is_empty() || self.cull_rect.contains(&bbh_bound))
                    || (bbh_bound.is_empty() && self.cull_rect.is_empty())
            );
            self.cull_rect = bbh_bound;
        }

        let sub_picture_bytes = self.recorder.approx_bytes_used_by_sub_pictures();

        // std::move(fRecord): the record leaves the recorder; the next recording starts a new one.
        let record = std::mem::take(&mut *self.record.borrow_mut());
        picture_priv::make_picture(
            self.cull_rect,
            Some(Arc::new(record)),
            self.bbh.take(),
            sub_picture_bytes,
        )
    }

    /// Replays the current (partially recorded) operation stream into `canvas`. This call doesn't
    /// close the current recording (`partialReplay`).
    // Port of: src/core/SkPictureRecorder.cpp#L103-L116 (chrome/m156)
    #[doc(alias = "partialReplay")]
    #[doc(hidden)]
    pub fn partial_replay(&self, canvas: &Canvas) {
        record_draw(
            &self.record.borrow(),
            canvas,
            None, /*bbh*/
            None, /*callback*/
        );
    }
}
