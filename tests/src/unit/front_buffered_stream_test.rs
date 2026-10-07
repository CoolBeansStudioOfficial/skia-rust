// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/FrontBufferedStreamTest.cpp (chrome/m156)

#![cfg(test)]

use std::cell::RefCell;
use std::rc::Rc;

use skia_rust_core::data::Data;
use skia_rust_core::front_buffered_stream::FrontBufferedStream as AndroidFrontBufferedStream;
use skia_rust_core::stream::{MemoryStream, Stream};

use crate::{Reporter, def_test, reporter_assert};

// skia-rust: the C++ tests "cheat and continue to refer to the wrapped stream" through a raw
// pointer after handing the stream to `FrontBufferedStream::Make`. Here the wrapped stream is
// shared through an `Rc<RefCell<..>>`, and `Shared` is the handle that is handed over.
struct Shared<T: Stream>(Rc<RefCell<T>>);

impl<T: Stream> Stream for Shared<T> {
    fn read(&mut self, buffer: &mut [u8]) -> usize {
        self.0.borrow_mut().read(buffer)
    }

    fn skip(&mut self, size: usize) -> usize {
        self.0.borrow_mut().skip(size)
    }

    fn peek(&mut self, buffer: &mut [u8]) -> usize {
        self.0.borrow_mut().peek(buffer)
    }

    fn is_at_end(&self) -> bool {
        self.0.borrow().is_at_end()
    }

    fn rewind(&mut self) -> bool {
        self.0.borrow_mut().rewind()
    }

    fn has_position(&self) -> bool {
        self.0.borrow().has_position()
    }

    fn get_position(&self) -> usize {
        self.0.borrow().get_position()
    }

    fn seek(&mut self, position: usize) -> bool {
        self.0.borrow_mut().seek(position)
    }

    fn move_by(&mut self, offset: i64) -> bool {
        self.0.borrow_mut().move_by(offset)
    }

    fn has_length(&self) -> bool {
        self.0.borrow().has_length()
    }

    fn get_length(&self) -> usize {
        self.0.borrow().get_length()
    }
}

// Port of: tests/FrontBufferedStreamTest.cpp#L20-L28 (chrome/m156)
fn test_read(
    reporter: &mut Reporter,
    buffered_stream: &mut dyn Stream,
    expectations: &[u8],
    bytes_to_read: usize,
) {
    // output for reading bufferedStream.
    let mut storage = vec![0u8; bytes_to_read];

    let bytes_read = buffered_stream.read(&mut storage);
    reporter_assert!(
        reporter,
        bytes_read == bytes_to_read || buffered_stream.is_at_end()
    );
    reporter_assert!(
        reporter,
        storage[..bytes_read] == expectations[..bytes_read]
    );
}

// Port of: tests/FrontBufferedStreamTest.cpp#L30-L34 (chrome/m156)
fn test_rewind(reporter: &mut Reporter, buffered_stream: &mut dyn Stream, should_succeed: bool) {
    let success = buffered_stream.rewind();
    reporter_assert!(reporter, success == should_succeed);
}

// Test that hasLength() returns the correct value, based on the stream
// being wrapped. A length can only be known if the wrapped stream has a
// length and it has a position (so its initial position can be taken into
// account when computing the length).
// Port of: tests/FrontBufferedStreamTest.cpp#L36-L48 (chrome/m156)
fn test_has_length(
    reporter: &mut Reporter,
    buffered_stream: &dyn Stream,
    stream_being_buffered: &dyn Stream,
) {
    if stream_being_buffered.has_length() && stream_being_buffered.has_position() {
        reporter_assert!(reporter, buffered_stream.has_length());
    } else {
        reporter_assert!(reporter, !buffered_stream.has_length());
    }
}

// All tests will buffer this string, and compare output to the original.
// The string is long to ensure that all of our lengths being tested are
// smaller than the string length.
// Port of: tests/FrontBufferedStreamTest.cpp#L53 (chrome/m156)
const G_ABCS: &[u8] =
    b"abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwx";

// Tests reading the stream across boundaries of what has been buffered so far and what
// the total buffer size is.
// Port of: tests/FrontBufferedStreamTest.cpp#L57-L88 (chrome/m156)
fn test_incremental_buffering(reporter: &mut Reporter, buffer_size: usize) {
    // NOTE: For this and other tests in this file, we cheat and continue to refer to the
    // wrapped stream, but that's okay because we know the wrapping stream has not been
    // deleted yet (and we only call const methods in it).
    let mem_stream = Rc::new(RefCell::new(*MemoryStream::make_direct(G_ABCS)));

    let mut buffered_stream =
        AndroidFrontBufferedStream::make(Some(Box::new(Shared(mem_stream.clone()))), buffer_size)
            .unwrap();

    test_has_length(reporter, buffered_stream.as_ref(), &*mem_stream.borrow());

    // First, test reading less than the max buffer size.
    test_read(reporter, buffered_stream.as_mut(), G_ABCS, buffer_size / 2);

    // Now test rewinding back to the beginning and reading less than what was
    // already buffered.
    test_rewind(reporter, buffered_stream.as_mut(), true);
    test_read(reporter, buffered_stream.as_mut(), G_ABCS, buffer_size / 4);

    // Now test reading part of what was buffered, and buffering new data.
    test_read(
        reporter,
        buffered_stream.as_mut(),
        &G_ABCS[buffer_size / 4..],
        buffer_size / 2,
    );

    // Now test reading what was buffered, buffering new data, and
    // reading directly from the stream.
    test_rewind(reporter, buffered_stream.as_mut(), true);
    test_read(reporter, buffered_stream.as_mut(), G_ABCS, buffer_size << 1);

    // We have reached the end of the buffer, so rewinding will fail.
    // This test assumes that the stream is larger than the buffer; otherwise the
    // result of rewind should be true.
    test_rewind(reporter, buffered_stream.as_mut(), false);
}

// Port of: tests/FrontBufferedStreamTest.cpp#L90-L108 (chrome/m156)
fn test_perfectly_sized_buffer(reporter: &mut Reporter, buffer_size: usize) {
    let mem_stream = Rc::new(RefCell::new(*MemoryStream::make_direct(G_ABCS)));
    let mut buffered_stream =
        AndroidFrontBufferedStream::make(Some(Box::new(Shared(mem_stream.clone()))), buffer_size)
            .unwrap();
    test_has_length(reporter, buffered_stream.as_ref(), &*mem_stream.borrow());

    // Read exactly the amount that fits in the buffer.
    test_read(reporter, buffered_stream.as_mut(), G_ABCS, buffer_size);

    // Rewinding should succeed.
    test_rewind(reporter, buffered_stream.as_mut(), true);

    // Once again reading buffered info should succeed
    test_read(reporter, buffered_stream.as_mut(), G_ABCS, buffer_size);

    // Read past the size of the buffer. At this point, we cannot return.
    let position = mem_stream.borrow().get_position();
    test_read(reporter, buffered_stream.as_mut(), &G_ABCS[position..], 1);
    test_rewind(reporter, buffered_stream.as_mut(), false);
}

// Port of: tests/FrontBufferedStreamTest.cpp#L110-L131 (chrome/m156)
fn test_skipping(reporter: &mut Reporter, buffer_size: usize) {
    let mem_stream = Rc::new(RefCell::new(*MemoryStream::make_direct(G_ABCS)));
    let mut buffered_stream =
        AndroidFrontBufferedStream::make(Some(Box::new(Shared(mem_stream.clone()))), buffer_size)
            .unwrap();
    test_has_length(reporter, buffered_stream.as_ref(), &*mem_stream.borrow());

    // Skip half the buffer.
    buffered_stream.skip(buffer_size / 2);

    // Rewind, then read part of the buffer, which should have been read.
    test_rewind(reporter, buffered_stream.as_mut(), true);
    test_read(reporter, buffered_stream.as_mut(), G_ABCS, buffer_size / 4);

    // Now skip beyond the buffered piece, but still within the total buffer.
    buffered_stream.skip(buffer_size / 2);

    // Test that reading will still work.
    let position = mem_stream.borrow().get_position();
    test_read(
        reporter,
        buffered_stream.as_mut(),
        &G_ABCS[position..],
        buffer_size / 4,
    );

    test_rewind(reporter, buffered_stream.as_mut(), true);
    test_read(reporter, buffered_stream.as_mut(), G_ABCS, buffer_size);
}

// A custom class whose isAtEnd behaves the way Android's stream does - since it is an adaptor to a
// Java InputStream, it does not know that it is at the end until it has attempted to read beyond
// the end and failed. Used by test_read_beyond_buffer.
// Port of: tests/FrontBufferedStreamTest.cpp#L136-L157 (chrome/m156)
struct AndroidLikeMemoryStream {
    base: MemoryStream,
    is_at_end: bool,
}

impl AndroidLikeMemoryStream {
    fn new(data: &'static [u8]) -> Self {
        Self {
            base: MemoryStream::from_data(Some(Data::new_static(data))),
            is_at_end: false,
        }
    }
}

impl Stream for AndroidLikeMemoryStream {
    fn read(&mut self, dst: &mut [u8]) -> usize {
        let requested = dst.len();
        let bytes_read = self.base.read(dst);
        if bytes_read < requested {
            self.is_at_end = true;
        }
        bytes_read
    }

    fn is_at_end(&self) -> bool {
        self.is_at_end
    }

    // The rest is `SkMemoryStream`'s.
    fn rewind(&mut self) -> bool {
        self.base.rewind()
    }

    fn has_position(&self) -> bool {
        self.base.has_position()
    }

    fn get_position(&self) -> usize {
        self.base.get_position()
    }

    fn seek(&mut self, position: usize) -> bool {
        self.base.seek(position)
    }

    fn move_by(&mut self, offset: i64) -> bool {
        self.base.move_by(offset)
    }

    fn has_length(&self) -> bool {
        self.base.has_length()
    }

    fn get_length(&self) -> usize {
        self.base.get_length()
    }
}

// This test ensures that buffering the exact length of the stream and attempting to read beyond it
// does not invalidate the buffer.
// Port of: tests/FrontBufferedStreamTest.cpp#L161-L177 (chrome/m156)
fn test_read_beyond_buffer(reporter: &mut Reporter, buffer_size: usize) {
    // Use a stream that behaves like Android's stream.
    let mem_stream = Rc::new(RefCell::new(AndroidLikeMemoryStream::new(
        &G_ABCS[..buffer_size],
    )));

    // Create a buffer that matches the length of the stream.
    let mut buffered_stream =
        AndroidFrontBufferedStream::make(Some(Box::new(Shared(mem_stream.clone()))), buffer_size)
            .unwrap();
    test_has_length(reporter, buffered_stream.as_ref(), &*mem_stream.borrow());

    // Attempt to read one more than the bufferSize
    test_read(reporter, buffered_stream.as_mut(), G_ABCS, buffer_size + 1);
    test_rewind(reporter, buffered_stream.as_mut(), true);

    // Ensure that the initial read did not invalidate the buffer.
    test_read(reporter, buffered_stream.as_mut(), G_ABCS, buffer_size);
}

// Mock stream that optionally has a length and/or position. Tests that FrontBufferedStream's
// length depends on the stream it's buffering having a length and position.
// Port of: tests/FrontBufferedStreamTest.cpp#L181-L207 (chrome/m156)
struct LengthOptionalStream {
    has_length: bool,
    has_position: bool,
}

impl Stream for LengthOptionalStream {
    fn has_length(&self) -> bool {
        self.has_length
    }

    fn has_position(&self) -> bool {
        self.has_position
    }

    fn read(&mut self, _buffer: &mut [u8]) -> usize {
        0
    }

    fn is_at_end(&self) -> bool {
        true
    }
}

// Test all possible combinations of the wrapped stream having a length and a position.
// Port of: tests/FrontBufferedStreamTest.cpp#L210-L220 (chrome/m156)
fn test_length_combos(reporter: &mut Reporter, buffer_size: usize) {
    for has_len in 0..=1 {
        for has_pos in 0..=1 {
            let stream = Rc::new(RefCell::new(LengthOptionalStream {
                has_length: has_len != 0,
                has_position: has_pos != 0,
            }));
            let buffered = AndroidFrontBufferedStream::make(
                Some(Box::new(Shared(stream.clone()))),
                buffer_size,
            )
            .unwrap();
            test_has_length(reporter, buffered.as_ref(), &*stream.borrow());
        }
    }
}

// Test using a stream with an initial offset.
// Port of: tests/FrontBufferedStreamTest.cpp#L223-L252 (chrome/m156)
fn test_initial_offset(reporter: &mut Reporter, buffer_size: usize) {
    let mut mem_stream = MemoryStream::from_data(Some(Data::new_static(G_ABCS)));

    // Skip a few characters into the memStream, so that bufferedStream represents an offset into
    // the stream it wraps.
    let arbitrary_offset = 17;
    mem_stream.skip(arbitrary_offset);
    let mem_stream = Rc::new(RefCell::new(mem_stream));
    let mut buffered_stream =
        AndroidFrontBufferedStream::make(Some(Box::new(Shared(mem_stream.clone()))), buffer_size)
            .unwrap();

    // Since MemoryStream has a length, bufferedStream must also.
    reporter_assert!(reporter, buffered_stream.has_length());

    let amount_to_read = 10;
    let buffered_length = buffered_stream.get_length();
    let mut current_position = 0;

    // Read the stream in chunks. After each read, the position must match currentPosition,
    // which sums the amount attempted to read, unless the end of the stream has been reached.
    // Importantly, the end should not have been reached until currentPosition == bufferedLength.
    while current_position < buffered_length {
        reporter_assert!(reporter, !buffered_stream.is_at_end());
        test_read(
            reporter,
            buffered_stream.as_mut(),
            &G_ABCS[arbitrary_offset + current_position..],
            amount_to_read,
        );
        current_position = (current_position + amount_to_read).min(buffered_length);
        let position = mem_stream.borrow().get_position();
        reporter_assert!(reporter, position - arbitrary_offset == current_position);
    }
    reporter_assert!(reporter, buffered_stream.is_at_end());
    reporter_assert!(reporter, buffered_length == current_position);
}

// Port of: tests/FrontBufferedStreamTest.cpp#L254-L261 (chrome/m156)
fn test_buffers(reporter: &mut Reporter, buffer_size: usize) {
    test_incremental_buffering(reporter, buffer_size);
    test_perfectly_sized_buffer(reporter, buffer_size);
    test_skipping(reporter, buffer_size);
    test_read_beyond_buffer(reporter, buffer_size);
    test_length_combos(reporter, buffer_size);
    test_initial_offset(reporter, buffer_size);
}

// Port of: tests/FrontBufferedStreamTest.cpp#L263-L268 (chrome/m156)
def_test!(FrontBufferedStream, |reporter| {
    // Test 6 and 64, which are used by Android, as well as another arbitrary length.
    test_buffers(reporter, 6);
    test_buffers(reporter, 15);
    test_buffers(reporter, 64);
});

// skia-rust: `ShortFrontBufferedStream` (tests/FrontBufferedStreamTest.cpp#L270-L301) is not
// ported: it needs `SkCodec::MakeFromStream`, and SkCodec is not ported yet (it stays `todo`).
