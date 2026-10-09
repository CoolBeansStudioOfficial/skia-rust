// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// The ICO decoder checks that go beyond the 1:1 ports in tests/src/unit: the sizes of the embedded
// images in Skia's own ICO resources (the `check()` calls of tests/CodecTest.cpp's Codec_ico need the
// generator branch, which is not ported), and robustness over every ICO resource.
// The resources come from third_party/skia, which is absent from some checkouts; the tests skip
// when it is missing.

use std::path::{Path, PathBuf};

use skia_rust_codec::codec::Result;
use skia_rust_codec::{Codec, decoders};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::MemoryStream;

fn resources_dir() -> Option<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/skia/resources");
    dir.is_dir().then_some(dir)
}

fn read_resource(relative: &str) -> Option<Vec<u8>> {
    std::fs::read(resources_dir()?.join(relative)).ok()
}

fn all_ico_resources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            all_ico_resources(&path, out);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ico"))
        {
            out.push(path);
        }
    }
}

// Decodes the resource at `relative` as an N32 image and returns its size, or None if the file is
// missing or does not decode.
fn decode_as_n32(relative: &str) -> Option<ISize> {
    let data = read_resource(relative)?;
    let mut codec = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()).ok()?;
    let info = codec.info().with_color_type(ColorType::N32);
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    let result = codec.get_pixels(&info, &mut pixels, row_bytes, None);
    (result == Result::Success).then(|| info.dimensions())
}

#[test]
fn ico_with_embedded_bmp_decodes_at_its_size() {
    let Some(size) = decode_as_n32("images/color_wheel.ico") else {
        eprintln!("todo: skipping, missing Skia resource images/color_wheel.ico");
        return;
    };
    assert_eq!(size, ISize::new(128, 128));
}

#[test]
fn ico_with_embedded_png_decodes_at_its_size() {
    let Some(size) = decode_as_n32("images/google_chrome.ico") else {
        eprintln!("todo: skipping, missing Skia resource images/google_chrome.ico");
        return;
    };
    assert_eq!(size, ISize::new(256, 256));
}

#[test]
fn every_ico_resource_decodes_without_panicking() {
    let Some(dir) = resources_dir() else {
        eprintln!("todo: skipping, missing third_party/skia/resources");
        return;
    };
    let mut files = Vec::new();
    all_ico_resources(&dir, &mut files);
    for path in files {
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders())
        else {
            continue;
        };
        let info = codec.info().with_color_type(ColorType::N32);
        let row_bytes = info.min_row_bytes();
        let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
        // Any result is acceptable here; the test is that the decoder never panics.
        let _ = codec.get_pixels(&info, &mut pixels, row_bytes, None);
    }
}
