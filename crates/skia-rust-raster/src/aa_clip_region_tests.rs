// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

// The region/AAClip comparisons of AAClipTest.cpp (`AAClip_setPath_RandomRegion_MatchesSkRegion`,
// `AAClip_setRect_RandomRects_MatchesSkRegion`). Their manifest entries wait for the Canvas (D6)
// because the `copyToMask(SkRegion)` helper draws through an SkCanvas; here the region's mask is
// filled directly (0xFF inside the region), which is what that drawing produces.

use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::random::Random;
use skia_rust_core::rect::IRect;
use skia_rust_core::region::{Iterator as RegionIterator, Op, Region};

use crate::aa_clip::AAClip;

fn region_mask(rgn: &Region) -> Option<(IRect, Vec<u8>)> {
    if rgn.is_empty() {
        return None;
    }
    let b = *rgn.bounds();
    let w = usize::try_from(b.width()).expect("width");
    let mut img = vec![0u8; w * usize::try_from(b.height()).expect("height")];
    let mut it = RegionIterator::new(rgn);
    while !it.is_done() {
        let r = *it.rect();
        for y in r.top..r.bottom {
            for x in r.left..r.right {
                let at = usize::try_from(y - b.top).expect("y") * w
                    + usize::try_from(x - b.left).expect("x");
                img[at] = 0xFF;
            }
        }
        it.next();
    }
    Some((b, img))
}

fn same_as_aa_clip(rgn: &Region, aa: &AAClip) -> bool {
    let mask = aa.copy_to_mask();
    match region_mask(rgn) {
        None => aa.is_empty() && mask.image.is_empty(),
        Some((b, img)) => mask.bounds == b && mask.image[..img.len()] == img[..],
    }
}

#[allow(clippy::cast_possible_wrap)] // mirrors the unsigned -> int conversions of the C++
#[test]
fn random_regions_match_aa_clip() {
    let mut rand = Random::default();
    for _ in 0..1000 {
        let mut rgn = Region::new();
        let count = rand.next_u() % 20;
        for _ in 0..count {
            let n = 100;
            let rect = IRect::from_xywh(
                rand.next_s() % n,
                rand.next_s() % n,
                (rand.next_u() % 100) as i32,
                (rand.next_u() % 100) as i32,
            );
            rgn.op_rect(rect, Op::XOR);
        }
        let mut aa = AAClip::new();
        aa.set_region(&rgn);
        assert!(same_as_aa_clip(&rgn, &aa));
    }
}

#[allow(clippy::cast_possible_wrap)] // mirrors the unsigned -> int conversions of the C++
#[test]
fn random_rect_ops_match_region_ops() {
    let mut rand = Random::default();
    let mut rand_irect = |n: u32| {
        let mut r = IRect::default();
        r.set_xywh(0, 0, (rand.next_u() % n) as i32, (rand.next_u() % n) as i32);
        let dx = (rand.next_u() % (2 * n)) as i32;
        let dy = (rand.next_u() % (2 * n)) as i32;
        r.offset((n as i32 - dx, n as i32 - dy));
        r
    };
    for _ in 0..10000 {
        let (r0, r1) = (rand_irect(10), rand_irect(10));
        let mut clip0 = AAClip::new();
        let mut clip1 = AAClip::new();
        clip0.set_rect(&r0);
        clip1.set_rect(&r1);
        let (rgn0, rgn1) = (Region::from_rect(r0), Region::from_rect(r1));
        for op in [ClipOp::Difference, ClipOp::Intersect] {
            let mut clip2 = clip0.clone();
            let mut rgn2 = Region::new();
            let aa_non_empty = clip2.op_aa_clip(&clip1, op);
            let bw_non_empty = rgn2.op_region_region(&rgn0, &rgn1, op.into());
            assert_eq!(aa_non_empty, bw_non_empty);
            if bw_non_empty {
                assert_eq!(clip2.bounds(), rgn2.bounds());
            }
            assert!(same_as_aa_clip(&rgn2, &clip2));
        }
    }
}
