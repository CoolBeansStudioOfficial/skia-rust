// Copyright 2016 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkScan_AAAPath.cpp

//! Analytic antialiased path filling (`SkScan_AAAPath.cpp`, `SkScan::AAAFillPath`).
//!
//! The following is a high-level overview of our analytic anti-aliasing algorithm (from the C++).
//! We consider a path as a collection of line segments, as quadratic/cubic curves are converted
//! to small line segments. Without loss of generality, let's assume that the draw region is
//! `[0, W] x [0, H]`.
//!
//! Our algorithm is based on horizontal scan lines (`y = c_i`) as the previous sampling-based
//! algorithm did. However, our algorithm uses non-equal-spaced scan lines, while the previous
//! method always uses equal-spaced scan lines, such as `(y = 1/2 + 0, 1/2 + 1, 1/2 + 2, ...)` in
//! the previous non-AA algorithm, and `(y = 1/8 + 1/4, 1/8 + 2/4, 1/8 + 3/4, ...)` in the previous
//! 16-supersampling AA algorithm.
//!
//! Our algorithm contains scan lines `y = c_i` for `c_i` that is either:
//!
//! 1. an integer between `[0, H]`
//! 2. the y value of a line segment endpoint
//! 3. the y value of an intersection of two line segments
//!
//! For two consecutive scan lines `y = c_i`, `y = c_{i+1}`, we analytically compute the coverage
//! of this horizontal strip of our path on each pixel. This can be done very efficiently because
//! the strip of our path now only consists of trapezoids whose top and bottom edges are
//! `y = c_i`, `y = c_{i+1}` (this includes rectangles and triangles as special cases).
//!
//! The coverage of a single pixel against such a trapezoid is the intersection area of a
//! rectangle (e.g., `[0, 1] x [c_i, c_{i+1}]`) and our trapezoid. Instead of computing that area
//! directly, we compute the excluded areas left and right of the trapezoid, which are simple
//! trapezoids (or their opposites are).
//!
//! skia-rust: the `AdditiveBlitter` class family becomes the [`AdditiveBlitter`] trait with
//! [`MaskAdditiveBlitter`] and [`RunBasedAdditiveBlitter`] (`SafeRLEAdditiveBlitter` is the
//! latter with `safe = true`). Their C++ destructors (which blit the mask or flush the last row)
//! are `Drop` impls. A "mask row" (`SkAlpha* maskRow`, a pointer into the mask) is an offset into
//! the mask blitter's storage ([`MaskRow`]).

use skia_rust_core::color::Alpha;
use skia_rust_core::fixed::{
    FIXED_1, Fixed, fixed_ceil_to_fixed, fixed_ceil_to_int, fixed_floor_to_fixed,
    fixed_floor_to_int, fixed_mul, fixed_round_to_int, int_to_fixed,
};
use skia_rust_core::math::{MAX_S32, MIN_S32};
use skia_rust_core::path_raw::PathRaw;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::{Contains, IRect, Rect, RoundOut};
use skia_rust_core::safe32::{abs32, sat_add, sat_sub};
use skia_rust_core::t_sort::t_q_sort;

use crate::analytic_edge::{AnalyticEdge, AnyAnalyticEdge, DEFAULT_ACCURACY, NO_EDGE};
use crate::blitter::Blitter;
use crate::edge_builder::{AnalyticEdgeBuilder, EdgeBuilder};
use crate::scan_priv::{
    backward_insert_edge_based_on_x, backward_insert_start, insert_edge_after, remove_edge,
};

mod additive_blitter;

use additive_blitter::safely_add_alpha;
pub use additive_blitter::{
    AdditiveBlitter, MaskAdditiveBlitter, MaskRow, RunBasedAdditiveBlitter,
};

// Return the alpha of a trapezoid whose height is 1
// Port of: src/core/SkScan_AAAPath.cpp#L534-L539 (chrome/m156)
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors SkTo<SkAlpha>
fn trapezoid_to_alpha(l1: Fixed, l2: Fixed) -> Alpha {
    debug_assert!(l1 >= 0 && l2 >= 0);
    let area = l1.wrapping_add(l2) / 2;
    (area >> 8) as Alpha
}

// The alpha of right-triangle (a, a*b)
// Port of: src/core/SkScan_AAAPath.cpp#L541-L559 (chrome/m156)
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors SkTo<SkAlpha>
fn partial_triangle_to_alpha(a: Fixed, b: Fixed) -> Alpha {
    debug_assert!(a <= FIXED_1);

    // Approximating...
    // SkFixed area = SkFixedMul(a, SkFixedMul(a,b)) / 2;
    let area = (a >> 11).wrapping_mul(a >> 11).wrapping_mul(b >> 11);

    ((area >> 8) & 0xFF) as Alpha
}

// `get_partial_alpha(SkAlpha alpha, SkFixed partialHeight)`
// Port of: src/core/SkScan_AAAPath.cpp#L561-L563 (chrome/m156)
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors SkToU8
fn get_partial_alpha_fixed(alpha: Alpha, partial_height: Fixed) -> Alpha {
    fixed_round_to_int(i32::from(alpha).wrapping_mul(partial_height)) as Alpha
}

// `get_partial_alpha(SkAlpha alpha, SkAlpha fullAlpha)`
// Port of: src/core/SkScan_AAAPath.cpp#L565-L567 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors the implicit int -> SkAlpha conversion
fn get_partial_alpha(alpha: Alpha, full_alpha: Alpha) -> Alpha {
    ((u32::from(alpha) * u32::from(full_alpha)) >> 8) as Alpha
}

// For SkFixed that's close to SK_Fixed1, we can't convert it to alpha by just shifting right.
// For example, when f = SK_Fixed1, right shifting 8 will get 256, but we need 255.
// This is rarely the problem so we'll only use this for blitting rectangles.
// Port of: src/core/SkScan_AAAPath.cpp#L569-L575 (chrome/m156)
fn fixed_to_alpha(f: Fixed) -> Alpha {
    debug_assert!(f <= FIXED_1);
    get_partial_alpha_fixed(0xFF, f)
}

// Suppose that line (l1, y)-(r1, y+1) intersects with (l2, y)-(r2, y+1),
// approximate (very coarsely) the x coordinate of the intersection.
// Port of: src/core/SkScan_AAAPath.cpp#L577-L587 (chrome/m156)
fn approximate_intersection(mut l1: Fixed, mut r1: Fixed, mut l2: Fixed, mut r2: Fixed) -> Fixed {
    if l1 > r1 {
        std::mem::swap(&mut l1, &mut r1);
    }
    if l2 > r2 {
        std::mem::swap(&mut l2, &mut r2);
    }
    l1.max(l2).wrapping_add(r1.min(r2)) / 2
}

// The int -> SkAlpha conversion of the C++ assignments `alphas[i] = <int expression>`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_alpha(v: i32) -> Alpha {
    v as Alpha
}

// Here we always send in l < SK_Fixed1, and the first alpha we want to compute is alphas[0]
// Port of: src/core/SkScan_AAAPath.cpp#L589-L614 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // R >= 2 here
fn compute_alpha_above_line(
    alphas: &mut [Alpha],
    l: Fixed,
    r: Fixed,
    dy: Fixed,
    full_alpha: Alpha,
) {
    debug_assert!(l <= r);
    debug_assert_eq!(l >> 16, 0);
    let big_r = fixed_ceil_to_int(r);
    if big_r == 0 {
        // nothing to do
    } else if big_r == 1 {
        // `((R << 17) - l - r) >> 9` converts to SkAlpha: the (SkAlpha, SkAlpha) overload.
        alphas[0] = get_partial_alpha(
            to_alpha((big_r << 17).wrapping_sub(l).wrapping_sub(r) >> 9),
            full_alpha,
        );
    } else {
        let first = FIXED_1 - l; // horizontal edge length of the left-most triangle
        let last = r.wrapping_sub((big_r - 1) << 16); // horizontal edge length of the right-most triangle
        let first_h = fixed_mul(first, dy); // vertical edge of the left-most triangle
        alphas[0] = to_alpha(fixed_mul(first, first_h) >> 9); // triangle alpha
        let mut alpha16 = sat_add(first_h, dy >> 1); // rectangle plus triangle
        for a in &mut alphas[1..(big_r - 1) as usize] {
            *a = to_alpha(alpha16 >> 8);
            alpha16 = sat_add(alpha16, dy);
        }
        alphas[(big_r - 1) as usize] =
            to_alpha(i32::from(full_alpha) - i32::from(partial_triangle_to_alpha(last, dy)));
    }
}

// Here we always send in l < SK_Fixed1, and the first alpha we want to compute is alphas[0]
// Port of: src/core/SkScan_AAAPath.cpp#L616-L641 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // R >= 2 here
fn compute_alpha_below_line(
    alphas: &mut [Alpha],
    l: Fixed,
    r: Fixed,
    dy: Fixed,
    full_alpha: Alpha,
) {
    debug_assert!(l <= r);
    debug_assert_eq!(l >> 16, 0);
    let big_r = fixed_ceil_to_int(r);
    if big_r == 0 {
        // nothing to do
    } else if big_r == 1 {
        alphas[0] = get_partial_alpha(trapezoid_to_alpha(l, r), full_alpha);
    } else {
        let first = FIXED_1 - l; // horizontal edge length of the left-most triangle
        let last = r.wrapping_sub((big_r - 1) << 16); // horizontal edge length of the right-most triangle
        let last_h = fixed_mul(last, dy); // vertical edge of the right-most triangle
        alphas[(big_r - 1) as usize] = to_alpha(fixed_mul(last, last_h) >> 9); // triangle alpha
        let mut alpha16 = sat_add(last_h, dy >> 1); // rectangle plus triangle
        let mut i = big_r - 2;
        while i > 0 {
            alphas[i as usize] = to_alpha((alpha16 >> 8) & 0xFF);
            alpha16 = sat_add(alpha16, dy);
            i -= 1;
        }
        alphas[0] =
            to_alpha(i32::from(full_alpha) - i32::from(partial_triangle_to_alpha(first, dy)));
    }
}

// Note that if fullAlpha != 0xFF, we'll multiply alpha by fullAlpha
// Port of: src/core/SkScan_AAAPath.cpp#L643-L664 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn blit_single_alpha(
    blitter: &mut dyn AdditiveBlitter,
    y: i32,
    x: i32,
    alpha: Alpha,
    full_alpha: Alpha,
    mask_row: Option<MaskRow>,
    no_real_blitter: bool,
) {
    if let Some(row) = mask_row {
        if full_alpha == 0xFF && !no_real_blitter {
            // noRealBlitter is needed for concave paths
            *blitter.mask_byte(row, x) = alpha;
        } else {
            safely_add_alpha(
                blitter.mask_byte(row, x),
                get_partial_alpha(alpha, full_alpha),
            );
        }
    } else if full_alpha == 0xFF && !no_real_blitter {
        blitter.real_blitter().blit_v(x, y, 1, alpha);
    } else {
        blitter.blit_anti_h_alpha(x, y, get_partial_alpha(alpha, full_alpha));
    }
}

// Port of: src/core/SkScan_AAAPath.cpp#L666-L685 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn blit_two_alphas(
    blitter: &mut dyn AdditiveBlitter,
    y: i32,
    x: i32,
    a1: Alpha,
    a2: Alpha,
    full_alpha: Alpha,
    mask_row: Option<MaskRow>,
    no_real_blitter: bool,
) {
    if let Some(row) = mask_row {
        safely_add_alpha(blitter.mask_byte(row, x), a1);
        safely_add_alpha(blitter.mask_byte(row, x + 1), a2);
    } else if full_alpha == 0xFF && !no_real_blitter {
        blitter
            .real_blitter()
            .blit_anti_h2(x, y, u32::from(a1), u32::from(a2));
    } else {
        blitter.blit_anti_h_alpha(x, y, a1);
        blitter.blit_anti_h_alpha(x + 1, y, a2);
    }
}

// Port of: src/core/SkScan_AAAPath.cpp#L687-L705 (chrome/m156)
fn blit_full_alpha(
    blitter: &mut dyn AdditiveBlitter,
    y: i32,
    x: i32,
    len: i32,
    full_alpha: Alpha,
    mask_row: Option<MaskRow>,
    no_real_blitter: bool,
) {
    if let Some(row) = mask_row {
        for i in 0..len {
            safely_add_alpha(blitter.mask_byte(row, x + i), full_alpha);
        }
    } else if full_alpha == 0xFF && !no_real_blitter {
        blitter.real_blitter().blit_h(x, y, len);
    } else {
        blitter.blit_anti_h_width(x, y, len, full_alpha);
    }
}

// `alphas[i] > a ? alphas[i] - a : 0`
fn sub_or_zero(alpha: &mut Alpha, a: Alpha) {
    *alpha = (*alpha).saturating_sub(a);
}

// Port of: src/core/SkScan_AAAPath.cpp#L707-L804 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
#[allow(clippy::many_single_char_names)] // mirrors the C++ names
#[allow(clippy::similar_names)] // mirrors the C++ names
#[allow(clippy::cast_sign_loss)] // indices are non-negative (ul <= ur, ll <= lr)
fn blit_aaa_trapezoid_row(
    blitter: &mut dyn AdditiveBlitter,
    y: i32,
    ul: Fixed,
    ur: Fixed,
    ll: Fixed,
    lr: Fixed,
    l_dy: Fixed,
    r_dy: Fixed,
    full_alpha: Alpha,
    mask_row: Option<MaskRow>,
    no_real_blitter: bool,
) {
    const QUICK_LEN: usize = 31;
    let big_l = fixed_floor_to_int(ul);
    let big_r = fixed_ceil_to_int(lr);
    let len = big_r - big_l;

    if len == 1 {
        let alpha = trapezoid_to_alpha(ur.wrapping_sub(ul), lr.wrapping_sub(ll));
        blit_single_alpha(
            blitter,
            y,
            big_l,
            alpha,
            full_alpha,
            mask_row,
            no_real_blitter,
        );
        return;
    }

    let n = len as usize;
    let mut quick_alphas = [0 as Alpha; QUICK_LEN + 1];
    let mut quick_temp = [0 as Alpha; QUICK_LEN + 1];
    let mut quick_runs = [0i16; QUICK_LEN + 1];
    let mut heap_alphas;
    let mut heap_temp;
    let mut heap_runs;
    let (alphas, temp_alphas, runs): (&mut [Alpha], &mut [Alpha], &mut [i16]) = if n <= QUICK_LEN {
        (&mut quick_alphas, &mut quick_temp, &mut quick_runs)
    } else {
        heap_alphas = vec![0 as Alpha; n + 1];
        heap_temp = vec![0 as Alpha; n + 1];
        heap_runs = vec![0i16; n + 1];
        (&mut heap_alphas, &mut heap_temp, &mut heap_runs)
    };

    for i in 0..n {
        runs[i] = 1;
        alphas[i] = full_alpha;
    }
    runs[n] = 0;

    let u_l = fixed_floor_to_int(ul);
    let l_l = fixed_ceil_to_int(ll);
    if u_l + 2 == l_l {
        // We only need to compute two triangles, accelerate this special case
        let first = int_to_fixed(u_l).wrapping_add(FIXED_1).wrapping_sub(ul);
        let second = ll.wrapping_sub(ul).wrapping_sub(first);
        let a1 =
            to_alpha(i32::from(full_alpha) - i32::from(partial_triangle_to_alpha(first, l_dy)));
        let a2 = partial_triangle_to_alpha(second, l_dy);
        sub_or_zero(&mut alphas[0], a1);
        sub_or_zero(&mut alphas[1], a2);
    } else {
        compute_alpha_below_line(
            &mut temp_alphas[(u_l - big_l) as usize..],
            ul.wrapping_sub(int_to_fixed(u_l)),
            ll.wrapping_sub(int_to_fixed(u_l)),
            l_dy,
            full_alpha,
        );
        for i in u_l..l_l {
            let k = (i - big_l) as usize;
            let t = temp_alphas[k];
            sub_or_zero(&mut alphas[k], t);
        }
    }

    let u_r = fixed_floor_to_int(ur);
    let l_r = fixed_ceil_to_int(lr);
    if u_r + 2 == l_r {
        // We only need to compute two triangles, accelerate this special case
        let first = int_to_fixed(u_r).wrapping_add(FIXED_1).wrapping_sub(ur);
        let second = lr.wrapping_sub(ur).wrapping_sub(first);
        let a1 = partial_triangle_to_alpha(first, r_dy);
        let a2 =
            to_alpha(i32::from(full_alpha) - i32::from(partial_triangle_to_alpha(second, r_dy)));
        sub_or_zero(&mut alphas[n - 2], a1);
        sub_or_zero(&mut alphas[n - 1], a2);
    } else {
        compute_alpha_above_line(
            &mut temp_alphas[(u_r - big_l) as usize..],
            ur.wrapping_sub(int_to_fixed(u_r)),
            lr.wrapping_sub(int_to_fixed(u_r)),
            r_dy,
            full_alpha,
        );
        for i in u_r..l_r {
            let k = (i - big_l) as usize;
            let t = temp_alphas[k];
            sub_or_zero(&mut alphas[k], t);
        }
    }

    if let Some(row) = mask_row {
        for i in 0..len {
            safely_add_alpha(blitter.mask_byte(row, big_l + i), alphas[i as usize]);
        }
    } else if full_alpha == 0xFF && !no_real_blitter {
        // Real blitter is faster than RunBasedAdditiveBlitter
        blitter.real_blitter().blit_anti_h(big_l, y, alphas, runs);
    } else {
        blitter.blit_anti_h_alphas(big_l, y, &alphas[..n]);
    }
}

// Port of: src/core/SkScan_AAAPath.cpp#L806-L947 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
#[allow(clippy::too_many_lines)] // mirrors the C++ function
#[allow(clippy::many_single_char_names)] // mirrors the C++ names
#[allow(clippy::similar_names)] // mirrors the C++ names
fn blit_trapezoid_row(
    blitter: &mut dyn AdditiveBlitter,
    y: i32,
    mut ul: Fixed,
    mut ur: Fixed,
    mut ll: Fixed,
    mut lr: Fixed,
    l_dy: Fixed,
    r_dy: Fixed,
    full_alpha: Alpha,
    mask_row: Option<MaskRow>,
    no_real_blitter: bool,
) {
    debug_assert!(l_dy >= 0 && r_dy >= 0); // We should only send in the absolte value

    if ul > ur {
        return;
    }

    // Edge crosses. Approximate it. This should only happend due to precision limit,
    // so the approximation could be very coarse.
    if ll > lr {
        ll = approximate_intersection(ul, ll, ur, lr);
        lr = ll;
    }

    if ul == ur && ll == lr {
        return; // empty trapzoid
    }

    // We're going to use the left line ul-ll and the rite line ur-lr
    // to exclude the area that's not covered by the path.
    // Swapping (ul, ll) or (ur, lr) won't affect that exclusion
    // so we'll do that for simplicity.
    if ul > ll {
        std::mem::swap(&mut ul, &mut ll);
    }
    if ur > lr {
        std::mem::swap(&mut ur, &mut lr);
    }

    let join_left = fixed_ceil_to_fixed(ll);
    let join_rite = fixed_floor_to_fixed(ur);
    if join_left <= join_rite {
        // There's a rect from joinLeft to joinRite that we can blit
        if ul < join_left {
            let len = fixed_ceil_to_int(join_left.wrapping_sub(ul));
            if len == 1 {
                let alpha =
                    trapezoid_to_alpha(join_left.wrapping_sub(ul), join_left.wrapping_sub(ll));
                blit_single_alpha(
                    blitter,
                    y,
                    ul >> 16,
                    alpha,
                    full_alpha,
                    mask_row,
                    no_real_blitter,
                );
            } else if len == 2 {
                let first = join_left.wrapping_sub(FIXED_1).wrapping_sub(ul);
                let second = ll.wrapping_sub(ul).wrapping_sub(first);
                let a1 = partial_triangle_to_alpha(first, l_dy);
                let a2 = to_alpha(
                    i32::from(full_alpha) - i32::from(partial_triangle_to_alpha(second, l_dy)),
                );
                blit_two_alphas(
                    blitter,
                    y,
                    ul >> 16,
                    a1,
                    a2,
                    full_alpha,
                    mask_row,
                    no_real_blitter,
                );
            } else {
                blit_aaa_trapezoid_row(
                    blitter,
                    y,
                    ul,
                    join_left,
                    ll,
                    join_left,
                    l_dy,
                    MAX_S32,
                    full_alpha,
                    mask_row,
                    no_real_blitter,
                );
            }
        }
        // SkAAClip requires that we blit from left to right.
        // Hence we must blit [ul, joinLeft] before blitting [joinLeft, joinRite]
        if join_left < join_rite {
            blit_full_alpha(
                blitter,
                y,
                fixed_floor_to_int(join_left),
                fixed_floor_to_int(join_rite.wrapping_sub(join_left)),
                full_alpha,
                mask_row,
                no_real_blitter,
            );
        }
        if lr > join_rite {
            let len = fixed_ceil_to_int(lr.wrapping_sub(join_rite));
            if len == 1 {
                let alpha =
                    trapezoid_to_alpha(ur.wrapping_sub(join_rite), lr.wrapping_sub(join_rite));
                blit_single_alpha(
                    blitter,
                    y,
                    join_rite >> 16,
                    alpha,
                    full_alpha,
                    mask_row,
                    no_real_blitter,
                );
            } else if len == 2 {
                let first = join_rite.wrapping_add(FIXED_1).wrapping_sub(ur);
                let second = lr.wrapping_sub(ur).wrapping_sub(first);
                let a1 = to_alpha(
                    i32::from(full_alpha) - i32::from(partial_triangle_to_alpha(first, r_dy)),
                );
                let a2 = partial_triangle_to_alpha(second, r_dy);
                blit_two_alphas(
                    blitter,
                    y,
                    join_rite >> 16,
                    a1,
                    a2,
                    full_alpha,
                    mask_row,
                    no_real_blitter,
                );
            } else {
                blit_aaa_trapezoid_row(
                    blitter,
                    y,
                    join_rite,
                    ur,
                    join_rite,
                    lr,
                    MAX_S32,
                    r_dy,
                    full_alpha,
                    mask_row,
                    no_real_blitter,
                );
            }
        }
    } else {
        blit_aaa_trapezoid_row(
            blitter,
            y,
            ul,
            ur,
            ll,
            lr,
            l_dy,
            r_dy,
            full_alpha,
            mask_row,
            no_real_blitter,
        );
    }
}

// Port of: src/core/SkScan_AAAPath.cpp#L949-L959 (chrome/m156)
fn compare_edges(a: &AnalyticEdge, b: &AnalyticEdge) -> bool {
    if a.upper_y != b.upper_y {
        return a.upper_y < b.upper_y;
    }

    if a.x != b.x {
        return a.x < b.x;
    }

    a.dx < b.dx
}

// Sorts the edges and links them in sorted order; returns the first and last edge.
// Port of: src/core/SkScan_AAAPath.cpp#L961-L972 (chrome/m156)
fn sort_edges(edges: &mut [AnyAnalyticEdge]) -> (usize, usize) {
    let count = edges.len();
    let mut list: Vec<usize> = (0..count).collect();
    t_q_sort(&mut list, |&a, &b| compare_edges(&edges[a], &edges[b]));

    // now make the edges linked in sorted order
    for i in 1..count {
        edges[list[i - 1]].next = list[i];
        edges[list[i]].prev = list[i - 1];
    }

    (list[0], list[count - 1])
}

// Port of: src/core/SkScan_AAAPath.cpp#L974-L986 (chrome/m156)
fn validate_sort(edges: &[AnyAnalyticEdge], mut edge: usize) {
    if cfg!(debug_assertions) {
        let mut y = int_to_fixed(-32768);

        while edges[edge].upper_y != MAX_S32 {
            validate(edges, edge);
            debug_assert!(y <= edges[edge].upper_y);

            y = edges[edge].upper_y;
            edge = edges[edge].next;
        }
    }
}

// `SkAnalyticEdge::validate`
// Port of: src/core/SkAnalyticEdge.h#L99-L106 (chrome/m156)
fn validate(edges: &[AnyAnalyticEdge], edge: usize) {
    let e = &edges[edge];
    debug_assert!(e.prev != NO_EDGE && e.next != NO_EDGE);
    debug_assert_eq!(edges[e.prev].next, edge);
    debug_assert_eq!(edges[e.next].prev, edge);

    debug_assert!(e.upper_y < e.lower_y);
}

// For an edge, we consider it smooth if the Dx doesn't change much, and Dy is large enough
// For curves that are updating, the Dx is not changing much if fQDx/fCDx and fQDy/fCDy are
// relatively large compared to fQDDx/QCDDx and fQDDy/fCDDy
// Port of: src/core/SkScan_AAAPath.cpp#L988-L1009 (chrome/m156)
fn is_smooth_enough_edge(this_edge: &AnyAnalyticEdge, next_edge: &AnyAnalyticEdge) -> bool {
    match this_edge {
        AnyAnalyticEdge::Cubic(c_edge) if c_edge.curve_count < 0 => {
            let ddshift = i32::from(c_edge.curve_shift);
            return abs32(c_edge.cdx) >> 1 >= abs32(c_edge.cddx) >> ddshift
                && abs32(c_edge.cdy) >> 1 >= abs32(c_edge.cddy) >> ddshift
                // current Dy is (fCDy - (fCDDy >> ddshift)) >> dshift
                && (c_edge.cdy.wrapping_sub(c_edge.cddy >> ddshift)) >> c_edge.to_fixed_shift
                    >= FIXED_1;
        }
        AnyAnalyticEdge::Quad(q_edge) if q_edge.curve_count > 0 => {
            return abs32(q_edge.qdx) >> 1 >= abs32(q_edge.qddx)
                && abs32(q_edge.qdy) >> 1 >= abs32(q_edge.qddy)
                // current Dy is (fQDy - fQDDy) >> shift
                && (q_edge.qdy.wrapping_sub(q_edge.qddy)) >> q_edge.curve_shift >= FIXED_1;
        }
        _ => debug_assert_eq!(this_edge.curve_count, 0),
    }
    // DDx should be small and Dy should be large
    abs32(sat_sub(next_edge.dx, this_edge.dx)) <= FIXED_1
        && next_edge.lower_y.wrapping_sub(next_edge.upper_y) >= FIXED_1
}

// Check if the leftE and riteE are changing smoothly in terms of fDX.
// If yes, we can later skip the fractional y and directly jump to integer y.
// Port of: src/core/SkScan_AAAPath.cpp#L1011-L1036 (chrome/m156)
fn is_smooth_enough(
    edges: &[AnyAnalyticEdge],
    left_e: usize,
    rite_e: usize,
    mut curr_e: usize,
    stop_y: i32,
) -> bool {
    if edges[curr_e].upper_y >= skia_rust_core::math::left_shift(stop_y, 16) {
        return false; // We're at the end so we won't skip anything
    }
    if edges[left_e].lower_y.wrapping_add(FIXED_1) < edges[rite_e].lower_y {
        return is_smooth_enough_edge(&edges[left_e], &edges[curr_e]); // Only leftE is changing
    } else if edges[left_e].lower_y > edges[rite_e].lower_y.wrapping_add(FIXED_1) {
        return is_smooth_enough_edge(&edges[rite_e], &edges[curr_e]); // Only riteE is changing
    }

    // Now both edges are changing, find the second next edge
    let mut next_curr_e = edges[curr_e].next;
    if edges[next_curr_e].upper_y >= stop_y << 16 {
        // Check if we're at the end
        return false;
    }
    // Ensure that currE is the next left edge and nextCurrE is the next right edge. Swap if not.
    if edges[next_curr_e].upper_x < edges[curr_e].upper_x {
        std::mem::swap(&mut curr_e, &mut next_curr_e);
    }
    is_smooth_enough_edge(&edges[left_e], &edges[curr_e])
        && is_smooth_enough_edge(&edges[rite_e], &edges[next_curr_e])
}

// Port of: src/core/SkScan_AAAPath.cpp#L1038-L1305 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
#[allow(clippy::too_many_lines)] // mirrors the C++ function
#[allow(clippy::similar_names)] // mirrors the C++ names
fn aaa_walk_convex_edges(
    edges: &mut [AnyAnalyticEdge],
    prev_head: usize,
    blitter: &mut dyn AdditiveBlitter,
    _start_y: i32,
    stop_y: i32,
    left_bound: Fixed,
    rite_bound: Fixed,
    is_using_mask: bool,
) {
    validate_sort(edges, edges[prev_head].next);

    let mut left_e = edges[prev_head].next;
    let mut rite_e = edges[left_e].next;
    let mut curr_e = edges[rite_e].next;

    let mut y = edges[left_e].upper_y.max(edges[rite_e].upper_y);

    loop {
        // We have to check fLowerY first because some edges might be alone (e.g., there's only
        // a left edge but no right edge in a given y scan line) due to precision limit.
        while edges[left_e].lower_y <= y {
            // Due to smooth jump, we may pass multiple short edges
            if !edges[left_e].update(y) {
                if fixed_floor_to_int(edges[curr_e].upper_y) >= stop_y {
                    return; // goto END_WALK
                }
                left_e = curr_e;
                curr_e = edges[curr_e].next;
            }
        }
        while edges[rite_e].lower_y <= y {
            // Due to smooth jump, we may pass multiple short edges
            if !edges[rite_e].update(y) {
                if fixed_floor_to_int(edges[curr_e].upper_y) >= stop_y {
                    return; // goto END_WALK
                }
                rite_e = curr_e;
                curr_e = edges[curr_e].next;
            }
        }

        // check our bottom clip
        if fixed_floor_to_int(y) >= stop_y {
            break;
        }

        debug_assert!(fixed_floor_to_int(edges[left_e].upper_y) <= stop_y);
        debug_assert!(fixed_floor_to_int(edges[rite_e].upper_y) <= stop_y);

        edges[left_e].go_y(y);
        edges[rite_e].go_y(y);

        if edges[left_e].x > edges[rite_e].x
            || (edges[left_e].x == edges[rite_e].x && edges[left_e].dx > edges[rite_e].dx)
        {
            std::mem::swap(&mut left_e, &mut rite_e);
        }

        let mut local_bot_fixed = edges[left_e].lower_y.min(edges[rite_e].lower_y);
        if is_smooth_enough(edges, left_e, rite_e, curr_e, stop_y) {
            local_bot_fixed = fixed_ceil_to_fixed(local_bot_fixed);
        }
        local_bot_fixed = local_bot_fixed.min(int_to_fixed(stop_y));

        let mut left = left_bound.max(edges[left_e].x);
        let d_left = edges[left_e].dx;
        let mut rite = rite_bound.min(edges[rite_e].x);
        let d_rite = edges[rite_e].dx;
        if 0 == (d_left | d_rite) {
            let full_left = fixed_ceil_to_int(left);
            let full_rite = fixed_floor_to_int(rite);
            let partial_left = int_to_fixed(full_left).wrapping_sub(left);
            let partial_rite = rite.wrapping_sub(int_to_fixed(full_rite));
            let full_top = fixed_ceil_to_int(y);
            let full_bot = fixed_floor_to_int(local_bot_fixed);
            let mut partial_top = int_to_fixed(full_top).wrapping_sub(y);
            let mut partial_bot = local_bot_fixed.wrapping_sub(int_to_fixed(full_bot));
            if full_top > full_bot {
                // The rectangle is within one pixel height...
                partial_top -= FIXED_1 - partial_bot;
                partial_bot = 0;
            }

            if full_rite >= full_left {
                if partial_top > 0 {
                    // blit first partial row
                    if partial_left > 0 {
                        blitter.blit_anti_h_alpha(
                            full_left - 1,
                            full_top - 1,
                            fixed_to_alpha(fixed_mul(partial_top, partial_left)),
                        );
                    }
                    blitter.blit_anti_h_width(
                        full_left,
                        full_top - 1,
                        full_rite - full_left,
                        fixed_to_alpha(partial_top),
                    );
                    if partial_rite > 0 {
                        blitter.blit_anti_h_alpha(
                            full_rite,
                            full_top - 1,
                            fixed_to_alpha(fixed_mul(partial_top, partial_rite)),
                        );
                    }
                    blitter.flush_if_y_changed(y, y.wrapping_add(partial_top));
                }

                // Blit all full-height rows from fullTop to fullBot
                if full_bot > full_top
                    // SkAAClip cannot handle the empty rect so check the non-emptiness here
                    // (bug chromium:662800)
                    && (full_rite > full_left
                        || fixed_to_alpha(partial_left) > 0
                        || fixed_to_alpha(partial_rite) > 0)
                {
                    blitter.real_blitter().blit_anti_rect(
                        full_left - 1,
                        full_top,
                        full_rite - full_left,
                        full_bot - full_top,
                        fixed_to_alpha(partial_left),
                        fixed_to_alpha(partial_rite),
                    );
                }

                if partial_bot > 0 {
                    // blit last partial row
                    if partial_left > 0 {
                        blitter.blit_anti_h_alpha(
                            full_left - 1,
                            full_bot,
                            fixed_to_alpha(fixed_mul(partial_bot, partial_left)),
                        );
                    }
                    blitter.blit_anti_h_width(
                        full_left,
                        full_bot,
                        full_rite - full_left,
                        fixed_to_alpha(partial_bot),
                    );
                    if partial_rite > 0 {
                        blitter.blit_anti_h_alpha(
                            full_rite,
                            full_bot,
                            fixed_to_alpha(fixed_mul(partial_bot, partial_rite)),
                        );
                    }
                }
            } else {
                // Normal conditions, this means left and rite are within the same pixel, but if
                // both left and rite were < leftBounds or > rightBounds, both edges are clipped
                // and we should not do any blitting (particularly since the negative width
                // saturates to full alpha).
                let width = rite.wrapping_sub(left);
                if width > 0 {
                    if partial_top > 0 {
                        blitter.blit_anti_h_width(
                            full_left - 1,
                            full_top - 1,
                            1,
                            fixed_to_alpha(fixed_mul(partial_top, width)),
                        );
                        blitter.flush_if_y_changed(y, y.wrapping_add(partial_top));
                    }
                    if full_bot > full_top {
                        blitter.real_blitter().blit_v(
                            full_left - 1,
                            full_top,
                            full_bot - full_top,
                            fixed_to_alpha(width),
                        );
                    }
                    if partial_bot > 0 {
                        blitter.blit_anti_h_width(
                            full_left - 1,
                            full_bot,
                            1,
                            fixed_to_alpha(fixed_mul(partial_bot, width)),
                        );
                    }
                }
            }

            y = local_bot_fixed;
        } else {
            // The following constant are used to snap X
            // We snap X mainly for speedup (no tiny triangle) and
            // avoiding edge cases caused by precision errors
            const SNAP_DIGIT: Fixed = FIXED_1 >> 4;
            const SNAP_HALF: Fixed = SNAP_DIGIT >> 1;
            const SNAP_MASK: Fixed = -1 ^ (SNAP_DIGIT - 1);
            left = left.wrapping_add(SNAP_HALF);
            rite = rite.wrapping_add(SNAP_HALF); // For fast rounding

            // Number of blit_trapezoid_row calls we'll have
            let mut count = fixed_ceil_to_int(local_bot_fixed) - fixed_floor_to_int(y);

            // If we're using mask blitter, we advance the mask row in this function
            // to save some "if" condition checks.
            let mut mask_row = if is_using_mask {
                Some(blitter.get_row(y >> 16))
            } else {
                None
            };

            // Instead of writing one loop that handles both partial-row blit_trapezoid_row
            // and full-row trapezoid_row together, we use the following 3-stage flow to
            // handle partial-row blit and full-row blit separately. It will save us much time
            // on changing y, left, and rite.
            if count > 1 {
                #[allow(clippy::cast_possible_wrap)] // mirrors the (int) cast
                if (y & (0xFFFF_0000_u32 as i32)) != y {
                    // There's a partial-row on the top
                    count -= 1;
                    let next_y = fixed_ceil_to_fixed(y.wrapping_add(1));
                    let d_y = next_y.wrapping_sub(y);
                    let next_left = left.wrapping_add(fixed_mul(d_left, d_y));
                    let next_rite = rite.wrapping_add(fixed_mul(d_rite, d_y));
                    debug_assert!(
                        (left & SNAP_MASK) >= left_bound
                            && (rite & SNAP_MASK) <= rite_bound
                            && (next_left & SNAP_MASK) >= left_bound
                            && (next_rite & SNAP_MASK) <= rite_bound
                    );
                    blit_trapezoid_row(
                        blitter,
                        y >> 16,
                        left & SNAP_MASK,
                        rite & SNAP_MASK,
                        next_left & SNAP_MASK,
                        next_rite & SNAP_MASK,
                        edges[left_e].dy,
                        edges[rite_e].dy,
                        get_partial_alpha_fixed(0xFF, d_y),
                        mask_row,
                        /*no_real_blitter=*/ false,
                    );
                    blitter.flush_if_y_changed(y, next_y);
                    left = next_left;
                    rite = next_rite;
                    y = next_y;
                }

                while count > 1 {
                    // Full rows in the middle
                    count -= 1;
                    if is_using_mask {
                        mask_row = Some(blitter.get_row(y >> 16));
                    }
                    let next_y = y.wrapping_add(FIXED_1);
                    let next_left = left.wrapping_add(d_left);
                    let next_rite = rite.wrapping_add(d_rite);
                    debug_assert!(
                        (left & SNAP_MASK) >= left_bound
                            && (rite & SNAP_MASK) <= rite_bound
                            && (next_left & SNAP_MASK) >= left_bound
                            && (next_rite & SNAP_MASK) <= rite_bound
                    );
                    blit_trapezoid_row(
                        blitter,
                        y >> 16,
                        left & SNAP_MASK,
                        rite & SNAP_MASK,
                        next_left & SNAP_MASK,
                        next_rite & SNAP_MASK,
                        edges[left_e].dy,
                        edges[rite_e].dy,
                        0xFF,
                        mask_row,
                        /*no_real_blitter=*/ false,
                    );
                    blitter.flush_if_y_changed(y, next_y);
                    left = next_left;
                    rite = next_rite;
                    y = next_y;
                }
            }

            if is_using_mask {
                mask_row = Some(blitter.get_row(y >> 16));
            }

            let d_y = local_bot_fixed.wrapping_sub(y); // partial-row on the bottom
            debug_assert!(d_y <= FIXED_1);
            // Smooth jumping to integer y may make the last nextLeft/nextRite out of bound.
            // Take them back into the bound here.
            // Note that we substract kSnapHalf later so we have to add them to
            // leftBound/riteBound
            let next_left = left
                .wrapping_add(fixed_mul(d_left, d_y))
                .max(left_bound.wrapping_add(SNAP_HALF));
            let next_rite = rite
                .wrapping_add(fixed_mul(d_rite, d_y))
                .min(rite_bound.wrapping_add(SNAP_HALF));
            debug_assert!(
                (left & SNAP_MASK) >= left_bound
                    && (rite & SNAP_MASK) <= rite_bound
                    && (next_left & SNAP_MASK) >= left_bound
                    && (next_rite & SNAP_MASK) <= rite_bound
            );
            blit_trapezoid_row(
                blitter,
                y >> 16,
                left & SNAP_MASK,
                rite & SNAP_MASK,
                next_left & SNAP_MASK,
                next_rite & SNAP_MASK,
                edges[left_e].dy,
                edges[rite_e].dy,
                get_partial_alpha_fixed(0xFF, d_y),
                mask_row,
                /*no_real_blitter=*/ false,
            );
            blitter.flush_if_y_changed(y, local_bot_fixed);
            left = next_left;
            rite = next_rite;
            y = local_bot_fixed;
            left = left.wrapping_sub(SNAP_HALF);
            rite = rite.wrapping_sub(SNAP_HALF);
        }

        edges[left_e].x = left;
        edges[rite_e].x = rite;
        edges[left_e].y = y;
        edges[rite_e].y = y;
    }
    // END_WALK:
}

// Port of: src/core/SkScan_AAAPath.cpp#L1307-L1309 (chrome/m156)
fn update_next_next_y(y: Fixed, next_y: Fixed, next_next_y: &mut Fixed) {
    *next_next_y = if y > next_y && y < *next_next_y {
        y
    } else {
        *next_next_y
    };
}

// Port of: src/core/SkScan_AAAPath.cpp#L1311-L1315 (chrome/m156)
fn check_intersection(
    edges: &[AnyAnalyticEdge],
    edge: usize,
    next_y: Fixed,
    next_next_y: &mut Fixed,
) {
    let e = &edges[edge];
    let prev = &edges[e.prev];
    if prev.prev != NO_EDGE && prev.x.wrapping_add(prev.dx) > e.x.wrapping_add(e.dx) {
        *next_next_y = next_y.wrapping_add(FIXED_1 >> DEFAULT_ACCURACY);
    }
}

// Port of: src/core/SkScan_AAAPath.cpp#L1317-L1321 (chrome/m156)
fn check_intersection_fwd(
    edges: &[AnyAnalyticEdge],
    edge: usize,
    next_y: Fixed,
    next_next_y: &mut Fixed,
) {
    let e = &edges[edge];
    let next = &edges[e.next];
    if next.next != NO_EDGE && e.x.wrapping_add(e.dx) > next.x.wrapping_add(next.dx) {
        *next_next_y = next_y.wrapping_add(FIXED_1 >> DEFAULT_ACCURACY);
    }
}

// Port of: src/core/SkScan_AAAPath.cpp#L1323-L1364 (chrome/m156)
fn insert_new_edges(
    edges: &mut [AnyAnalyticEdge],
    mut new_edge: usize,
    y: Fixed,
    next_next_y: &mut Fixed,
) {
    if edges[new_edge].upper_y > y {
        update_next_next_y(edges[new_edge].upper_y, y, next_next_y);
        return;
    }
    let prev = edges[new_edge].prev;
    if edges[prev].x <= edges[new_edge].x {
        while edges[new_edge].upper_y <= y {
            check_intersection(edges, new_edge, y, next_next_y);
            update_next_next_y(edges[new_edge].lower_y, y, next_next_y);
            new_edge = edges[new_edge].next;
        }
        update_next_next_y(edges[new_edge].upper_y, y, next_next_y);
        return;
    }
    // find first x pos to insert
    let mut start = backward_insert_start(edges, prev, edges[new_edge].x);
    // insert the lot, fixing up the links as we go
    loop {
        let next = edges[new_edge].next;
        let mut already_in_place = false;
        loop {
            if edges[start].next == new_edge {
                already_in_place = true; // goto nextEdge
                break;
            }
            let after = edges[start].next;
            if edges[after].x >= edges[new_edge].x {
                break;
            }
            debug_assert_ne!(start, after);
            start = after;
        }
        if !already_in_place {
            remove_edge(edges, new_edge);
            insert_edge_after(edges, new_edge, start);
        }
        // nextEdge:
        check_intersection(edges, new_edge, y, next_next_y);
        check_intersection_fwd(edges, new_edge, y, next_next_y);
        update_next_next_y(edges[new_edge].lower_y, y, next_next_y);
        start = new_edge;
        new_edge = next;
        if edges[new_edge].upper_y > y {
            break;
        }
    }
    update_next_next_y(edges[new_edge].upper_y, y, next_next_y);
}

// Port of: src/core/SkScan_AAAPath.cpp#L1366-L1377 (chrome/m156)
fn validate_edges_for_y(edges: &[AnyAnalyticEdge], mut edge: usize, y: Fixed) {
    if cfg!(debug_assertions) {
        while edges[edge].upper_y <= y {
            let e = &edges[edge];
            debug_assert!(e.prev != NO_EDGE && e.next != NO_EDGE);
            debug_assert_eq!(edges[e.prev].next, edge);
            debug_assert_eq!(edges[e.next].prev, edge);
            debug_assert!(e.upper_y <= e.lower_y);
            debug_assert!(edges[e.prev].prev == NO_EDGE || edges[e.prev].x <= e.x);
            edge = e.next;
        }
    }
}

// Return true if prev->fX, next->fX are too close in the current pixel row.
// Port of: src/core/SkScan_AAAPath.cpp#L1379-L1397 (chrome/m156)
fn edges_too_close(edges: &[AnyAnalyticEdge], prev: usize, next: usize, lower_y: Fixed) -> bool {
    // When next->fDX == 0, prev->fX >= next->fX - SkAbs32(next->fDX) would be false
    // even if prev->fX and next->fX are close and within one pixel (e.g., prev->fX == 0.1,
    // next->fX == 0.9). Adding SLACK = 1 to the formula would guarantee it to be true if two
    // edges prev and next are within one pixel.
    const SLACK: Fixed = FIXED_1;

    // Note that even if the following test failed, the edges might still be very close to each
    // other at some point within the current pixel row because of prev->fDX and next->fDX.
    // However, to handle that case, we have to sacrafice more performance.
    // I think the current quality is good enough (mainly by looking at Nebraska-StateSeal.svg)
    // so I'll ignore fDX for performance tradeoff.
    next != NO_EDGE
        && prev != NO_EDGE
        && edges[next].upper_y < lower_y
        && edges[prev].x.wrapping_add(SLACK) >= edges[next].x.wrapping_sub(abs32(edges[next].dx))
}

// This function exists for the case where the previous rite edge is removed because
// its fLowerY <= nextY
// Port of: src/core/SkScan_AAAPath.cpp#L1399-L1403 (chrome/m156)
fn edges_too_close_rite(prev_rite: i32, ul: Fixed, ll: Fixed) -> bool {
    prev_rite > fixed_floor_to_int(ul) || prev_rite > fixed_floor_to_int(ll)
}

// Port of: src/core/SkScan_AAAPath.cpp#L1405-L1602 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
#[allow(clippy::too_many_lines)] // mirrors the C++ function
#[allow(clippy::fn_params_excessive_bools)] // mirrors the C++ signature
fn aaa_walk_edges(
    edges: &mut [AnyAnalyticEdge],
    prev_head: usize,
    next_tail: usize,
    fill_type: PathFillType,
    blitter: &mut dyn AdditiveBlitter,
    mut start_y: i32,
    stop_y: i32,
    left_clip: Fixed,
    right_clip: Fixed,
    is_using_mask: bool,
    force_rle: bool,
    skip_intersect: bool,
) {
    edges[prev_head].x = left_clip;
    edges[prev_head].upper_x = left_clip;
    edges[next_tail].x = right_clip;
    edges[next_tail].upper_x = right_clip;
    let mut y = edges[edges[prev_head].next]
        .upper_y
        .max(int_to_fixed(start_y));
    let mut next_next_y = MAX_S32;

    {
        let mut edge = edges[prev_head].next;
        while edges[edge].upper_y <= y {
            edges[edge].go_y(y);
            update_next_next_y(edges[edge].lower_y, y, &mut next_next_y);
            edge = edges[edge].next;
        }
        update_next_next_y(edges[edge].upper_y, y, &mut next_next_y);
    }

    let winding_mask = if fill_type.is_even_odd() { 1 } else { -1 };
    let is_inverse = fill_type.is_inverse();

    if is_inverse && int_to_fixed(start_y) != y {
        let width = fixed_floor_to_int(right_clip.wrapping_sub(left_clip));
        if fixed_floor_to_int(y) != start_y {
            blitter.real_blitter().blit_rect(
                fixed_floor_to_int(left_clip),
                start_y,
                width,
                fixed_floor_to_int(y) - start_y,
            );
            start_y = fixed_floor_to_int(y);
        }
        let mask_row = if is_using_mask {
            Some(blitter.get_row(start_y))
        } else {
            None
        };
        blit_full_alpha(
            blitter,
            start_y,
            fixed_floor_to_int(left_clip),
            width,
            fixed_to_alpha(y.wrapping_sub(int_to_fixed(start_y))),
            mask_row,
            false,
        );
    }

    loop {
        let mut w: i32 = 0;
        let mut in_interval = is_inverse;
        let mut prev_x = edges[prev_head].x;
        let mut next_y = next_next_y.min(fixed_ceil_to_fixed(y.wrapping_add(1)));
        let mut curr_e = edges[prev_head].next;
        let mut left_e = prev_head;
        let mut left = left_clip;
        let mut left_dy: Fixed = 0;
        let mut prev_rite = fixed_floor_to_int(left_clip);

        next_next_y = MAX_S32;

        debug_assert_eq!((next_y & ((FIXED_1 >> 2) - 1)), 0);
        let mut y_shift = 0;
        if (next_y.wrapping_sub(y)) & (FIXED_1 >> 2) != 0 {
            y_shift = 2;
            next_y = y.wrapping_add(FIXED_1 >> 2);
        } else if (next_y.wrapping_sub(y)) & (FIXED_1 >> 1) != 0 {
            y_shift = 1;
            debug_assert_eq!(next_y, y.wrapping_add(FIXED_1 >> 1));
        }

        let full_alpha = fixed_to_alpha(next_y.wrapping_sub(y));

        // If we're using mask blitter, we advance the mask row in this function
        // to save some "if" condition checks.
        let mask_row = if is_using_mask {
            Some(blitter.get_row(fixed_floor_to_int(y)))
        } else {
            None
        };

        debug_assert_eq!(edges[curr_e].prev, prev_head);
        validate_edges_for_y(edges, curr_e, y);

        // Even if next - y == SK_Fixed1, we can still break the left-to-right order requirement
        // of the SKAAClip: |\| (two trapezoids with overlapping middle wedges)
        let no_real_blitter = force_rle; // forceRLE && (nextY - y != SK_Fixed1);

        while edges[curr_e].upper_y <= y {
            debug_assert!(edges[curr_e].lower_y >= next_y);
            debug_assert_eq!(edges[curr_e].y, y);

            w += edges[curr_e].winding as i32;
            let prev_in_interval = in_interval;
            in_interval = ((w & winding_mask) == 0) == is_inverse;

            let is_left = in_interval && !prev_in_interval;
            let is_rite = !in_interval && prev_in_interval;

            if is_rite {
                let mut rite = edges[curr_e].x;
                edges[curr_e].go_y_shift(next_y, y_shift);
                let next_left = left_clip.max(edges[left_e].x);
                rite = right_clip.min(rite);
                let next_rite = right_clip.min(edges[curr_e].x);
                let too_close = full_alpha == 0xFF
                    && (edges_too_close_rite(prev_rite, left, edges[left_e].x)
                        || edges_too_close(edges, curr_e, edges[curr_e].next, next_y));
                blit_trapezoid_row(
                    blitter,
                    y >> 16,
                    left,
                    rite,
                    next_left,
                    next_rite,
                    left_dy,
                    edges[curr_e].dy,
                    full_alpha,
                    mask_row,
                    no_real_blitter || too_close,
                );
                prev_rite = fixed_ceil_to_int(rite.max(edges[curr_e].x));
            } else {
                if is_left {
                    left = edges[curr_e].x.max(left_clip);
                    left_dy = edges[curr_e].dy;
                    left_e = curr_e;
                }
                edges[curr_e].go_y_shift(next_y, y_shift);
            }

            let next = edges[curr_e].next;

            while edges[curr_e].lower_y <= next_y {
                match &mut edges[curr_e] {
                    AnyAnalyticEdge::Cubic(cubic_edge) if cubic_edge.curve_count < 0 => {
                        cubic_edge.keep_continuous();
                        if !cubic_edge.update_cubic() {
                            break;
                        }
                    }
                    AnyAnalyticEdge::Quad(quad_edge) if quad_edge.curve_count > 0 => {
                        quad_edge.keep_continuous();
                        if !quad_edge.update_quadratic() {
                            break;
                        }
                    }
                    _ => break,
                }
            }
            debug_assert_eq!(edges[curr_e].y, next_y);

            if edges[curr_e].lower_y <= next_y {
                remove_edge(edges, curr_e);
            } else {
                update_next_next_y(edges[curr_e].lower_y, next_y, &mut next_next_y);
                let new_x = edges[curr_e].x;
                debug_assert!(edges[curr_e].lower_y > next_y);
                if new_x < prev_x {
                    // ripple currE backwards until it is x-sorted
                    backward_insert_edge_based_on_x(edges, curr_e);
                } else {
                    prev_x = new_x;
                }
                if !skip_intersect {
                    check_intersection(edges, curr_e, next_y, &mut next_next_y);
                }
            }

            curr_e = next;
            debug_assert!(curr_e != NO_EDGE);
        }

        // was our right-edge culled away?
        if in_interval {
            let too_close =
                full_alpha == 0xFF && edges_too_close(edges, edges[left_e].prev, left_e, next_y);
            blit_trapezoid_row(
                blitter,
                y >> 16,
                left,
                right_clip,
                left_clip.max(edges[left_e].x),
                right_clip,
                left_dy,
                0,
                full_alpha,
                mask_row,
                no_real_blitter || too_close,
            );
        }

        if force_rle {
            blitter.flush_if_y_changed(y, next_y);
        }

        y = next_y;
        if y >= int_to_fixed(stop_y) {
            break;
        }

        // now currE points to the first edge with a fUpperY larger than the previous y
        insert_new_edges(edges, curr_e, y, &mut next_next_y);
    }
}

// Port of: src/core/SkScan_AAAPath.cpp#L1604-L1703 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
#[allow(clippy::fn_params_excessive_bools)] // mirrors the C++ signature
fn aaa_fill_path(
    path: &PathRaw<'_>,
    clip_rect: &IRect,
    blitter: &mut dyn AdditiveBlitter,
    mut start_y: i32,
    mut stop_y: i32,
    path_contained_in_clip: bool,
    is_using_mask: bool,
    force_rle: bool, // forceRLE implies that SkAAClip is calling us
) {
    let mut builder = AnalyticEdgeBuilder::new();
    let count = builder.build_edges(
        path,
        if path_contained_in_clip {
            None
        } else {
            Some(clip_rect)
        },
    );
    let mut edges = builder.into_edges();
    debug_assert_eq!(count, edges.len());

    let mut rect = *clip_rect;
    if 0 == count {
        if path.is_inverse_fill_type() {
            // Since we are in inverse-fill, our caller has already drawn above
            // our top (start_y) and will draw below our bottom (stop_y). Thus
            // we need to restrict our drawing to the intersection of the clip
            // and those two limits.
            if rect.top < start_y {
                rect.top = start_y;
            }
            if rect.bottom > stop_y {
                rect.bottom = stop_y;
            }
            if !rect.is_empty() {
                blitter
                    .real_blitter()
                    .blit_rect(rect.left, rect.top, rect.width(), rect.height());
            }
        }
        return;
    }

    // this returns the first and last edge after they're sorted into a dlink list
    let (edge, last) = sort_edges(&mut edges);

    let head_edge = count;
    let tail_edge = count + 1;
    edges.push(AnyAnalyticEdge::Line(AnalyticEdge {
        prev: NO_EDGE,
        next: edge,
        upper_y: MIN_S32,
        lower_y: MIN_S32,
        x: MIN_S32,
        dx: 0,
        dy: MAX_S32,
        upper_x: MIN_S32,
        ..AnalyticEdge::default()
    }));
    edges[edge].prev = head_edge;

    edges.push(AnyAnalyticEdge::Line(AnalyticEdge {
        prev: last,
        next: NO_EDGE,
        upper_y: MAX_S32,
        lower_y: MAX_S32,
        x: MAX_S32,
        dx: 0,
        dy: MAX_S32,
        upper_x: MAX_S32,
        ..AnalyticEdge::default()
    }));
    edges[last].next = tail_edge;

    // now edge is the head of the sorted linklist

    if !path_contained_in_clip && start_y < clip_rect.top {
        start_y = clip_rect.top;
    }
    if !path_contained_in_clip && stop_y > clip_rect.bottom {
        stop_y = clip_rect.bottom;
    }

    let mut left_bound = int_to_fixed(rect.left);
    let mut right_bound = int_to_fixed(rect.right);
    if is_using_mask {
        // If we're using mask, then we have to limit the bound within the path bounds.
        // Otherwise, the edge drift may access an invalid address inside the mask.
        let ir: IRect = path.bounds().round_out();
        left_bound = left_bound.max(int_to_fixed(ir.left));
        right_bound = right_bound.min(int_to_fixed(ir.right));
    }

    if !path.is_inverse_fill_type() && path.is_known_to_be_convex() && count >= 2 {
        aaa_walk_convex_edges(
            &mut edges,
            head_edge,
            blitter,
            start_y,
            stop_y,
            left_bound,
            right_bound,
            is_using_mask,
        );
    } else {
        // We skip intersection computation if there are many points which probably already
        // give us enough fractional scan lines.
        // (SkToSizeT of a negative int is a huge size_t in Skia's release builds.)
        let skip_intersect =
            usize::try_from((stop_y - start_y) * 2).is_ok_and(|limit| path.points().len() > limit);

        aaa_walk_edges(
            &mut edges,
            head_edge,
            tail_edge,
            path.fill_type(),
            blitter,
            start_y,
            stop_y,
            left_bound,
            right_bound,
            is_using_mask,
            force_rle,
            skip_intersect,
        );
    }
}

// Check if the path is a rect and fat enough after clipping; if so, blit it.
// Port of: src/core/SkScan_AAAPath.cpp#L1705-L1722 (chrome/m156)
fn try_blit_fat_anti_rect(blitter: &mut dyn Blitter, raw: &PathRaw<'_>, clip: &IRect) -> bool {
    let Some(mut rect) = raw.is_rect() else {
        return false;
    };
    if !rect.intersect(Rect::from(*clip)) {
        return true; // The intersection is empty. Hence consider it done.
    }
    let bounds: IRect = rect.round_out();
    if bounds.width() < 3 {
        return false; // not fat
    }
    blitter.blit_fat_anti_rect(&rect);
    true
}

/// Fills `path` with analytic antialiasing (`SkScan::AAAFillPath`). `ir` is the path's bounds
/// rounded out, `clip_bounds` the bounds of the clip (already applied to `blitter`). With
/// `force_rle` (used by `SkAAClip`), the mask blitter is never used.
// Port of: src/core/SkScan_AAAPath.cpp#L1724-L1781 (chrome/m156)
#[doc(alias = "AAAFillPath")]
pub fn aaa_fill_path_raw(
    path: &PathRaw<'_>,
    blitter: &mut dyn Blitter,
    ir: &IRect,
    clip_bounds: &IRect,
    force_rle: bool,
) {
    let contained_in_clip = clip_bounds.contains(ir);
    let is_inverse = path.is_inverse_fill_type();

    // The mask blitter (where we store intermediate alpha values directly in a mask, and then
    // call the real blitter once in the end to blit the whole mask) is faster than the RLE
    // blitter when the blit region is small enough (i.e., CanHandleRect(ir)). When isInverse is
    // true, the blit region is no longer the rectangle ir so we won't use the mask blitter. The
    // caller may also use the forceRLE flag to force not using the mask blitter. Also, when the
    // path is a simple rect, preparing a mask and blitting it might have too much overhead.
    // Hence we'll use blitFatAntiRect to avoid the mask and its overhead.
    if MaskAdditiveBlitter::can_handle_rect(ir) && !is_inverse && !force_rle {
        // blitFatAntiRect is slower than the normal AAA flow without MaskAdditiveBlitter.
        // Hence only tryBlitFatAntiRect when MaskAdditiveBlitter would have been used.
        if !try_blit_fat_anti_rect(blitter, path, clip_bounds) {
            let mut additive_blitter =
                MaskAdditiveBlitter::new(blitter, ir, clip_bounds, is_inverse);
            aaa_fill_path(
                path,
                clip_bounds,
                &mut additive_blitter,
                ir.top,
                ir.bottom,
                contained_in_clip,
                true,
                force_rle,
            );
        }
    } else if !is_inverse && path.is_known_to_be_convex() {
        // If the filling area is convex (i.e., path.isConvex && !isInverse), our simpler
        // aaa_walk_convex_edges won't generate alphas above 255. Hence we don't need
        // SafeRLEAdditiveBlitter (which is slow due to clamping). The basic RLE blitter
        // RunBasedAdditiveBlitter would suffice.
        let mut additive_blitter =
            RunBasedAdditiveBlitter::new(blitter, ir, clip_bounds, is_inverse, false);
        aaa_fill_path(
            path,
            clip_bounds,
            &mut additive_blitter,
            ir.top,
            ir.bottom,
            contained_in_clip,
            false,
            force_rle,
        );
    } else {
        // If the filling area might not be convex, the more involved aaa_walk_edges would
        // be called and we have to clamp the alpha downto 255. The SafeRLEAdditiveBlitter
        // does that at a cost of performance.
        let mut additive_blitter =
            RunBasedAdditiveBlitter::new(blitter, ir, clip_bounds, is_inverse, true);
        aaa_fill_path(
            path,
            clip_bounds,
            &mut additive_blitter,
            ir.top,
            ir.bottom,
            contained_in_clip,
            false,
            force_rle,
        );
    }
}
