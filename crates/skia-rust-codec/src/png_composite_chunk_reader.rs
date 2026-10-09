// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/codec/SkPngChunkReader.h (chrome/m156), src/codec/SkPngCompositeChunkReader.h
// and src/codec/SkPngCompositeChunkReader.cpp (chrome/m156)
// Ported from: include/codec/SkPngChunkReader.h, src/codec/SkPngCompositeChunkReader.{h,cpp}
//
// Not ported yet: the gainmap chunks (`gmAP`, `gdAT`) and the HDR chunks (`cLLI`, `mDCV`). They
// are parsed into `SkGainmapInfo` and `skhdr::Metadata`, which core does not have yet (codecs.md
// C13). Until then the composite reader only forwards every chunk to the client's reader.

//! The client's PNG chunk reader, and the composite reader the PNG codec installs in libpng.

use std::sync::{Arc, Mutex};

/// Port of `SkPngChunkReader`: receives the unknown chunks of a PNG file, as the PNG codec reads
/// them. Implementations must be `Send` because the codec can outlive the thread that made it.
#[doc(alias = "SkPngChunkReader")]
pub trait PngChunkReader: Send {
    /// Port of `readChunk(tag, data, length)`. `tag` is the four-byte chunk type. Returning
    /// `false` makes the decode fail at this chunk.
    fn read_chunk(&mut self, tag: &[u8; 4], data: &[u8]) -> bool;
}

/// Port of `SkPngCompositeChunkReader`: forwards each unknown chunk to the client's reader.
// Port of: src/codec/SkPngCompositeChunkReader.h#L18-L44 (chrome/m156)
pub struct PngCompositeChunkReader {
    client: Option<Box<dyn PngChunkReader>>,
}

impl std::fmt::Debug for PngCompositeChunkReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PngCompositeChunkReader")
            .field("has_client_reader", &self.client.is_some())
            .finish()
    }
}

impl PngCompositeChunkReader {
    /// Port of the constructor, which takes the client's reader (`SkSafeRef`) or none.
    // Port of: src/codec/SkPngCompositeChunkReader.h#L22-L24 (chrome/m156)
    #[must_use]
    pub fn new(client: Option<Box<dyn PngChunkReader>>) -> Self {
        Self { client }
    }

    /// Port of `SkPngCompositeChunkReader::readChunk`.
    // Port of: src/codec/SkPngCompositeChunkReader.cpp#L11-L20 (chrome/m156), up to the gainmap
    // and HDR branches, which are not ported (see the module comment)
    pub fn read_chunk(&mut self, tag: &[u8; 4], data: &[u8]) -> bool {
        if let Some(client) = self.client.as_mut() {
            // Only fail if the client's chunk reader failed.
            if !client.read_chunk(tag, data) {
                return false;
            }
        }
        true
    }
}

/// Adapter that installs the shared composite reader as libpng's `user_chunk_fn`. Port of
/// `sk_read_user_chunk` (src/codec/SkPngCodec.cpp#L56-L63).
pub(crate) struct UserChunkAdapter(pub(crate) Arc<Mutex<PngCompositeChunkReader>>);

impl skia_rust_libpng::UserChunkReader for UserChunkAdapter {
    // Port of: src/codec/SkPngCodec.cpp#L56-L63 (sk_read_user_chunk): 1 continues decoding, -1
    // stops it.
    fn read_chunk(&mut self, name: &[u8; 4], data: &[u8]) -> i32 {
        let mut reader = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if reader.read_chunk(name, data) { 1 } else { -1 }
    }
}
