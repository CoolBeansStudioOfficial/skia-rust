// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CodecPriv.h (chrome/m156): the registry guard for the serial codec tests, and the
// helpers that build ICO files around a PNG. Shared by the codec and ICO tests.

use std::sync::{Mutex, MutexGuard, PoisonError};

use skia_rust_codec::codecs::{self, Decoder};
use skia_rust_codec::{ico_codec, png_codec};

use crate::resources::get_resource_as_data;

// The registry is process-wide, so the tests that change it run one at a time. Port of the
// `DEF_SERIAL_TEST` ordering that the C++ harness gives these cases.
static SERIAL_TESTS: Mutex<()> = Mutex::new(());

/// Takes the lock for a serial test that changes the decoder registry (`DEF_SERIAL_TEST`).
pub fn serial_test_lock() -> MutexGuard<'static, ()> {
    SERIAL_TESTS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Port of `ScopedCodecDecoders`: saves the registry on construction and restores it on drop, so
/// a test's registrations do not outlive it.
// Port of: tests/CodecPriv.h#L32-L46 (chrome/m156)
#[derive(Debug)]
pub struct ScopedCodecDecoders {
    saved: Vec<Decoder>,
}

impl ScopedCodecDecoders {
    /// Saves the current registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            saved: codecs::registered(),
        }
    }

    /// Removes every registered decoder. Port of `ScopedCodecDecoders::clear`.
    pub fn clear(&self) {
        codecs::set_registered(Vec::new());
    }
}

impl Default for ScopedCodecDecoders {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ScopedCodecDecoders {
    fn drop(&mut self) {
        codecs::set_registered(std::mem::take(&mut self.saved));
    }
}

/// The PNG decoder entry as Skia's `SkPngDecoder::Decoder()` gives it.
#[must_use]
pub fn png_decoder() -> Decoder {
    Decoder {
        id: "png",
        is_format: png_codec::is_png_format,
        make_from_stream: png_codec::make_from_stream,
    }
}

/// The ICO decoder entry as Skia's `SkIcoDecoder::Decoder()` gives it.
#[must_use]
pub fn ico_decoder() -> Decoder {
    Decoder {
        id: "ico",
        is_format: ico_codec::is_ico,
        make_from_stream: ico_codec::make_from_stream,
    }
}

/// Port of `make_ico_with_png`: an ICO file with one image, which is the PNG given.
// Port of: tests/CodecPriv.h#L73-L110 (chrome/m156)
#[must_use]
pub fn make_ico_with_png(png: &[u8]) -> Vec<u8> {
    const HEADER_SIZE: u32 = 22;
    let size32 = u32::try_from(png.len()).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(HEADER_SIZE as usize + png.len());
    // Reserved, image type 1 (ICO), and one image.
    out.extend_from_slice(&[0x00, 0x00, 0x01, 0x00, 0x01, 0x00]);
    // Width and height (0 means 256), colour count, reserved, colour planes 1, 32 bits per pixel.
    out.extend_from_slice(&[0, 0, 0, 0, 0x01, 0x00, 0x20, 0x00]);
    out.extend_from_slice(&size32.to_le_bytes());
    out.extend_from_slice(&HEADER_SIZE.to_le_bytes());
    out.extend_from_slice(png);
    out
}

/// Port of `make_ico_from_png_resource`: [`make_ico_with_png`] around a resource's PNG. `None` if
/// the resource is missing.
// Port of: tests/CodecPriv.h#L112-L121 (chrome/m156)
#[must_use]
pub fn make_ico_from_png_resource(resource: &str) -> Option<Vec<u8>> {
    let png = get_resource_as_data(resource)?;
    Some(make_ico_with_png(&png))
}
