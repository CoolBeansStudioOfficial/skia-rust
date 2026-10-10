// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/Writer32Test.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::data::Data;
use skia_rust_core::random::Random;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::rect::Rect;
use skia_rust_core::write_buffer::{SWriter32, Writer32};

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/Writer32Test.cpp#L27-L33 (chrome/m156)
fn check_contents(reporter: &mut Reporter, writer: &Writer32, expected: &[u8]) {
    let size = expected.len();
    reporter_assert!(reporter, writer.bytes_written() == size);
    let mut storage = vec![0u8; writer.bytes_written()];
    writer.flatten(&mut storage);
    reporter_assert!(reporter, storage == expected);
}

// The bytes of `values` as 4-byte native-endian words, the layout `SkWriter32` writes.
fn words(values: &[i32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

// Port of: tests/Writer32Test.cpp#L36-L42 (chrome/m156)
fn test_reserve(_reporter: &mut Reporter) {
    // There used to be a bug where we'd assert your first reservation had to
    // fit in external storage if you used it.  This would crash in debug mode.
    // The external storage of the C++ test is only a capacity here (see `Writer32::with_capacity`).
    let mut writer = Writer32::with_capacity(4);
    writer.reserve(40);
}

// Port of: tests/Writer32Test.cpp#L44-L52 (chrome/m156)
fn test_string_null(reporter: &mut Reporter) {
    let mut writer = Writer32::with_capacity(8);

    // Can we write nullptr?
    writer.write_string(None);
    check_contents(reporter, &writer, &words(&[0x0, 0x0]));
}

// Port of: tests/Writer32Test.cpp#L54-L82 (chrome/m156)
fn test_rewind(reporter: &mut Reporter) {
    let mut swriter = SWriter32::<32>::default();
    let mut array: [i32; 3] = [1, 2, 4];

    reporter_assert!(reporter, 0 == swriter.bytes_written());
    for value in array {
        swriter.write32(value);
    }
    check_contents(reporter, &swriter, &words(&array));

    swriter.rewind_to_offset(2 * size_of::<i32>());
    reporter_assert!(reporter, size_of_val(&array) - 4 == swriter.bytes_written());
    swriter.write32(3);
    reporter_assert!(reporter, size_of_val(&array) == swriter.bytes_written());
    array[2] = 3;
    check_contents(reporter, &swriter, &words(&array));

    // test rewinding past allocated chunks. This used to crash because we
    // didn't truncate our link-list after freeing trailing blocks
    let mut writer = Writer32::new();
    for i in 0..100 {
        writer.write32(i);
    }
    reporter_assert!(reporter, 100 * 4 == writer.bytes_written());
    for j in (0..=100 * 4).rev().step_by(16) {
        writer.rewind_to_offset(j);
    }
    reporter_assert!(reporter, writer.bytes_written() < 16);
}

// Port of: tests/Writer32Test.cpp#L84-L96 (chrome/m156)
fn test1(reporter: &mut Reporter, writer: &mut Writer32) {
    let data: [u32; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    for (i, &value) in data.iter().enumerate() {
        reporter_assert!(reporter, i * 4 == writer.bytes_written());
        writer.write_u32(value);
        reporter_assert!(reporter, value == writer.read32_at(i * 4));
    }

    let mut buffer = [0u8; 40];
    reporter_assert!(reporter, buffer.len() == writer.bytes_written());
    writer.flatten(&mut buffer);
    let expected: Vec<u8> = data.iter().flat_map(|v| v.to_ne_bytes()).collect();
    reporter_assert!(reporter, buffer[..] == expected[..]);
}

// Port of: tests/Writer32Test.cpp#L98-L134 (chrome/m156)
fn test_write_pad(reporter: &mut Reporter, writer: &mut Writer32) {
    // Create some random data to write.
    let data_size = 10;

    let mut rand = Random::new(0);
    let original_data: Vec<u32> = (0..data_size).map(|_| rand.next_u()).collect();
    let original_bytes: Vec<u8> = original_data.iter().flat_map(|v| v.to_ne_bytes()).collect();

    // Write  the random data to the writer at different lengths for
    // different alignments.
    for len in 0..data_size {
        writer.write_pad(&original_bytes[..len]);
    }

    let total_bytes = writer.bytes_written();

    let mut read_storage = vec![0u8; total_bytes];
    writer.flatten(&mut read_storage);

    let mut reader = ReadBuffer::new(&read_storage);

    for len in 0..data_size {
        let read_ptr = reader.skip(len).unwrap_or_default();
        // Ensure that the data read is the same as what was written.
        reporter_assert!(
            reporter,
            read_ptr.get(..len) == Some(&original_bytes[..len])
        );
        // Ensure that the rest is padded with zeroes.
        for &byte in read_ptr.get(len..).unwrap_or_default() {
            reporter_assert!(reporter, byte == 0);
        }
    }
}

// Port of: tests/Writer32Test.cpp#L136-L171 (chrome/m156)
// The C++ test compares the scalars it stored, bit for bit, so exact equality is the assertion.
#[allow(clippy::float_cmp)]
fn test_overwrite_t(reporter: &mut Reporter, writer: &mut Writer32) {
    let padding: usize = 64;

    let uint1: u32 = 0x1234_5678;
    let uint2: u32 = 0x9876_5432;
    let scalar1: f32 = 1234.5678;
    // the C++ literal, verbatim
    #[allow(clippy::excessive_precision)]
    let scalar2: f32 = 9876.5432;
    let rect1 = Rect::from_xywh(1.0, 2.0, 3.0, 4.0);
    let rect2 = Rect::from_xywh(5.0, 6.0, 7.0, 8.0);

    for _ in 0..(padding / 4) {
        writer.write32(0);
    }

    writer.write_u32(uint1);
    writer.write_rect(&rect1);
    writer.write_scalar(scalar1);

    for _ in 0..(padding / 4) {
        writer.write32(0);
    }

    reporter_assert!(reporter, writer.read32_at(padding) == uint1);
    reporter_assert!(
        reporter,
        writer.read_rect_at(padding + size_of::<u32>()) == rect1
    );
    reporter_assert!(
        reporter,
        writer.read_scalar_at(padding + size_of::<u32>() + size_of::<Rect>()) == scalar1
    );

    writer.overwrite32(padding, uint2);
    writer.overwrite_rect_at(padding + size_of::<u32>(), &rect2);
    writer.overwrite_scalar_at(padding + size_of::<u32>() + size_of::<Rect>(), scalar2);

    reporter_assert!(reporter, writer.read32_at(padding) == uint2);
    reporter_assert!(
        reporter,
        writer.read_rect_at(padding + size_of::<u32>()) == rect2
    );
    reporter_assert!(
        reporter,
        writer.read_scalar_at(padding + size_of::<u32>() + size_of::<Rect>()) == scalar2
    );
}

// Port of: tests/Writer32Test.cpp#L173-L182 (chrome/m156)
def_test!(Writer32_dynamic, |reporter| {
    let mut writer = Writer32::new();
    test1(reporter, &mut writer);

    writer.reset();
    test_write_pad(reporter, &mut writer);

    writer.reset();
    test_overwrite_t(reporter, &mut writer);
});

// Port of: tests/Writer32Test.cpp#L184-L193 (chrome/m156)
def_test!(Writer32_small, |reporter| {
    let mut writer = SWriter32::<{ isize::BITS as usize }>::default();
    test1(reporter, &mut writer);

    writer.reset(); // should just rewind our storage
    test_write_pad(reporter, &mut writer);

    writer.reset();
    test_overwrite_t(reporter, &mut writer);
});

// Port of: tests/Writer32Test.cpp#L195-L204 (chrome/m156)
def_test!(Writer32_large, |reporter| {
    let mut writer = SWriter32::<{ 1024 * size_of::<isize>() }>::default();
    test1(reporter, &mut writer);

    writer.reset(); // should just rewind our storage
    test_write_pad(reporter, &mut writer);

    writer.reset();
    test_overwrite_t(reporter, &mut writer);
});

// Port of: tests/Writer32Test.cpp#L206-L210 (chrome/m156)
def_test!(Writer32_misc, |reporter| {
    test_reserve(reporter);
    test_string_null(reporter);
    test_rewind(reporter);
});

// Port of: tests/Writer32Test.cpp#L212-L252 (chrome/m156)
def_test!(Writer32_data, |reporter| {
    let str_bytes = b"0123456789";
    let data0 = Data::new_with_cstring(Some(c"0123456789"));
    let data1 = Data::new_empty();

    let sizes = [
        Writer32::write_data_size(None),
        Writer32::write_data_size(Some(&data0)),
        Writer32::write_data_size(Some(&data1)),
    ];
    let mut writer = SWriter32::<1000>::default();
    let mut size_written = 0;

    writer.write_data(None);
    size_written += sizes[0];
    reporter_assert!(reporter, size_written == writer.bytes_written());

    writer.write_data(Some(&data0));
    size_written += sizes[1];
    reporter_assert!(reporter, size_written == writer.bytes_written());

    writer.write_data(Some(&data1));
    size_written += sizes[2];
    reporter_assert!(reporter, size_written == writer.bytes_written());

    let result = writer.snapshot_as_data();

    let mut reader = ReadBuffer::new(result.as_bytes());
    let d0 = reader
        .read_byte_array_as_data()
        .expect("the first byte array");
    let d1 = reader
        .read_byte_array_as_data()
        .expect("the second byte array");
    let d2 = reader
        .read_byte_array_as_data()
        .expect("the third byte array");

    reporter_assert!(reporter, 0 == d0.size());
    reporter_assert!(reporter, str_bytes.len() + 1 == d1.size());
    reporter_assert!(reporter, d1.as_bytes() == b"0123456789\0");
    reporter_assert!(reporter, 0 == d2.size());

    reporter_assert!(reporter, reader.offset() == size_written);
    reporter_assert!(reporter, reader.eof());
});
