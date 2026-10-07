// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/AAClipTest.cpp (chrome/m156)

#![cfg(test)]
#![allow(clippy::excessive_precision)] // literals are copied verbatim from the C++

use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::mask::{AllocType, Mask, MaskBuilder, MaskFormat};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::random::Random;
use skia_rust_core::rect::{IRect, Rect, RoundOut};
use skia_rust_core::region::{Op, Region};
use skia_rust_core::rrect::RRect;
use skia_rust_core::scalar::{int_to_scalar, scalar};
use skia_rust_raster::aa_clip::AAClip;
use skia_rust_raster::raster_clip::RasterClip;
use skia_rust_raster::region_path::RegionExt;

use crate::{Reporter, def_test, errorf, reporter_assert};

// Port of: tests/AAClipTest.cpp#L43-L83 (chrome/m156)
fn masks_equal(a: &Mask<'_>, b: &Mask<'_>) -> bool {
    if a.format != b.format || a.bounds != b.bounds {
        return false;
    }
    if a.image.is_empty() && b.image.is_empty() {
        return true;
    }
    if a.image.is_empty() || b.image.is_empty() {
        return false;
    }

    let mut wbytes = usize::try_from(a.bounds.width()).expect("width");
    match a.format {
        MaskFormat::BW => wbytes = (wbytes + 7) >> 3,
        MaskFormat::A8 | MaskFormat::ThreeD => {}
        MaskFormat::Lcd16 => wbytes <<= 1,
        MaskFormat::Argb32 => wbytes <<= 2,
        MaskFormat::Sdf => return false, // unknown mask format
    }

    let h = usize::try_from(a.bounds.height()).expect("height");
    // The C++ steps both pointers by `wbytes` (not by the row bytes) per row.
    for y in 0..h {
        let (aptr, bptr) = (&a.image[y * wbytes..], &b.image[y * wbytes..]);
        if aptr[..wbytes] != bptr[..wbytes] {
            return false;
        }
    }
    true
}

// Port of: tests/AAClipTest.cpp#L85-L116 (chrome/m156)
fn copy_to_mask_region(rgn: &Region, mask: &mut MaskBuilder) {
    mask.format = MaskFormat::A8;

    if rgn.is_empty() {
        mask.bounds.set_empty();
        mask.row_bytes = 0;
        mask.image = Vec::new();
        return;
    }

    mask.bounds = *rgn.bounds();
    mask.row_bytes = u32::try_from(mask.bounds.width()).expect("width");
    mask.image = MaskBuilder::alloc_image(mask.compute_image_size(), AllocType::ZeroInit);

    // The C++ now installs the mask as the pixels of an A8 SkBitmap, translates a copy of the
    // region to (0, 0), and draws black through it with an SkCanvas (`clipRegion`, `drawColor`).
    todo!("needs Canvas (D6): SkCanvas::clipRegion and drawColor");
}

// Port of: tests/AAClipTest.cpp#L118-L125 (chrome/m156)
fn copy_to_mask_raster_clip(rc: &RasterClip, mask: &mut MaskBuilder) {
    if rc.is_bw() {
        copy_to_mask_region(rc.bw_rgn(), mask);
    } else {
        *mask = rc.aa_rgn().copy_to_mask();
    }
}

// Port of: tests/AAClipTest.cpp#L127-L141 (chrome/m156)
fn raster_clips_equal(a: &RasterClip, b: &RasterClip) -> bool {
    if a.is_empty() && b.is_empty() {
        return true;
    } else if a.is_empty() != b.is_empty() || a.is_bw() != b.is_bw() || a.is_rect() != b.is_rect() {
        return false;
    }

    let mut mask0 = MaskBuilder::default();
    let mut mask1 = MaskBuilder::default();
    copy_to_mask_raster_clip(a, &mut mask0);
    copy_to_mask_raster_clip(b, &mut mask1);
    masks_equal(&mask0.as_mask(), &mask1.as_mask())
}

// Port of: tests/AAClipTest.cpp#L143-L149 (chrome/m156)
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names
)] // mirrors `rand.nextU() % n` converted to int
fn rand_rect(rand: &mut Random, n: i32) -> IRect {
    let x = rand.next_s() % n;
    let y = rand.next_s() % n;
    let w = (rand.next_u() % n as u32) as i32;
    let h = (rand.next_u() % n as u32) as i32;
    IRect::from_xywh(x, y, w, h)
}

// Port of: tests/AAClipTest.cpp#L151-L156 (chrome/m156)
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names
)] // mirrors `rand.nextU() % 20` converted to int
fn make_rand_rgn(rgn: &mut Region, rand: &mut Random) {
    let count = (rand.next_u() % 20) as i32;
    for _ in 0..count {
        rgn.op_rect(rand_rect(rand, 100), Op::XOR);
    }
}

// Port of: tests/AAClipTest.cpp#L158-L166 (chrome/m156)
fn region_equals_aa_clip(rgn: &Region, aaclip: &AAClip) -> bool {
    let mut mask0 = MaskBuilder::default();
    copy_to_mask_region(rgn, &mut mask0);
    let mask1 = aaclip.copy_to_mask();
    masks_equal(&mask0.as_mask(), &mask1.as_mask())
}

// Port of: tests/AAClipTest.cpp#L168-L172 (chrome/m156)
fn equals_aa_clip(rgn: &Region) -> bool {
    let mut aaclip = AAClip::new();
    aaclip.set_region(rgn);
    region_equals_aa_clip(rgn, &aaclip)
}

// Port of: tests/AAClipTest.cpp#L174-L178 (chrome/m156)
fn set_rgn_to_path(rgn: &mut Region, path: &Path) {
    let ir: IRect = path.bounds().round();
    rgn.set_path(path, &Region::from_rect(ir));
}

// Port of: tests/AAClipTest.cpp#L180-L202 (chrome/m156)
def_test!(
    #[ignore = "needs Canvas (D6): copyToMask(SkRegion) draws with SkCanvas"]
    AAClip_setPath_RandomRegion_MatchesSkRegion,
    |reporter| {
        let mut rand = Random::default();
        for _ in 0..1000 {
            let mut rgn = Region::new();
            make_rand_rgn(&mut rgn, &mut rand);
            reporter_assert!(reporter, equals_aa_clip(&rgn));
        }

        {
            let mut rgn = Region::new();
            set_rgn_to_path(&mut rgn, &Path::circle((0.0, 0.0), 30.0, None));
            reporter_assert!(reporter, equals_aa_clip(&rgn));

            let mut builder = PathBuilder::new();
            builder
                .move_to((0.0, 0.0))
                .line_to((100.0, 0.0))
                .line_to((100.0 - 20.0, 20.0))
                .line_to((20.0, 20.0));
            let path = builder.detach();
            set_rgn_to_path(&mut rgn, &path);
            reporter_assert!(reporter, equals_aa_clip(&rgn));
        }
    }
);

// Port of: tests/AAClipTest.cpp#L204-L228 (chrome/m156)
def_test!(
    #[allow(clippy::float_cmp)] // the C++ compares the scalars with ==
    AAClip_setPath_ClipBoundsMatchExpectations,
    |reporter| {
        let mut clip = AAClip::new();
        let height = 40;
        let sheight: scalar = int_to_scalar(height);

        let mut path = Path::oval(Rect::from_wh(sheight, sheight), None);
        reporter_assert!(reporter, sheight == path.bounds().height());
        clip.set_path(&path, &path.bounds().round_out(), true);
        reporter_assert!(reporter, height == clip.bounds().height());

        // this is the trimmed height of this cubic (with aa). The critical thing
        // for this test is that it is less than height, which represents just
        // the bounds of the path's control-points.
        //
        // This used to fail until we tracked the MinY in the BuilderBlitter.
        //
        let teardrop_height = 12;
        let mut builder = PathBuilder::new();
        builder
            .move_to((0.0, 20.0))
            .cubic_to((40.0, 40.0), (40.0, 0.0), (0.0, 20.0));
        path = builder.detach();
        reporter_assert!(reporter, sheight == path.bounds().height());
        clip.set_path(&path, &path.bounds().round_out(), true);
        reporter_assert!(reporter, teardrop_height == clip.bounds().height());
    }
);

// Port of: tests/AAClipTest.cpp#L230-L255 (chrome/m156)
def_test!(AAClip_BasicFunctionality, |reporter| {
    let mut clip = AAClip::new();

    reporter_assert!(reporter, clip.is_empty());
    reporter_assert!(reporter, clip.bounds().is_empty());

    clip.translate_in_place(10, 10); // should have no effect on empty
    reporter_assert!(reporter, clip.is_empty());
    reporter_assert!(reporter, clip.bounds().is_empty());

    let r = IRect::new(10, 10, 40, 50);
    clip.set_rect(&r);
    reporter_assert!(reporter, !clip.is_empty());
    reporter_assert!(reporter, !clip.bounds().is_empty());
    reporter_assert!(reporter, *clip.bounds() == r);

    clip.set_empty();
    reporter_assert!(reporter, clip.is_empty());
    reporter_assert!(reporter, clip.bounds().is_empty());

    let mask = clip.copy_to_mask();
    reporter_assert!(reporter, mask.image.is_empty());
    reporter_assert!(reporter, mask.bounds.is_empty());
});

// Port of: tests/AAClipTest.cpp#L257-L265 (chrome/m156)
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names
)] // mirrors `rand.nextU() % N` converted to int
fn rand_irect(r: &mut IRect, n: i32, rand: &mut Random) {
    r.set_xywh(
        0,
        0,
        (rand.next_u() % n as u32) as i32,
        (rand.next_u() % n as u32) as i32,
    );
    let dx = (rand.next_u() % (2 * n) as u32) as i32;
    let dy = (rand.next_u() % (2 * n) as u32) as i32;
    // use int dx,dy to make the subtract be signed
    r.offset((n - dx, n - dy));
}

// Port of: tests/AAClipTest.cpp#L267-L308 (chrome/m156)
def_test!(
    #[ignore = "needs Canvas (D6): copyToMask(SkRegion) draws with SkCanvas"]
    AAClip_setRect_RandomRects_MatchesSkRegion,
    |reporter| {
        let mut rand = Random::default();

        for _ in 0..10000 {
            let mut clip0 = AAClip::new();
            let mut clip1 = AAClip::new();
            let mut rgn0 = Region::new();
            let mut rgn1 = Region::new();
            let mut r0 = IRect::default();
            let mut r1 = IRect::default();

            rand_irect(&mut r0, 10, &mut rand);
            rand_irect(&mut r1, 10, &mut rand);
            clip0.set_rect(&r0);
            clip1.set_rect(&r1);
            rgn0.set_rect(r0);
            rgn1.set_rect(r1);
            for op in [ClipOp::Difference, ClipOp::Intersect] {
                let mut clip2 = clip0.clone(); // leave clip0 unchanged for future iterations
                let mut rgn2 = Region::new();
                let non_empty_aa = clip2.op_aa_clip(&clip1, op);
                let non_empty_bw = rgn2.op_region_region(&rgn0, &rgn1, op.into());
                if non_empty_aa != non_empty_bw || *clip2.bounds() != *rgn2.bounds() {
                    errorf!(
                        reporter,
                        "{} {} [{} {} {} {}] {} [{} {} {} {}] = BW:[{} {} {} {}] AA:[{} {} {} {}]\n",
                        if non_empty_aa == non_empty_bw {
                            "true"
                        } else {
                            "false"
                        },
                        if *clip2.bounds() == *rgn2.bounds() {
                            "true"
                        } else {
                            "false"
                        },
                        r0.left,
                        r0.top,
                        r0.right,
                        r0.bottom,
                        if op == ClipOp::Difference {
                            "DIFF"
                        } else {
                            "INTERSECT"
                        },
                        r1.left,
                        r1.top,
                        r1.right,
                        r1.bottom,
                        rgn2.bounds().left,
                        rgn2.bounds().top,
                        rgn2.bounds().right,
                        rgn2.bounds().bottom,
                        clip2.bounds().left,
                        clip2.bounds().top,
                        clip2.bounds().right,
                        clip2.bounds().bottom
                    );
                }

                let mut mask_bw = MaskBuilder::default();
                copy_to_mask_region(&rgn2, &mut mask_bw);
                let mask_aa = clip2.copy_to_mask();
                reporter_assert!(
                    reporter,
                    masks_equal(&mask_bw.as_mask(), &mask_aa.as_mask())
                );
            }
        }
    }
);

// Port of: tests/AAClipTest.cpp#L310-L348 (chrome/m156)
def_test!(AAClip_setPath_PathHasHole_MaskIsCorrect, |reporter| {
    #[rustfmt::skip]
    static EXPECTED_IMAGE: [u8; 24] = [
        0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF,
    ];
    let expected = Mask::new(&EXPECTED_IMAGE, IRect::from_wh(4, 6), 4, MaskFormat::A8);

    let mut builder = PathBuilder::new();
    builder
        .add_rect(Rect::from_xywh(0.0, 0.0, 4.0, 2.0), None, None)
        .add_rect(Rect::from_xywh(0.0, 4.0, 4.0, 2.0), None, None);
    let path = builder.detach();

    {
        reporter.set_context(Some("noAA".to_string()));
        let mut clip = AAClip::new();
        clip.set_path(&path, &path.bounds().round_out(), false);

        let mask = clip.copy_to_mask();

        reporter_assert!(reporter, masks_equal(&expected, &mask.as_mask()));
        reporter.set_context(None);
    }
    {
        reporter.set_context(Some("withAA".to_string()));
        let mut clip = AAClip::new();
        clip.set_path(&path, &path.bounds().round_out(), true);

        let mask = clip.copy_to_mask();

        reporter_assert!(reporter, masks_equal(&expected, &mask.as_mask()));
        reporter.set_context(None);
    }
});

// Port of: tests/AAClipTest.cpp#L350-L371 (chrome/m156)
def_test!(AAClip_RRectIsReallyARect_ClipIsRect, |reporter| {
    let mut rrect = RRect::default();
    rrect.set_rect_xy(Rect::from_wh(100.0, 100.0), 5.0, 5.0);

    let path = Path::rrect(rrect, None);

    let mut clip = AAClip::new();
    clip.set_path(&path, &path.bounds().round_out(), true);

    reporter_assert!(reporter, *clip.bounds() == IRect::from_wh(100, 100));
    reporter_assert!(reporter, !clip.is_rect());

    // This rect should intersect the clip, but slice-out all of the "soft" parts,
    // leaving just a rect.
    let ir = IRect::new(10, -10, 50, 90);

    clip.op_irect(&ir, ClipOp::Intersect);

    reporter_assert!(reporter, *clip.bounds() == IRect::new(10, 0, 50, 90));
    // the clip recognized that that it is just a rect!
    reporter_assert!(reporter, clip.is_rect());
});

// Port of: tests/AAClipTest.cpp#L373-L395 (chrome/m156)
fn did_dx_affect(reporter: &mut Reporter, dx: &[scalar], count: usize, changed: bool) {
    let ir = IRect::new(0, 0, 10, 10);

    for &dx in dx.iter().take(count) {
        let mut r = Rect::from_irect(ir);

        let mut rc0 = RasterClip::from_rect(&ir);
        let mut rc1 = RasterClip::from_rect(&ir);
        let mut rc2 = RasterClip::from_rect(&ir);

        rc0.op_rect(&r, Matrix::i(), ClipOp::Intersect, false);
        r.offset((dx, 0.0));
        rc1.op_rect(&r, Matrix::i(), ClipOp::Intersect, true);
        r.offset((-2.0 * dx, 0.0));
        rc2.op_rect(&r, Matrix::i(), ClipOp::Intersect, true);

        reporter_assert!(reporter, changed != raster_clips_equal(&rc0, &rc1));
        reporter_assert!(reporter, changed != raster_clips_equal(&rc0, &rc2));
    }
}

// Port of: tests/AAClipTest.cpp#L397-L409 (chrome/m156)
def_test!(
    #[ignore = "needs Canvas (D6): copyToMask(SkRegion) draws with SkCanvas"]
    #[allow(clippy::items_after_statements)] // mirrors the two static tables of the C++
    AAClip_op_NearlyIntegral_GenerateSameRasterClips,
    |reporter| {
        // All of these should generate equivalent rasterclips

        static SAFE_X: [scalar; 4] = [0.0, 1.0 / 1000.0, 1.0 / 100.0, 1.0 / 10.0];
        did_dx_affect(reporter, &SAFE_X, SAFE_X.len(), false);

        static UNSAFE_X: [scalar; 2] = [1.0 / 4.0, 1.0 / 3.0];
        did_dx_affect(reporter, &UNSAFE_X, UNSAFE_X.len(), true);
    }
);

// Port of: tests/AAClipTest.cpp#L411-L421 (chrome/m156)
def_test!(AAClip_setPath_AvoidAssertion, |reporter| {
    // Should not assert in the debug build
    // bug was introduced in rev. 3209
    let mut clip = AAClip::new();
    let r = Rect {
        left: 129.892_18,
        top: 10.399_999_6,
        right: 130.892_18,
        bottom: 20.399_999_6,
    };
    reporter_assert!(
        reporter,
        clip.set_path(&Path::rect(r, None), &r.round_out(), true)
    );
});

// Building aaclip meant aa-scan-convert a path into a huge clip.
// the old algorithm sized the supersampler to the size of the clip, which overflowed
// its internal 16bit coordinates. The fix was to intersect the clip+path_bounds before
// sizing the supersampler.
//
// Before the fix, the following code would assert in debug builds.
//
// Port of: tests/AAClipTest.cpp#L423-L434 (chrome/m156)
def_test!(AAClip_crbug_422693_AvoidOverflow, |reporter| {
    let mut rc = RasterClip::from_rect(&IRect::new(-25000, -25000, 25000, 25000));
    let path = Path::circle((50.0, 50.0), 50.0, None);
    reporter_assert!(
        reporter,
        rc.op_path(&path, Matrix::i(), ClipOp::Intersect, true)
    );
});

// Port of: tests/AAClipTest.cpp#L436-L443 (chrome/m156)
def_test!(AAClip_setRect_HugeRect_ReturnsFalse, |reporter| {
    let mut clip = AAClip::new();
    let big: i32 = 0x7000_0000;
    let r = IRect::new(-big, -big, big, big);
    debug_assert!(r.width() < 0 && r.height() < 0);

    reporter_assert!(reporter, !clip.set_rect(&r));
});

// Port of: tests/AAClipTest.cpp#L445-L469 (chrome/m156)
def_test!(AAClip_setPath_LargePathSmallClip_StillBlits, |reporter| {
    // This test verifies the root cause of https://bugzilla.mozilla.org/show_bug.cgi?id=1909796
    // does not regress.
    let mut clip = AAClip::new();

    // Be advised that 2^31 will get turned into a float...
    #[allow(clippy::cast_precision_loss)] // the rounding to float is the point of the test
    let large_path = Path::rect(Rect::new(-1000.0, 10.0, i32::MAX as f32, 20.0), None);
    // ... and then back into an integer, so it won't be 2^31 any more
    // (e.g. 2147483520). Therefore, we pick the left to be big enough
    // to make the bounds exceed 31 bits again.
    let mut bounds: IRect = large_path.bounds().round_out();
    // SkIRect expects to work with 32 bit integers. If the width
    // or height exceeds 32 bits, isEmpty() returns •true•
    debug_assert!(bounds.is_empty());
    // But the 64 bit version works fine.
    debug_assert!(!bounds.is_empty_64());

    // Make sure the clip overlaps a little bit
    let small_clip = IRect::new(5, 5, 15, 15);
    // (`SkASSERT(bounds.intersect(smallClip))`; the result is not used afterwards.)
    if let Some(r) = IRect::intersect(&bounds, &small_clip) {
        bounds = r;
    }
    debug_assert!(!bounds.is_empty());

    reporter_assert!(reporter, clip.set_path(&large_path, &small_clip, true));
    reporter_assert!(reporter, clip.set_path(&large_path, &small_clip, false));
});
