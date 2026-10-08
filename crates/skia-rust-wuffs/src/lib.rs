// Copyright (c) 2018 Google Inc. (the Wuffs sources this crate ports)
// Copyright (C) 2025 The skia-rust Authors.
// Use of the ported code is governed by the BSD-style licence in the LICENSE file.
//
//! A port of the GIF decoder of Wuffs v0.3, as Skia's `SkWuffsCodec` drives it
//! (`wuffs-v0.3.c`, `google/wuffs-mirror-release-c@e3f919cc`, the `DEPS` pin of
//! `chrome/m156`), used by skia-rust's codecs.
//!
//! The decoder is a coroutine machine. [`gif::GifDecoder`] exposes the entry points Skia calls
//! (`decode_image_config`, `decode_frame_config`, `decode_frame`, `restart_frame` and the
//! accessors), and takes an [`base::IoBuffer`] that the caller refills whenever a call returns
//! [`strings::SUSPENSION_SHORT_READ`]. Each call resumes exactly where the previous one suspended,
//! as the C coroutines do, so the number of rows a truncated file yields is the same.
//!
//! The crate is `unsafe`-free. The differential harness in `oracle/codec-diff/wuffs` checks the
//! output and the status strings against the C library. The Rust side of that check is
//! `tests/dump`, which `tests/differential.rs` replays against the committed C output.
//!
//! Not ported: `tell_me_more` and metadata reporting, the workbuf argument (GIF needs none), the
//! non-GIF formats of the base module, the indexed and planar pixel buffers, and the swizzler
//! paths Skia does not request. Each module says which items it omits.

pub mod base;
pub mod gif;
pub mod lzw;
pub mod strings;

pub use base::{
    AnimationDisposal, FrameConfig, ImageConfig, IoBuffer, IoMeta, PixelBlend, PixelBuffer,
    PixelConfig, PixelFormat, RectIeU32, Status, Table,
};
pub use gif::GifDecoder;
pub use lzw::LzwDecoder;
