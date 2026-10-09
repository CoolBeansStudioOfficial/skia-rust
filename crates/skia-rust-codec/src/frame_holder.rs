// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkFrameHolder.h (chrome/m156), SkFrame::fillIn and
// SkFrameHolder::setAlphaAndRequiredFrame from src/codec/SkCodec.cpp#L347-L1078 (chrome/m156)

//! The frames of an animated image: each frame's rectangle, disposal, blend, opacity and the
//! earlier frame it depends on (`SkFrame`, `SkFrameHolder`).

// The frame ids and required-frame indices are checked against NO_FRAME before they are cast, so
// the casts to `usize` index the frame list as the C++ does.
#![allow(clippy::cast_sign_loss)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::rect::{Contains, IRect};

use crate::codec::{FrameInfo, NO_FRAME};
use crate::codec_animation::{Blend, DisposalMethod};
use crate::encoded_info::Alpha;

/// Port of `SkFrame::kUninitialized`: the required frame of a frame whose start of data has not
/// been read yet.
const UNINITIALIZED: i32 = -2;

/// One frame of an animated image. Port of `SkFrame`. The format-specific part (what Wuffs
/// reports for a frame, such as its I/O position) lives in the codec that owns the frame.
#[derive(Debug, Clone, PartialEq, Eq)]
#[doc(alias = "SkFrame")]
pub struct Frame {
    id: i32,
    has_alpha: bool,
    required: i32,
    rect: IRect,
    disposal_method: DisposalMethod,
    duration: i32,
    blend: Blend,
    reported_alpha: Alpha,
}

impl Frame {
    /// Port of the `SkFrame` constructor: a frame with index `id` that reports `reported_alpha`.
    // Port of: src/codec/SkFrameHolder.h (SkFrame::SkFrame)
    #[must_use]
    pub fn new(id: i32, reported_alpha: Alpha) -> Self {
        Self {
            id,
            has_alpha: false,
            required: UNINITIALIZED,
            rect: IRect::from_xywh(0, 0, 0, 0),
            disposal_method: DisposalMethod::Keep,
            duration: 0,
            blend: Blend::SrcOver,
            reported_alpha,
        }
    }

    /// Port of `SkFrame::frameId`: the 0-based index of the frame.
    #[must_use]
    pub fn frame_id(&self) -> i32 {
        self.id
    }

    /// Port of `SkFrame::reportedAlpha`: how the frame reports its alpha, considering only its
    /// own rectangle.
    #[must_use]
    pub fn reported_alpha(&self) -> Alpha {
        self.reported_alpha
    }

    /// Port of `SkFrame::hasAlpha`: the cached opacity after compositing with the prior frame.
    #[must_use]
    pub fn has_alpha(&self) -> bool {
        self.has_alpha
    }

    /// Port of `SkFrame::setHasAlpha`.
    pub fn set_has_alpha(&mut self, alpha: bool) {
        self.has_alpha = alpha;
    }

    /// Port of `SkFrame::reachedStartOfData`.
    #[must_use]
    pub fn reached_start_of_data(&self) -> bool {
        self.required != UNINITIALIZED
    }

    /// Port of `SkFrame::getRequiredFrame`: the frame this one depends on, or [`NO_FRAME`].
    #[must_use]
    pub fn required_frame(&self) -> i32 {
        debug_assert!(self.reached_start_of_data());
        self.required
    }

    /// Port of `SkFrame::setRequiredFrame`.
    pub fn set_required_frame(&mut self, required: i32) {
        self.required = required;
    }

    /// Port of `SkFrame::setXYWH`: the rectangle the frame updates.
    pub fn set_xywh(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.rect = IRect::from_xywh(x, y, width, height);
    }

    /// Port of `SkFrame::frameRect`.
    #[must_use]
    pub fn frame_rect(&self) -> IRect {
        self.rect
    }

    /// Port of `SkFrame::getDisposalMethod`.
    #[must_use]
    pub fn disposal_method(&self) -> DisposalMethod {
        self.disposal_method
    }

    /// Port of `SkFrame::setDisposalMethod`.
    pub fn set_disposal_method(&mut self, disposal_method: DisposalMethod) {
        self.disposal_method = disposal_method;
    }

    /// Port of `SkFrame::setDuration`: the milliseconds to show the frame.
    pub fn set_duration(&mut self, duration: i32) {
        self.duration = duration;
    }

    /// Port of `SkFrame::getDuration`.
    #[must_use]
    pub fn duration(&self) -> i32 {
        self.duration
    }

    /// Port of `SkFrame::setBlend`.
    pub fn set_blend(&mut self, blend: Blend) {
        self.blend = blend;
    }

    /// Port of `SkFrame::getBlend`.
    #[must_use]
    pub fn blend(&self) -> Blend {
        self.blend
    }

    /// Port of `SkFrame::fillIn`: the client-facing description of the frame.
    // Port of: src/codec/SkCodec.cpp#L949-L960 (chrome/m156)
    #[must_use]
    pub fn fill_in(&self, fully_received: bool) -> FrameInfo {
        FrameInfo {
            required_frame: self.required,
            duration: self.duration,
            fully_received,
            alpha_type: if self.has_alpha {
                AlphaType::Unpremul
            } else {
                AlphaType::Opaque
            },
            has_alpha_within_bounds: self.reported_alpha != Alpha::Opaque,
            disposal_method: self.disposal_method,
            blend: self.blend,
            frame_rect: self.rect,
        }
    }
}

/// Port of `SkFrameHolder`: the frames of an image sequence, and the size of the screen they are
/// drawn on.
#[doc(alias = "SkFrameHolder")]
pub trait FrameHolder {
    /// Port of `SkFrameHolder::screenWidth`.
    fn screen_width(&self) -> i32;
    /// Port of `SkFrameHolder::screenHeight`.
    fn screen_height(&self) -> i32;
    /// Port of `SkFrameHolder::getFrame`: the frame with index `i`, if there is one.
    fn get_frame(&self, i: i32) -> Option<&Frame>;
}

/// Port of `frame_rect_on_screen`: the part of `frame_rect` on the screen, or an empty rectangle.
// Port of: src/codec/SkCodec.cpp#L347-L354 (chrome/m156)
fn frame_rect_on_screen(frame_rect: IRect, screen_rect: IRect) -> IRect {
    let left = frame_rect.left().max(screen_rect.left());
    let top = frame_rect.top().max(screen_rect.top());
    let right = frame_rect.right().min(screen_rect.right());
    let bottom = frame_rect.bottom().min(screen_rect.bottom());
    if left < right && top < bottom {
        IRect::from_ltrb(left, top, right, bottom)
    } else {
        IRect::from_xywh(0, 0, 0, 0)
    }
}

fn independent(frame: &Frame) -> bool {
    frame.required_frame() == NO_FRAME
}

fn restore_bg(frame: &Frame) -> bool {
    frame.disposal_method() == DisposalMethod::RestoreBgColor
}

/// Port of `SkFrameHolder::setAlphaAndRequiredFrame`: computes the opacity and the required frame
/// of `frame`, given the frames before it in `frames` (indexed by frame id). The frame's own
/// rectangle, blend, disposal and reported alpha must already be set.
///
/// The rules are numbered as in the C++ (IND1 to IND6, DEP5 and DEP7).
// Port of: src/codec/SkCodec.cpp#L1013-L1078 (chrome/m156)
pub fn set_alpha_and_required_frame(
    screen_width: i32,
    screen_height: i32,
    frames: &[Frame],
    frame: &mut Frame,
) {
    let reports_alpha = frame.reported_alpha() != Alpha::Opaque;
    let screen_rect = IRect::from_wh(screen_width, screen_height);
    let frame_rect = frame_rect_on_screen(frame.frame_rect(), screen_rect);

    let i = frame.frame_id();
    if 0 == i {
        frame.set_has_alpha(reports_alpha || frame_rect != screen_rect);
        frame.set_required_frame(NO_FRAME); // IND1
        return;
    }

    let blend_with_prev_frame = frame.blend() == Blend::SrcOver;
    if (!reports_alpha || !blend_with_prev_frame) && frame_rect == screen_rect {
        frame.set_has_alpha(reports_alpha);
        frame.set_required_frame(NO_FRAME); // IND2
        return;
    }

    let mut prev_frame = &frames[(i - 1) as usize];
    while prev_frame.disposal_method() == DisposalMethod::RestorePrevious {
        let prev_id = prev_frame.frame_id();
        if 0 == prev_id {
            frame.set_has_alpha(true);
            frame.set_required_frame(NO_FRAME); // IND3
            return;
        }
        prev_frame = &frames[(prev_id - 1) as usize];
    }

    let clear_prev_frame = restore_bg(prev_frame);
    let mut prev_frame_rect = frame_rect_on_screen(prev_frame.frame_rect(), screen_rect);

    if clear_prev_frame && (prev_frame_rect == screen_rect || independent(prev_frame)) {
        frame.set_has_alpha(true);
        frame.set_required_frame(NO_FRAME); // IND4
        return;
    }

    if reports_alpha && blend_with_prev_frame {
        frame.set_required_frame(prev_frame.frame_id()); // DEP5
        frame.set_has_alpha(prev_frame.has_alpha() || clear_prev_frame);
        return;
    }

    while frame_rect.contains(prev_frame_rect) {
        let prev_required_frame = prev_frame.required_frame();
        if prev_required_frame == NO_FRAME {
            frame.set_required_frame(NO_FRAME); // IND6
            frame.set_has_alpha(true);
            return;
        }
        prev_frame = &frames[prev_required_frame as usize];
        prev_frame_rect = frame_rect_on_screen(prev_frame.frame_rect(), screen_rect);
    }

    frame.set_required_frame(prev_frame.frame_id()); // DEP7
    if restore_bg(prev_frame) {
        frame.set_has_alpha(true);
        return;
    }
    debug_assert_eq!(prev_frame.disposal_method(), DisposalMethod::Keep);
    frame.set_has_alpha(prev_frame.has_alpha() || (reports_alpha && !blend_with_prev_frame));
}
