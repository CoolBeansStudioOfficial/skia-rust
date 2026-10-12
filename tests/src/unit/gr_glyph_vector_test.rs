// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/GrGlyphVectorTest.cpp (chrome/m156)
//
// The tests make their glyphs in a `SubRunAllocator`. Rust owns the glyphs of a `GlyphVector`, so
// there is no allocator to pass.

#![cfg(test)]

use skia_rust_core::packed_glyph_id::PackedGlyphId;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::strike::StrikePromise;
use skia_rust_core::strike_spec::StrikeSpec;
use skia_rust_core::write_buffer::BinaryWriteBuffer;
use skia_rust_gpu::text_gpu::glyph_vector::GlyphVector;
use skia_rust_tools::font_tool_utils::default_font;

use crate::{def_test, reporter_assert};

// Port of: tests/GrGlyphVectorTest.cpp#L49-L83 (chrome/m156)
def_test!(GlyphVector_Serialization, |r| {
    const N: u16 = 10;

    let font = default_font();
    let (strike_spec, _) = StrikeSpec::make_canonicalized(&font, None);

    let glyphs: Vec<PackedGlyphId> = (0..N).map(PackedGlyphId::from_glyph_id).collect();

    let promise = StrikePromise::from_strike(strike_spec.find_or_create_strike());

    let src = GlyphVector::make(promise, &glyphs);

    let mut w_buffer = BinaryWriteBuffer::new();
    src.flatten(&mut w_buffer);

    let data = w_buffer.snapshot_as_data();
    let mut r_buffer = ReadBuffer::new(data.as_bytes());
    let dst = GlyphVector::make_from_buffer(&mut r_buffer, None);
    reporter_assert!(r, dst.is_some());
    let Some(dst) = dst else { return };
    reporter_assert!(
        r,
        src.strike_promise().descriptor() == dst.strike_promise().descriptor()
    );

    reporter_assert!(r, src.glyph_count() == dst.glyph_count());
    let n = src.glyph_count().min(dst.glyph_count());
    for i in 0..n {
        let s = src.get_packed_glyph_id(i);
        let d = dst.get_packed_glyph_id(i);
        reporter_assert!(r, s == d);
    }
});

// Port of: tests/GrGlyphVectorTest.cpp#L85-L136 (chrome/m156)
def_test!(GlyphVector_BadLengths, |r| {
    let (strike_spec, _) = StrikeSpec::make_canonicalized(&default_font(), None);

    // Strike to keep in the strike cache.
    let strike = strike_spec.find_or_create_strike();

    // Be sure to keep the strike alive. The promise to serialize as the first part of the
    // GlyphVector.
    let promise = StrikePromise::from_strike(strike.clone());
    {
        // Make broken stream by hand - zero length
        let mut w_buffer = BinaryWriteBuffer::new();
        promise.flatten(&mut w_buffer);
        w_buffer.write_int(0); // length
        let data = w_buffer.snapshot_as_data();
        let mut r_buffer = ReadBuffer::new(data.as_bytes());
        let dst = GlyphVector::make_from_buffer(&mut r_buffer, None);
        reporter_assert!(r, dst.is_none());
    }

    {
        // Make broken stream by hand - zero length
        let mut w_buffer = BinaryWriteBuffer::new();
        promise.flatten(&mut w_buffer);
        // Make broken stream by hand - stream is too short
        w_buffer.write_int(5); // length
        w_buffer.write_uint(12); // random data
        w_buffer.write_uint(12); // random data
        w_buffer.write_uint(12); // random data
        let data = w_buffer.snapshot_as_data();
        let mut r_buffer = ReadBuffer::new(data.as_bytes());
        let dst = GlyphVector::make_from_buffer(&mut r_buffer, None);
        reporter_assert!(r, dst.is_none());
    }

    {
        // Make broken stream by hand - length out of range of safe calculations
        let mut w_buffer = BinaryWriteBuffer::new();
        promise.flatten(&mut w_buffer);
        w_buffer.write_int(i32::MAX - 10); // length
        w_buffer.write_uint(12); // random data
        w_buffer.write_uint(12); // random data
        w_buffer.write_uint(12); // random data
        let data = w_buffer.snapshot_as_data();
        let mut r_buffer = ReadBuffer::new(data.as_bytes());
        let dst = GlyphVector::make_from_buffer(&mut r_buffer, None);
        reporter_assert!(r, dst.is_none());
    }
});
