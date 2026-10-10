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
//! Serialization (`serialize`, `from_data`, `from_stream`) covers the pictures of paints, paths
//! and draw ops that are ported; the sections that are not (text, images, nested pictures) are
//! listed in `notes/picture-serialization.md`.
//! `makeShader` (the picture shader is Phase 3) is not ported, and a picture holds no drawable
//! snapshots (`SkDrawable` is not ported).

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::bbh_factory::BBoxHierarchy;
use crate::canvas::Canvas;
use crate::data::Data;
use crate::flattenable::FlattenableRegistry;
use crate::picture_data::{PictInfo, PictureData};
use crate::picture_playback::forward_port;
use crate::picture_priv::{self, CURRENT_VERSION};
use crate::picture_record;
use crate::record::Record;
use crate::record_draw::record_draw;
use crate::rect::{Contains, Rect};
use crate::serial_procs::{DeserialProcs, SerialProcs};
use crate::stream::{DynamicMemoryWStream, MemoryStream, Stream, WStream};

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

/// The byte after the header of a serialized picture that says what the data is: the picture
/// data (`kPictureData_TrailingStreamByteAfterPictInfo`).
// Port of: src/core/SkPicture.cpp#L41-L45 (chrome/m156)
const PICTURE_DATA_TRAILING_BYTE: u8 = 1;

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
    // `fAddedToCache`: whether a cache entry (a picture shader's image) was made from the picture.
    added_to_cache: AtomicBool,
}

impl Drop for PictureInner {
    // Port of: src/core/SkPicture.cpp#L69-L73 (chrome/m156), `~SkPicture`
    fn drop(&mut self) {
        if self.added_to_cache.load(Ordering::Relaxed) {
            crate::resource_cache::post_purge_shared_id(picture_priv::make_shared_id(
                self.unique_id,
            ));
        }
    }
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
                added_to_cache: AtomicBool::new(false),
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

    /// Records that a cache entry was made from the picture (`SkPicturePriv::AddedToCache`), so
    /// dropping the picture purges its entries.
    // Port of: src/core/SkPicturePriv.h#L66-L70 (chrome/m156), `AddedToCache`
    pub(crate) fn set_added_to_cache(&self) {
        self.inner.added_to_cache.store(true, Ordering::Relaxed);
    }

    /// Returns true if this is the only reference to the picture (`SkRefCnt::unique`).
    #[must_use]
    pub fn unique(&self) -> bool {
        Arc::strong_count(&self.inner) == 1
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

    /// Serializes the picture to a blob of bytes, which [`from_data`](Self::from_data) reads back.
    /// `procs` decides how the typefaces are written; `None` is the default (`serialize`).
    ///
    /// Returns `None` if the picture has a command that is not serialized yet.
    // Port of: src/core/SkPicture.cpp#L267-L271 (chrome/m156), serialize(SkSerialProcs*)
    #[must_use]
    pub fn serialize(&self, procs: Option<&SerialProcs>) -> Option<Data> {
        let mut stream = DynamicMemoryWStream::new();
        self.serialize_into(&mut stream, procs)
            .then(|| stream.detach_as_data())
    }

    /// Serializes the picture to `stream` (`serialize(SkWStream*)`). Returns false, writing
    /// nothing, if the picture has a command that is not serialized yet.
    // Port of: src/core/SkPicture.cpp#L301-L330 (chrome/m156), the private serialize
    pub fn serialize_into(&self, stream: &mut dyn WStream, procs: Option<&SerialProcs>) -> bool {
        let procs = procs.cloned().unwrap_or_default();
        let Some(data) = picture_record::backport(self) else {
            return false;
        };
        // The data is made first, so a failure leaves the stream without a partial picture.
        let mut body = DynamicMemoryWStream::new();
        if !data.serialize(&mut body, &procs) {
            return false;
        }
        let body = body.detach_as_data();
        let info = PictInfo::new(self.cull_rect(), CURRENT_VERSION);
        info.write_to(stream)
            && stream.write8(PICTURE_DATA_TRAILING_BYTE)
            && stream.write(body.as_bytes())
    }

    /// Reads a picture from bytes that [`serialize`](Self::serialize) wrote, with the default
    /// flattenables (`MakeFromData`). Path effects and mask filters need a registry, see
    /// [`from_data_with_registry`](Self::from_data_with_registry).
    // Port of: src/core/SkPicture.cpp#L166-L173 (chrome/m156), MakeFromData
    #[doc(alias = "MakeFromData")]
    #[must_use]
    pub fn from_data(data: &[u8], procs: Option<&DeserialProcs>) -> Option<Picture> {
        Self::from_data_with_registry(data, procs, &FlattenableRegistry::EMPTY)
    }

    /// Like [`from_data`](Self::from_data), reading the path effects and mask filters with
    /// `registry`.
    #[must_use]
    pub fn from_data_with_registry(
        data: &[u8],
        procs: Option<&DeserialProcs>,
        registry: &FlattenableRegistry,
    ) -> Option<Picture> {
        let mut stream = MemoryStream::from_data(Some(Data::new_copy(data)));
        Self::make_from_stream_priv(&mut stream, procs, registry)
    }

    /// Reads a picture from a stream that [`serialize_into`](Self::serialize_into) wrote, with the
    /// default flattenables (`MakeFromStream`).
    // Port of: src/core/SkPicture.cpp#L162-L164 (chrome/m156), MakeFromStream
    #[doc(alias = "MakeFromStream")]
    #[must_use]
    pub fn from_stream(stream: &mut dyn Stream, procs: Option<&DeserialProcs>) -> Option<Picture> {
        Self::make_from_stream_priv(stream, procs, &FlattenableRegistry::EMPTY)
    }

    /// `MakeFromStreamPriv`: reads the header, then the data that follows it. A picture whose
    /// data is a custom format needs a picture procedure, which is not ported.
    // Port of: src/core/SkPicture.cpp#L183-L226 (chrome/m156), MakeFromStreamPriv
    fn make_from_stream_priv(
        stream: &mut dyn Stream,
        procs: Option<&DeserialProcs>,
        registry: &FlattenableRegistry,
    ) -> Option<Picture> {
        let info = PictInfo::read_from(stream)?;
        if !info.is_valid() {
            return None;
        }
        let procs = procs.cloned().unwrap_or_default();
        match stream.read_u8()? {
            PICTURE_DATA_TRAILING_BYTE => {
                let data = PictureData::parse_stream(stream, info.version, &procs, registry)?;
                forward_port(&info, &data)
            }
            // The custom format (and the failure marker) has no reader here.
            _ => None,
        }
    }

    /// The commands of the picture, `None` for a placeholder.
    pub(crate) fn record(&self) -> Option<&Arc<Record>> {
        self.inner.record.as_ref()
    }
}

#[cfg(test)]
mod serial_tests {
    use super::*;
    use crate::canvas::{PointMode, SaveLayerRec};
    use crate::clip_op::ClipOp;
    use crate::color::Color4f;
    use crate::matrix::Matrix;
    use crate::paint::{Paint, Style};
    use crate::path_builder::PathBuilder;
    use crate::picture_recorder::PictureRecorder;
    use crate::point::Point;
    use crate::rrect::RRect;

    /// A tag as Skia writes it: its four characters, read as a big-endian word, in native order.
    fn push_tag(bytes: &mut Vec<u8>, tag: [u8; 4], size: u32) {
        bytes.extend_from_slice(&u32::from_be_bytes(tag).to_ne_bytes());
        bytes.extend_from_slice(&size.to_ne_bytes());
    }

    // An empty picture is the header, then the sections that SkPictureData::serialize writes
    // with no ops: the empty op stream, no factories, no typefaces, and a buffer that holds
    // only the (empty) slugs.
    #[test]
    fn empty_picture_bytes_follow_the_sections() {
        let mut recorder = PictureRecorder::new();
        recorder.begin_recording(Rect::new(0.0, 0.0, 10.0, 10.0), false);
        let picture = recorder.finish_recording_as_picture(None).unwrap();
        let data = picture.serialize(None).unwrap();

        let mut expected = Vec::new();
        expected.extend_from_slice(b"skiapict");
        expected.extend_from_slice(&110u32.to_ne_bytes());
        // A recording with no commands is `SkPicturePriv::MakeEmptyPicture`, whose cull is empty
        // whatever the recording's bounds were.
        for edge in [0.0f32, 0.0, 0.0, 0.0] {
            expected.extend_from_slice(&edge.to_ne_bytes());
        }
        expected.push(1); // the picture data follows the header
        push_tag(&mut expected, *b"read", 0); // the op stream, which is empty
        push_tag(&mut expected, *b"fact", 4); // the factories
        expected.extend_from_slice(&0u32.to_ne_bytes());
        push_tag(&mut expected, *b"tpfc", 0); // the typefaces
        push_tag(&mut expected, *b"aray", 8); // the buffer of tables, 8 bytes
        push_tag(&mut expected, *b"slug", 0); // no slugs
        expected.extend_from_slice(&u32::from_be_bytes(*b"eof ").to_ne_bytes());

        assert_eq!(data.as_bytes(), expected.as_slice());
    }

    // Serializing a picture, reading it back and serializing it again gives the same bytes.
    #[test]
    fn mixed_ops_round_trip_to_the_same_bytes() {
        let mut recorder = PictureRecorder::new();
        let canvas = recorder.begin_recording(Rect::new(0.0, 0.0, 200.0, 200.0), false);

        let mut stroke = Paint::new(Color4f::new(1.0, 0.0, 0.0, 1.0), None);
        stroke
            .set_style(Style::Stroke)
            .set_stroke_width(3.0)
            .set_anti_alias(true);
        let fill = Paint::default();

        let mut builder = PathBuilder::new();
        builder
            .move_to((0.0, 0.0))
            .line_to((50.0, 0.0))
            .line_to((25.0, 40.0))
            .close();
        let path = builder.detach();

        canvas.save();
        canvas.translate((10.0, 20.0));
        canvas.clip_rect(Rect::new(0.0, 0.0, 100.0, 100.0), ClipOp::Intersect, true);
        canvas.draw_rect(Rect::new(1.0, 1.0, 50.0, 50.0), &stroke);
        canvas.draw_oval(Rect::new(2.0, 2.0, 60.0, 30.0), &fill);
        canvas.draw_arc(Rect::new(0.0, 0.0, 40.0, 40.0), 0.0, 90.0, true, &stroke);
        let outer = RRect::new_rect_xy(Rect::new(0.0, 0.0, 80.0, 80.0), 8.0, 8.0);
        let inner = RRect::new_rect_xy(Rect::new(10.0, 10.0, 70.0, 70.0), 4.0, 4.0);
        canvas.draw_rrect(outer, &fill);
        canvas.draw_drrect(outer, inner, &stroke);
        canvas.draw_path(&path, &stroke);
        canvas.draw_path(&path, &fill); // the same path, so one entry in the table
        canvas.draw_points(
            PointMode::Lines,
            &[Point::new(0.0, 0.0), Point::new(9.0, 9.0)],
            &stroke,
        );
        canvas.clip_path(&path, ClipOp::Intersect, false);
        canvas.clip_rrect(outer, ClipOp::Difference, true);
        canvas.save_layer(
            &SaveLayerRec::default()
                .bounds(&Rect::new(0.0, 0.0, 50.0, 50.0))
                .paint(&stroke),
        );
        canvas.draw_paint(&fill);
        canvas.restore();
        canvas.concat(&Matrix::scale((2.0, 2.0)));
        canvas.reset_clip();
        canvas.restore();
        let picture = recorder.finish_recording_as_picture(None).unwrap();

        let first = picture.serialize(None).unwrap();
        let read_back = Picture::from_data(first.as_bytes(), None).unwrap();
        let second = read_back.serialize(None).unwrap();
        assert_eq!(first.as_bytes(), second.as_bytes());
    }

    #[test]
    fn bytes_that_are_not_a_picture_do_not_load() {
        assert!(Picture::from_data(b"not a picture at all, sorry!", None).is_none());
        assert!(Picture::from_data(&[], None).is_none());
    }
}
