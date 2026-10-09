// Self-checks for the sampled Android decode (SkSampledCodec). These are not Skia tests: each one
// compares a sampled decode with the full decode of the same file. Output pixel (x, y) of a decode
// with sample factor s must equal the full decode's pixel (s/2 + s*x, s/2 + s*y), which is the first
// kept row and column (`SkCodecPriv::GetStartCoord`) and every s-th one after it. Subsets are chosen
// so their origin is a multiple of s, which keeps the mapping exact.
//
// The resources come from third_party/skia, which is absent from some checkouts; the tests skip
// themselves when it is missing.

use std::path::Path;

use skia_rust_codec::android_codec::{AndroidCodec, AndroidOptions};
use skia_rust_codec::codecs;
use skia_rust_codec::{Options, Result};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::MemoryStream;

fn read_resource(relative: &str) -> Option<Vec<u8>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/skia/resources");
    std::fs::read(dir.join(relative)).ok()
}

// The N32 pixels of the whole image, decoded without any scaling.
fn full_pixels(data: &[u8]) -> (ImageInfo, Vec<u8>) {
    let mut codec = codecs::make_codec_from_stream(MemoryStream::make_copy(data))
        .expect("the full decode must make a codec");
    let info = codec.info().with_color_type(ColorType::N32);
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    assert_eq!(
        codec.get_pixels(&info, &mut pixels, row_bytes, None),
        Result::Success
    );
    (info, pixels)
}

// The 4 bytes of the pixel at (x, y) of a buffer `row_bytes` apart.
fn pixel(pixels: &[u8], row_bytes: usize, x: i32, y: i32) -> [u8; 4] {
    // The coordinates checked here are inside the image, so they are never negative.
    let x = usize::try_from(x).expect("non-negative x");
    let y = usize::try_from(y).expect("non-negative y");
    let offset = y * row_bytes + x * 4;
    [
        pixels[offset],
        pixels[offset + 1],
        pixels[offset + 2],
        pixels[offset + 3],
    ]
}

// Decodes `path` with `sample` (and an optional subset) and checks every output pixel against the
// full decode at the sampled position.
fn check_sampled(path: &str, sample: i32, subset: Option<IRect>) {
    let Some(data) = read_resource(path) else {
        eprintln!("skipping: Skia resources are not present ({path})");
        return;
    };
    let (full_info, full) = full_pixels(&data);
    let full_row_bytes = full_info.min_row_bytes();

    let codec = codecs::make_codec_from_stream(MemoryStream::make_copy(&data))
        .expect("the sampled decode must make a codec");
    let mut android = AndroidCodec::make_from_codec(codec).expect("an Android codec");

    let options = AndroidOptions {
        base: Options {
            subset,
            ..Options::default()
        },
        sample_size: sample,
    };
    let out_size = match subset {
        None => android.get_sampled_dimensions(sample),
        Some(sub) => android.get_sampled_subset_dimensions(sample, sub),
    };
    assert!(
        out_size.width >= 1 && out_size.height >= 1,
        "{path}: empty output"
    );

    let info = android
        .info()
        .with_dimensions(ISize::new(out_size.width, out_size.height))
        .with_color_type(ColorType::N32);
    let row_bytes = info.min_row_bytes();
    let mut out = vec![0u8; info.compute_byte_size(row_bytes)];
    let result = android.get_android_pixels(&info, &mut out, row_bytes, Some(&options));
    assert_eq!(result, Result::Success, "{path}: sampled decode failed");

    // Output column x of a subset starts at subset.x / sample of the sampled image.
    let (origin_x, origin_y) = subset.map_or((0, 0), |s| (s.x() / sample, s.y() / sample));
    let start = sample / 2;
    for y in 0..out_size.height {
        for x in 0..out_size.width {
            let full_x = start + sample * (origin_x + x);
            let full_y = start + sample * (origin_y + y);
            assert_eq!(
                pixel(&out, row_bytes, x, y),
                pixel(&full, full_row_bytes, full_x, full_y),
                "{path}: sample {sample}, output ({x}, {y}) is full ({full_x}, {full_y})"
            );
        }
    }
}

#[test]
fn png_sampled_matches_full_decode() {
    check_sampled("images/plane.png", 5, None);
}

#[test]
fn png_interlaced_sampled_matches_full_decode() {
    check_sampled("images/plane_interlaced.png", 3, None);
}

#[test]
fn png_sampled_subset_matches_full_decode() {
    check_sampled(
        "images/plane.png",
        5,
        Some(IRect::from_xywh(10, 20, 100, 50)),
    );
}

#[test]
fn wbmp_sampled_matches_full_decode() {
    // WBMP has no incremental decode, so this takes the scanline path of SkSampledCodec.
    check_sampled("images/mandrill.wbmp", 4, None);
}

#[test]
fn bmp_rle_sampled_matches_full_decode() {
    check_sampled("images/rle.bmp", 2, None);
}

#[test]
fn bmp_standard_sampled_matches_full_decode() {
    check_sampled("images/randPixels.bmp", 2, None);
}

// The invariants of AndroidCodec_computeSampleSize (tests/AndroidCodecTest.cpp#L41-L133), for the
// formats that are ported. The Skia test also runs JPEG, WebP and GIF, which have no decoder yet.
// The float products are truncated to integers, as the C++ does; the dimensions are small.
#[test]
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn compute_sample_size_invariants_hold_for_ported_formats() {
    // Port of `times` in tests/AndroidCodecTest.cpp. The C++ truncates the float product too.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)] // dimensions are small
    fn times(size: ISize, factor: f32) -> ISize {
        ISize::new(
            (size.width as f32 * factor) as i32,
            (size.height as f32 * factor) as i32,
        )
    }
    // Port of `plus` in tests/AndroidCodecTest.cpp.
    fn plus(size: ISize, term: i32) -> ISize {
        ISize::new(size.width + term, size.height + term)
    }

    for path in [
        "images/plane.png",
        "images/mandrill.wbmp",
        "images/rle.bmp",
        "images/randPixels.bmp",
    ] {
        let Some(data) = read_resource(path) else {
            eprintln!("skipping: Skia resources are not present ({path})");
            continue;
        };
        let codec = codecs::make_codec_from_stream(MemoryStream::make_copy(&data))
            .expect("the codec must be made");
        let android = AndroidCodec::make_from_codec(codec).expect("an Android codec");
        let dims = android.info().dimensions();

        let downscales = [
            plus(dims, -1),
            times(dims, 0.15),
            times(dims, 0.6),
            ISize::new(
                (dims.width as f32 * 0.25) as i32,
                (dims.height as f32 * 0.75) as i32,
            ),
            ISize::new(1, 1),
            ISize::new(1, 2),
            ISize::new(2, 1),
            ISize::new(0, -1),
            ISize::new(dims.width, dims.height - 1),
        ];
        for requested in downscales {
            let mut size = requested;
            let computed_sample_size = android.compute_sample_size(&mut size);
            assert!(
                size.width >= 1 && size.height >= 1,
                "{path}: {requested:?} gave {size:?}"
            );
            if computed_sample_size == 1 {
                assert_eq!(size, dims, "{path}: {requested:?}");
            } else {
                assert!(computed_sample_size > 1, "{path}: {requested:?}");
                assert!(
                    size.width < dims.width && size.height < dims.height,
                    "{path}: {requested:?} computed {computed_sample_size} gave {size:?}"
                );
                assert!(
                    size.width >= requested.width && size.height >= requested.height,
                    "{path}: {requested:?} gave {size:?}"
                );
            }
        }
        // Upscales keep the original size and need no sampling.
        for requested in [dims, plus(dims, 5), times(dims, 2.0)] {
            let mut size = requested;
            assert_eq!(android.compute_sample_size(&mut size), 1, "{path}");
            assert_eq!(dims, size, "{path}");
        }

        // A client's size from getSampledDimensions must map back to itself.
        for sample_size in [1, 2, 3, 4, 8, 16, 32] {
            let sampled_dims = android.get_sampled_dimensions(sample_size);
            let mut size = sampled_dims;
            let _sample = android.compute_sample_size(&mut size);
            assert_eq!(sampled_dims, size, "{path}: sample size {sample_size}");
        }
    }
}
