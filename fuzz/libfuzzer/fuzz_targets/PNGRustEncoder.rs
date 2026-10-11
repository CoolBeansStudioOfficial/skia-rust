// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! libFuzzer entry point for Skia's `PNGRustEncoder` fuzzer (docs/design/fuzz.md §4).

#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    skia_rust_fuzz::run("PNGRustEncoder", data).expect("registered fuzzer");
});
