// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// The PNG decoder checks that go beyond the 1:1 ports in tests/src/unit: the expectations of
// tests/CodecTest.cpp's Codec_png_plte_trns cases, decoded with `get_pixels` (the C++ test goes
// through `getImage`, which is not ported yet), and robustness over every Skia PNG resource.
// The resources come from third_party/skia, which is absent from some checkouts; the tests skip
// when it is missing.

use std::path::{Path, PathBuf};

use skia_rust_codec::codec::Result;
use skia_rust_codec::png_codec::make_from_stream;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::stream::MemoryStream;

fn resources_dir() -> Option<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/skia/resources");
    dir.is_dir().then_some(dir)
}

fn read_resource(relative: &str) -> Option<Vec<u8>> {
    std::fs::read(resources_dir()?.join(relative)).ok()
}

fn all_png_resources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            all_png_resources(&path, out);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        {
            out.push(path);
        }
    }
}

// The first pixel of a decode, as it lies in memory. Port of decodeSingleRawPixelAsUint32 for the
// case where the pixel is read with `get_pixels`.
fn first_pixel(file: &[u8], color: ColorType, alpha: AlphaType) -> Option<[u8; 4]> {
    let mut codec = make_from_stream(MemoryStream::make_copy(file)).ok()?;
    let info = ImageInfo::new(
        codec.dimensions(),
        color,
        alpha,
        Some(ColorSpace::new_srgb()),
    );
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    if codec.get_pixels(&info, &mut pixels, row_bytes, None) != Result::Success {
        return None;
    }
    Some([pixels[0], pixels[1], pixels[2], pixels[3]])
}

// Port of the expectations of tests/CodecTest.cpp#L706-L717 (Codec_png_plte_trns): the palette
// colour (100, 150, 200) with a 25% alpha from tRNS, unpremultiplied and premultiplied.
#[test]
fn png_plte_trns_pixels() {
    let Some(file) = read_resource("images/plte_trns.png") else {
        eprintln!("skipping: Skia resources are not present");
        return;
    };
    assert_eq!(
        first_pixel(&file, ColorType::RGBA8888, AlphaType::Unpremul),
        Some([100, 150, 200, 64])
    );
    assert_eq!(
        first_pixel(&file, ColorType::BGRA8888, AlphaType::Unpremul),
        Some([200, 150, 100, 64])
    );
    assert_eq!(
        first_pixel(&file, ColorType::RGBA8888, AlphaType::Premul),
        Some([25, 38, 50, 64])
    );
    assert_eq!(
        first_pixel(&file, ColorType::BGRA8888, AlphaType::Premul),
        Some([50, 38, 25, 64])
    );
}

// Port of the expectations of tests/CodecTest.cpp#L719-L732 (Codec_png_plte_trns_gama): the same
// palette with a gAMA chunk, converted to sRGB by the colour transform.
#[test]
fn png_plte_trns_gama_pixels() {
    let Some(file) = read_resource("images/plte_trns_gama.png") else {
        eprintln!("skipping: Skia resources are not present");
        return;
    };
    assert_eq!(
        first_pixel(&file, ColorType::RGBA8888, AlphaType::Unpremul),
        Some([161, 197, 227, 64])
    );
    assert_eq!(
        first_pixel(&file, ColorType::BGRA8888, AlphaType::Unpremul),
        Some([227, 197, 161, 64])
    );
    assert_eq!(
        first_pixel(&file, ColorType::RGBA8888, AlphaType::Premul),
        Some([40, 49, 57, 64])
    );
    assert_eq!(
        first_pixel(&file, ColorType::BGRA8888, AlphaType::Premul),
        Some([57, 49, 40, 64])
    );
}

// Every PNG resource decodes without a panic, both whole and from truncated prefixes. An
// incremental decode of the whole file gives the same pixels as a one-shot decode.
#[test]
fn every_png_resource_decodes_without_panicking() {
    let Some(dir) = resources_dir() else {
        eprintln!("skipping: Skia resources are not present");
        return;
    };
    let mut files = Vec::new();
    all_png_resources(&dir, &mut files);
    assert!(!files.is_empty(), "no PNG resources found");
    files.sort();

    for path in &files {
        let data = std::fs::read(path).expect("read resource");
        let len = data.len();
        let prefixes = [
            len,
            len / 10,
            len / 4,
            len / 2,
            (len * 3) / 4,
            len.saturating_sub(12),
            len.saturating_sub(1),
            41,
            8,
            0,
        ];
        for cut in prefixes {
            let truncated = &data[..cut.min(len)];
            let Ok(mut codec) = make_from_stream(MemoryStream::make_copy(truncated)) else {
                continue;
            };
            let dims = codec.dimensions();
            let info = ImageInfo::new(
                dims,
                ColorType::RGBA8888,
                AlphaType::Unpremul,
                None::<ColorSpace>,
            );
            let row_bytes = info.min_row_bytes();
            let mut one_shot = vec![0u8; info.compute_byte_size(row_bytes)];
            let result = codec.get_pixels(&info, &mut one_shot, row_bytes, None);
            // The outcomes a PNG decode can report: a palette without a PLTE chunk is InvalidInput.
            assert!(
                matches!(
                    result,
                    Result::Success
                        | Result::IncompleteInput
                        | Result::ErrorInInput
                        | Result::InvalidInput
                ),
                "{}: unexpected result {result:?} at {cut} bytes",
                path.display()
            );
            if cut != len {
                continue;
            }
            // The incremental decode of the whole file must match the one-shot decode.
            let mut incremental_pixels = vec![0u8; info.compute_byte_size(row_bytes)];
            let mut codec = make_from_stream(MemoryStream::make_copy(&data))
                .expect("a codec for the whole file");
            // A decode that the one-shot path rejects at setup (for example a palette without a
            // PLTE chunk) is rejected by the start call too.
            let incremental_result = match codec.start_incremental_decode(
                &info,
                &mut incremental_pixels,
                row_bytes,
                None,
            ) {
                Ok(mut guard) => guard.incremental_decode().0,
                Err(start_result) => start_result,
            };
            assert_eq!(
                incremental_result,
                result,
                "{}: incremental result differs",
                path.display()
            );
            if result == Result::Success {
                assert!(
                    incremental_pixels == one_shot,
                    "{}: incremental pixels differ",
                    path.display()
                );
            }
        }
    }
}
