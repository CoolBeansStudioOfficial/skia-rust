// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/AAClipBench.cpp

//! AA and BW clipping benches: `AAClipBench` clips with a rect or a rounded rect and draws
//! through it, `NestedAAClipBench` nests clips three deep, `AAClipBuilderBench` and
//! `AAClipRegionBench` build an `SkAAClip` directly (rendering).

use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::{IRect, Rect, RoundOut};
use skia_rust_core::region::Region;
use skia_rust_core::scalar::scalar;
use skia_rust_raster::aa_clip::AAClip;
use skia_rust_raster::region_path::RegionExt;

use crate::def_bench;
use crate::prelude::*;

/// `class AAClipBench`.
// Port of: bench/AAClipBench.cpp#L21-L82 (chrome/m156)
struct AAClipBench {
    /// `fName`.
    name: String,
    clip_path: Path,
    clip_rect: Rect,
    draw_rect: Rect,
    do_path: bool,
    do_aa: bool,
}

impl AAClipBench {
    // Port of: bench/AAClipBench.cpp#L25-L37 (chrome/m156)
    fn new(do_path: bool, do_aa: bool) -> Self {
        // fName.printf("aaclip_%s_%s", doPath ? "path" : "rect", doAA ? "AA" : "BW");
        let name = format!(
            "aaclip_{}_{}",
            if do_path { "path" } else { "rect" },
            if do_aa { "AA" } else { "BW" }
        );
        // fClipRect.setLTRB(10.5f, 10.5f, 50.5f, 50.5f);
        let clip_rect = Rect::from_ltrb(10.5, 10.5, 50.5, 50.5);
        // fClipPath = SkPath::RRect(fClipRect, 10, 10);
        let clip_path = Path::rrect_xy(clip_rect, 10.0, 10.0, None);
        // fDrawRect.setWH(100, 100);
        let draw_rect = Rect::from_ltrb(0.0, 0.0, 100.0, 100.0);
        // SkASSERT(fClipPath.isConvex());
        debug_assert!(clip_path.is_convex());
        Self {
            name,
            clip_path,
            clip_rect,
            draw_rect,
            do_path,
            do_aa,
        }
    }
}

impl Benchmark for AAClipBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/AAClipBench.cpp#L42-L72 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("AAClipBench is a rendering bench");
        let mut paint = Paint::default();
        self.setup_paint(&mut paint);
        for i in 0..loops {
            // jostle the clip regions each time to prevent caching
            // fClipRect.offset((i % 2) == 0 ? SkIntToScalar(10) : SkIntToScalar(-10), 0);
            let dx: scalar = if i % 2 == 0 { 10.0 } else { -10.0 };
            self.clip_rect.offset((dx, 0.0));
            // fClipPath = SkPath::RRect(fClipRect, 5, 5);
            self.clip_path = Path::rrect_xy(self.clip_rect, 5.0, 5.0, None);
            // SkASSERT(fClipPath.isConvex());
            debug_assert!(self.clip_path.is_convex());
            let save_count = canvas.save_count();
            canvas.save();
            if self.do_path {
                canvas.clip_path(&self.clip_path, ClipOp::Intersect, self.do_aa);
            } else {
                canvas.clip_rect(self.clip_rect, ClipOp::Intersect, self.do_aa);
            }
            canvas.draw_rect(self.draw_rect, &paint);
            canvas.restore_to_count(save_count);
        }
    }
}

/// Nesting depth of `NestedAAClipBench`.
// Port of: bench/AAClipBench.cpp#L93-L94 (chrome/m156)
const NESTING_DEPTH: usize = 3;
/// Size of the outermost clip.
// Port of: bench/AAClipBench.cpp#L95 (chrome/m156)
const IMAGE_SIZE: scalar = 400.0;

/// `class NestedAAClipBench`.
// Port of: bench/AAClipBench.cpp#L87-L165 (chrome/m156)
struct NestedAAClipBench {
    /// `fName`.
    name: String,
    do_aa: bool,
    draw_rect: Rect,
    random: Random,
    /// `fSizes`: the size of the clip at each depth.
    sizes: [Point; NESTING_DEPTH + 1],
}

impl NestedAAClipBench {
    // Port of: bench/AAClipBench.cpp#L99-L110 (chrome/m156)
    fn new(do_aa: bool) -> Self {
        // fName.printf("nested_aaclip_%s", doAA ? "AA" : "BW");
        let name = format!("nested_aaclip_{}", if do_aa { "AA" } else { "BW" });
        // fDrawRect = SkRect::MakeLTRB(0, 0, kImageSize, kImageSize);
        let draw_rect = Rect::from_ltrb(0.0, 0.0, IMAGE_SIZE, IMAGE_SIZE);
        let mut sizes = [Point::new(0.0, 0.0); NESTING_DEPTH + 1];
        // fSizes[0].set(kImageSize, kImageSize);
        sizes[0] = Point::new(IMAGE_SIZE, IMAGE_SIZE);
        // for (int i = 1; i < kNestingDepth+1; ++i) fSizes[i].set(fSizes[i-1].fX/2, fSizes[i-1].fY/2);
        for i in 1..=NESTING_DEPTH {
            sizes[i] = Point::new(sizes[i - 1].x / 2.0, sizes[i - 1].y / 2.0);
        }
        Self {
            name,
            do_aa,
            draw_rect,
            random: Random::default(),
            sizes,
        }
    }

    /// `recurse(canvas, depth, offset)`.
    // Port of: bench/AAClipBench.cpp#L113-L143 (chrome/m156)
    fn recurse(&mut self, canvas: &Canvas, depth: usize, offset: Point) {
        let save_count = canvas.save_count();
        canvas.save();
        // SkRect temp = SkRect::MakeLTRB(0, 0, fSizes[depth].fX, fSizes[depth].fY);
        let size = self.sizes[depth];
        let mut temp = Rect::from_ltrb(0.0, 0.0, size.x, size.y);
        // temp.offset(offset);
        temp.offset((offset.x, offset.y));
        // SkPath path = SkPath::RRect(temp, 3, 3);
        let path = Path::rrect_xy(temp, 3.0, 3.0, None);
        // SkASSERT(path.isConvex());
        debug_assert!(path.is_convex());
        canvas.clip_path(&path, ClipOp::Intersect, self.do_aa);
        if NESTING_DEPTH == depth {
            // we only draw the draw rect at the lowest nesting level
            // SkPaint paint; paint.setColor(0xff000000 | fRandom.nextU());
            let mut paint = Paint::default();
            paint.set_color(0xff00_0000 | self.random.next_u());
            canvas.draw_rect(self.draw_rect, &paint);
        } else {
            // SkPoint childOffset = offset;
            let mut child_offset = offset;
            self.recurse(canvas, depth + 1, child_offset);
            // childOffset += fSizes[depth+1];
            child_offset.x += self.sizes[depth + 1].x;
            child_offset.y += self.sizes[depth + 1].y;
            self.recurse(canvas, depth + 1, child_offset);
            child_offset.x = offset.x + self.sizes[depth + 1].x;
            child_offset.y = offset.y;
            self.recurse(canvas, depth + 1, child_offset);
            child_offset.x = offset.x;
            child_offset.y = offset.y + self.sizes[depth + 1].y;
            self.recurse(canvas, depth + 1, child_offset);
        }
        canvas.restore_to_count(save_count);
    }
}

impl Benchmark for NestedAAClipBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/AAClipBench.cpp#L145-L151 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("NestedAAClipBench is a rendering bench");
        for _ in 0..loops {
            // SkPoint offset = SkPoint::Make(0, 0);
            let offset = Point::new(0.0, 0.0);
            self.recurse(canvas, 0, offset);
        }
    }
}

/// `class AAClipBuilderBench`.
// Port of: bench/AAClipBench.cpp#L168-L210 (chrome/m156)
struct AAClipBuilderBench {
    /// `fName`.
    name: String,
    path: Path,
    rect: Rect,
    bounds: IRect,
    do_path: bool,
    do_aa: bool,
}

impl AAClipBuilderBench {
    // Port of: bench/AAClipBench.cpp#L170-L181 (chrome/m156)
    fn new(do_path: bool, do_aa: bool) -> Self {
        // fName.printf("aaclip_build_%s_%s", doPath ? "path" : "rect", doAA ? "AA" : "BW");
        let name = format!(
            "aaclip_build_{}_{}",
            if do_path { "path" } else { "rect" },
            if do_aa { "AA" } else { "BW" }
        );
        // fBounds = {0, 0, 640, 480};
        let bounds = IRect::from_ltrb(0, 0, 640, 480);
        // fRect.set(fBounds);
        let mut rect = Rect::from_ltrb(0.0, 0.0, 640.0, 480.0);
        // fRect.inset(SK_Scalar1/4, SK_Scalar1/4);
        rect.inset((0.25, 0.25));
        // fPath = SkPath::RRect(fRect, 20, 20);
        let path = Path::rrect_xy(rect, 20.0, 20.0, None);
        Self {
            name,
            path,
            rect,
            bounds,
            do_path,
            do_aa,
        }
    }
}

impl Benchmark for AAClipBuilderBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/AAClipBench.cpp#L183-L203 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            // SkAAClip clip;
            let mut clip = AAClip::new();
            if self.do_path {
                // clip.setPath(fPath, fBounds, fDoAA);
                let _ = clip.set_path(&self.path, &self.bounds, self.do_aa);
            } else if self.do_aa {
                // clip.setPath(SkPath::Rect(fRect), fBounds, fDoAA);
                let path = Path::rect(self.rect, None);
                let _ = clip.set_path(&path, &self.bounds, self.do_aa);
            } else {
                // clip.setRect(fBounds);
                let _ = clip.set_rect(&self.bounds);
            }
        }
    }
}

/// `class AAClipRegionBench`.
// Port of: bench/AAClipBench.cpp#L213-L239 (chrome/m156)
struct AAClipRegionBench {
    region: Region,
}

impl AAClipRegionBench {
    // Port of: bench/AAClipBench.cpp#L214-L224 (chrome/m156)
    fn new() -> Self {
        // test conversion of a complex clip to a aaclip
        // evenodd means we've constructed basically a stroked circle
        // SkPath path = SkPathBuilder(SkPathFillType::kEvenOdd).addCircle(0, 0, 200)
        //                   .addCircle(0, 0, 180).detach();
        let path = PathBuilder::new_with_fill_type(PathFillType::EvenOdd)
            .add_circle((0.0, 0.0), 200.0, None)
            .add_circle((0.0, 0.0), 180.0, None)
            .detach();
        // SkIRect bounds = path.getBounds().roundOut();
        let bounds: IRect = path.bounds().round_out();
        // fRegion.setPath(path, SkRegion(bounds));
        let mut region = Region::new();
        region.set_path(&path, &Region::from_rect(bounds));
        Self { region }
    }
}

impl Benchmark for AAClipRegionBench {
    // Port of: bench/AAClipBench.cpp#L225-L226 (chrome/m156)
    fn name(&self) -> String {
        "aaclip_setregion".to_owned()
    }

    // Port of: bench/AAClipBench.cpp#L227-L233 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            // SkAAClip clip;
            let mut clip = AAClip::new();
            // clip.setRegion(fRegion);
            let _ = clip.set_region(&self.region);
        }
    }
}

// Port of: bench/AAClipBench.cpp#L243 (chrome/m156)
def_bench!(
    aa_clip_builder_rect_bw = "AAClipBuilderBench(false, false)",
    AAClipBuilderBench::new(false, false)
);
// Port of: bench/AAClipBench.cpp#L244 (chrome/m156)
def_bench!(
    aa_clip_builder_rect_aa = "AAClipBuilderBench(false, true)",
    AAClipBuilderBench::new(false, true)
);
// Port of: bench/AAClipBench.cpp#L245 (chrome/m156)
def_bench!(
    aa_clip_builder_path_bw = "AAClipBuilderBench(true, false)",
    AAClipBuilderBench::new(true, false)
);
// Port of: bench/AAClipBench.cpp#L246 (chrome/m156)
def_bench!(
    aa_clip_builder_path_aa = "AAClipBuilderBench(true, true)",
    AAClipBuilderBench::new(true, true)
);
// Port of: bench/AAClipBench.cpp#L247 (chrome/m156)
def_bench!(
    aa_clip_region = "AAClipRegionBench()",
    AAClipRegionBench::new()
);
// Port of: bench/AAClipBench.cpp#L248 (chrome/m156)
def_bench!(
    aa_clip_path_bw = "AAClipBench(false, false)",
    AAClipBench::new(false, false)
);
// Port of: bench/AAClipBench.cpp#L249 (chrome/m156)
def_bench!(
    aa_clip_path_aa = "AAClipBench(false, true)",
    AAClipBench::new(false, true)
);
// Port of: bench/AAClipBench.cpp#L250 (chrome/m156)
def_bench!(
    aa_clip_path_bw_path = "AAClipBench(true, false)",
    AAClipBench::new(true, false)
);
// Port of: bench/AAClipBench.cpp#L251 (chrome/m156)
def_bench!(
    aa_clip_path_aa_path = "AAClipBench(true, true)",
    AAClipBench::new(true, true)
);
// Port of: bench/AAClipBench.cpp#L252 (chrome/m156)
def_bench!(
    nested_aa_clip_bw = "NestedAAClipBench(false)",
    NestedAAClipBench::new(false)
);
// Port of: bench/AAClipBench.cpp#L253 (chrome/m156)
def_bench!(
    nested_aa_clip_aa = "NestedAAClipBench(true)",
    NestedAAClipBench::new(true)
);
