// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PixelsRecTest.cpp (chrome/m156)
//
// skia-rust: `rec.fPixels` is a pointer that `trim` advances; the Rust recs keep the buffer and
// the byte `offset` of `fPixels` from its start, so `rec.fPixels == pixels` is `rec.offset == 0`
// and `rec.fPixels == (char*)pixels + n` is `rec.offset == n`. `skiatest::ReporterContext` only
// labels failures with the sub-test name: the name is attached to each assertion message.

#![cfg(test)]

use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::read_pixels_rec::ReadPixelsRec;
use skia_rust_core::write_pixels_rec::WritePixelsRec;

use crate::{def_test, reporter_assert};

// Port of: tests/PixelsRecTest.cpp#L18-L113 (chrome/m156)
def_test!(ReadPixelsRec_trim, |reporter| {
    const W: i32 = 100;
    const H: i32 = 100;
    let info = ImageInfo::new_n32_premul((W, H), None);
    let row_bytes = info.min_row_bytes();
    let mut storage = vec![0u8; H as usize * row_bytes];

    {
        let name = "Normal valid trim";
        let mut rec = ReadPixelsRec::new(&info, Some(&mut storage), row_bytes, 0, 0);
        reporter_assert!(reporter, rec.trim(W, H), "{name}");
        reporter_assert!(reporter, rec.offset == 0, "{name}");
        reporter_assert!(reporter, rec.info.width() == W, "{name}");
        reporter_assert!(reporter, rec.info.height() == H, "{name}");
    }

    {
        let name = "Trim with negative x, y";
        let mut rec = ReadPixelsRec::new(&info, Some(&mut storage), row_bytes, -10, -10);
        reporter_assert!(reporter, rec.trim(W, H), "{name}");
        // fPixels should be adjusted: pixels + 10 * rowBytes + 10 * 4
        reporter_assert!(
            reporter,
            rec.offset == 10 * row_bytes + 10 * info.bytes_per_pixel(),
            "{name}"
        );
        reporter_assert!(reporter, rec.info.width() == W - 10, "{name}");
        reporter_assert!(reporter, rec.info.height() == H - 10, "{name}");
        reporter_assert!(reporter, rec.x == 0, "{name}");
        reporter_assert!(reporter, rec.y == 0, "{name}");
    }

    {
        let name = "Trim with x, y partially outside";
        let mut rec = ReadPixelsRec::new(&info, Some(&mut storage), row_bytes, 50, 50);
        reporter_assert!(reporter, rec.trim(W, H), "{name}");
        reporter_assert!(reporter, rec.offset == 0, "{name}");
        reporter_assert!(reporter, rec.info.width() == 50, "{name}");
        reporter_assert!(reporter, rec.info.height() == 50, "{name}");
        reporter_assert!(reporter, rec.x == 50, "{name}");
        reporter_assert!(reporter, rec.y == 50, "{name}");
    }

    {
        let name = "Trim with x, y completely outside (positive)";
        let mut rec = ReadPixelsRec::new(&info, Some(&mut storage), row_bytes, 150, 150);
        reporter_assert!(reporter, !rec.trim(W, H), "{name}");
    }

    {
        let name = "Trim with x, y completely outside (negative)";
        let mut rec = ReadPixelsRec::new(&info, Some(&mut storage), row_bytes, -150, -150);
        reporter_assert!(reporter, !rec.trim(W, H), "{name}");
    }

    {
        let name = "Explicitly trigger y_offset overflow";
        // arbitrarily large value that will overflow when multiplied by hugeY
        let huge_row_bytes = (i32::MAX as usize) >> 13;
        let huge_y = i32::MIN;
        let mut rec = ReadPixelsRec::new(&info, Some(&mut storage), huge_row_bytes, 0, huge_y);
        reporter_assert!(reporter, !rec.trim(W, H), "{name}");
    }

    {
        if size_of::<usize>() == 4 {
            let name = "Explicitly trigger x_offset overflow";
            let huge_x = i32::MIN;
            let mut rec = ReadPixelsRec::new(&info, Some(&mut storage), row_bytes, huge_x, 0);
            reporter_assert!(reporter, !rec.trim(W, H), "{name}");
        }
    }

    {
        let name = "Explicitly trigger total overflow";
        let mut rec = ReadPixelsRec::new(&info, Some(&mut storage), usize::MAX, -1, -1);
        reporter_assert!(reporter, !rec.trim(W, H), "{name}");
    }

    {
        let name = "Trim with large rowBytes (initial check)";
        let huge_info = ImageInfo::new_n32_premul((1 << 30, 1), None);
        let mut rec = ReadPixelsRec::new(&huge_info, Some(&mut storage), 100, 0, 0);
        reporter_assert!(reporter, !rec.trim(W, H), "{name}");
    }

    {
        let name = "Invalid ImageInfo (width <= 0)";
        let mut rec = ReadPixelsRec::new(
            &ImageInfo::new_n32_premul((-1, 100), None),
            Some(&mut storage),
            row_bytes,
            0,
            0,
        );
        reporter_assert!(reporter, !rec.trim(W, H), "{name}");
    }

    {
        let name = "rowBytes too small";
        let mut rec = ReadPixelsRec::new(&info, Some(&mut storage), info.min_row_bytes() - 1, 0, 0);
        reporter_assert!(reporter, !rec.trim(W, H), "{name}");
    }
});

// Port of: tests/PixelsRecTest.cpp#L115-L210 (chrome/m156)
def_test!(WritePixelsRec_trim, |reporter| {
    const W: i32 = 100;
    const H: i32 = 100;
    let info = ImageInfo::new_n32_premul((W, H), None);
    let row_bytes = info.min_row_bytes();
    let storage = vec![0u8; H as usize * row_bytes];
    let pixels: &[u8] = &storage;

    {
        let name = "Normal valid trim";
        let mut rec = WritePixelsRec::new(&info, Some(pixels), row_bytes, 0, 0);
        reporter_assert!(reporter, rec.trim(W, H), "{name}");
        reporter_assert!(reporter, rec.offset == 0, "{name}");
        reporter_assert!(reporter, rec.info.width() == W, "{name}");
        reporter_assert!(reporter, rec.info.height() == H, "{name}");
    }

    {
        let name = "Trim with negative x, y";
        let mut rec = WritePixelsRec::new(&info, Some(pixels), row_bytes, -10, -10);
        reporter_assert!(reporter, rec.trim(W, H), "{name}");
        // fPixels should be adjusted: pixels + 10 * rowBytes + 10 * 4
        reporter_assert!(
            reporter,
            rec.offset == 10 * row_bytes + 10 * info.bytes_per_pixel(),
            "{name}"
        );
        reporter_assert!(reporter, rec.info.width() == W - 10, "{name}");
        reporter_assert!(reporter, rec.info.height() == H - 10, "{name}");
        reporter_assert!(reporter, rec.x == 0, "{name}");
        reporter_assert!(reporter, rec.y == 0, "{name}");
    }

    {
        let name = "Trim with x, y partially outside";
        let mut rec = WritePixelsRec::new(&info, Some(pixels), row_bytes, 50, 50);
        reporter_assert!(reporter, rec.trim(W, H), "{name}");
        reporter_assert!(reporter, rec.offset == 0, "{name}");
        reporter_assert!(reporter, rec.info.width() == 50, "{name}");
        reporter_assert!(reporter, rec.info.height() == 50, "{name}");
        reporter_assert!(reporter, rec.x == 50, "{name}");
        reporter_assert!(reporter, rec.y == 50, "{name}");
    }

    {
        let name = "Trim with x, y completely outside (positive)";
        let mut rec = WritePixelsRec::new(&info, Some(pixels), row_bytes, 150, 150);
        reporter_assert!(reporter, !rec.trim(W, H), "{name}");
    }

    {
        let name = "Trim with x, y completely outside (negative)";
        let mut rec = WritePixelsRec::new(&info, Some(pixels), row_bytes, -150, -150);
        reporter_assert!(reporter, !rec.trim(W, H), "{name}");
    }

    {
        let name = "Explicitly trigger y_offset overflow";
        // arbitrarily large value that will overflow when multiplied by hugeY
        let huge_row_bytes = (i32::MAX as usize) >> 13;
        let huge_y = i32::MIN;
        let mut rec = WritePixelsRec::new(&info, Some(pixels), huge_row_bytes, 0, huge_y);
        reporter_assert!(reporter, !rec.trim(W, H), "{name}");
    }

    {
        if size_of::<usize>() == 4 {
            let name = "Explicitly trigger x_offset overflow";
            let huge_x = i32::MIN;
            let mut rec = WritePixelsRec::new(&info, Some(pixels), row_bytes, huge_x, 0);
            reporter_assert!(reporter, !rec.trim(W, H), "{name}");
        }
    }

    {
        let name = "Explicitly trigger total overflow";
        let mut rec = WritePixelsRec::new(&info, Some(pixels), usize::MAX, -1, -1);
        reporter_assert!(reporter, !rec.trim(W, H), "{name}");
    }

    {
        let name = "Trim with large rowBytes (initial check)";
        let huge_info = ImageInfo::new_n32_premul((1 << 30, 1), None);
        let mut rec = WritePixelsRec::new(&huge_info, Some(pixels), 100, 0, 0);
        reporter_assert!(reporter, !rec.trim(W, H), "{name}");
    }

    {
        let name = "Invalid ImageInfo (width <= 0)";
        let mut rec = WritePixelsRec::new(
            &ImageInfo::new_n32_premul((-1, 100), None),
            Some(pixels),
            row_bytes,
            0,
            0,
        );
        reporter_assert!(reporter, !rec.trim(W, H), "{name}");
    }

    {
        let name = "rowBytes too small";
        let mut rec = WritePixelsRec::new(&info, Some(pixels), info.min_row_bytes() - 1, 0, 0);
        reporter_assert!(reporter, !rec.trim(W, H), "{name}");
    }
});
