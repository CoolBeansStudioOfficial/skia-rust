// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/codec/SkCodecAnimation.h (chrome/m156)

//! How an animation frame is disposed of and blended (`SkCodecAnimation`).

/// Port of `SkCodecAnimation::DisposalMethod`: what happens to a frame's rectangle once the frame
/// has been shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[doc(alias = "SkCodecAnimation::DisposalMethod")]
pub enum DisposalMethod {
    /// Port of `kKeep`: the frame's pixels stay on the canvas.
    #[default]
    Keep = 1,
    /// Port of `kRestoreBGColor`: the frame's rectangle is cleared to transparent.
    RestoreBgColor = 2,
    /// Port of `kRestorePrevious`: the canvas goes back to what it was before this frame.
    RestorePrevious = 3,
}

/// Port of `SkCodecAnimation::Blend`: how a frame is combined with the frames before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[doc(alias = "SkCodecAnimation::Blend")]
pub enum Blend {
    /// Port of `kSrcOver`: the frame is blended over what is already there.
    #[default]
    SrcOver,
    /// Port of `kSrc`: the frame replaces the pixels in its rectangle.
    Src,
}
