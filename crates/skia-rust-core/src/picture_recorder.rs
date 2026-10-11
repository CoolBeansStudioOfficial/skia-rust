// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPictureRecorder.h, src/core/SkPictureRecorder.cpp

//! `SkPictureRecorder`: records drawing commands into a [`Picture`] through a [`Canvas`].
//!
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::bbh_factory::{BBHFactory, BBoxHierarchy, Metadata, RTreeFactory};
use crate::canvas::Canvas;
use crate::drawable::{Drawable, DrawableBase};
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

    /// `finishRecordingAsDrawable`: the recording as a drawable, whose bounds are the cull rect
    /// and which draws the recorded calls. Returns `None` if recording was not active.
    // Port of: src/core/SkPictureRecorder.cpp#L119-L136 (chrome/m156)
    #[doc(alias = "finishRecordingAsDrawable")]
    pub fn finish_recording_as_drawable(&mut self) -> Option<Drawable> {
        self.recording_canvas()?;
        self.actively_recording = false;
        self.recorder.restore_to_count(1); // If we were missing any restores, add them now.

        record_optimize(&mut self.record.borrow_mut());

        if let Some(bbh) = &self.bbh {
            let record = self.record.borrow();
            let count = record.count();
            let mut bounds = vec![Rect::new_empty(); count];
            let mut meta = vec![Metadata::default(); count];
            record_fill_bounds(&self.cull_rect, &record, &mut bounds, &mut meta);
            bbh.insert_with_metadata(&bounds, &meta);
        }

        // std::move(fRecord): the record leaves the recorder; the next recording starts a new one.
        let record = std::mem::take(&mut *self.record.borrow_mut());
        Some(Drawable::new(Arc::new(RecordedDrawable {
            record: Arc::new(record),
            bbh: self.bbh.take(),
            drawable_list: self.recorder.detach_drawable_list(),
            bounds: self.cull_rect,
        })))
    }

    fn finish_recording(&mut self) -> Picture {
        self.actively_recording = false;
        self.recorder.restore_to_count(1); // If we were missing any restores, add them now.

        if self.record.borrow().count() == 0 {
            return picture_priv::make_empty_picture();
        }

        // TODO: delay as much of this work until just before first playback?
        record_optimize(&mut self.record.borrow_mut());

        let drawable_list = self.recorder.drawable_list();
        let pict_list = drawable_list.as_deref().and_then(snapshot_drawables);

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

        let mut sub_picture_bytes = self.recorder.approx_bytes_used_by_sub_pictures();
        for pict in pict_list.iter().flatten() {
            sub_picture_bytes += pict.approximate_bytes_used();
        }

        // std::move(fRecord): the record leaves the recorder; the next recording starts a new one.
        let record = std::mem::take(&mut *self.record.borrow_mut());
        picture_priv::make_picture(
            self.cull_rect,
            Some(Arc::new(record)),
            pict_list,
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
        let drawable_list = self.recorder.drawable_list();
        record_draw(
            &self.record.borrow(),
            canvas,
            None, /*drawable picts*/
            drawable_list.as_deref(),
            None, /*bbh*/
            None, /*callback*/
        );
    }
}

/// The snapshots of a drawable list, or `None` if one of the drawables has none
/// (`SkDrawableList::newDrawableSnapshot`).
// Port of: src/core/SkRecordCanvas.cpp#L62-L77 (chrome/m156)
fn snapshot_drawables(drawables: &[Drawable]) -> Option<Vec<Picture>> {
    if drawables.is_empty() {
        return None;
    }
    // if a snapshot is null, then all the previous pics are released and the result is null
    drawables
        .iter()
        .map(Drawable::make_picture_snapshot)
        .collect()
}

/// `SkRecordedDrawable`: a recording that draws as a drawable, within its cull rect.
// Port of: src/core/SkRecordedDrawable.{h,cpp} (chrome/m156)
struct RecordedDrawable {
    record: Arc<Record>,
    bbh: Option<Arc<dyn BBoxHierarchy>>,
    drawable_list: Option<Vec<Drawable>>,
    bounds: Rect,
}

impl std::fmt::Debug for RecordedDrawable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecordedDrawable")
            .field("bounds", &self.bounds)
            .finish_non_exhaustive()
    }
}

impl DrawableBase for RecordedDrawable {
    /// `onGetBounds`: the cull rect.
    // Port of: src/core/SkRecordedDrawable.h#L40 (chrome/m156)
    fn on_get_bounds(&self) -> Rect {
        self.bounds
    }

    /// `onDraw`: the recorded calls, drawn onto the canvas.
    // Port of: src/core/SkRecordedDrawable.cpp#L36-L44 (chrome/m156)
    fn on_draw(&self, canvas: &Canvas) {
        record_draw(
            &self.record,
            canvas,
            None,
            self.drawable_list.as_deref(),
            self.bbh.as_deref(),
            None,
        );
    }

    /// `onApproximateBytesUsed`.
    // Port of: src/core/SkRecordedDrawable.cpp#L23-L34 (chrome/m156)
    fn on_approximate_bytes_used(&self) -> usize {
        let drawables_size: usize = self
            .drawable_list
            .iter()
            .flatten()
            .map(Drawable::approximate_bytes_used)
            .sum();
        std::mem::size_of::<Self>()
            + self.record.bytes_used()
            + self.bbh.as_ref().map_or(0, |bbh| bbh.bytes_used())
            + drawables_size
    }

    /// `onMakePictureSnapshot`.
    // Port of: src/core/SkRecordedDrawable.cpp#L46-L59 (chrome/m156)
    fn on_make_picture_snapshot(&self) -> Option<Picture> {
        // TODO: should we plumb-down the BBHFactory and recordFlags from our host
        //       PictureRecorder?
        let pict_list = self.drawable_list.as_deref().and_then(snapshot_drawables);

        let mut sub_picture_bytes = 0;
        for pict in pict_list.iter().flatten() {
            sub_picture_bytes += pict.approximate_bytes_used();
        }

        Some(picture_priv::make_picture(
            self.bounds,
            Some(Arc::clone(&self.record)),
            pict_list,
            self.bbh.clone(),
            sub_picture_bytes,
        ))
    }
}
