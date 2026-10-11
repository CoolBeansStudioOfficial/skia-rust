// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/codec/SkPngChunkReader.h (chrome/m156), src/codec/SkPngCompositeChunkReader.h
// and src/codec/SkPngCompositeChunkReader.cpp (chrome/m156)
// Ported from: include/codec/SkPngChunkReader.h, src/codec/SkPngCompositeChunkReader.{h,cpp}
//
// Not ported yet: the HDR chunks (`cLLI`, `mDCV`), which parse into `skhdr::Metadata` (codecs.md
// C13). The gainmap chunks (`gmAP`, `gdAT`) are read here.

//! The client's PNG chunk reader, and the composite reader the PNG codec installs in libpng.

use std::sync::{Arc, Mutex};

use skia_rust_core::data::Data;
use skia_rust_core::gainmap_info::GainmapInfo;

/// Port of `SkPngChunkReader`: receives the unknown chunks of a PNG file, as the PNG codec reads
/// them. Implementations must be `Send` because the codec can outlive the thread that made it.
#[doc(alias = "SkPngChunkReader")]
pub trait PngChunkReader: Send {
    /// Port of `readChunk(tag, data, length)`. `tag` is the four-byte chunk type. Returning
    /// `false` makes the decode fail at this chunk.
    fn read_chunk(&mut self, tag: &[u8; 4], data: &[u8]) -> bool;
}

/// Port of `SkPngCompositeChunkReader`: forwards each unknown chunk to the client's reader, and
/// keeps the gainmap chunks (`gmAP`, `gdAT`).
// Port of: src/codec/SkPngCompositeChunkReader.h#L18-L44 (chrome/m156)
pub struct PngCompositeChunkReader {
    client: Option<Box<dyn PngChunkReader>>,
    /// The gainmap parameters of the `gmAP` chunk, if one parsed.
    gainmap_info: Option<GainmapInfo>,
    /// The encoded gainmap image of the `gdAT` chunk.
    gainmap_stream: Option<Vec<u8>>,
}

impl std::fmt::Debug for PngCompositeChunkReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PngCompositeChunkReader")
            .field("has_client_reader", &self.client.is_some())
            .field("has_gainmap_info", &self.gainmap_info.is_some())
            .field("has_gainmap_stream", &self.gainmap_stream.is_some())
            .finish()
    }
}

impl PngCompositeChunkReader {
    /// Port of the constructor, which takes the client's reader (`SkSafeRef`) or none.
    // Port of: src/codec/SkPngCompositeChunkReader.h#L22-L24 (chrome/m156)
    #[must_use]
    pub fn new(client: Option<Box<dyn PngChunkReader>>) -> Self {
        Self {
            client,
            gainmap_info: None,
            gainmap_stream: None,
        }
    }

    /// Port of `SkPngCompositeChunkReader::readChunk`.
    // Port of: src/codec/SkPngCompositeChunkReader.cpp#L11-L40 (chrome/m156), up to the HDR
    // branches (`cLLI`, `mDCV`), which are not ported: they parse into `skhdr::Metadata`.
    pub fn read_chunk(&mut self, tag: &[u8; 4], data: &[u8]) -> bool {
        if let Some(client) = self.client.as_mut() {
            // Only fail if the client's chunk reader failed. Without a client the gainmap chunks
            // are still read.
            if !client.read_chunk(tag, data) {
                return false;
            }
        }

        // If we found a chunk but there's no data, then just skip it!
        if data.is_empty() {
            return true;
        }

        if tag == b"gmAP" {
            let mut info = GainmapInfo::default();
            if GainmapInfo::parse(Some(&Data::new_copy(data)), &mut info) {
                self.gainmap_info = Some(info);
            }
        } else if tag == b"gdAT" {
            self.gainmap_stream = Some(data.to_vec());
        }
        true
    }

    /// The encoded gainmap image, moved out of the reader (`takeGainmapStream`).
    // Port of: src/codec/SkPngCompositeChunkReader.h#L37 (chrome/m156)
    pub fn take_gainmap_stream(&mut self) -> Option<Vec<u8>> {
        self.gainmap_stream.take()
    }

    /// The gainmap parameters of the `gmAP` chunk (`getGainmapInfo`).
    // Port of: src/codec/SkPngCompositeChunkReader.h#L39 (chrome/m156)
    #[must_use]
    pub fn gainmap_info(&self) -> Option<GainmapInfo> {
        self.gainmap_info.clone()
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
