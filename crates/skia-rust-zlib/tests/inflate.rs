// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: zlib test/example.c (inflate round trips), chromium zlib@646b7f56 (checked against the
// pinned C library; see the note in the crate docs).

use skia_rust_zlib::{Flush, Inflate, ReturnCode};

/// Inflates `input` in one call with an output buffer of `out_len` bytes.
fn inflate_once(window_bits: i32, input: &[u8], out_len: usize) -> (ReturnCode, usize, Vec<u8>) {
    let mut inflate = Inflate::new(window_bits).expect("valid window bits");
    let mut out = vec![0u8; out_len];
    let r = inflate.inflate(input, &mut out, Flush::NoFlush);
    out.truncate(r.produced);
    (r.ret, r.consumed, out)
}

#[test]
fn zlib_stream_round_trip() {
    // zlib.compress(b"hello world")
    let input = [
        0x78, 0x9c, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0x28, 0xcf, 0x2f, 0xca, 0x49, 0x01, 0x00,
        0x1a, 0x0b, 0x04, 0x5d,
    ];
    let mut inflate = Inflate::new(15).unwrap();
    let mut out = [0u8; 64];
    let r = inflate.inflate(&input, &mut out, Flush::Finish);
    assert_eq!(r.ret, ReturnCode::StreamEnd);
    assert_eq!(r.consumed, input.len());
    assert_eq!(&out[..r.produced], b"hello world");
    assert_eq!(inflate.total_out(), 11);
    assert_eq!(inflate.total_in(), input.len() as u64);
    // The Adler-32 of "hello world" is 0x1a0b045d.
    assert_eq!(inflate.adler(), 0x1a0b_045d);
}

#[test]
fn raw_deflate_stream() {
    // raw deflate (windowBits -15) of "hello hello hello hello"
    let input = [0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0xc8, 0x40, 0x27, 0x01];
    let (ret, consumed, out) = inflate_once(-15, &input, 100);
    assert_eq!(ret, ReturnCode::StreamEnd);
    assert_eq!(consumed, input.len());
    assert_eq!(out, b"hello hello hello hello");
}

#[test]
fn stored_block() {
    // zlib.compress(b"stored block", 0)
    let input = [
        0x78, 0x01, 0x01, 0x0c, 0x00, 0xf3, 0xff, 0x73, 0x74, 0x6f, 0x72, 0x65, 0x64, 0x20, 0x62,
        0x6c, 0x6f, 0x63, 0x6b, 0x1f, 0x80, 0x04, 0xbd,
    ];
    let (ret, _, out) = inflate_once(15, &input, 64);
    assert_eq!(ret, ReturnCode::StreamEnd);
    assert_eq!(out, b"stored block");
}

#[test]
fn truncated_input_produces_the_complete_prefix_and_no_error() {
    // The stream with its last byte removed: the checksum is missing, so the call returns Ok (more
    // input needed) and every byte of data is already out, as zlib does.
    let input = [
        0x78, 0x9c, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0x28, 0xcf, 0x2f, 0xca, 0x49, 0x01, 0x00,
        0x1a, 0x0b, 0x04,
    ];
    let (ret, consumed, out) = inflate_once(15, &input, 64);
    assert_eq!(ret, ReturnCode::Ok);
    assert_eq!(consumed, input.len());
    assert_eq!(out, b"hello world");
}

#[test]
fn corrupt_checksum_is_a_data_error() {
    let input = [
        0x78, 0x9c, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0x28, 0xcf, 0x2f, 0xca, 0x49, 0x01, 0x00,
        0x1a, 0x0b, 0x04, 0x5c,
    ];
    let mut inflate = Inflate::new(15).unwrap();
    let mut out = [0u8; 64];
    let r = inflate.inflate(&input, &mut out, Flush::NoFlush);
    assert_eq!(r.ret, ReturnCode::DataError);
    assert_eq!(inflate.msg(), Some("incorrect data check"));
    assert_eq!(&out[..r.produced], b"hello world");
}

#[test]
fn invalid_window_bits_are_rejected() {
    assert!(Inflate::new(7).is_err());
    assert!(Inflate::new(16).is_err(), "gzip framing is not ported");
    assert!(Inflate::new(-16).is_err());
    assert!(Inflate::new(0).is_ok());
}
