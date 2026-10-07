// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/StreamTest.cpp (chrome/m156)

#![cfg(test)]

use std::io::{Seek, SeekFrom};
use std::path::Path;

use skia_rust_core::buffer::RBuffer;
use skia_rust_core::data::Data;
use skia_rust_core::front_buffered_stream::FrontBufferedStream;
use skia_rust_core::random::Random;
use skia_rust_core::stream::{
    DynamicMemoryWStream, FileStream, FileWStream, MemoryStream, Stream, StreamAsset, WStream,
};
use skia_rust_core::stream_priv;

use crate::resources::{get_resource_as_data, resource_dir};
use crate::tmp_dir::get_tmp_dir;
use crate::{Reporter, def_test, errorf, reporter_assert, skip_missing_resource};

// Port of: tests/StreamTest.cpp#L44-L60 (chrome/m156)
fn test_loop_stream(
    reporter: &mut Reporter,
    stream: &mut dyn Stream,
    src: &[u8],
    len: usize,
    repeat: usize,
) {
    let mut tmp = vec![0u8; len];

    for _ in 0..repeat {
        let bytes = stream.read(&mut tmp);
        reporter_assert!(reporter, bytes == len);
        reporter_assert!(reporter, tmp[..len] == src[..len]);
    }

    // expect EOF
    let bytes = stream.read(&mut tmp[..1]);
    reporter_assert!(reporter, 0 == bytes);
    // isAtEnd might not return true until after the first failing read.
    reporter_assert!(reporter, stream.is_at_end());
}

// Port of: tests/StreamTest.cpp#L62-L97 (chrome/m156)
fn test_filestreams(reporter: &mut Reporter, tmp_dir: &Path) {
    let path = tmp_dir.join("wstream_test");

    let s = b"abcdefghijklmnopqrstuvwxyz";

    {
        let mut writer = FileWStream::new(&path);
        if !writer.is_valid() {
            errorf!(reporter, "Failed to create tmp file {}\n", path.display());
            return;
        }

        for _ in 0..100 {
            writer.write(&s[..26]);
        }
    }

    {
        let mut stream = FileStream::new(&path);
        reporter_assert!(reporter, stream.is_valid());
        test_loop_stream(reporter, &mut stream, s, 26, 100);

        let mut stream2 = stream.duplicate();
        test_loop_stream(reporter, stream2.as_mut(), s, 26, 100);
    }

    {
        let file = std::fs::File::open(&path).unwrap();
        let mut stream = FileStream::from_file(file);
        reporter_assert!(reporter, stream.is_valid());
        test_loop_stream(reporter, &mut stream, s, 26, 100);

        let mut stream2 = stream.duplicate();
        test_loop_stream(reporter, stream2.as_mut(), s, 26, 100);
    }
}

// Port of: tests/StreamTest.cpp#L99-L154 (chrome/m156)
fn test_wstream(reporter: &mut Reporter) {
    let mut ds = DynamicMemoryWStream::new();
    let s = b"abcdefghijklmnopqrstuvwxyz";
    for _ in 0..100 {
        reporter_assert!(reporter, ds.write(&s[..26]));
    }
    reporter_assert!(reporter, ds.bytes_written() == 100 * 26);

    let mut dst = vec![0u8; 100 * 26 + 1];
    dst[100 * 26] = b'*';
    ds.copy_to(&mut dst);
    reporter_assert!(reporter, dst[100 * 26] == b'*');
    for i in 0..100 {
        reporter_assert!(reporter, dst[i * 26..i * 26 + 26] == s[..26]);
    }

    {
        let mut stream = ds.detach_as_stream();
        reporter_assert!(reporter, 100 * 26 == stream.get_length());
        reporter_assert!(reporter, ds.bytes_written() == 0);
        test_loop_stream(reporter, stream.as_mut(), s, 26, 100);

        let mut stream2 = stream.duplicate_asset();
        test_loop_stream(reporter, stream2.as_mut(), s, 26, 100);

        let mut stream3 = stream.fork_asset();
        reporter_assert!(reporter, stream3.is_at_end());
        let mut tmp = [0u8; 1];
        let bytes = stream.read(&mut tmp);
        reporter_assert!(reporter, 0 == bytes);
        stream3.rewind();
        test_loop_stream(reporter, stream3.as_mut(), s, 26, 100);
    }

    for _ in 0..100 {
        reporter_assert!(reporter, ds.write(&s[..26]));
    }
    reporter_assert!(reporter, ds.bytes_written() == 100 * 26);

    {
        // Test that this works after a snapshot.
        let mut stream = ds.detach_as_stream();
        reporter_assert!(reporter, ds.bytes_written() == 0);
        test_loop_stream(reporter, stream.as_mut(), s, 26, 100);

        let mut stream2 = stream.duplicate_asset();
        test_loop_stream(reporter, stream2.as_mut(), s, 26, 100);
    }

    if let Some(tmp_dir) = get_tmp_dir() {
        test_filestreams(reporter, &tmp_dir);
    }
}

// Port of: tests/StreamTest.cpp#L156-L185 (chrome/m156)
fn test_packed_uint(reporter: &mut Reporter) {
    // we know that packeduint tries to write 1, 2 or 4 bytes for the length,
    // so we test values around each of those transitions (and a few others)
    let sizes: [usize; 28] = [
        0,
        1,
        2,
        0xFC,
        0xFD,
        0xFE,
        0xFF,
        0x100,
        0x101,
        32767,
        32768,
        32769,
        0xFFFD,
        0xFFFE,
        0xFFFF,
        0x10000,
        0x10001,
        0xFF_FFFD,
        0xFF_FFFE,
        0xFF_FFFF,
        0x100_0000,
        0x100_0001,
        0x7FFF_FFFE,
        0x7FFF_FFFF,
        0x8000_0000,
        0x8000_0001,
        0xFFFF_FFFE,
        0xFFFF_FFFF,
    ];

    let mut wstream = DynamicMemoryWStream::new();

    for size in &sizes {
        let success = wstream.write_packed_uint(*size);
        reporter_assert!(reporter, success);
    }

    let mut rstream = wstream.detach_as_stream();
    for (i, size) in sizes.iter().enumerate() {
        let n = rstream.read_packed_uint();
        let Some(n) = n else {
            errorf!(reporter, "[{}] sizes:{:x} could not be read\n", i, size);
            // `n` keeps its (uninitialised) value in C++; the next comparison is then moot.
            continue;
        };
        if *size != n {
            errorf!(reporter, "[{}] sizes:{:x} != n:{:x}\n", i, size, n);
        }
    }
}

// Test that setting a MemoryStream to a nullptr data does not result in a crash when calling
// methods that access fData.
// Port of: tests/StreamTest.cpp#L187-L196 (chrome/m156)
fn test_dereferencing_data(reporter: &mut Reporter, mem_stream: &mut MemoryStream) {
    reporter_assert!(reporter, mem_stream.read(&mut []) == 0);
    // Reading non-zero bytes from an empty stream should cleanly read zero bytes.
    let mut buf = [0u8; 1];
    reporter_assert!(reporter, mem_stream.read(&mut buf) == 0);
    let _ = mem_stream.get_memory_base();
    let _ = mem_stream.get_data();
}

// Port of: tests/StreamTest.cpp#L198-L205 (chrome/m156)
fn test_null_data(reporter: &mut Reporter) {
    let mut mem_stream = MemoryStream::from_data(None);
    test_dereferencing_data(reporter, &mut mem_stream);

    mem_stream.set_data(None);
    test_dereferencing_data(reporter, &mut mem_stream);
}

// Port of: tests/StreamTest.cpp#L207-L211 (chrome/m156)
def_test!(Stream, |reporter| {
    test_wstream(reporter);
    test_packed_uint(reporter);
    test_null_data(reporter);
});

/// Tests peeking and then reading the same amount. The two should provide the same results.
/// Returns the amount successfully read minus the amount successfully peeked.
// Port of: tests/StreamTest.cpp#L214-L243 (chrome/m156)
#[allow(clippy::eq_op)] // see below: the C++ compares a buffer with itself
fn compare_peek_to_read(
    reporter: &mut Reporter,
    stream: &mut dyn Stream,
    bytes_to_peek: usize,
) -> usize {
    // The rest of our tests won't be very interesting if bytesToPeek is zero.
    reporter_assert!(reporter, bytes_to_peek > 0);
    let mut peek_storage = vec![0u8; bytes_to_peek];
    let _read_storage = vec![0u8; bytes_to_peek];

    let bytes_peeked = stream.peek(&mut peek_storage);
    // The C++ sets `readPtr = peekStorage.get()` (not `readStorage`), so the read below lands
    // in the buffer that was just peeked into.
    let bytes_read = stream.read(&mut peek_storage);

    // bytesRead should only be less than attempted if the stream is at the end.
    reporter_assert!(reporter, bytes_read == bytes_to_peek || stream.is_at_end());

    // peek and read should behave the same, except peek returned to the original position, so
    // they read the same data.
    reporter_assert!(
        reporter,
        peek_storage[..bytes_peeked] == peek_storage[..bytes_peeked]
    );

    // A stream should never be able to peek more than it can read.
    reporter_assert!(reporter, bytes_read >= bytes_peeked);

    bytes_read - bytes_peeked
}

// Port of: tests/StreamTest.cpp#L245-L249 (chrome/m156)
fn test_fully_peekable_stream(r: &mut Reporter, stream: &mut dyn Stream, _limit: usize) {
    let mut i = 1;
    while !stream.is_at_end() {
        reporter_assert!(r, compare_peek_to_read(r, stream, i) == 0);
        i += 1;
    }
}

// Port of: tests/StreamTest.cpp#L252-L304 (chrome/m156)
fn test_peeking_front_buffered_stream(r: &mut Reporter, original: &dyn Stream, buffer_size: usize) {
    let dupe = original.duplicate();
    reporter_assert!(r, dupe.is_some());
    let mut buffered_stream = FrontBufferedStream::make(dupe, buffer_size);
    reporter_assert!(r, buffered_stream.is_some());
    let buffered = buffered_stream.as_mut().unwrap();

    let mut peeked = 0;
    let mut i = 1;
    while !buffered.is_at_end() {
        let unpeekable_bytes = compare_peek_to_read(r, buffered.as_mut(), i);
        if unpeekable_bytes > 0 {
            // This could not have returned a number greater than i.
            reporter_assert!(r, unpeekable_bytes <= i);

            // We have reached the end of the buffer. Verify that it was at least bufferSize.
            reporter_assert!(r, peeked + i - unpeekable_bytes >= buffer_size);
            // No more peeking is supported.
            break;
        }
        peeked += i;
        i += 1;
    }

    // Test that attempting to peek beyond the length of the buffer does not prevent rewinding.
    let mut buffered_stream = FrontBufferedStream::make(original.duplicate(), buffer_size);
    reporter_assert!(r, buffered_stream.is_some());
    let buffered = buffered_stream.as_mut().unwrap();

    let bytes_to_peek = buffer_size + 1;
    let mut peek_storage = vec![0u8; bytes_to_peek];
    let mut read_storage = vec![0u8; bytes_to_peek];

    for start in 0..=buffer_size {
        // Skip to the starting point
        reporter_assert!(r, buffered.skip(start) == start);

        let bytes_peeked = buffered.peek(&mut peek_storage[..bytes_to_peek]);
        if 0 == bytes_peeked {
            // Peeking should only fail completely if we have read/skipped beyond the buffer.
            reporter_assert!(r, start >= buffer_size);
            break;
        }

        // Only read the amount that was successfully peeked.
        let bytes_read = buffered.read(&mut read_storage[..bytes_peeked]);
        reporter_assert!(r, bytes_read == bytes_peeked);
        reporter_assert!(
            r,
            peek_storage[..bytes_peeked] == read_storage[..bytes_peeked]
        );

        // This should be safe to rewind.
        reporter_assert!(r, buffered.rewind());
    }
}

// This test uses file system operations that don't work out of the
// box on iOS. It's likely that we don't need them on iOS. Ignoring for now.
// TODO(stephana): Re-evaluate if we need this in the future.
// Port of: tests/StreamTest.cpp#L310-L352 (chrome/m156)
def_test!(StreamPeek, |reporter| {
    // Test a memory stream.
    let g_abcs: &'static [u8] = b"abcdefghijklmnopqrstuvwxyz";
    let mut mem_stream = MemoryStream::make_direct(g_abcs);
    let length = mem_stream.get_length();
    test_fully_peekable_stream(reporter, mem_stream.as_mut(), length);

    // Test an arbitrary file stream. file streams do not support peeking.
    let Some(tmpdir) = get_tmp_dir() else {
        errorf!(reporter, "no tmp dir!");
        return;
    };
    let path = tmpdir.join("file");
    {
        let mut w_stream = FileWStream::new(&path);
        let filename = "images/baby_tux.webp";
        let data = skip_missing_resource!(get_resource_as_data(filename), filename);
        if data.is_empty() {
            errorf!(reporter, "resource missing: {}\n", filename);
            return;
        }
        if !w_stream.is_valid() || !w_stream.write(&data) {
            errorf!(reporter, "error wrtiting to file {}", path.display());
            return;
        }
    }
    let mut file_stream = FileStream::new(&path);
    reporter_assert!(reporter, file_stream.is_valid());
    if !file_stream.is_valid() {
        return;
    }
    let mut storage = vec![0u8; file_stream.get_length()];
    for i in 1..file_stream.get_length() {
        reporter_assert!(reporter, file_stream.peek(&mut storage[..i]) == 0);
    }

    // Now test some FrontBufferedStreams
    for i in 1..mem_stream.get_length() {
        test_peeking_front_buffered_stream(reporter, mem_stream.as_ref(), i);
    }
});

// Asserts that asset == expected and is peekable.
// Port of: tests/StreamTest.cpp#L355-L388 (chrome/m156)
fn stream_peek_test(rep: &mut Reporter, asset: &mut dyn StreamAsset, expected: &Data) {
    if asset.get_length() != expected.size() {
        errorf!(rep, "Unexpected length.");
        return;
    }
    let mut rand = Random::default();
    let mut buffer = [0u8; 4096];
    let expect = expected.as_bytes();
    for i in 0..asset.get_length() {
        let max_size = u32::try_from((buffer.len()).min(asset.get_length() - i)).unwrap();
        let size = rand.next_range_u(1, max_size) as usize;
        debug_assert!(size >= 1);
        debug_assert!(size <= buffer.len());
        debug_assert!(size + i <= asset.get_length());
        if asset.peek(&mut buffer[..size]) < size {
            errorf!(rep, "Peek Failed!");
            return;
        }
        if buffer[..size] != expect[i..i + size] {
            errorf!(rep, "Peek returned wrong bytes!");
            return;
        }
        let mut value = [0u8; 1];
        reporter_assert!(rep, 1 == asset.read(&mut value));
        if value[0] != expect[i] {
            errorf!(rep, "Read Failed!");
            return;
        }
    }
}

// Port of: tests/StreamTest.cpp#L390-L418 (chrome/m156)
def_test!(StreamPeek_BlockMemoryStream, |rep| {
    const K_SEED: u32 = 1234;
    let mut value_source = Random::new(K_SEED);
    let mut rand = Random::new(K_SEED << 1);
    let mut buffer = [0u8; 4096];
    let mut dynamic_memory_wstream = DynamicMemoryWStream::new();
    let mut total_written = 0;
    for _ in 0..32 {
        // Randomize the length of the blocks.
        let size = rand.next_range_u(1, u32::try_from(buffer.len()).unwrap()) as usize;
        for b in &mut buffer[..size] {
            *b = (value_source.next_u() & 0xFF) as u8;
        }
        dynamic_memory_wstream.write(&buffer[..size]);
        total_written += size;
        reporter_assert!(rep, total_written == dynamic_memory_wstream.bytes_written());
    }
    let mut asset = dynamic_memory_wstream.detach_as_stream();
    let mut expected = Data::new_uninitialized(asset.get_length());
    let expected_ptr = expected.writable_data().unwrap();
    value_source.set_seed(K_SEED); // reseed.
    // We want the exact same same "random" string of numbers to put
    // in expected. i.e.: don't rely on DynamicMemoryWStream to work
    // correctly while we are testing DynamicMemoryWStream.
    for b in expected_ptr.iter_mut().take(asset.get_length()) {
        *b = (value_source.next_u() & 0xFF) as u8;
    }
    stream_peek_test(rep, asset.as_mut(), &expected);
});

// Port of: tests/StreamTest.cpp#L420-L439 (chrome/m156)
def_test!(RemainingLengthIsBelow_MemoryStream, |rep| {
    let mut stream = MemoryStream::with_length(100);
    reporter_assert!(rep, !stream_priv::remaining_length_is_below(&stream, 0));
    reporter_assert!(rep, !stream_priv::remaining_length_is_below(&stream, 90));
    reporter_assert!(rep, !stream_priv::remaining_length_is_below(&stream, 100));

    reporter_assert!(rep, stream_priv::remaining_length_is_below(&stream, 101));
    reporter_assert!(
        rep,
        stream_priv::remaining_length_is_below(&stream, usize::MAX)
    );

    let mut buff = [0u8; 75];
    reporter_assert!(rep, stream.read(&mut buff) == 75);

    reporter_assert!(rep, !stream_priv::remaining_length_is_below(&stream, 0));
    reporter_assert!(rep, !stream_priv::remaining_length_is_below(&stream, 24));
    reporter_assert!(rep, !stream_priv::remaining_length_is_below(&stream, 25));

    reporter_assert!(rep, stream_priv::remaining_length_is_below(&stream, 26));
    reporter_assert!(rep, stream_priv::remaining_length_is_below(&stream, 100));
    reporter_assert!(
        rep,
        stream_priv::remaining_length_is_below(&stream, usize::MAX)
    );
});

// Port of: tests/StreamTest.cpp#L441-L461 (chrome/m156)
struct DumbStream<'a> {
    data: &'a [u8],
    count: usize,
    idx: usize,
}

impl<'a> DumbStream<'a> {
    fn new(data: &'a [u8], n: usize) -> Self {
        Self {
            data,
            count: n,
            idx: 0,
        }
    }
}

impl Stream for DumbStream<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> usize {
        let copy_count = (self.count - self.idx).min(buffer.len());
        if copy_count != 0 {
            buffer[..copy_count].copy_from_slice(&self.data[self.idx..self.idx + copy_count]);
            self.idx += copy_count;
        }
        copy_count
    }

    fn is_at_end(&self) -> bool {
        self.count == self.idx
    }
}

// Port of: tests/StreamTest.cpp#L463-L480 (chrome/m156)
fn stream_copy_test(reporter: &mut Reporter, src_data: &[u8], n: usize, stream: &mut dyn Stream) {
    let mut tgt = DynamicMemoryWStream::new();
    if !stream_priv::copy(&mut tgt, stream) {
        errorf!(reporter, "SkStreamPriv::Copy failed");
        return;
    }
    let data = tgt.detach_as_data();
    if data.size() != n {
        errorf!(reporter, "SkStreamPriv::Copy incorrect size");
        return;
    }
    if data.as_bytes()[..n] != src_data[..n] {
        errorf!(reporter, "SkStreamPriv::Copy bad copy");
    }
}

// Port of: tests/StreamTest.cpp#L482-L500 (chrome/m156)
def_test!(DynamicMemoryWStream_detachAsData, |r| {
    const N: usize = 40000;
    let az = "abcdefghijklmnopqrstuvwxyz";
    let mut dmws = DynamicMemoryWStream::new();
    for _ in 0..N {
        dmws.write_text(az);
    }
    reporter_assert!(r, dmws.bytes_written() == N * az.len());
    let data = dmws.detach_as_data();
    reporter_assert!(r, data.size() == N * az.len());
    let mut ptr = data.as_bytes();
    for _ in 0..N {
        if ptr[..az.len()] != *az.as_bytes() {
            errorf!(r, "detachAsData() memcmp failed");
            return;
        }
        ptr = &ptr[az.len()..];
    }
});

// Port of: tests/StreamTest.cpp#L502-L518 (chrome/m156)
def_test!(DynamicMemoryWStream_detachAsVector, |r| {
    const N: usize = 40000;
    let az = "abcdefghijklmnopqrstuvwxyz";
    let mut dmws = DynamicMemoryWStream::new();
    for _ in 0..N {
        dmws.write_text(az);
    }
    reporter_assert!(r, dmws.bytes_written() == N * az.len());
    let data = dmws.detach_as_vector();
    reporter_assert!(r, data.len() == N * az.len());
    for i in 0..N {
        if data[i * az.len()..(i + 1) * az.len()] != *az.as_bytes() {
            errorf!(r, "detachAsVector() memcmp failed");
            return;
        }
    }
});

// Port of: tests/StreamTest.cpp#L520-L532 (chrome/m156)
def_test!(StreamCopy, |reporter| {
    const N: usize = 10000;
    let mut random = Random::new(123_456);
    let mut src = vec![0u8; N];
    for item in &mut src {
        *item = (random.next_u() & 0xff) as u8;
    }
    // SkStreamPriv::Copy had two code paths; this test both.
    let mut dumb_stream = DumbStream::new(&src, N);
    stream_copy_test(reporter, &src, N, &mut dumb_stream);
    // skia-rust: `SkMemoryStream(src, N)` borrows the buffer; a `MemoryStream` shares `Data`,
    // so the buffer is copied into one.
    let mut smart_stream = MemoryStream::from_data(Some(Data::new_copy(&src[..N])));
    stream_copy_test(reporter, &src, N, &mut smart_stream);
});

// Port of: tests/StreamTest.cpp#L534-L538 (chrome/m156)
def_test!(StreamEmptyStreamMemoryBase, |r| {
    let mut tmp = DynamicMemoryWStream::new();
    let asset = tmp.detach_as_stream();
    reporter_assert!(r, asset.get_memory_base().is_none());
});

/// The state the closures of `FILEStreamWithOffset` capture in C++.
struct OffsetTest {
    expected: Vec<u8>,
    size: usize,
    middle: usize,
    remaining: usize,
}

impl OffsetTest {
    // Port of: tests/StreamTest.cpp#L581-L588 (chrome/m156)
    fn test_full_read(&self, r: &mut Reporter, stream: &mut dyn Stream) {
        let mut actual = vec![0u8; self.remaining];
        reporter_assert!(r, stream.read(&mut actual) == self.remaining);
        reporter_assert!(
            r,
            self.expected[..self.remaining] == actual[..self.remaining]
        );

        reporter_assert!(r, stream.get_position() == stream.get_length());
        reporter_assert!(r, stream.is_at_end());
    }

    // Port of: tests/StreamTest.cpp#L590-L597 (chrome/m156)
    fn test_rewind(&self, r: &mut Reporter, stream: &mut dyn Stream) {
        // Rewind goes back to original offset.
        reporter_assert!(r, stream.rewind());
        reporter_assert!(r, stream.get_position() == 0);
        let mut actual = vec![0u8; self.remaining];
        reporter_assert!(r, stream.read(&mut actual) == self.remaining);
        reporter_assert!(
            r,
            self.expected[..self.remaining] == actual[..self.remaining]
        );
    }

    // Port of: tests/StreamTest.cpp#L599-L618 (chrome/m156)
    fn test_move(&self, r: &mut Reporter, stream: &mut dyn Stream) {
        // Cannot move to before the original offset.
        reporter_assert!(r, stream.move_by(-i64::try_from(self.size).unwrap()));
        reporter_assert!(r, stream.get_position() == 0);

        reporter_assert!(r, stream.move_by(i64::MIN));
        reporter_assert!(r, stream.get_position() == 0);

        let mut actual = vec![0u8; self.remaining];
        reporter_assert!(r, stream.read(&mut actual) == self.remaining);
        reporter_assert!(
            r,
            self.expected[..self.remaining] == actual[..self.remaining]
        );

        reporter_assert!(r, stream.is_at_end());
        reporter_assert!(r, stream.get_position() == self.remaining);

        // Cannot move beyond the end.
        reporter_assert!(r, stream.move_by(1));
        reporter_assert!(r, stream.is_at_end());
        reporter_assert!(r, stream.get_position() == self.remaining);
    }

    // Port of: tests/StreamTest.cpp#L620-L629 (chrome/m156)
    fn test_seek(&self, r: &mut Reporter, stream: &mut dyn Stream) {
        // Seek to an arbitrary position.
        let arbitrary = self.middle / 2;
        reporter_assert!(r, stream.seek(arbitrary));
        reporter_assert!(r, stream.get_position() == arbitrary);
        let mini_remaining = self.remaining - arbitrary;
        let mut actual = vec![0u8; mini_remaining];
        reporter_assert!(r, stream.read(&mut actual) == mini_remaining);
        reporter_assert!(
            r,
            self.expected[arbitrary..arbitrary + mini_remaining] == actual[..mini_remaining]
        );
    }

    // Port of: tests/StreamTest.cpp#L631-L638 (chrome/m156)
    fn test_seek_beginning(&self, r: &mut Reporter, stream: &mut dyn Stream) {
        // Seek to the beginning.
        reporter_assert!(r, stream.seek(0));
        reporter_assert!(r, stream.get_position() == 0);
        let mut actual = vec![0u8; self.remaining];
        reporter_assert!(r, stream.read(&mut actual) == self.remaining);
        reporter_assert!(
            r,
            self.expected[..self.remaining] == actual[..self.remaining]
        );
    }

    // Port of: tests/StreamTest.cpp#L640-L660 (chrome/m156)
    fn test_seek_end(&self, r: &mut Reporter, stream: &mut dyn Stream) {
        let remaining = self.remaining;
        // Cannot seek past the end.
        reporter_assert!(r, stream.is_at_end());

        reporter_assert!(r, stream.seek(remaining + 1));
        reporter_assert!(r, stream.is_at_end());
        reporter_assert!(r, stream.get_position() == remaining);

        let middle = remaining / 2;
        reporter_assert!(r, stream.seek(middle));
        reporter_assert!(r, !stream.is_at_end());
        reporter_assert!(r, stream.get_position() == middle);

        reporter_assert!(r, stream.seek(remaining * 2));
        reporter_assert!(r, stream.is_at_end());
        reporter_assert!(r, stream.get_position() == remaining);

        reporter_assert!(r, stream.seek(usize::try_from(i64::MAX).unwrap()));
        reporter_assert!(r, stream.is_at_end());
        reporter_assert!(r, stream.get_position() == remaining);
    }

    // Port of: tests/StreamTest.cpp#L663-L696 (chrome/m156)
    fn test_all(&self, r: &mut Reporter, stream: &mut dyn Stream, recurse: bool) {
        reporter_assert!(r, stream.get_length() == self.remaining);
        reporter_assert!(r, stream.get_position() == 0);

        self.test_full_read(r, stream);
        self.test_rewind(r, stream);
        self.test_move(r, stream);
        self.test_seek(r, stream);
        self.test_seek_beginning(r, stream);
        self.test_seek_end(r, stream);

        if recurse {
            // Duplicate shares the original offset.
            let duplicate = stream.duplicate();
            if let Some(mut duplicate) = duplicate {
                self.test_all(r, duplicate.as_mut(), false);
            } else {
                errorf!(r, "Failed to duplicate the stream!");
            }

            // Fork shares the original offset, too.
            let fork = stream.fork();
            if let Some(mut fork) = fork {
                reporter_assert!(r, fork.is_at_end());
                reporter_assert!(r, fork.get_length() == self.remaining);
                reporter_assert!(r, fork.rewind());

                self.test_all(r, fork.as_mut(), false);
            } else {
                errorf!(r, "Failed to fork the stream!");
            }
        }
    }
}

// Port of: tests/StreamTest.cpp#L540-L699 (chrome/m156)
def_test!(FILEStreamWithOffset, |r| {
    let Some(resources) = resource_dir() else {
        return;
    };

    let filename = resources.join("images").join("baby_tux.png");
    let mut stream1 = FileStream::new(&filename);
    if !stream1.is_valid() {
        errorf!(
            r,
            "Could not create SkFILEStream from {}",
            filename.display()
        );
        return;
    }
    reporter_assert!(r, stream1.has_length());
    reporter_assert!(r, stream1.has_position());

    // Seek halfway through the file. The second SkFILEStream will be created
    // with the same filename and offset and therefore will treat that offset as
    // the beginning.
    let size = stream1.get_length();
    let middle = size / 2;
    if !stream1.seek(middle) {
        errorf!(
            r,
            "Could not seek SkFILEStream to {} out of {}",
            middle,
            size
        );
        return;
    }
    reporter_assert!(r, stream1.get_position() == middle);

    let Ok(mut file) = std::fs::File::open(&filename) else {
        errorf!(r, "Could not open {} as a FILE", filename.display());
        return;
    };

    if file.seek(SeekFrom::Start(middle as u64)).is_err() {
        errorf!(r, "Could not fseek FILE to {} out of {}", middle, size);
        return;
    }
    let mut stream2 = FileStream::from_file(file);

    let remaining = size - middle;
    let mut expected = vec![0u8; remaining];
    reporter_assert!(r, stream1.read(&mut expected) == remaining);

    let test = OffsetTest {
        expected,
        size,
        middle,
        remaining,
    };
    test.test_all(r, &mut stream2, true);
});

// Port of: tests/StreamTest.cpp#L701-L712 (chrome/m156)
def_test!(RBuffer, |reporter| {
    let value = 0i32.to_ne_bytes();
    let mut buffer = RBuffer::new(&value);
    reporter_assert!(reporter, buffer.is_valid());

    let mut tmp = [0u8; 4];
    reporter_assert!(reporter, buffer.read(&mut tmp));
    reporter_assert!(reporter, buffer.is_valid());

    reporter_assert!(reporter, !buffer.read(&mut tmp));
    reporter_assert!(reporter, !buffer.is_valid());
});
