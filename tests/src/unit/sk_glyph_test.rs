// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkGlyphTest.cpp (chrome/m156), the glyph rect cases and the glyph serialization
// cases

#![cfg(test)]

use skia_rust_core::glyph::skglyph::{empty_rect, full_rect, rect_intersection, rect_union};
use skia_rust_core::glyph::{Glyph, GlyphRect};
use skia_rust_core::mask::MaskFormat;
use skia_rust_core::packed_glyph_id::PackedGlyphId;
use skia_rust_core::path::Path;
use skia_rust_core::point::Point;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::rect::Rect;
use skia_rust_core::write_buffer::BinaryWriteBuffer;

use crate::{def_test, reporter_assert};

/// Converts a small integer test coordinate to a scalar. The values are well inside f32's exact
/// integer range.
#[allow(clippy::cast_precision_loss)] // test coordinates are small integers, exact in f32
fn sc(v: i32) -> f32 {
    v as f32
}

// Port of: tests/SkGlyphTest.cpp#L36-L64 (chrome/m156)
def_test!(
    #[allow(clippy::float_cmp)] // exact comparisons, as in the C++ test
    SkGlyphRectBasic,
    |reporter| {
        let r = GlyphRect::new(1.0, 1.0, 10.0, 10.0);
        reporter_assert!(reporter, !r.is_empty());

        let mut a = rect_union(r, empty_rect());
        reporter_assert!(reporter, a.rect() == Rect::from_ltrb(1.0, 1.0, 10.0, 10.0));
        let width_height = a.width_height();
        reporter_assert!(reporter, width_height.x == 9.0 && width_height.y == 9.0);

        a = rect_intersection(r, full_rect());
        reporter_assert!(reporter, a.rect() == Rect::from_ltrb(1.0, 1.0, 10.0, 10.0));

        let mut acc = full_rect();
        for x in -10..10 {
            for y in -10..10 {
                acc = rect_intersection(acc, GlyphRect::new(sc(x), sc(y), sc(x + 20), sc(y + 20)));
            }
        }
        reporter_assert!(
            reporter,
            acc.rect() == Rect::from_ltrb(9.0, 9.0, 10.0, 10.0)
        );

        acc = empty_rect();
        for x in -10..10 {
            for y in -10..10 {
                acc = rect_union(acc, GlyphRect::new(sc(x), sc(y), sc(x + 20), sc(y + 20)));
            }
        }
        reporter_assert!(
            reporter,
            acc.rect() == Rect::from_ltrb(-10.0, -10.0, 29.0, 29.0)
        );
    }
);

/// `SkGlyphTestPeer::SetGlyph1`: the metrics of the first test glyph.
fn set_glyph1(glyph: &mut Glyph) {
    glyph.set_metrics_for_testing(Point::new(10.0, 11.0), -1, -2, 8, 9, MaskFormat::A8);
}

/// `SkGlyphTestPeer::SetGlyph2`: the metrics of the second test glyph.
fn set_glyph2(glyph: &mut Glyph) {
    glyph.set_metrics_for_testing(Point::new(10.0, 11.0), 0, -1, 8, 9, MaskFormat::A8);
}

/// The image of the test glyph, one row of 8 bytes per line (`imageData`).
const GLYPH_IMAGE: [[u8; 8]; 9] = {
    const X: u8 = 0xff;
    const O: u8 = 0x00;
    [
        [X, X, X, X, X, X, X, X],
        [X, O, O, O, O, O, O, X],
        [X, O, O, O, O, O, O, X],
        [X, O, O, O, O, O, O, X],
        [X, O, O, X, X, O, O, X],
        [X, O, O, O, O, O, O, X],
        [X, O, O, O, O, O, O, X],
        [X, O, O, O, O, O, O, X],
        [X, X, X, X, X, X, X, X],
    ]
};

// Port of: tests/SkGlyphTest.cpp#L88-L129 (chrome/m156), SkGlyph_SendMetrics
def_test!(SkGlyph_SendMetrics, |reporter| {
    {
        let mut src_glyph = Glyph::new(PackedGlyphId::from_glyph_id(12));
        set_glyph1(&mut src_glyph);

        let mut write_buffer = BinaryWriteBuffer::new();
        src_glyph.flatten_metrics(&mut write_buffer);

        let data = write_buffer.snapshot_as_data();
        let mut read_buffer = ReadBuffer::new(data.as_bytes());
        let dst_glyph = Glyph::make_from_buffer(&mut read_buffer);
        reporter_assert!(reporter, read_buffer.is_valid());
        reporter_assert!(reporter, dst_glyph.is_some());
        if let Some(dst_glyph) = dst_glyph {
            reporter_assert!(
                reporter,
                src_glyph.advance_vector() == dst_glyph.advance_vector()
            );
            reporter_assert!(reporter, src_glyph.rect() == dst_glyph.rect());
            reporter_assert!(reporter, src_glyph.mask_format() == dst_glyph.mask_format());
        }
    }
    {
        let mut src_glyph = Glyph::new(PackedGlyphId::from_glyph_id(12));
        set_glyph2(&mut src_glyph);

        let mut write_buffer = BinaryWriteBuffer::new();
        src_glyph.flatten_metrics(&mut write_buffer);

        let data = write_buffer.snapshot_as_data();
        let mut read_buffer = ReadBuffer::new(data.as_bytes());
        let dst_glyph = Glyph::make_from_buffer(&mut read_buffer);
        reporter_assert!(reporter, read_buffer.is_valid());
        reporter_assert!(reporter, dst_glyph.is_some());
        if let Some(dst_glyph) = dst_glyph {
            reporter_assert!(
                reporter,
                src_glyph.advance_vector() == dst_glyph.advance_vector()
            );
            reporter_assert!(reporter, src_glyph.rect() == dst_glyph.rect());
            reporter_assert!(reporter, src_glyph.mask_format() == dst_glyph.mask_format());
        }
    }

    let bad_data: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
    let mut bad_buffer = ReadBuffer::new(&bad_data);
    let dst_glyph = Glyph::make_from_buffer(&mut bad_buffer);
    reporter_assert!(reporter, !bad_buffer.is_valid());
    reporter_assert!(reporter, dst_glyph.is_none());
});

// Port of: tests/SkGlyphTest.cpp#L131-L194 (chrome/m156), SkGlyph_SendWithImage
def_test!(SkGlyph_SendWithImage, |reporter| {
    let mut src_glyph = Glyph::new(PackedGlyphId::from_glyph_id(12));
    set_glyph1(&mut src_glyph);

    let image: Box<[u8]> = GLYPH_IMAGE.iter().flatten().copied().collect();
    src_glyph.set_image(image);

    let mut write_buffer = BinaryWriteBuffer::new();
    src_glyph.flatten_metrics(&mut write_buffer);
    src_glyph.flatten_image(&mut write_buffer);

    let data = write_buffer.snapshot_as_data();
    let mut read_buffer = ReadBuffer::new(data.as_bytes());
    let dst_glyph = Glyph::make_from_buffer(&mut read_buffer);
    reporter_assert!(reporter, read_buffer.is_valid());
    reporter_assert!(reporter, dst_glyph.is_some());
    if let Some(mut dst_glyph) = dst_glyph {
        reporter_assert!(
            reporter,
            src_glyph.advance_vector() == dst_glyph.advance_vector()
        );
        reporter_assert!(reporter, src_glyph.rect() == dst_glyph.rect());
        reporter_assert!(reporter, src_glyph.mask_format() == dst_glyph.mask_format());

        dst_glyph.add_image_from_buffer(&mut read_buffer);
        reporter_assert!(reporter, read_buffer.is_valid());
        let row_bytes = dst_glyph.row_bytes();
        let dst_image = dst_glyph.image().unwrap_or(&[]);
        for (y, row) in GLYPH_IMAGE
            .iter()
            .enumerate()
            .take(usize::from(dst_glyph.height()))
        {
            for (x, expected) in row.iter().enumerate().take(usize::from(dst_glyph.width())) {
                reporter_assert!(reporter, dst_image.get(y * row_bytes + x) == Some(expected));
            }
        }
    }

    // Add good metrics, but mess up image data
    let mut bad_write_buffer = BinaryWriteBuffer::new();
    src_glyph.flatten_metrics(&mut bad_write_buffer);
    bad_write_buffer.write_int(7);
    bad_write_buffer.write_int(8);

    let data = bad_write_buffer.snapshot_as_data();
    let mut bad_read_buffer = ReadBuffer::new(data.as_bytes());
    let dst_glyph = Glyph::make_from_buffer(&mut bad_read_buffer);
    reporter_assert!(reporter, bad_read_buffer.is_valid()); // Reading glyph metrics is okay.
    reporter_assert!(reporter, dst_glyph.is_some());
    if let Some(mut dst_glyph) = dst_glyph {
        reporter_assert!(
            reporter,
            src_glyph.advance_vector() == dst_glyph.advance_vector()
        );
        reporter_assert!(reporter, src_glyph.rect() == dst_glyph.rect());
        reporter_assert!(reporter, src_glyph.mask_format() == dst_glyph.mask_format());

        dst_glyph.add_image_from_buffer(&mut bad_read_buffer);
        reporter_assert!(reporter, !bad_read_buffer.is_valid());
        reporter_assert!(reporter, !dst_glyph.set_image_has_been_called());
    }
});

// Port of: tests/SkGlyphTest.cpp#L196-L265 (chrome/m156), SkGlyph_SendWithPath
def_test!(SkGlyph_SendWithPath, |reporter| {
    let mut src_glyph = Glyph::new(PackedGlyphId::from_glyph_id(12));
    set_glyph1(&mut src_glyph);

    let src_path = Path::rect(src_glyph.rect(), None);
    src_glyph.set_path(Some(src_path.clone()), false, false);

    let mut write_buffer = BinaryWriteBuffer::new();
    src_glyph.flatten_metrics(&mut write_buffer);
    src_glyph.flatten_path(&mut write_buffer);

    let data = write_buffer.snapshot_as_data();
    let mut read_buffer = ReadBuffer::new(data.as_bytes());
    let dst_glyph = Glyph::make_from_buffer(&mut read_buffer);
    reporter_assert!(reporter, read_buffer.is_valid());
    reporter_assert!(reporter, dst_glyph.is_some());
    if let Some(mut dst_glyph) = dst_glyph {
        reporter_assert!(
            reporter,
            src_glyph.advance_vector() == dst_glyph.advance_vector()
        );
        reporter_assert!(reporter, src_glyph.rect() == dst_glyph.rect());
        reporter_assert!(reporter, src_glyph.mask_format() == dst_glyph.mask_format());

        dst_glyph.add_path_from_buffer(&mut read_buffer);
        reporter_assert!(reporter, read_buffer.is_valid());
        reporter_assert!(reporter, dst_glyph.set_path_has_been_called());
        reporter_assert!(reporter, dst_glyph.path() == Some(&src_path));
    }

    {
        // Add good metrics, but mess up path data
        let mut bad_write_buffer = BinaryWriteBuffer::new();
        src_glyph.flatten_metrics(&mut bad_write_buffer);
        // Force a false value to be read in addPathFromBuffer for hasPath.
        bad_write_buffer.write_int(8);
        bad_write_buffer.write_int(9);

        let data = bad_write_buffer.snapshot_as_data();
        let mut bad_read_buffer = ReadBuffer::new(data.as_bytes());
        let dst_glyph = Glyph::make_from_buffer(&mut bad_read_buffer);
        reporter_assert!(reporter, dst_glyph.is_some());
        if let Some(mut dst_glyph) = dst_glyph {
            reporter_assert!(
                reporter,
                src_glyph.advance_vector() == dst_glyph.advance_vector()
            );
            reporter_assert!(reporter, src_glyph.rect() == dst_glyph.rect());
            reporter_assert!(reporter, src_glyph.mask_format() == dst_glyph.mask_format());

            dst_glyph.add_path_from_buffer(&mut bad_read_buffer);
            reporter_assert!(reporter, !bad_read_buffer.is_valid());
            reporter_assert!(reporter, !dst_glyph.set_path_has_been_called());
        }
    }
    {
        // Add good metrics, but no path data.
        let mut bad_write_buffer = BinaryWriteBuffer::new();
        src_glyph.flatten_metrics(&mut bad_write_buffer);

        let data = bad_write_buffer.snapshot_as_data();
        let mut bad_read_buffer = ReadBuffer::new(data.as_bytes());
        let dst_glyph = Glyph::make_from_buffer(&mut bad_read_buffer);
        // Reading glyph metrics is okay.
        reporter_assert!(reporter, bad_read_buffer.is_valid());
        reporter_assert!(reporter, dst_glyph.is_some());
        if let Some(mut dst_glyph) = dst_glyph {
            reporter_assert!(
                reporter,
                src_glyph.advance_vector() == dst_glyph.advance_vector()
            );
            reporter_assert!(reporter, src_glyph.rect() == dst_glyph.rect());
            reporter_assert!(reporter, src_glyph.mask_format() == dst_glyph.mask_format());

            dst_glyph.add_path_from_buffer(&mut bad_read_buffer);
            reporter_assert!(reporter, !bad_read_buffer.is_valid());
            reporter_assert!(reporter, !dst_glyph.set_path_has_been_called());
        }
    }
});
