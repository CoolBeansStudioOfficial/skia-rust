// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ClipStackTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{Reporter, def_test, errorf, reporter_assert};
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::clip_stack::{
    B2TIter, BoundsType, ClipStack, DeviceSpaceType, EMPTY_GEN_ID, Iter, IterStart,
    WIDE_OPEN_GEN_ID,
};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::{Op, Region};
use skia_rust_core::rrect::RRect;
use skia_rust_core::scalar::int_to_scalar;
use skia_rust_raster::region_path::RegionExt;

// Port of: tests/ClipStackTest.cpp#L26-L120 (chrome/m156)
fn test_assign_and_comparison(reporter: &mut Reporter) {
    let mut s = ClipStack::new();
    let do_aa = false;

    reporter_assert!(reporter, 0 == s.save_count());

    // Build up a clip stack with a path, an empty clip, and a rect.
    s.save();
    reporter_assert!(reporter, 1 == s.save_count());

    let mut builder = PathBuilder::new();
    builder.move_to((5.0, 6.0));
    builder.line_to((7.0, 8.0));
    builder.line_to((5.0, 9.0));
    builder.close();
    s.clip_path(&builder.snapshot(), Matrix::i(), ClipOp::Intersect, do_aa);

    s.save();
    reporter_assert!(reporter, 2 == s.save_count());

    let mut r = Rect::from_ltrb(1.0, 2.0, 103.0, 104.0);
    s.clip_rect(&r, Matrix::i(), ClipOp::Intersect, do_aa);
    r = Rect::from_ltrb(4.0, 5.0, 56.0, 57.0);
    s.clip_rect(&r, Matrix::i(), ClipOp::Intersect, do_aa);

    s.save();
    reporter_assert!(reporter, 3 == s.save_count());

    r = Rect::from_ltrb(14.0, 15.0, 16.0, 17.0);
    s.clip_rect(&r, Matrix::i(), ClipOp::Difference, do_aa);

    // Test that assignment works.
    let mut copy = s.clone();
    reporter_assert!(reporter, s == copy);

    // Test that different save levels triggers not equal.
    s.restore();
    reporter_assert!(reporter, 2 == s.save_count());
    reporter_assert!(reporter, s != copy);

    // Test that an equal, but not copied version is equal.
    s.save();
    reporter_assert!(reporter, 3 == s.save_count());
    r = Rect::from_ltrb(14.0, 15.0, 16.0, 17.0);
    s.clip_rect(&r, Matrix::i(), ClipOp::Difference, do_aa);
    reporter_assert!(reporter, s == copy);

    // Test that a different op on one level triggers not equal.
    s.restore();
    reporter_assert!(reporter, 2 == s.save_count());
    s.save();
    reporter_assert!(reporter, 3 == s.save_count());
    r = Rect::from_ltrb(14.0, 15.0, 16.0, 17.0);
    s.clip_rect(&r, Matrix::i(), ClipOp::Intersect, do_aa);
    reporter_assert!(reporter, s != copy);

    // Test that version constructed with rect-path rather than a rect is still considered equal.
    s.restore();
    s.save();
    let rp = Path::rect(r, None);
    s.clip_path(&rp, Matrix::i(), ClipOp::Difference, do_aa);
    reporter_assert!(reporter, s == copy);

    // Test that different rects triggers not equal.
    s.restore();
    reporter_assert!(reporter, 2 == s.save_count());
    s.save();
    reporter_assert!(reporter, 3 == s.save_count());

    r = Rect::from_ltrb(24.0, 25.0, 26.0, 27.0);
    s.clip_rect(&r, Matrix::i(), ClipOp::Difference, do_aa);
    reporter_assert!(reporter, s != copy);

    s.restore();
    reporter_assert!(reporter, 2 == s.save_count());

    copy.restore();
    reporter_assert!(reporter, 2 == copy.save_count());
    reporter_assert!(reporter, s == copy);
    s.restore();
    reporter_assert!(reporter, 1 == s.save_count());
    copy.restore();
    reporter_assert!(reporter, 1 == copy.save_count());
    reporter_assert!(reporter, s == copy);

    // Test that different paths triggers not equal.
    s.restore();
    reporter_assert!(reporter, 0 == s.save_count());
    s.save();
    reporter_assert!(reporter, 1 == s.save_count());

    builder.add_rect(r, None, None);
    s.clip_path(&builder.detach(), Matrix::i(), ClipOp::Intersect, do_aa);
    reporter_assert!(reporter, s != copy);
}

// Port of: tests/ClipStackTest.cpp#L122-L130 (chrome/m156)
fn assert_count(reporter: &mut Reporter, stack: &ClipStack, count: i32) {
    let mut iter = B2TIter::new(stack);
    let mut counter = 0;
    while iter.next().is_some() {
        counter += 1;
    }
    reporter_assert!(reporter, count == counter);
}

// Exercise the SkClipStack's bottom to top and bidirectional iterators
// (including the skipToTopmost functionality)
// Port of: tests/ClipStackTest.cpp#L132-L194 (chrome/m156)
fn test_iterators(reporter: &mut Reporter) {
    const G_RECTS: [Rect; 4] = [
        Rect::new(0.0, 0.0, 40.0, 40.0),
        Rect::new(60.0, 0.0, 100.0, 40.0),
        Rect::new(0.0, 60.0, 40.0, 100.0),
        Rect::new(60.0, 60.0, 100.0, 100.0),
    ];

    let mut stack = ClipStack::new();

    for r in &G_RECTS {
        // the difference op will prevent these from being fused together
        stack.clip_rect(r, Matrix::i(), ClipOp::Difference, false);
    }

    assert_count(reporter, &stack, 4);

    // bottom to top iteration
    {
        let mut iter = B2TIter::new(&stack);
        let mut i = 0;

        let mut element = iter.next();
        while let Some(e) = element {
            reporter_assert!(reporter, DeviceSpaceType::Rect == e.device_space_type());
            reporter_assert!(reporter, *e.device_space_rect() == G_RECTS[i]);
            i += 1;
            element = iter.next();
        }

        debug_assert_eq!(i, 4);
    }

    // top to bottom iteration
    {
        let mut iter = Iter::new(&stack, IterStart::Top);
        let mut i: i32 = 3;

        let mut element = iter.prev();
        while let Some(e) = element {
            reporter_assert!(reporter, DeviceSpaceType::Rect == e.device_space_type());
            // `i` is in 0..=3 inside the loop (the C++ indexes the array the same way)
            #[allow(clippy::cast_sign_loss)]
            {
                reporter_assert!(reporter, *e.device_space_rect() == G_RECTS[i as usize]);
            }
            i -= 1;
            element = iter.prev();
        }

        debug_assert_eq!(i, -1);
    }

    // skipToTopmost
    {
        let mut iter = Iter::new(&stack, IterStart::Bottom);

        let element = iter.skip_to_topmost(ClipOp::Difference);
        reporter_assert!(reporter, element.is_some());
        let element = element.expect("skipToTopmost found an element");
        reporter_assert!(
            reporter,
            DeviceSpaceType::Rect == element.device_space_type()
        );
        reporter_assert!(reporter, *element.device_space_rect() == G_RECTS[3]);
    }
}

// Exercise the SkClipStack's getConservativeBounds computation
// Port of: tests/ClipStackTest.cpp#L196-L292 (chrome/m156)
fn test_bounds(reporter: &mut Reporter, prim_type: DeviceSpaceType) {
    const G_NUM_CASES: usize = 8;
    const G_ANSWER_RECTS_BW: [Rect; G_NUM_CASES] = [
        // A op B
        Rect::new(40.0, 40.0, 50.0, 50.0),
        Rect::new(10.0, 10.0, 50.0, 50.0),
        // invA op B
        Rect::new(40.0, 40.0, 80.0, 80.0),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        // A op invB
        Rect::new(10.0, 10.0, 50.0, 50.0),
        Rect::new(40.0, 40.0, 50.0, 50.0),
        // invA op invB
        Rect::new(0.0, 0.0, 100.0, 100.0),
        Rect::new(40.0, 40.0, 80.0, 80.0),
    ];

    const G_OPS: [ClipOp; 2] = [ClipOp::Intersect, ClipOp::Difference];

    let mut rect_a = Rect::new_empty();
    let mut rect_b = Rect::new_empty();

    rect_a.set_ltrb(10.0, 10.0, 50.0, 50.0);
    rect_b.set_ltrb(40.0, 40.0, 80.0, 80.0);

    let mut rr_a = RRect::new();
    let mut rr_b = RRect::new();
    rr_a.set_oval(rect_a);
    rr_b.set_rect_xy(rect_b, int_to_scalar(1), int_to_scalar(2));

    let mut path_a = Path::rrect(RRect::new_rect_xy(rect_a, 5.0, 5.0), None);
    let mut path_b = Path::rrect(RRect::new_rect_xy(rect_b, 5.0, 5.0), None);

    let mut stack = ClipStack::new();

    let mut test_case = 0;
    let num_bit_tests = if DeviceSpaceType::Path == prim_type {
        4
    } else {
        1
    };
    for inv_bits in 0..num_bit_tests {
        for op in &G_OPS {
            stack.save();
            let do_inv_a = (inv_bits & 1) != 0;
            let do_inv_b = (inv_bits & 2) != 0;

            path_a.set_fill_type(if do_inv_a {
                PathFillType::InverseEvenOdd
            } else {
                PathFillType::EvenOdd
            });
            path_b.set_fill_type(if do_inv_b {
                PathFillType::InverseEvenOdd
            } else {
                PathFillType::EvenOdd
            });

            match prim_type {
                DeviceSpaceType::Shader | DeviceSpaceType::Empty => {
                    debug_assert!(false, "Don't call this with kEmpty or kShader.");
                }
                DeviceSpaceType::Rect => {
                    stack.clip_rect(&rect_a, Matrix::i(), ClipOp::Intersect, false);
                    stack.clip_rect(&rect_b, Matrix::i(), *op, false);
                }
                DeviceSpaceType::RRect => {
                    stack.clip_rrect(&rr_a, Matrix::i(), ClipOp::Intersect, false);
                    stack.clip_rrect(&rr_b, Matrix::i(), *op, false);
                }
                DeviceSpaceType::Path => {
                    stack.clip_path(&path_a, Matrix::i(), ClipOp::Intersect, false);
                    stack.clip_path(&path_b, Matrix::i(), *op, false);
                }
            }

            reporter_assert!(reporter, !stack.is_wide_open());
            reporter_assert!(reporter, WIDE_OPEN_GEN_ID != stack.topmost_gen_id());

            let (dev_clip_bound, is_intersection_of_rects) =
                stack.get_conservative_bounds(0, 0, 100, 100);

            if DeviceSpaceType::Rect == prim_type {
                reporter_assert!(
                    reporter,
                    is_intersection_of_rects == (*op == ClipOp::Intersect)
                );
            } else {
                reporter_assert!(reporter, !is_intersection_of_rects);
            }

            debug_assert!(test_case < G_NUM_CASES);
            reporter_assert!(reporter, dev_clip_bound == G_ANSWER_RECTS_BW[test_case]);
            test_case += 1;

            stack.restore();
        }
    }
}

// Test out 'isWideOpen' entry point
// Port of: tests/ClipStackTest.cpp#L294-L345 (chrome/m156)
fn test_is_wide_open(reporter: &mut Reporter) {
    {
        // Empty stack is wide open. Wide open stack means that gen id is wide open.
        let stack = ClipStack::new();
        reporter_assert!(reporter, stack.is_wide_open());
        reporter_assert!(reporter, WIDE_OPEN_GEN_ID == stack.topmost_gen_id());
    }

    let mut rect_a = Rect::new_empty();
    let mut rect_b = Rect::new_empty();

    rect_a.set_ltrb(10.0, 10.0, 40.0, 40.0);
    rect_b.set_ltrb(50.0, 50.0, 80.0, 80.0);
    let _ = rect_b;

    // Stack should initially be wide open
    {
        let stack = ClipStack::new();

        reporter_assert!(reporter, stack.is_wide_open());
        reporter_assert!(reporter, WIDE_OPEN_GEN_ID == stack.topmost_gen_id());
    }

    // Test out empty difference from a wide open clip
    {
        let mut stack = ClipStack::new();

        let mut empty_rect = Rect::new_empty();
        empty_rect.set_empty();

        stack.clip_rect(&empty_rect, Matrix::i(), ClipOp::Difference, false);

        reporter_assert!(reporter, stack.is_wide_open());
        reporter_assert!(reporter, WIDE_OPEN_GEN_ID == stack.topmost_gen_id());
    }

    // Test out return to wide open
    {
        let mut stack = ClipStack::new();

        stack.save();

        stack.clip_rect(&rect_a, Matrix::i(), ClipOp::Intersect, false);

        reporter_assert!(reporter, !stack.is_wide_open());
        reporter_assert!(reporter, WIDE_OPEN_GEN_ID != stack.topmost_gen_id());

        stack.restore();

        reporter_assert!(reporter, stack.is_wide_open());
        reporter_assert!(reporter, WIDE_OPEN_GEN_ID == stack.topmost_gen_id());
    }
}

// Port of: tests/ClipStackTest.cpp#L347-L358 (chrome/m156)
fn count(stack: &ClipStack) -> i32 {
    let mut iter = Iter::new(stack, IterStart::Top);

    let mut count = 0;

    let mut element = iter.prev();
    while element.is_some() {
        element = iter.prev();
        count += 1;
    }

    count
}

// Port of: tests/ClipStackTest.cpp#L360-L374 (chrome/m156)
fn test_rect_inverse_fill(reporter: &mut Reporter) {
    // non-intersecting rectangles
    let rect = Rect::from_ltrb(0.0, 0.0, 10.0, 10.0);

    let mut path = Path::rect(rect, None);
    path.toggle_inverse_fill_type();
    let mut stack = ClipStack::new();
    stack.clip_path(&path, Matrix::i(), ClipOp::Intersect, false);

    let (bounds, bounds_type, _) = stack.get_bounds();
    reporter_assert!(reporter, BoundsType::InsideOut == bounds_type);
    reporter_assert!(reporter, bounds == rect);
}

// Port of: tests/ClipStackTest.cpp#L376-L449 (chrome/m156)
fn test_rect_replace(reporter: &mut Reporter) {
    let rect = Rect::from_wh(100.0, 100.0);
    let rect2 = Rect::from_xywh(50.0, 50.0, 100.0, 100.0);

    // Adding a new rect with the replace operator should not increase
    // the stack depth. BW replacing BW.
    {
        let mut stack = ClipStack::new();
        reporter_assert!(reporter, 0 == count(&stack));
        stack.replace_clip(&rect, false);
        reporter_assert!(reporter, 1 == count(&stack));
        stack.replace_clip(&rect, false);
        reporter_assert!(reporter, 1 == count(&stack));
    }

    // Adding a new rect with the replace operator should not increase
    // the stack depth. AA replacing AA.
    {
        let mut stack = ClipStack::new();
        reporter_assert!(reporter, 0 == count(&stack));
        stack.replace_clip(&rect, true);
        reporter_assert!(reporter, 1 == count(&stack));
        stack.replace_clip(&rect, true);
        reporter_assert!(reporter, 1 == count(&stack));
    }

    // Adding a new rect with the replace operator should not increase
    // the stack depth. BW replacing AA replacing BW.
    {
        let mut stack = ClipStack::new();
        reporter_assert!(reporter, 0 == count(&stack));
        stack.replace_clip(&rect, false);
        reporter_assert!(reporter, 1 == count(&stack));
        stack.replace_clip(&rect, true);
        reporter_assert!(reporter, 1 == count(&stack));
        stack.replace_clip(&rect, false);
        reporter_assert!(reporter, 1 == count(&stack));
    }

    // Make sure replace clip rects don't collapse too much.
    {
        let mut stack = ClipStack::new();
        stack.replace_clip(&rect, false);
        stack.clip_rect(&rect2, Matrix::i(), ClipOp::Intersect, false);
        reporter_assert!(reporter, 1 == count(&stack));

        stack.save();
        stack.replace_clip(&rect, false);
        reporter_assert!(reporter, 2 == count(&stack));
        let (bound, _, _) = stack.get_bounds();
        reporter_assert!(reporter, bound == rect);
        stack.restore();
        reporter_assert!(reporter, 1 == count(&stack));

        stack.save();
        stack.replace_clip(&rect, false);
        stack.replace_clip(&rect, false);
        reporter_assert!(reporter, 2 == count(&stack));
        stack.restore();
        reporter_assert!(reporter, 1 == count(&stack));

        stack.save();
        stack.replace_clip(&rect, false);
        stack.clip_rect(&rect2, Matrix::i(), ClipOp::Intersect, false);
        stack.replace_clip(&rect, false);
        reporter_assert!(reporter, 2 == count(&stack));
        stack.restore();
        reporter_assert!(reporter, 1 == count(&stack));
    }
}

// Simplified path-based version of test_rect_replace.
// Port of: tests/ClipStackTest.cpp#L451-L482 (chrome/m156)
fn test_path_replace(reporter: &mut Reporter) {
    let replace_path = |stack: &mut ClipStack, path: &Path, do_aa: bool| {
        let wide_open = Rect::from_ltrb(-1000.0, -1000.0, 1000.0, 1000.0);
        stack.replace_clip(&wide_open, false);
        stack.clip_path(path, Matrix::i(), ClipOp::Intersect, do_aa);
    };
    let rect = Rect::from_wh(100.0, 100.0);
    let path = Path::circle((50.0, 50.0), 50.0, None);

    // Emulating replace operations with more complex geometry is not atomic, it's a replace
    // with a wide-open rect and then an intersection with the complex geometry. The replace can
    // combine with prior elements, but the subsequent intersect cannot be combined so the stack
    // continues to grow.
    {
        let mut stack = ClipStack::new();
        reporter_assert!(reporter, 0 == count(&stack));
        replace_path(&mut stack, &path, false);
        reporter_assert!(reporter, 2 == count(&stack));
        replace_path(&mut stack, &path, false);
        reporter_assert!(reporter, 2 == count(&stack));
    }

    // Replacing rect with path.
    {
        let mut stack = ClipStack::new();
        stack.replace_clip(&rect, true);
        reporter_assert!(reporter, 1 == count(&stack));
        replace_path(&mut stack, &path, true);
        reporter_assert!(reporter, 2 == count(&stack));
    }
}

// Test out SkClipStack's merging of rect clips. In particular exercise
// merging of aa vs. bw rects.
// Port of: tests/ClipStackTest.cpp#L484-L575 (chrome/m156)
fn test_rect_merging(reporter: &mut Reporter) {
    let overlap_left = Rect::from_ltrb(10.0, 10.0, 50.0, 50.0);
    let overlap_right = Rect::from_ltrb(40.0, 40.0, 80.0, 80.0);

    let nested_parent = Rect::from_ltrb(10.0, 10.0, 90.0, 90.0);
    let nested_child = Rect::from_ltrb(40.0, 40.0, 60.0, 60.0);

    // all bw overlapping - should merge
    {
        let mut stack = ClipStack::new();
        stack.clip_rect(&overlap_left, Matrix::i(), ClipOp::Intersect, false);
        stack.clip_rect(&overlap_right, Matrix::i(), ClipOp::Intersect, false);

        reporter_assert!(reporter, 1 == count(&stack));

        let (_, _, is_intersection_of_rects) = stack.get_bounds();

        reporter_assert!(reporter, is_intersection_of_rects);
    }

    // all aa overlapping - should merge
    {
        let mut stack = ClipStack::new();
        stack.clip_rect(&overlap_left, Matrix::i(), ClipOp::Intersect, true);
        stack.clip_rect(&overlap_right, Matrix::i(), ClipOp::Intersect, true);

        reporter_assert!(reporter, 1 == count(&stack));

        let (_, _, is_intersection_of_rects) = stack.get_bounds();

        reporter_assert!(reporter, is_intersection_of_rects);
    }

    // mixed overlapping - should _not_ merge
    {
        let mut stack = ClipStack::new();
        stack.clip_rect(&overlap_left, Matrix::i(), ClipOp::Intersect, true);
        stack.clip_rect(&overlap_right, Matrix::i(), ClipOp::Intersect, false);

        reporter_assert!(reporter, 2 == count(&stack));

        let (_, _, is_intersection_of_rects) = stack.get_bounds();

        reporter_assert!(reporter, !is_intersection_of_rects);
    }

    // mixed nested (bw inside aa) - should merge
    {
        let mut stack = ClipStack::new();
        stack.clip_rect(&nested_parent, Matrix::i(), ClipOp::Intersect, true);
        stack.clip_rect(&nested_child, Matrix::i(), ClipOp::Intersect, false);

        reporter_assert!(reporter, 1 == count(&stack));

        let (_, _, is_intersection_of_rects) = stack.get_bounds();

        reporter_assert!(reporter, is_intersection_of_rects);
    }

    // mixed nested (aa inside bw) - should merge
    {
        let mut stack = ClipStack::new();
        stack.clip_rect(&nested_parent, Matrix::i(), ClipOp::Intersect, false);
        stack.clip_rect(&nested_child, Matrix::i(), ClipOp::Intersect, true);

        reporter_assert!(reporter, 1 == count(&stack));

        let (_, _, is_intersection_of_rects) = stack.get_bounds();

        reporter_assert!(reporter, is_intersection_of_rects);
    }

    // reverse nested (aa inside bw) - should _not_ merge
    {
        let mut stack = ClipStack::new();
        stack.clip_rect(&nested_child, Matrix::i(), ClipOp::Intersect, false);
        stack.clip_rect(&nested_parent, Matrix::i(), ClipOp::Intersect, true);

        reporter_assert!(reporter, 2 == count(&stack));

        let (_, _, is_intersection_of_rects) = stack.get_bounds();

        reporter_assert!(reporter, !is_intersection_of_rects);
    }
}

// Port of: tests/ClipStackTest.cpp#L577-L745 (chrome/m156)
#[allow(clippy::too_many_lines)] // mirrors the C++ function
fn test_quick_contains(reporter: &mut Reporter) {
    let test_rect = Rect::from_ltrb(10.0, 10.0, 40.0, 40.0);
    let inside_rect = Rect::from_ltrb(20.0, 20.0, 30.0, 30.0);
    let intersecting_rect = Rect::from_ltrb(25.0, 25.0, 50.0, 50.0);
    let outside_rect = Rect::from_ltrb(0.0, 0.0, 50.0, 50.0);
    let non_intersecting_rect = Rect::from_ltrb(100.0, 100.0, 110.0, 110.0);

    let inside_circle = Path::circle((25.0, 25.0), 5.0, None);
    let intersecting_circle = Path::circle((25.0, 40.0), 10.0, None);
    let outside_circle = Path::circle((25.0, 25.0), 50.0, None);
    let non_intersecting_circle = Path::circle((100.0, 100.0), 5.0, None);

    {
        let mut stack = ClipStack::new();
        stack.clip_rect(&outside_rect, Matrix::i(), ClipOp::Difference, false);
        // return false because quickContains currently does not care for kDifference
        reporter_assert!(reporter, !stack.quick_contains_rect(&test_rect));
    }

    // Replace Op tests
    {
        let mut stack = ClipStack::new();
        stack.replace_clip(&outside_rect, false);
        reporter_assert!(reporter, stack.quick_contains_rect(&test_rect));
    }

    {
        let mut stack = ClipStack::new();
        stack.clip_rect(&inside_rect, Matrix::i(), ClipOp::Intersect, false);
        stack.save(); // To prevent in-place substitution by replace OP
        stack.replace_clip(&outside_rect, false);
        reporter_assert!(reporter, stack.quick_contains_rect(&test_rect));
        stack.restore();
    }

    {
        let mut stack = ClipStack::new();
        stack.clip_rect(&outside_rect, Matrix::i(), ClipOp::Intersect, false);
        stack.save(); // To prevent in-place substitution by replace OP
        stack.replace_clip(&inside_rect, false);
        reporter_assert!(reporter, !stack.quick_contains_rect(&test_rect));
        stack.restore();
    }

    // Verify proper traversal of multi-element clip
    {
        let mut stack = ClipStack::new();
        stack.clip_rect(&inside_rect, Matrix::i(), ClipOp::Intersect, false);
        // Use a path for second clip to prevent in-place intersection
        stack.clip_path(&outside_circle, Matrix::i(), ClipOp::Intersect, false);
        reporter_assert!(reporter, !stack.quick_contains_rect(&test_rect));
    }

    // Intersect Op tests with rectangles
    {
        let mut stack = ClipStack::new();
        stack.clip_rect(&outside_rect, Matrix::i(), ClipOp::Intersect, false);
        reporter_assert!(reporter, stack.quick_contains_rect(&test_rect));
    }

    {
        let mut stack = ClipStack::new();
        stack.clip_rect(&inside_rect, Matrix::i(), ClipOp::Intersect, false);
        reporter_assert!(reporter, !stack.quick_contains_rect(&test_rect));
    }

    {
        let mut stack = ClipStack::new();
        stack.clip_rect(&intersecting_rect, Matrix::i(), ClipOp::Intersect, false);
        reporter_assert!(reporter, !stack.quick_contains_rect(&test_rect));
    }

    {
        let mut stack = ClipStack::new();
        stack.clip_rect(
            &non_intersecting_rect,
            Matrix::i(),
            ClipOp::Intersect,
            false,
        );
        reporter_assert!(reporter, !stack.quick_contains_rect(&test_rect));
    }

    // Intersect Op tests with circle paths
    {
        let mut stack = ClipStack::new();
        stack.clip_path(&outside_circle, Matrix::i(), ClipOp::Intersect, false);
        reporter_assert!(reporter, stack.quick_contains_rect(&test_rect));
    }

    {
        let mut stack = ClipStack::new();
        stack.clip_path(&inside_circle, Matrix::i(), ClipOp::Intersect, false);
        reporter_assert!(reporter, !stack.quick_contains_rect(&test_rect));
    }

    {
        let mut stack = ClipStack::new();
        stack.clip_path(&intersecting_circle, Matrix::i(), ClipOp::Intersect, false);
        reporter_assert!(reporter, !stack.quick_contains_rect(&test_rect));
    }

    {
        let mut stack = ClipStack::new();
        stack.clip_path(
            &non_intersecting_circle,
            Matrix::i(),
            ClipOp::Intersect,
            false,
        );
        reporter_assert!(reporter, !stack.quick_contains_rect(&test_rect));
    }

    // Intersect Op tests with inverse filled rectangles
    {
        let mut stack = ClipStack::new();
        let mut path = Path::rect(outside_rect, None);
        path.toggle_inverse_fill_type();
        stack.clip_path(&path, Matrix::i(), ClipOp::Intersect, false);
        reporter_assert!(reporter, !stack.quick_contains_rect(&test_rect));
    }

    {
        let mut stack = ClipStack::new();
        let mut path = Path::rect(inside_rect, None);
        path.toggle_inverse_fill_type();
        stack.clip_path(&path, Matrix::i(), ClipOp::Intersect, false);
        reporter_assert!(reporter, !stack.quick_contains_rect(&test_rect));
    }

    {
        let mut stack = ClipStack::new();
        let mut path = Path::rect(intersecting_rect, None);
        path.toggle_inverse_fill_type();
        stack.clip_path(&path, Matrix::i(), ClipOp::Intersect, false);
        reporter_assert!(reporter, !stack.quick_contains_rect(&test_rect));
    }

    {
        let mut stack = ClipStack::new();
        let mut path = Path::rect(non_intersecting_rect, None);
        path.toggle_inverse_fill_type();
        stack.clip_path(&path, Matrix::i(), ClipOp::Intersect, false);
        reporter_assert!(reporter, stack.quick_contains_rect(&test_rect));
    }

    // Intersect Op tests with inverse filled circles
    {
        let mut stack = ClipStack::new();
        let mut path = outside_circle.clone();
        path.toggle_inverse_fill_type();
        stack.clip_path(&path, Matrix::i(), ClipOp::Intersect, false);
        reporter_assert!(reporter, !stack.quick_contains_rect(&test_rect));
    }

    {
        let mut stack = ClipStack::new();
        let mut path = inside_circle.clone();
        path.toggle_inverse_fill_type();
        stack.clip_path(&path, Matrix::i(), ClipOp::Intersect, false);
        reporter_assert!(reporter, !stack.quick_contains_rect(&test_rect));
    }

    {
        let mut stack = ClipStack::new();
        let mut path = intersecting_circle.clone();
        path.toggle_inverse_fill_type();
        stack.clip_path(&path, Matrix::i(), ClipOp::Intersect, false);
        reporter_assert!(reporter, !stack.quick_contains_rect(&test_rect));
    }

    {
        let mut stack = ClipStack::new();
        let mut path = non_intersecting_circle.clone();
        path.toggle_inverse_fill_type();
        stack.clip_path(&path, Matrix::i(), ClipOp::Intersect, false);
        reporter_assert!(reporter, stack.quick_contains_rect(&test_rect));
    }
}

// Port of: tests/ClipStackTest.cpp#L747-L766 (chrome/m156)
fn set_region_to_stack(stack: &ClipStack, bounds: &IRect, region: &mut Region) {
    region.set_rect(bounds);
    let mut iter = Iter::new(stack, IterStart::Bottom);
    while let Some(element) = iter.next() {
        let mut elem_region = Region::new();
        let bounds_rgn = Region::from_rect(bounds);

        match element.device_space_type() {
            DeviceSpaceType::Empty => {
                elem_region.set_empty();
            }
            _ => {
                elem_region.set_path(&element.as_device_space_path(), &bounds_rgn);
            }
        }

        region.op_region(
            &elem_region,
            if element.is_replace_op() {
                Op::Replace
            } else {
                Op::from(element.op())
            },
        );
    }
}

// Port of: tests/ClipStackTest.cpp#L768-L789 (chrome/m156)
fn test_invfill_diff_bug(reporter: &mut Reporter) {
    let mut stack = ClipStack::new();
    stack.clip_rect(
        &Rect::new(10.0, 10.0, 20.0, 20.0),
        Matrix::i(),
        ClipOp::Intersect,
        false,
    );

    let mut path = Path::rect(Rect::new(30.0, 10.0, 40.0, 20.0), None);
    path.set_fill_type(PathFillType::InverseWinding);
    stack.clip_path(&path, Matrix::i(), ClipOp::Difference, false);

    reporter_assert!(reporter, EMPTY_GEN_ID == stack.topmost_gen_id());

    let (stack_bounds, stack_bounds_type, _) = stack.get_bounds();

    reporter_assert!(reporter, stack_bounds.is_empty());
    reporter_assert!(reporter, BoundsType::Normal == stack_bounds_type);

    let mut region = Region::new();
    set_region_to_stack(&stack, &IRect::new(0, 0, 50, 30), &mut region);

    reporter_assert!(reporter, region.is_empty());
}

///////////////////////////////////////////////////////////////////////////////////////////////////

// Port of: tests/ClipStackTest.cpp#L793-L826 (chrome/m156)
fn test_is_rrect_deep_rect_stack(reporter: &mut Reporter) {
    const K_TARGET_BOUNDS: Rect = Rect::new(0.0, 0.0, 1000.0, 500.0);
    // All antialiased or all not antialiased.
    for aa in [false, true] {
        let mut stack = ClipStack::new();
        for i in 0..=100 {
            stack.save();
            stack.clip_rect(
                &Rect::from_ltrb(
                    int_to_scalar(i),
                    0.5,
                    K_TARGET_BOUNDS.width(),
                    K_TARGET_BOUNDS.height(),
                ),
                Matrix::i(),
                ClipOp::Intersect,
                aa,
            );
        }
        let expected = RRect::new_rect(Rect::from_ltrb(
            100.0,
            0.5,
            K_TARGET_BOUNDS.width(),
            K_TARGET_BOUNDS.height(),
        ));
        if let Some((rrect, is_aa)) = stack.is_rrect(&K_TARGET_BOUNDS) {
            reporter_assert!(reporter, rrect == expected);
            reporter_assert!(reporter, aa == is_aa);
        } else {
            errorf!(reporter, "Expected to be an rrect.");
        }
    }
    // Mixed AA and non-AA without simple containment.
    let mut stack = ClipStack::new();
    for i in 0..=100 {
        let aa = (i & 0b1) != 0;
        let j = 100 - i;
        stack.save();
        stack.clip_rect(
            &Rect::from_ltrb(
                int_to_scalar(i),
                int_to_scalar(j) + 0.5,
                K_TARGET_BOUNDS.width(),
                K_TARGET_BOUNDS.height(),
            ),
            Matrix::i(),
            ClipOp::Intersect,
            aa,
        );
    }
    reporter_assert!(reporter, stack.is_rrect(&K_TARGET_BOUNDS).is_none());
}

// Port of: tests/ClipStackTest.cpp#L828-L875 (chrome/m156)
def_test!(ClipStack, |reporter| {
    const G_RECTS: [IRect; 4] = [
        IRect::new(0, 0, 100, 100),
        IRect::new(25, 25, 125, 125),
        IRect::new(0, 0, 1000, 1000),
        IRect::new(0, 0, 75, 75),
    ];

    let mut stack = ClipStack::new();

    reporter_assert!(reporter, 0 == stack.save_count());
    assert_count(reporter, &stack, 0);

    for r in &G_RECTS {
        stack.clip_dev_rect(r, ClipOp::Intersect);
    }

    // all of the above rects should have been intersected, leaving only 1 rect
    let mut iter = B2TIter::new(&stack);
    let element = iter.next();
    let mut answer = Rect::new_empty();
    answer.set_ltrb(25.0, 25.0, 75.0, 75.0);

    reporter_assert!(reporter, element.is_some());
    let element = element.expect("the stack has an element");
    reporter_assert!(
        reporter,
        DeviceSpaceType::Rect == element.device_space_type()
    );
    reporter_assert!(reporter, ClipOp::Intersect == element.op());
    reporter_assert!(reporter, *element.device_space_rect() == answer);
    // now check that we only had one in our iterator
    reporter_assert!(reporter, iter.next().is_none());

    stack.reset();
    reporter_assert!(reporter, 0 == stack.save_count());
    assert_count(reporter, &stack, 0);

    test_assign_and_comparison(reporter);
    test_iterators(reporter);
    test_bounds(reporter, DeviceSpaceType::Rect);
    test_bounds(reporter, DeviceSpaceType::RRect);
    test_bounds(reporter, DeviceSpaceType::Path);
    test_is_wide_open(reporter);
    test_rect_merging(reporter);
    test_rect_replace(reporter);
    test_rect_inverse_fill(reporter);
    test_path_replace(reporter);
    test_quick_contains(reporter);
    test_invfill_diff_bug(reporter);
    test_is_rrect_deep_rect_stack(reporter);
});
