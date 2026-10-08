// Copyright 1995-2023 Mark Adler (zlib); port by The skia-rust Authors.
// Use of this source code is governed by the zlib licence in the LICENSE file.
// Port of: zlib.h (chromium zlib@646b7f56, zlib 1.3.0.1-motley), the inflate half.

//! A port of the inflate half of Chromium's zlib (`zlib@646b7f56`, version 1.3.0.1-motley).
//!
//! Skia decodes PNG and other formats through libpng, which calls zlib. The output bytes and the
//! point where a truncated stream stops producing output both depend on zlib's exact decoder, so
//! this crate ports zlib's decoder function by function rather than depending on another inflate.
//!
//! The stream interface is [`Inflate::inflate`]. It reads from the front of the input slice and
//! writes at the front of the output slice, and returns how many bytes it used, which is what
//! zlib's `next_in`/`avail_in` and `next_out`/`avail_out` pointers give. The checksum, message
//! and totals that zlib keeps in `z_stream` are read from the [`Inflate`].
//!
//! Not ported (not used by libpng or Skia): gzip framing, preset dictionaries and the
//! `Z_TREES`/`Z_BLOCK` break points beyond what the state machine itself needs.
//! Deflate (the compressor) is a separate port.
//!
//! The crate is `unsafe`-free and depends on nothing but `std`.

pub mod adler32;
mod inffast;
mod inffixed;
pub mod inflate;
pub mod inftrees;

pub use adler32::adler32;
pub use inflate::{DEF_WBITS, Inflate};
pub use inftrees::Code;

/// Port of the `Z_*` flush values that inflate reads (zlib.h#L170-L175).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "Z_NO_FLUSH")]
pub enum Flush {
    /// Port of `Z_NO_FLUSH`.
    NoFlush,
    /// Port of `Z_PARTIAL_FLUSH`. inflate treats it as `NoFlush`.
    PartialFlush,
    /// Port of `Z_SYNC_FLUSH`. inflate treats it as `NoFlush`.
    SyncFlush,
    /// Port of `Z_FULL_FLUSH`. inflate treats it as `NoFlush`.
    FullFlush,
    /// Port of `Z_FINISH`: the caller expects the stream to end in this call.
    Finish,
    /// Port of `Z_BLOCK`: stop at the end of each block.
    Block,
    /// Port of `Z_TREES`: stop after the block header and its code tables.
    Trees,
}

/// Port of the `Z_*` return codes (zlib.h#L180-L189).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ReturnCode {
    /// Port of `Z_OK`.
    Ok = 0,
    /// Port of `Z_STREAM_END`: the end of the stream was reached and checked.
    StreamEnd = 1,
    /// Port of `Z_NEED_DICT`: the stream needs a preset dictionary, which is not ported.
    NeedDict = 2,
    /// Port of `Z_DATA_ERROR`: the input is corrupt. The message is in [`Inflate::msg`].
    DataError = -3,
    /// Port of `Z_STREAM_ERROR`: an invalid argument or stream state.
    StreamError = -2,
    /// Port of `Z_MEM_ERROR`. Not produced by this port (allocation aborts instead).
    MemError = -4,
    /// Port of `Z_BUF_ERROR`: no progress was possible with the buffers given.
    BufError = -5,
}
