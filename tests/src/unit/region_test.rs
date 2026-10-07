// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RegionTest.cpp (chrome/m156)

#![cfg(test)]

// Not ported (they need `SkRegion::setPath` / `SkPath` / `SkRRect` / scan conversion, which are
// not ported yet); their manifest entries stay `todo`:
//   - `Region` (its `test_empties` calls `setPath`, so the whole test waits for it)
//   - `region_inverse_union_skbug_7491`
//   - `giant_path_region`
//   - `rrect_region_crbug_850350`
//   - `region_very_large`
//   - `region_b510359475`

use skia_rust_core::rect::IRect;
use skia_rust_core::region::{Cliperator, Iterator, Op, Region, Spanerator};

use crate::{def_test, reporter_assert};

fn union(rgn: &mut Region, rect: &IRect) {
    rgn.op_rect(rect, Op::Union);
}

/// Serializes `data` as native-endian bytes, like passing a `const int32_t[]` to `readFromMemory`.
fn as_bytes(data: &[i32]) -> Vec<u8> {
    data.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

// Test that writeToMemory reports the same number of bytes whether there was a
// buffer to write to or not.
// Port of: tests/RegionTest.cpp#L227-L239 (chrome/m156)
fn test_write(region: &Region, r: &mut crate::Reporter) {
    let bytes_needed = region.write_to_memory_size();
    let mut storage = Vec::new();
    region.write_to_memory(&mut storage);
    let bytes_written = storage.len();
    reporter_assert!(r, bytes_written == bytes_needed);

    // Also check that the bytes are meaningful.
    let mut copy = Region::new();
    reporter_assert!(r, copy.read_from_memory(&storage[..bytes_needed]) != 0);
    reporter_assert!(r, *region == copy);
}

// Port of: tests/RegionTest.cpp#L241-L269 (chrome/m156)
def_test!(Region_writeToMemory, |r| {
    // Test an empty region.
    let mut region = Region::new();
    reporter_assert!(r, region.is_empty());
    test_write(&region, r);

    // Test a rectangular region
    let mut non_empty = region.set_rect(IRect::new(0, 0, 50, 50));
    reporter_assert!(r, non_empty);
    reporter_assert!(r, region.is_rect());
    test_write(&region, r);

    // Test a complex region
    non_empty = region.op_rect(IRect::new(50, 50, 100, 100), Op::Union);
    reporter_assert!(r, non_empty);
    reporter_assert!(r, region.is_complex());
    test_write(&region, r);

    let mut complex_region = Region::new();
    union(&mut complex_region, &IRect::from_xywh(0, 0, 1, 1));
    union(&mut complex_region, &IRect::from_xywh(0, 0, 3, 3));
    union(&mut complex_region, &IRect::from_xywh(10, 0, 3, 3));
    union(&mut complex_region, &IRect::from_xywh(0, 10, 13, 3));
    test_write(&complex_region, r);

    union(&mut complex_region, &IRect::from_xywh(10, 20, 3, 3));
    union(&mut complex_region, &IRect::from_xywh(0, 20, 3, 3));
    test_write(&complex_region, r);
});

// Port of: tests/RegionTest.cpp#L271-L358 (chrome/m156)
def_test!(Region_readFromMemory_bad, |r| {
    // These assume what our binary format is: conceivably we could change it
    // and might need to remove or change some of these tests.
    let mut region = Region::new();

    {
        // invalid boundary rectangle
        let data: [i32; 5] = [0, 4, 4, 8, 2];
        let bytes = as_bytes(&data);
        reporter_assert!(r, 0 == region.read_from_memory(&bytes));
    }
    // Region Layout, Serialized Format:
    //    COUNT LEFT TOP RIGHT BOTTOM Y_SPAN_COUNT TOTAL_INTERVAL_COUNT
    //    Top ( Bottom Span_Interval_Count ( Left Right )* Sentinel )+ Sentinel
    {
        // Example of valid data
        let data: [i32; 16] = [
            9,
            0,
            0,
            10,
            10,
            1,
            2,
            0,
            10,
            2,
            0,
            4,
            6,
            10,
            2_147_483_647,
            2_147_483_647,
        ];
        let bytes = as_bytes(&data);
        reporter_assert!(r, 0 != region.read_from_memory(&bytes));
    }
    {
        // Example of valid data with 4 intervals
        let data: [i32; 26] = [
            19,
            0,
            0,
            30,
            30,
            3,
            4,
            0,
            10,
            2,
            0,
            10,
            20,
            30,
            2_147_483_647,
            20,
            0,
            2_147_483_647,
            30,
            2,
            0,
            10,
            20,
            30,
            2_147_483_647,
            2_147_483_647,
        ];
        let bytes = as_bytes(&data);
        reporter_assert!(r, 0 != region.read_from_memory(&bytes));
    }
    {
        // Short count
        let data: [i32; 16] = [
            8,
            0,
            0,
            10,
            10,
            1,
            2,
            0,
            10,
            2,
            0,
            4,
            6,
            10,
            2_147_483_647,
            2_147_483_647,
        ];
        let bytes = as_bytes(&data);
        reporter_assert!(r, 0 == region.read_from_memory(&bytes));
    }
    {
        // bounds don't match
        let data: [i32; 16] = [
            9,
            0,
            0,
            10,
            11,
            1,
            2,
            0,
            10,
            2,
            0,
            4,
            6,
            10,
            2_147_483_647,
            2_147_483_647,
        ];
        let bytes = as_bytes(&data);
        reporter_assert!(r, 0 == region.read_from_memory(&bytes));
    }
    {
        //  bad yspan count
        let data: [i32; 16] = [
            9,
            0,
            0,
            10,
            10,
            2,
            2,
            0,
            10,
            2,
            0,
            4,
            6,
            10,
            2_147_483_647,
            2_147_483_647,
        ];
        let bytes = as_bytes(&data);
        reporter_assert!(r, 0 == region.read_from_memory(&bytes));
    }
    {
        // bad int count
        let data: [i32; 16] = [
            9,
            0,
            0,
            10,
            10,
            1,
            3,
            0,
            10,
            2,
            0,
            4,
            6,
            10,
            2_147_483_647,
            2_147_483_647,
        ];
        let bytes = as_bytes(&data);
        reporter_assert!(r, 0 == region.read_from_memory(&bytes));
    }
    {
        // bad final sentinal
        let data: [i32; 16] = [
            9,
            0,
            0,
            10,
            10,
            1,
            2,
            0,
            10,
            2,
            0,
            4,
            6,
            10,
            2_147_483_647,
            -1,
        ];
        let bytes = as_bytes(&data);
        reporter_assert!(r, 0 == region.read_from_memory(&bytes));
    }
    {
        // bad row sentinal
        let data: [i32; 16] = [
            9,
            0,
            0,
            10,
            10,
            1,
            2,
            0,
            10,
            2,
            0,
            4,
            6,
            10,
            -1,
            2_147_483_647,
        ];
        let bytes = as_bytes(&data);
        reporter_assert!(r, 0 == region.read_from_memory(&bytes));
    }
    {
        // starts with empty yspan
        let data: [i32; 19] = [
            12,
            0,
            0,
            10,
            10,
            2,
            2,
            -5,
            0,
            0,
            2_147_483_647,
            10,
            2,
            0,
            4,
            6,
            10,
            2_147_483_647,
            2_147_483_647,
        ];
        let bytes = as_bytes(&data);
        reporter_assert!(r, 0 == region.read_from_memory(&bytes));
    }
    {
        // ends with empty yspan
        let data: [i32; 19] = [
            12,
            0,
            0,
            10,
            10,
            2,
            2,
            0,
            10,
            2,
            0,
            4,
            6,
            10,
            2_147_483_647,
            15,
            0,
            2_147_483_647,
            2_147_483_647,
        ];
        let bytes = as_bytes(&data);
        reporter_assert!(r, 0 == region.read_from_memory(&bytes));
    }
    {
        // y intervals out of order
        let data: [i32; 26] = [
            19,
            0,
            -20,
            30,
            10,
            3,
            4,
            0,
            10,
            2,
            0,
            10,
            20,
            30,
            2_147_483_647,
            -20,
            0,
            2_147_483_647,
            -10,
            2,
            0,
            10,
            20,
            30,
            2_147_483_647,
            2_147_483_647,
        ];
        let bytes = as_bytes(&data);
        reporter_assert!(r, 0 == region.read_from_memory(&bytes));
    }
    {
        // x intervals out of order
        let data: [i32; 16] = [
            9,
            0,
            0,
            10,
            10,
            1,
            2,
            0,
            10,
            2,
            6,
            10,
            0,
            4,
            2_147_483_647,
            2_147_483_647,
        ];
        let bytes = as_bytes(&data);
        reporter_assert!(r, 0 == region.read_from_memory(&bytes));
    }
});

// Port of: tests/RegionTest.cpp#L360-L380 (chrome/m156)
def_test!(region_toobig, |reporter| {
    let big: i32 = 1 << 30;
    let neg = IRect::from_xywh(-big, -big, 10, 10);
    let pos = IRect::from_xywh(big, big, 10, 10);

    reporter_assert!(reporter, !neg.is_empty());
    reporter_assert!(reporter, !pos.is_empty());

    let neg_r = Region::from_rect(neg);
    let pos_r = Region::from_rect(pos);

    reporter_assert!(reporter, !neg_r.is_empty());
    reporter_assert!(reporter, !pos_r.is_empty());

    let mut rgn = Region::new();
    rgn.op_region_region(&neg_r, &pos_r, Op::Union);

    // If we union those to rectangles, the resulting coordinates span more than int32_t, so
    // we must mark the region as empty.
    reporter_assert!(reporter, rgn.is_empty());
});

// Port of: tests/RegionTest.cpp#L432-L438 (chrome/m156)
def_test!(region_bug_chromium_873051, |reporter| {
    let mut region = Region::new();
    reporter_assert!(
        reporter,
        region.set_rect(IRect::new(0, 0, 0x7FFF_FFFE, 0x7FFF_FFFE))
    );
    reporter_assert!(
        reporter,
        !region.set_rect(IRect::new(0, 0, 0x7FFF_FFFE, 0x7FFF_FFFF))
    );
    reporter_assert!(
        reporter,
        !region.set_rect(IRect::new(0, 0, 0x7FFF_FFFF, 0x7FFF_FFFE))
    );
    reporter_assert!(
        reporter,
        !region.set_rect(IRect::new(0, 0, 0x7FFF_FFFF, 0x7FFF_FFFF))
    );
});

// Port of: tests/RegionTest.cpp#L440-L479 (chrome/m156)
def_test!(region_empty_iter, |reporter| {
    let mut empty_iter = Iterator::new_empty();
    reporter_assert!(reporter, !empty_iter.rewind());
    reporter_assert!(reporter, empty_iter.is_done());
    let e_rect = *empty_iter.rect();
    reporter_assert!(reporter, e_rect.is_empty());
    reporter_assert!(reporter, IRect::new_empty() == e_rect);
    reporter_assert!(reporter, empty_iter.rgn().is_none());

    let region = Region::new();
    let mut reset_iter = Iterator::new_empty();
    reset_iter = reset_iter.reset(&region);
    reporter_assert!(reporter, reset_iter.rewind());
    reporter_assert!(reporter, reset_iter.is_done());
    let r_rect = *reset_iter.rect();
    reporter_assert!(reporter, r_rect.is_empty());
    reporter_assert!(reporter, IRect::new_empty() == r_rect);
    reporter_assert!(reporter, reset_iter.rgn().is_some());
    reporter_assert!(reporter, reset_iter.rgn().is_some_and(Region::is_empty));

    let iter = Iterator::new(&region);
    reporter_assert!(reporter, iter.is_done());
    let i_rect = *iter.rect();
    reporter_assert!(reporter, i_rect.is_empty());
    reporter_assert!(reporter, IRect::new_empty() == i_rect);
    reporter_assert!(reporter, iter.rgn().is_some());
    reporter_assert!(reporter, iter.rgn().is_some_and(Region::is_empty));

    let clip_iter = Cliperator::new(&region, IRect::new(0, 0, 100, 100));
    reporter_assert!(reporter, clip_iter.is_done());
    let c_rect = *clip_iter.rect();
    reporter_assert!(reporter, c_rect.is_empty());
    reporter_assert!(reporter, IRect::new_empty() == c_rect);

    let mut span_iter = Spanerator::new(&region, 0, 0, 100);
    let mut left = 0;
    let mut right = 0;
    // `next(&left, &right)` returns the span instead of filling out-parameters.
    let next = std::iter::Iterator::next(&mut span_iter);
    if let Some((l, r)) = next {
        left = l;
        right = r;
    }
    reporter_assert!(reporter, next.is_none());
    reporter_assert!(reporter, left == 0);
    reporter_assert!(reporter, right == 0);
});

// Port of: tests/RegionTest.cpp#L558-L602 (chrome/m156)
def_test!(SkRegion_Iterator_StepsThroughAllScanlines, |reporter| {
    let mut rgn1 = Region::new();
    rgn1.op_rect(IRect::new(12, 10, 17, 20), Op::Union);
    rgn1.op_rect(IRect::new(31, 10, 39, 25), Op::Union);
    rgn1.op_rect(IRect::new(16, 30, 23, 40), Op::Union);

    // skia-rust: not expressible in Rust: `memset(&buffer, 0, 128)` (the Vec starts empty and is
    // sized by `write_to_memory`).
    let mut buffer = Vec::new();
    rgn1.write_to_memory(&mut buffer);
    let len = buffer.len();
    assert!(len < 128); // SkASSERT_RELEASE
    let mut rgn2 = Region::new();
    let len2 = rgn2.read_from_memory(&buffer[..len]);
    reporter_assert!(reporter, len == len2);

    // Make sure the serialized/deserialzed version is the same as the original.
    for rgn in [&rgn1, &rgn2] {
        let mut iter = Iterator::new(rgn);

        // The first scanline strip starts at Y=10 and ends at Y=20.
        //   The first rectangle there starts at X=12 and ends at X=17
        reporter_assert!(reporter, !iter.is_done());
        reporter_assert!(reporter, *iter.rect() == IRect::from_ltrb(12, 10, 17, 20));

        //    There's a second rectangle in that section from X=31 to X=39.
        iter.next();
        reporter_assert!(reporter, !iter.is_done());
        reporter_assert!(reporter, *iter.rect() == IRect::from_ltrb(31, 10, 39, 20));

        // The next scanline strip continues at Y=20 and goes til Y=25
        //     The one and only rectangle here starts at X=31 and goes to X=39
        iter.next();
        reporter_assert!(reporter, !iter.is_done());
        reporter_assert!(reporter, *iter.rect() == IRect::from_ltrb(31, 20, 39, 25));

        // There's a jump to the final scanline strip from Y=30 to Y=40
        //     The one and only rectangle here starts at X=16 and goes to X=23
        iter.next();
        reporter_assert!(reporter, !iter.is_done());
        reporter_assert!(reporter, *iter.rect() == IRect::from_ltrb(16, 30, 23, 40));

        // Call next() -> no more rectangles
        iter.next();
        reporter_assert!(reporter, iter.is_done());
    }
});

// Port of: tests/RegionTest.cpp#L604-L638 (chrome/m156)
def_test!(
    SkRegion_ReadFromMemory_ConsecutiveEmptySlices_Invalid,
    |reporter| {
        const K_SENTINEL: i32 = 0x7FFF_FFFF;
        let corrupt: [i32; 49] = [
            42, // number of int32s in the RLE portion (after 7 metadata int32s)
            0, 0, 100, 100, // bounds
            12,  // 12 spans (10 are empty)
            2,   // 2 rectangles
            0, 5, // first stripe is from Y=0 to Y=5,
            1, // 1 rectangle
            0, 100,        // from x = 0 to 100 (arbitrary)
            K_SENTINEL, //
            10, 0, K_SENTINEL, // Empty from  5-10
            20, 0, K_SENTINEL, // Empty from 10-20...
            30, 0, K_SENTINEL, //
            40, 0, K_SENTINEL, //
            50, 0, K_SENTINEL, //
            60, 0, K_SENTINEL, //
            70, 0, K_SENTINEL, //
            80, 0, K_SENTINEL, //
            90, 0, K_SENTINEL, //
            95, 0, K_SENTINEL, //
            100, 1, 20, 30, K_SENTINEL, // one final real rectangle until Y=100
            K_SENTINEL, // final sentinal
        ];

        let mut rgn = Region::new();
        let len = rgn.read_from_memory(&as_bytes(&corrupt));
        reporter_assert!(reporter, len == 0); // len == 0 means "could not read"

        // When there was a buggy version of this, the following iteration caused a crash.
        let mut iter = Iterator::new(&rgn);
        while !iter.is_done() {
            iter.next();
        }
    }
);

// Port of: tests/RegionTest.cpp#L640-L665 (chrome/m156)
def_test!(SkRegion_ReadFromMemory_SingleEmptySlice_Valid, |reporter| {
    const K_SENTINEL: i32 = 0x7FFF_FFFF;
    let valid: [i32; 22] = [
        15, // number of int32s in the RLE portion (after 7 metadata int32s)
        0, 0, 100, 100, // bounds
        3,   // 3 spans (1 is empty)
        2,   // 2 rectangles
        0, 5, // first stripe is from Y=0 to Y=5,
        1, // 1 rectangle
        0, 100,        // from x = 0 to 100 (arbitrary)
        K_SENTINEL, //
        80, 0, K_SENTINEL, // Empty from  5-80
        100, 1, 20, 30, K_SENTINEL, // one final real rectangle
        K_SENTINEL, // final sentinal
    ];

    let mut rgn = Region::new();
    let len = rgn.read_from_memory(&as_bytes(&valid));
    reporter_assert!(reporter, len > 0); // len == 0 means "could not read"

    // Make sure we don't read any memory we aren't supposed to.
    let mut iter = Iterator::new(&rgn);
    while !iter.is_done() {
        iter.next();
    }
});

// Port of: tests/RegionTest.cpp#L667-L686 (chrome/m156)
fn test_rects(rects: &[IRect]) -> bool {
    let mut rgn0 = Region::new();
    let mut rgn1 = Region::new();

    for rect in rects {
        rgn0.op_rect(rect, Op::Union);
    }
    rgn1.set_rects(rects);

    if rgn0 != rgn1 {
        eprintln!();
        for rect in rects {
            eprintln!(
                " {{ {}, {}, {}, {} }},",
                rect.left, rect.top, rect.right, rect.bottom
            );
        }
        eprintln!();
        return false;
    }
    true
}

// Port of: tests/RegionTest.cpp#L688-L730 (chrome/m156)
def_test!(Region_setRects, |reporter| {
    // `skiatest::ReporterContext` only labels failures with the sub-test name: the name is
    // attached to each assertion message instead.
    #[allow(clippy::nonminimal_bool)] // mirrors `nonEmpty == !rgn.isEmpty()`
    let mut test = |name: &str, rects: &[IRect]| {
        reporter_assert!(reporter, test_rects(rects), "{name}");

        let mut rgn = Region::new();
        let non_empty = rgn.set_rects(rects);
        reporter_assert!(reporter, non_empty == !rgn.is_empty(), "{name}");
    };

    test("0_rects", &[]);

    let r1 = [IRect::new(0, 0, 10, 10)];
    test("1_rect", &r1);

    let r2 = [IRect::new(0, 0, 1, 1), IRect::new(2, 2, 3, 3)];
    test("2_rects", &r2);

    let r3 = [
        IRect::new(0, 0, 1, 1),
        IRect::new(2, 2, 3, 3),
        IRect::new(4, 4, 5, 5),
    ];
    test("3_rects", &r3);

    let r4 = [
        IRect::new(0, 0, 1, 2),
        IRect::new(2, 1, 3, 3),
        IRect::new(4, 0, 5, 1),
        IRect::new(6, 0, 7, 4),
    ];
    test("4_rects", &r4);

    let r_overlap = [
        IRect::new(0, 0, 10, 10),
        IRect::new(5, 0, 15, 10),
        IRect::new(0, 5, 10, 15),
        IRect::new(5, 5, 15, 15),
    ];
    test("overlapping_rects", &r_overlap);

    let r_empty = [IRect::new(0, 0, 0, 0), IRect::new(10, 10, 10, 10)];
    test("empty_rects", &r_empty);

    let mut grid = [IRect::new_empty(); 100];
    for y in 0..10 {
        for x in 0..10 {
            grid[usize::try_from(y * 10 + x).unwrap()] = IRect::from_xywh(x * 20, y * 20, 10, 10);
        }
    }
    test("100_rects_grid", &grid);
});
