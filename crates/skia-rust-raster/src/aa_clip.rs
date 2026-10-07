// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkAAClip.h, src/core/SkAAClip.cpp

//! [`AAClip`]: an antialiased clip stored as run-length encoded A8 coverage, its builder, and
//! [`AAClipBlitter`], the blitter wrapper that applies a clip to the calls of a real blitter.
//!
//! The data layout is Skia's: a list of rows (`YOffset`: the last scanline the row covers,
//! relative to the clip's top, and the offset of its data) over one byte buffer of `[count,
//! alpha]` run pairs (so a row of a clip that is 300 wide is at least two pairs: runs are at most
//! 255 long). The runs are shared between clones of a clip through an [`Arc`] (Skia's
//! `RunHead::fRefCnt`) and never modified once built, except by the builder's trimming, which
//! runs while the head is still unique.
//!
//! skia-rust: Skia's debug-only `SkAAClip::debug` dump is not ported. `validate` is, and runs
//! (in debug builds) where `AUTO_AACLIP_VALIDATE` does.

use std::sync::Arc;

use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::Alpha;
use skia_rust_core::color_data::{get_packed_b16, get_packed_g16, get_packed_r16, pack_rgb16};
use skia_rust_core::mask::{AllocType, Mask, MaskBuilder, MaskFormat};
use skia_rust_core::math::mul_div_255_round;
use skia_rust_core::path::Path;
use skia_rust_core::path_enums::ResolveConvexity;
use skia_rust_core::path_priv;
use skia_rust_core::path_raw::PathRaw;
use skia_rust_core::rect::{Contains, IRect, Rect, RoundOut};
use skia_rust_core::region::{Iterator as RegionIterator, Region};

use crate::blitter::{BlitMemory, Blitter};
use crate::scan::fill_path;
use crate::scan_anti_path::anti_fill_path_region;

const MAX_INT32: i32 = i32::MAX;

/// Asserts we're exactly `width` wide, and returns the number of bytes used.
// Port of: src/core/SkAAClip.cpp#L53-L65 (chrome/m156)
fn compute_row_length(row: &[u8], width: i32) -> usize {
    let mut width = width;
    let mut p = 0usize;
    while width > 0 {
        let n = i32::from(row[p]);
        debug_assert!(n > 0);
        debug_assert!(n <= width);
        p += 2;
        width -= n;
    }
    debug_assert_eq!(0, width);
    p
}

/// Data runs are packed `[count, alpha]`.
// Port of: src/core/SkAAClip.cpp#L67-L73 (chrome/m156)
#[derive(Copy, Clone, Debug)]
struct YOffset {
    /// `fY`: the last scanline of the row, relative to the top of the clip.
    y: i32,
    /// `fOffset`: where the row's runs start in `RunHead::data`.
    offset: u32,
}

// Port of: src/core/SkAAClip.cpp#L75-L127 (chrome/m156)
struct RowIter<'a> {
    row: &'a [u8],
    /// Index of the current run pair in `row` (Skia bumps `fRow`).
    pos: usize,
    left: i32,
    right: i32,
    bounds_right: i32,
    done: bool,
    alpha: u8,
}

impl<'a> RowIter<'a> {
    fn new(row: Option<&'a [u8]>, bounds: &IRect) -> Self {
        let left = bounds.left;
        let bounds_right = bounds.right;
        if let Some(row) = row {
            let right = bounds.left + i32::from(row[0]);
            debug_assert!(right <= bounds_right);
            RowIter {
                row,
                pos: 0,
                left,
                right,
                bounds_right,
                done: false,
                alpha: row[1],
            }
        } else {
            RowIter {
                row: &[],
                pos: 0,
                left,
                right: MAX_INT32,
                bounds_right,
                done: true,
                alpha: 0,
            }
        }
    }

    fn done(&self) -> bool {
        self.done
    }

    fn left(&self) -> i32 {
        self.left
    }

    fn right(&self) -> i32 {
        self.right
    }

    fn alpha(&self) -> u32 {
        u32::from(self.alpha)
    }

    fn next(&mut self) {
        if !self.done {
            self.left = self.right;
            if self.right == self.bounds_right {
                self.done = true;
                self.right = MAX_INT32;
                self.alpha = 0;
            } else {
                self.pos += 2;
                self.right += i32::from(self.row[self.pos]);
                self.alpha = self.row[self.pos + 1];
                debug_assert!(self.right <= self.bounds_right);
            }
        }
    }
}

// Port of: src/core/SkAAClip.cpp#L129-L170 (chrome/m156)
struct Iter<'a> {
    head: Option<&'a RunHead>,
    /// Index of `fCurrYOff`.
    curr: usize,
    /// `fStopYOff`: the number of rows.
    stop: usize,
    /// Offset of `fData` in `head.data`, `None` for Skia's null.
    data: Option<usize>,
    top: i32,
    bottom: i32,
    done: bool,
}

impl<'a> Iter<'a> {
    /// The already-finished iterator of `Iter()`.
    fn finished() -> Self {
        Iter {
            head: None,
            curr: 0,
            stop: 0,
            data: None,
            top: MAX_INT32,
            bottom: MAX_INT32,
            done: true,
        }
    }

    fn new(y: i32, head: &'a RunHead) -> Self {
        let start = &head.y_offsets[0];
        Iter {
            head: Some(head),
            curr: 0,
            stop: head.y_offsets.len(),
            data: Some(start.offset as usize),
            top: y,
            bottom: y + start.y + 1,
            done: false,
        }
    }

    fn done(&self) -> bool {
        self.done
    }

    fn top(&self) -> i32 {
        self.top
    }

    fn bottom(&self) -> i32 {
        self.bottom
    }

    /// The runs of the current row (`data()`), or `None` when finished.
    fn data(&self) -> Option<&'a [u8]> {
        match (self.head, self.data) {
            (Some(head), Some(off)) => Some(&head.data[off..]),
            _ => None,
        }
    }

    fn next(&mut self) {
        if !self.done {
            let head = self.head.expect("an unfinished iterator has runs");
            let prev = self.curr;
            let curr = prev + 1;
            debug_assert!(curr <= self.stop);

            self.top = self.bottom;
            if curr >= self.stop {
                self.done = true;
                self.bottom = MAX_INT32;
                self.data = None;
            } else {
                let (p, c) = (head.y_offsets[prev], head.y_offsets[curr]);
                self.bottom += c.y - p.y;
                let off = self.data.expect("an unfinished iterator has data");
                self.data = Some(off + (c.offset - p.offset) as usize);
                self.curr = curr;
            }
        }
    }
}

/// The shared run data of a clip.
// Port of: src/core/SkAAClip.cpp#L172-L252 (chrome/m156)
#[derive(Clone, Debug)]
struct RunHead {
    y_offsets: Vec<YOffset>,
    data: Vec<u8>,
}

impl RunHead {
    // Port of: src/core/SkAAClip.cpp#L203-L212 (chrome/m156)
    fn compute_row_size_for_width(width: i32) -> usize {
        // 2 bytes per segment, where each segment can store up to 255 for count
        let mut width = width;
        let mut segments = 0usize;
        while width > 0 {
            segments += 1;
            let n = width.min(255);
            width -= n;
        }
        segments * 2 // each segment is row[0] + row[1] (n + alpha)
    }

    // Port of: src/core/SkAAClip.cpp#L214-L233 (chrome/m156)
    fn alloc_rect(bounds: &IRect) -> RunHead {
        debug_assert!(!bounds.is_empty());
        let mut width = bounds.width();
        let row_size = Self::compute_row_size_for_width(width);
        let mut data = Vec::with_capacity(row_size);
        while width > 0 {
            let n = width.min(255);
            data.push(u8::try_from(n).expect("at most 255"));
            data.push(0xFF);
            width -= n;
        }
        RunHead {
            y_offsets: vec![YOffset {
                y: bounds.height() - 1,
                offset: 0,
            }],
            data,
        }
    }

    // Port of: src/core/SkAAClip.cpp#L235-L246 (chrome/m156)
    fn iterate(clip: &AAClip) -> Iter<'_> {
        match &clip.run_head {
            // A null run head is an empty clip, so return an already finished iterator.
            None => Iter::finished(),
            Some(head) => Iter::new(clip.bounds.top, head),
        }
    }
}

///////////////////////////////////////////////////////////////////////////////

/// One scanline (or several equal ones) being built.
struct Row {
    /// `fY`: relative to the top of the builder's bounds, the last scanline of the row.
    y: i32,
    width: i32,
    data: Vec<u8>,
}

// Port of: src/core/SkAAClip.cpp#L250-L254 (chrome/m156)
type AlphaProc = fn(u32, u32) -> u32;

// Port of: src/core/SkAAClip.cpp#L548-L556 (chrome/m156)
fn advance_row_iter(iter: &mut RowIter<'_>, iter_left: &mut i32, iter_rite: &mut i32, rite: i32) {
    if rite == *iter_rite {
        iter.next();
        *iter_left = iter.left();
        *iter_rite = iter.right();
    }
}

// Port of: src/core/SkAAClip.cpp#L621-L630 (chrome/m156)
fn advance_iter(iter: &mut Iter<'_>, iter_top: &mut i32, iter_bot: &mut i32, bot: i32) {
    if bot == *iter_bot {
        iter.next();
        *iter_top = *iter_bot;
        debug_assert_eq!(*iter_bot, iter.top());
        *iter_bot = iter.bottom();
    }
}

// Port of: src/core/SkAAClip.cpp#L1373-L1382 (chrome/m156)
fn append_x_run(x_array: &mut Vec<u8>, value: u8, count: i32) {
    debug_assert!(count >= 0);
    let mut count = count;
    while count > 0 {
        let n = count.min(255);
        x_array.push(u8::try_from(n).expect("at most 255"));
        x_array.push(value);
        count -= n;
    }
}

/// `SkAAClip::Builder`.
// Port of: src/core/SkAAClip.cpp#L250-L605 (chrome/m156)
struct Builder {
    bounds: IRect,
    rows: Vec<Row>,
    /// Index of `fCurrRow`.
    curr_row: Option<usize>,
    prev_y: i32,
    width: i32,
    min_y: i32,
}

impl Builder {
    fn new(bounds: IRect) -> Builder {
        Builder {
            bounds,
            rows: Vec::new(),
            curr_row: None,
            prev_y: -1,
            width: bounds.width(),
            min_y: bounds.top,
        }
    }

    fn apply_clip_op(&mut self, target: &mut AAClip, other: &AAClip, op: ClipOp) -> bool {
        self.operate_y(target, other, op);
        self.finish(target)
    }

    fn blit_path(&mut self, target: &mut AAClip, path: &Path, do_aa: bool) -> bool {
        let clip = Region::from_rect(self.bounds);

        let raw = path_priv::raw(path, ResolveConvexity::Yes)
            .unwrap_or_else(|| PathRaw::empty(path.fill_type()));
        {
            let mut blitter = BuilderBlitter::new(self);
            if do_aa {
                anti_fill_path_region(&raw, &clip, &mut blitter, true);
            } else {
                fill_path(&raw, &clip, &mut blitter);
            }
            blitter.finish();
        }
        self.finish(target)
    }

    // Port of: src/core/SkAAClip.cpp#L299-L330 (chrome/m156)
    fn add_run(&mut self, x: i32, y: i32, alpha: u32, count: i32) {
        debug_assert!(count > 0);
        debug_assert!(
            self.bounds
                .contains(skia_rust_core::point::IPoint::new(x, y))
        );
        debug_assert!(
            self.bounds
                .contains(skia_rust_core::point::IPoint::new(x + count - 1, y))
        );

        let x = x - self.bounds.left;
        let y = y - self.bounds.top;

        if y != self.prev_y {
            debug_assert!(y > self.prev_y);
            self.prev_y = y;
            let row = self.flush_row(true).expect("flushRow(true) returns a row");
            self.rows[row].y = y;
            self.rows[row].width = 0;
            debug_assert_eq!(self.rows[row].data.len(), 0);
            self.curr_row = Some(row);
        }
        let row = &mut self.rows[self.curr_row.expect("a current row")];

        debug_assert!(row.width <= x);
        debug_assert!(row.width < self.bounds.width());

        let gap = x - row.width;
        if gap != 0 {
            Self::append_run(&mut row.data, 0, gap);
            row.width += gap;
            debug_assert!(row.width < self.bounds.width());
        }

        Self::append_run(&mut row.data, alpha, count);
        row.width += count;
        debug_assert!(row.width <= self.bounds.width());
    }

    // Port of: src/core/SkAAClip.cpp#L332-L340 (chrome/m156)
    fn add_column(&mut self, x: i32, y: i32, alpha: u32, height: i32) {
        self.add_run(x, y, alpha, 1);
        let curr = self.curr_row.expect("a current row");
        self.flush_row_h(curr);
        let y = y - self.bounds.top;
        debug_assert_eq!(y, self.rows[curr].y);
        self.rows[curr].y = y + height - 1;
    }

    // Port of: src/core/SkAAClip.cpp#L342-L355 (chrome/m156)
    fn add_rect_run(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.add_run(x, y, 0xFF, width);

        // we assum the rect must be all we'll see for these scanlines
        // so we ensure our row goes all the way to our right
        let curr = self.curr_row.expect("a current row");
        self.flush_row_h(curr);

        let y = y - self.bounds.top;
        debug_assert_eq!(y, self.rows[curr].y);
        self.rows[curr].y = y + height - 1;
    }

    // Port of: src/core/SkAAClip.cpp#L357-L399 (chrome/m156)
    fn add_anti_rect_run(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        left_alpha: Alpha,
        right_alpha: Alpha,
    ) {
        // According to SkBlitter.cpp, no matter whether leftAlpha is 0 or positive,
        // we should always consider [x, x+1] as the left-most column and [x+1, x+1+width]
        // as the rect with full alpha.
        debug_assert!(width >= 0);

        let mut x = x;
        let mut width = width;
        // Conceptually we're always adding 3 runs, but we should
        // merge or omit them if possible.
        if left_alpha == 0xFF {
            width += 1;
        } else if left_alpha > 0 {
            self.add_run(x, y, u32::from(left_alpha), 1);
            x += 1;
        } else {
            // leftAlpha is 0, ignore the left column
            x += 1;
        }
        if right_alpha == 0xFF {
            width += 1;
        }
        if width > 0 {
            self.add_run(x, y, 0xFF, width);
        }
        if right_alpha > 0 && right_alpha < 255 {
            self.add_run(x + width, y, u32::from(right_alpha), 1);
        }

        // if we never called addRun, we might not have a fCurrRow yet
        if let Some(curr) = self.curr_row {
            // we assume the rect must be all we'll see for these scanlines
            // so we ensure our row goes all the way to our right
            self.flush_row_h(curr);

            let y = y - self.bounds.top;
            debug_assert_eq!(y, self.rows[curr].y);
            self.rows[curr].y = y + height - 1;
        }
    }

    // Port of: src/core/SkAAClip.cpp#L401-L449 (chrome/m156)
    fn finish(&mut self, target: &mut AAClip) -> bool {
        self.flush_row(false);

        let data_size: usize = self.rows.iter().map(|row| row.data.len()).sum();

        if 0 == data_size {
            return target.set_empty();
        }

        debug_assert!(self.min_y >= self.bounds.top);
        debug_assert!(self.min_y < self.bounds.bottom);
        let adjust_y = self.min_y - self.bounds.top;
        self.bounds.top = self.min_y;

        let mut y_offsets = Vec::with_capacity(self.rows.len());
        let mut data = Vec::with_capacity(data_size);

        let mut prev_y = self.rows[0].y - 1;
        for row in &self.rows {
            debug_assert!(prev_y < row.y); // must be monotonic
            prev_y = row.y;

            y_offsets.push(YOffset {
                y: row.y - adjust_y,
                offset: u32::try_from(data.len()).expect("SkToU32"),
            });

            data.extend_from_slice(&row.data);
            debug_assert_eq!(
                compute_row_length(&row.data, self.bounds.width()),
                row.data.len()
            );
        }

        target.free_runs();
        target.bounds = self.bounds;
        target.run_head = Some(Arc::new(RunHead { y_offsets, data }));
        target.trim_bounds()
    }

    // Port of: src/core/SkAAClip.cpp#L487-L497 (chrome/m156)
    fn flush_row_h(&mut self, row: usize) {
        // flush current row if needed
        let width = self.width;
        let row = &mut self.rows[row];
        if row.width < width {
            Self::append_run(&mut row.data, 0, width - row.width);
            row.width = width;
        }
    }

    // Port of: src/core/SkAAClip.cpp#L499-L531 (chrome/m156)
    fn flush_row(&mut self, ready_for_another: bool) -> Option<usize> {
        let mut next = None;
        let count = self.rows.len();
        if count > 0 {
            self.flush_row_h(count - 1);
        }
        if count > 1 {
            // are our last two runs the same?
            debug_assert_eq!(self.rows[count - 2].width, self.width);
            debug_assert_eq!(self.rows[count - 1].width, self.width);
            if self.rows[count - 2].data == self.rows[count - 1].data {
                self.rows[count - 2].y = self.rows[count - 1].y;
                if ready_for_another {
                    self.rows[count - 1].data.clear();
                    next = Some(count - 1);
                } else {
                    self.rows.pop();
                }
            } else if ready_for_another {
                next = Some(self.append_row());
            }
        } else if ready_for_another {
            next = Some(self.append_row());
        }
        next
    }

    fn append_row(&mut self) -> usize {
        self.rows.push(Row {
            y: 0,
            width: 0,
            data: Vec::new(),
        });
        self.rows.len() - 1
    }

    // Port of: src/core/SkAAClip.cpp#L533-L545 (chrome/m156)
    fn append_run(data: &mut Vec<u8>, alpha: u32, count: i32) {
        let mut count = count;
        loop {
            let n = count.min(255);
            // `ptr[0] = n; ptr[1] = alpha;`: both are bytes in Skia.
            data.push(u8::try_from(n).expect("at most 255"));
            data.push(u8::try_from(alpha & 0xFF).expect("masked"));
            count -= n;
            if count <= 0 {
                break;
            }
        }
    }

    // Port of: src/core/SkAAClip.cpp#L547-L605 (chrome/m156)
    #[allow(clippy::comparison_chain)] // mirrors the C++ if/else-if chain
    fn operate_x(
        &mut self,
        last_y: i32,
        iter_a: &mut RowIter<'_>,
        iter_b: &mut RowIter<'_>,
        proc: AlphaProc,
    ) {
        let mut left_a = iter_a.left();
        let mut rite_a = iter_a.right();
        let mut left_b = iter_b.left();
        let mut rite_b = iter_b.right();

        let mut prev_rite = self.bounds.left;

        loop {
            let mut alpha_a = 0u32;
            let mut alpha_b = 0u32;
            let left;
            let mut rite;

            if left_a < left_b {
                left = left_a;
                alpha_a = iter_a.alpha();
                if rite_a <= left_b {
                    rite = rite_a;
                } else {
                    left_a = left_b;
                    rite = left_b;
                }
            } else if left_b < left_a {
                left = left_b;
                alpha_b = iter_b.alpha();
                if rite_b <= left_a {
                    rite = rite_b;
                } else {
                    left_b = left_a;
                    rite = left_a;
                }
            } else {
                left = left_a; // or leftB, since leftA == leftB
                rite = rite_a.min(rite_b);
                left_a = rite;
                left_b = rite;
                alpha_a = iter_a.alpha();
                alpha_b = iter_b.alpha();
            }

            if left >= self.bounds.right {
                break;
            }
            if rite > self.bounds.right {
                rite = self.bounds.right;
            }

            if left >= self.bounds.left {
                debug_assert!(rite > left);
                self.add_run(left, last_y, proc(alpha_a, alpha_b), rite - left);
                prev_rite = rite;
            }

            advance_row_iter(iter_a, &mut left_a, &mut rite_a, rite);
            advance_row_iter(iter_b, &mut left_b, &mut rite_b, rite);

            if iter_a.done() && iter_b.done() {
                break;
            }
        }

        if prev_rite < self.bounds.right {
            self.add_run(prev_rite, last_y, 0, self.bounds.right - prev_rite);
        }
    }

    // Port of: src/core/SkAAClip.cpp#L607-L685 (chrome/m156)
    #[allow(clippy::comparison_chain)] // mirrors the C++ if/else-if chain
    fn operate_y(&mut self, a: &AAClip, b: &AAClip, op: ClipOp) {
        let proc: AlphaProc = if op == ClipOp::Difference {
            |a, b| mul_div_255_round(a, 0xFF - b)
        } else {
            |a, b| mul_div_255_round(a, b)
        };

        let mut iter_a = RunHead::iterate(a);
        let mut iter_b = RunHead::iterate(b);

        debug_assert!(!iter_a.done());
        let mut top_a = iter_a.top();
        let mut bot_a = iter_a.bottom();
        debug_assert!(!iter_b.done());
        let mut top_b = iter_b.top();
        let mut bot_b = iter_b.bottom();

        loop {
            let mut row_a: Option<&[u8]> = None;
            let mut row_b: Option<&[u8]> = None;
            let top;
            let mut bot;

            if top_a < top_b {
                top = top_a;
                row_a = iter_a.data();
                if bot_a <= top_b {
                    bot = bot_a;
                } else {
                    top_a = top_b;
                    bot = top_b;
                }
            } else if top_b < top_a {
                top = top_b;
                row_b = iter_b.data();
                if bot_b <= top_a {
                    bot = bot_b;
                } else {
                    top_b = top_a;
                    bot = top_a;
                }
            } else {
                top = top_a; // or topB, since topA == topB
                bot = bot_a.min(bot_b);
                top_a = bot;
                top_b = bot;
                row_a = iter_a.data();
                row_b = iter_b.data();
            }

            if top >= self.bounds.bottom {
                break;
            }

            if bot > self.bounds.bottom {
                bot = self.bounds.bottom;
            }
            debug_assert!(top < bot);

            if row_a.is_none() && row_b.is_none() {
                self.add_run(self.bounds.left, bot - 1, 0, self.bounds.width());
            } else if top >= self.bounds.top {
                debug_assert!(bot <= self.bounds.bottom);
                let bounds_a = if row_a.is_some() {
                    a.bounds
                } else {
                    self.bounds
                };
                let bounds_b = if row_b.is_some() {
                    b.bounds
                } else {
                    self.bounds
                };
                let mut row_iter_a = RowIter::new(row_a, &bounds_a);
                let mut row_iter_b = RowIter::new(row_b, &bounds_b);
                self.operate_x(bot - 1, &mut row_iter_a, &mut row_iter_b, proc);
            }

            advance_iter(&mut iter_a, &mut top_a, &mut bot_a, bot);
            advance_iter(&mut iter_b, &mut top_b, &mut bot_b, bot);

            if iter_a.done() && iter_b.done() {
                break;
            }
        }
    }
}

/// The blitter the scan converter draws a path into when building a clip
/// (`SkAAClip::Builder::Blitter`).
// Port of: src/core/SkAAClip.cpp#L687-L835 (chrome/m156)
struct BuilderBlitter<'a> {
    builder: &'a mut Builder,
    last_y: i32,
    /// Cache of the builder's bounds' left edge.
    left: i32,
    right: i32,
    min_y: i32,
    memory: BlitMemory,
}

impl<'a> BuilderBlitter<'a> {
    // Port of: src/core/SkAAClip.cpp#L705-L710 (chrome/m156)
    fn new(builder: &'a mut Builder) -> Self {
        let left = builder.bounds.left;
        let right = builder.bounds.right;
        BuilderBlitter {
            builder,
            last_y: -i32::MAX, // sentinel
            left,
            right,
            min_y: i32::MAX,
            memory: BlitMemory::default(),
        }
    }

    // If we see a gap of 1 or more empty scanlines while building in Y-order,
    // we inject an explicit empty scanline (alpha==0)
    //
    // See AAClipTest.cpp : test_path_with_hole()
    // Port of: src/core/SkAAClip.cpp#L694-L704 (chrome/m156)
    fn check_for_y_gap(&mut self, y: i32) {
        debug_assert!(y >= self.last_y);
        if self.last_y > -i32::MAX {
            let gap = y - self.last_y;
            if gap > 1 {
                self.builder
                    .add_run(self.left, y - 1, 0, self.right - self.left);
            }
        }
        self.last_y = y;
    }

    // Port of: src/core/SkAAClip.cpp#L712-L716 (chrome/m156)
    fn finish(&mut self) {
        if self.min_y < i32::MAX {
            self.builder.min_y = self.min_y;
        }
    }

    // We track this, in case the scan converter skipped some number of
    // scanlines at the (relative to the bounds it was given). This allows
    // the builder, during its finish, to trip its bounds down to the "real"
    // top.
    // Port of: src/core/SkAAClip.cpp#L821-L825 (chrome/m156)
    fn record_min_y(&mut self, y: i32) {
        if y < self.min_y {
            self.min_y = y;
        }
    }
}

impl Blitter for BuilderBlitter<'_> {
    // Must evaluate clips in scan-line order, so don't want to allow blitV(),
    // but an AAClip can be clipped down to a single pixel wide, so we
    // must support it (given AntiRect semantics: minimum width is 2).
    // Instead we'll rely on the runtime asserts to guarantee Y monotonicity;
    // any failure cases that misses may have minor artifacts.
    // Port of: src/core/SkAAClip.cpp#L725-L738 (chrome/m156)
    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: Alpha) {
        if height == 1 {
            // We're still in scan-line order if height is 1
            // This is useful for Analytic AA
            let mut alphas = [alpha, 0];
            let mut runs = [1i16, 0];
            self.blit_anti_h(x, y, &mut alphas, &mut runs);
        } else {
            self.record_min_y(y);
            self.builder.add_column(x, y, u32::from(alpha), height);
            self.last_y = y + height - 1;
        }
    }

    // Port of: src/core/SkAAClip.cpp#L740-L745 (chrome/m156)
    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.record_min_y(y);
        self.check_for_y_gap(y);
        self.builder.add_rect_run(x, y, width, height);
        self.last_y = y + height - 1;
    }

    // Port of: src/core/SkAAClip.cpp#L747-L754 (chrome/m156)
    fn blit_anti_rect(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        left_alpha: Alpha,
        right_alpha: Alpha,
    ) {
        self.record_min_y(y);
        self.check_for_y_gap(y);
        self.builder
            .add_anti_rect_run(x, y, width, height, left_alpha, right_alpha);
        self.last_y = y + height - 1;
    }

    // Port of: src/core/SkAAClip.cpp#L756-L757 (chrome/m156)
    fn blit_mask(&mut self, _mask: &Mask<'_>, _clip: &IRect) {
        panic!("---- did not expect to get called here");
    }

    // Port of: src/core/SkAAClip.cpp#L759-L763 (chrome/m156)
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        self.record_min_y(y);
        self.check_for_y_gap(y);
        self.builder.add_run(x, y, 0xFF, width);
    }

    // Port of: src/core/SkAAClip.cpp#L765-L805 (chrome/m156)
    fn blit_anti_h(&mut self, x: i32, y: i32, alpha: &mut [Alpha], runs: &mut [i16]) {
        self.record_min_y(y);
        self.check_for_y_gap(y);
        let mut x = x;
        let mut idx = 0usize;
        loop {
            let count = i32::from(runs[idx]);
            if count <= 0 {
                return;
            }

            // The supersampler's buffer can be the width of the device, so
            // we may have to trim the run to our bounds. Previously, we assert that
            // the extra spans are always alpha==0.
            // However, the analytic AA is too sensitive to precision errors
            // so it may have extra spans with very tiny alpha because after several
            // arithmatic operations, the edge may bleed the path boundary a little bit.
            // Therefore, instead of always asserting alpha==0, we assert alpha < 0x10.
            let mut local_x = x;
            let mut local_count = count;
            if x < self.left {
                debug_assert!(0x10 > alpha[idx]);
                let gap = self.left - x;
                debug_assert!(gap <= count);
                local_x += gap;
                local_count -= gap;
            }
            let right = x + count;
            if right > self.right {
                debug_assert!(0x10 > alpha[idx]);
                local_count -= right - self.right;
                debug_assert!(local_count >= 0);
            }

            if local_count != 0 {
                self.builder
                    .add_run(local_x, y, u32::from(alpha[idx]), local_count);
            }
            // Next run
            idx += usize::try_from(count).expect("positive");
            x += count;
        }
    }

    fn blit_memory(&mut self) -> &mut BlitMemory {
        &mut self.memory
    }
}

///////////////////////////////////////////////////////////////////////////////

// Count the number of zeros on the left and right edges of the passed in
// RLE row. If 'row' is all zeros return 'width' in both variables.
// Port of: src/core/SkAAClip.cpp#L1006-L1043 (chrome/m156)
fn count_left_right_zeros(row: &[u8], width: i32) -> (i32, i32) {
    let mut width = width;
    let mut p = 0usize;
    let mut zeros = 0;
    loop {
        if row[p + 1] != 0 {
            break;
        }
        let n = i32::from(row[p]);
        debug_assert!(n > 0);
        debug_assert!(n <= width);
        zeros += n;
        p += 2;
        width -= n;
        if width <= 0 {
            break;
        }
    }
    let left_z = zeros;

    if 0 == width {
        // this line is completely empty return 'width' in both variables
        return (left_z, left_z);
    }

    zeros = 0;
    while width > 0 {
        let n = i32::from(row[p]);
        debug_assert!(n > 0);
        if 0 == row[p + 1] {
            zeros += n;
        } else {
            zeros = 0;
        }
        p += 2;
        width -= n;
    }
    (left_z, zeros)
}

// modify row in place, trimming off (zeros) from the left and right sides.
// return the number of bytes that were completely eliminated from the left
// Port of: src/core/SkAAClip.cpp#L1045-L1094 (chrome/m156)
fn trim_row_left_right(row: &mut [u8], width: i32, left_z: i32, rite_z: i32) -> u32 {
    let mut width = width;
    let mut left_z = left_z;
    let mut rite_z = rite_z;
    let mut p = 0usize;
    let mut trim = 0;
    while left_z > 0 {
        debug_assert_eq!(0, row[p + 1]);
        let n = i32::from(row[p]);
        debug_assert!(n > 0);
        debug_assert!(n <= width);
        width -= n;
        p += 2;
        if n > left_z {
            row[p - 2] = u8::try_from(n - left_z).expect("fits a run");
            break;
        }
        trim += 2;
        left_z -= n;
        debug_assert!(left_z >= 0);
    }

    if rite_z != 0 {
        // walk row to the end, and then we'll back up to trim riteZ
        while width > 0 {
            let n = i32::from(row[p]);
            debug_assert!(n <= width);
            width -= n;
            p += 2;
        }
        // now skip whole runs of zeros
        loop {
            p -= 2;
            debug_assert_eq!(0, row[p + 1]);
            let n = i32::from(row[p]);
            debug_assert!(n > 0);
            if n > rite_z {
                row[p] = u8::try_from(n - rite_z).expect("fits a run");
                break;
            }
            rite_z -= n;
            debug_assert!(rite_z >= 0);
            if rite_z <= 0 {
                break;
            }
        }
    }

    trim
}

// Port of: src/core/SkAAClip.cpp#L1121-L1135 (chrome/m156)
fn row_is_all_zeros(row: &[u8], width: i32) -> bool {
    debug_assert!(width > 0);
    let mut width = width;
    let mut p = 0usize;
    loop {
        if row[p + 1] != 0 {
            return false;
        }
        let n = i32::from(row[p]);
        debug_assert!(n <= width);
        width -= n;
        p += 2;
        if width <= 0 {
            break;
        }
    }
    debug_assert_eq!(0, width);
    true
}

///////////////////////////////////////////////////////////////////////////////

/// An antialiased clip: a bounds and the coverage of every pixel in it, stored run-length
/// encoded (`SkAAClip`). Cloning shares the run data.
// Port of: src/core/SkAAClip.h#L25-L87 (chrome/m156)
#[doc(alias = "SkAAClip")]
#[derive(Clone, Debug, Default)]
pub struct AAClip {
    bounds: IRect,
    /// `fRunHead`: `None` for an empty clip.
    run_head: Option<Arc<RunHead>>,
}

impl AAClip {
    /// An empty clip (`SkAAClip()`).
    // Port of: src/core/SkAAClip.cpp#L1288-L1291 (chrome/m156)
    #[must_use]
    pub fn new() -> AAClip {
        AAClip {
            bounds: IRect::new_empty(),
            run_head: None,
        }
    }

    /// True if the clip is empty (`isEmpty`).
    #[doc(alias = "isEmpty")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.run_head.is_none()
    }

    /// The bounds of the clip (`getBounds`).
    #[doc(alias = "getBounds")]
    #[must_use]
    pub fn bounds(&self) -> &IRect {
        &self.bounds
    }

    /// Returns true iff the clip is not empty, and is just a hard-edged rect (no partial alpha).
    /// If true, [`Self::bounds`] can be used in place of this clip.
    // Port of: src/core/SkAAClip.cpp#L1334-L1358 (chrome/m156)
    #[doc(alias = "isRect")]
    #[must_use]
    pub fn is_rect(&self) -> bool {
        let Some(head) = &self.run_head else {
            return false;
        };

        if head.y_offsets.len() != 1 {
            return false;
        }
        let yoff = head.y_offsets[0];
        if yoff.y != self.bounds.bottom - 1 {
            return false;
        }

        let mut p = yoff.offset as usize;
        let mut width = self.bounds.width();
        loop {
            if head.data[p + 1] != 0xFF {
                return false;
            }
            let n = i32::from(head.data[p]);
            debug_assert!(n <= width);
            width -= n;
            p += 2;
            if width <= 0 {
                break;
            }
        }
        true
    }

    /// Makes the clip empty; always returns false (`setEmpty`).
    // Port of: src/core/SkAAClip.cpp#L1308-L1313 (chrome/m156)
    #[doc(alias = "setEmpty")]
    pub fn set_empty(&mut self) -> bool {
        self.free_runs();
        self.bounds.set_empty();
        self.run_head = None;
        false
    }

    /// Sets the clip to the hard-edged `bounds`; false if `bounds` is empty (`setRect`).
    // Port of: src/core/SkAAClip.cpp#L1315-L1332 (chrome/m156)
    #[doc(alias = "setRect")]
    pub fn set_rect(&mut self, bounds: &IRect) -> bool {
        if bounds.is_empty() {
            return self.set_empty();
        }

        self.validate();

        self.free_runs();
        self.bounds = *bounds;
        self.run_head = Some(Arc::new(RunHead::alloc_rect(bounds)));
        debug_assert!(!self.is_empty());
        self.validate();
        true
    }

    /// Sets the clip to the coverage of `path` within `clip`; false if the result is empty
    /// (`setPath`).
    // Port of: src/core/SkAAClip.cpp#L1403-L1428 (chrome/m156)
    #[doc(alias = "setPath")]
    pub fn set_path(&mut self, path: &Path, clip: &IRect, do_aa: bool) -> bool {
        self.validate();

        if clip.is_empty() {
            return self.set_empty();
        }

        let ibounds;
        // Since we assert that the BuilderBlitter will never blit outside the intersection
        // of clip and ibounds, we create the builder with the snug bounds.
        if path.is_inverse_fill_type() {
            ibounds = *clip;
        } else {
            let b: IRect = path.bounds().round_out();
            // It's possible the bounds of our path might exceed SK_MaxS32 in width
            // but since our clip is within that width (otherwise isEmpty() above
            // would catch it), we can use isEmpty64() safely here. blitPath will
            // interesect the two bounds before drawing.
            if b.is_empty_64() {
                return self.set_empty();
            }
            match IRect::intersect(&b, clip) {
                Some(r) => ibounds = r,
                None => return self.set_empty(),
            }
        }

        let mut builder = Builder::new(ibounds);
        builder.blit_path(self, path, do_aa)
    }

    /// Sets the clip to the (hard-edged) area of `rgn` (`setRegion`).
    ///
    /// # Panics
    /// Only if the region's runs are corrupt.
    // Port of: src/core/SkAAClip.cpp#L1360-L1401 (chrome/m156)
    #[doc(alias = "setRegion")]
    pub fn set_region(&mut self, rgn: &Region) -> bool {
        if rgn.is_empty() {
            return self.set_empty();
        }
        if rgn.is_rect() {
            return self.set_rect(rgn.bounds());
        }

        let bounds = *rgn.bounds();
        let offset_x = bounds.left;
        let offset_y = bounds.top;

        let mut y_array: Vec<YOffset> = Vec::new();
        let mut x_array: Vec<u8> = Vec::new();

        let offset_of = |x_array: &Vec<u8>| u32::try_from(x_array.len()).expect("offset");

        let mut iter = RegionIterator::new(rgn);
        let mut prev_right = 0;
        let mut prev_bot = 0;
        let mut have_curr_y = false;

        while !iter.is_done() {
            let r = *iter.rect();
            debug_assert!(bounds.contains(&r));

            let bot = r.bottom - offset_y;
            debug_assert!(bot >= prev_bot);
            if bot > prev_bot {
                if have_curr_y {
                    // flush current row
                    append_x_run(&mut x_array, 0, bounds.width() - prev_right);
                }
                // did we introduce an empty-gap from the prev row?
                let top = r.top - offset_y;
                if top > prev_bot {
                    y_array.push(YOffset {
                        y: top - 1,
                        offset: offset_of(&x_array),
                    });
                    append_x_run(&mut x_array, 0, bounds.width());
                }
                // create a new record for this Y value
                y_array.push(YOffset {
                    y: bot - 1,
                    offset: offset_of(&x_array),
                });
                have_curr_y = true;
                prev_right = 0;
                prev_bot = bot;
            }

            let x = r.left - offset_x;
            append_x_run(&mut x_array, 0, x - prev_right);

            let w = r.right - r.left;
            append_x_run(&mut x_array, 0xFF, w);
            prev_right = x + w;
            debug_assert!(prev_right <= bounds.width());
            iter.next();
        }
        // flush last row
        append_x_run(&mut x_array, 0, bounds.width() - prev_right);

        // now pack everything into a RunHead
        let head = RunHead {
            y_offsets: y_array,
            data: x_array,
        };

        self.set_empty();
        self.bounds = bounds;
        self.run_head = Some(Arc::new(head));
        self.validate();
        true
    }

    /// Combines the clip with `other` (`op(const SkAAClip&, SkClipOp)`). Returns true if the
    /// result is not empty.
    // Port of: src/core/SkAAClip.cpp#L1430-L1469 (chrome/m156)
    #[doc(alias = "op")]
    pub fn op_aa_clip(&mut self, other: &AAClip, op: ClipOp) -> bool {
        self.validate();

        if self.is_empty() {
            // Once the clip is empty, it cannot become un-empty.
            return false;
        }

        let mut bounds = self.bounds;
        match op {
            ClipOp::Difference => {
                if other.is_empty() || !IRect::intersects(&self.bounds, &other.bounds) {
                    // this remains unmodified and isn't empty
                    return true;
                }
            }
            ClipOp::Intersect => {
                if other.is_empty() {
                    // the intersected clip becomes empty
                    return self.set_empty();
                }
                match IRect::intersect(&bounds, &other.bounds) {
                    Some(r) => bounds = r,
                    // the intersected clip becomes empty
                    None => return self.set_empty(),
                }
            }
        }

        debug_assert!(IRect::intersects(&bounds, &self.bounds));
        debug_assert!(IRect::intersects(&bounds, &other.bounds));

        let mut builder = Builder::new(bounds);
        builder.apply_clip_op(self, other, op)
    }

    /// Combines the clip with the hard-edged `rect` (`op(const SkIRect&, SkClipOp)`). Returns
    /// true if the result is not empty.
    // Port of: src/core/SkAAClip.cpp#L1471-L1498 (chrome/m156)
    #[doc(alias = "op")]
    pub fn op_irect(&mut self, rect: &IRect, op: ClipOp) -> bool {
        // It can be expensive to build a local aaclip before applying the op, so
        // we first see if we can restrict the bounds of new rect to our current
        // bounds, or note that the new rect subsumes our current clip.
        let Some(pixel_bounds) = IRect::intersect(&self.bounds, rect) else {
            // No change or clip becomes empty depending on 'op'
            return match op {
                ClipOp::Difference => !self.is_empty(),
                ClipOp::Intersect => self.set_empty(),
            };
        };
        if pixel_bounds == self.bounds {
            // Wholly inside 'rect', so clip becomes empty or remains unchanged
            match op {
                ClipOp::Difference => self.set_empty(),
                ClipOp::Intersect => !self.is_empty(),
            }
        } else if op == ClipOp::Intersect && self.quick_contains(&pixel_bounds) {
            // We become just the remaining rectangle
            self.set_rect(&pixel_bounds)
        } else {
            let mut clip = AAClip::new();
            clip.set_rect(rect);
            self.op_aa_clip(&clip, op)
        }
    }

    /// Combines the clip with `rect`, antialiased if `do_aa` (`op(const SkRect&, SkClipOp,
    /// bool)`). Returns true if the result is not empty.
    // Port of: src/core/SkAAClip.cpp#L1500-L1537 (chrome/m156)
    #[doc(alias = "op")]
    pub fn op_rect(&mut self, rect: &Rect, op: ClipOp, do_aa: bool) -> bool {
        if do_aa {
            // Tighten bounds for "path" aaclip of the rect
            let Some(pixel_bounds) = IRect::intersect(&self.bounds, &rect.round_out()) else {
                // No change or clip becomes empty depending on 'op'
                return match op {
                    ClipOp::Difference => !self.is_empty(),
                    ClipOp::Intersect => self.set_empty(),
                };
            };
            if rect.contains(&Rect::from_irect(self.bounds)) {
                // Wholly inside 'rect', so clip becomes empty or remains unchanged
                match op {
                    ClipOp::Difference => self.set_empty(),
                    ClipOp::Intersect => !self.is_empty(),
                }
            } else if op == ClipOp::Intersect && self.quick_contains(&pixel_bounds) {
                // We become just the rect intersected with pixel bounds (preserving fractional
                // coords for AA edges).
                self.set_path(&Path::rect(rect, None), &pixel_bounds, true)
            } else {
                let mut rect_clip = AAClip::new();
                let clip_bounds = if op == ClipOp::Difference {
                    self.bounds
                } else {
                    pixel_bounds
                };
                rect_clip.set_path(&Path::rect(rect, None), &clip_bounds, true);
                self.op_aa_clip(&rect_clip, op)
            }
        } else {
            self.op_irect(&rect.round(), op)
        }
    }

    /// Offsets the clip by `(dx, dy)` into `dst` (`translate`, with `dst` not null). Returns
    /// true if the clip is not empty. See [`Self::translate_in_place`] for `dst == this`.
    // Port of: src/core/SkAAClip.cpp#L1541-L1560 (chrome/m156)
    pub fn translate(&self, dx: i32, dy: i32, dst: &mut AAClip) -> bool {
        if self.is_empty() {
            return dst.set_empty();
        }

        dst.free_runs();
        dst.run_head.clone_from(&self.run_head);
        dst.bounds = self.bounds;
        dst.bounds.offset((dx, dy));
        true
    }

    /// `translate(dx, dy, this)`.
    // Port of: src/core/SkAAClip.cpp#L1541-L1560 (chrome/m156)
    #[doc(alias = "translate")]
    pub fn translate_in_place(&mut self, dx: i32, dy: i32) -> bool {
        if self.is_empty() {
            return self.set_empty();
        }

        self.bounds.offset((dx, dy));
        true
    }

    /// Allocates a mask the size of the aaclip, and expands its data into the mask, using
    /// `A8`. An empty clip gives an empty mask without an image. Used for tests and visualization
    /// purposes (`copyToMask`).
    ///
    /// # Panics
    /// Only if the clip's runs are corrupt.
    // Port of: src/core/SkAAClip.cpp#L837-L874 (chrome/m156)
    #[doc(alias = "copyToMask")]
    #[must_use]
    pub fn copy_to_mask(&self) -> MaskBuilder {
        fn expand_row_to_mask(dst: &mut [u8], row: &[u8], width: i32) {
            let mut width = width;
            let mut d = 0usize;
            let mut p = 0usize;
            while width > 0 {
                let n = usize::from(row[p]);
                debug_assert!(width >= i32::from(row[p]));
                dst[d..d + n].fill(row[p + 1]);
                d += n;
                width -= i32::from(row[p]);
                p += 2;
            }
            debug_assert_eq!(0, width);
        }

        let mut mask = MaskBuilder {
            format: MaskFormat::A8,
            ..MaskBuilder::default()
        };
        if self.is_empty() {
            mask.bounds.set_empty();
            mask.image = Vec::new();
            mask.row_bytes = 0;
            return mask;
        }

        mask.bounds = self.bounds;
        mask.row_bytes = u32::try_from(self.bounds.width()).expect("positive width");
        let size = mask.compute_image_size();
        mask.image = MaskBuilder::alloc_image(size, AllocType::Uninit);

        let mut iter = RunHead::iterate(self);
        let width = self.bounds.width();
        let row_bytes = mask.row_bytes as usize;

        let mut dst = 0usize;
        let mut y = self.bounds.top;
        while !iter.done() {
            loop {
                expand_row_to_mask(
                    &mut mask.image[dst..],
                    iter.data().expect("rows have data"),
                    width,
                );
                dst += row_bytes;
                y += 1;
                if y >= iter.bottom() {
                    break;
                }
            }
            iter.next();
        }
        mask
    }

    /// True if every pixel of `r` is fully covered (`quickContains(const SkIRect&)`).
    // Port of: src/core/SkAAClip.h#L67-L69 (chrome/m156)
    #[doc(alias = "quickContains")]
    #[must_use]
    pub fn quick_contains(&self, r: &IRect) -> bool {
        self.quick_contains_ltrb(r.left, r.top, r.right, r.bottom)
    }

    /// Checks the invariants of the clip (`validate`, debug builds only).
    ///
    /// # Panics
    /// In debug builds, if an invariant does not hold.
    // Port of: src/core/SkAAClip.cpp#L876-L910 (chrome/m156)
    pub fn validate(&self) {
        if !cfg!(debug_assertions) {
            return;
        }
        let Some(head) = &self.run_head else {
            debug_assert!(self.bounds.is_empty());
            return;
        };
        debug_assert!(!self.bounds.is_empty());

        debug_assert!(!head.y_offsets.is_empty());

        let last_y = self.bounds.height() - 1;

        // Y and offset must be monotonic
        let mut prev_y = -1;
        let mut prev_offset: i64 = -1;
        for yoff in &head.y_offsets {
            debug_assert!(prev_y < yoff.y);
            debug_assert!(yoff.y <= last_y);
            prev_y = yoff.y;
            debug_assert!(prev_offset < i64::from(yoff.offset));
            prev_offset = i64::from(yoff.offset);
            let row = &head.data[yoff.offset as usize..];
            let row_length = compute_row_length(row, self.bounds.width());
            debug_assert!(yoff.offset as usize + row_length <= head.data.len());
        }
        // check the last entry;
        debug_assert_eq!(head.y_offsets.last().expect("rows").y, last_y);
    }

    // Port of: src/core/SkAAClip.cpp#L1562-L1569 (chrome/m156)
    fn free_runs(&mut self) {
        self.run_head = None;
    }

    // Port of: src/core/SkAAClip.cpp#L1609-L1638 (chrome/m156)
    fn quick_contains_ltrb(&self, left: i32, top: i32, right: i32, bottom: i32) -> bool {
        if self.is_empty() {
            return false;
        }
        if !self.bounds.contains(&IRect::new(left, top, right, bottom)) {
            return false;
        }

        let Some((row, last_y)) = self.find_row(top) else {
            return false;
        };
        if last_y < bottom {
            return false;
        }
        // now just need to check in X
        let (mut row, mut count) = self.find_x(row, left);

        let mut rect_width = right - left;
        while 0xFF == row[1] {
            if count >= rect_width {
                return true;
            }
            rect_width -= count;
            row = &row[2..];
            count = i32::from(row[0]);
        }
        false
    }

    /// The runs of the row that covers scanline `y` (starting at the first run of the row), and
    /// the last scanline that row covers. `None` outside the bounds (`findRow`).
    // Port of: src/core/SkAAClip.cpp#L1571-L1590 (chrome/m156)
    fn find_row(&self, y: i32) -> Option<(&[u8], i32)> {
        let head = self.run_head.as_ref().expect("findRow needs runs");

        if y < self.bounds.top || y >= self.bounds.bottom {
            return None;
        }
        let y = y - self.bounds.y(); // our yoffs values are relative to the top

        let mut idx = 0usize;
        while head.y_offsets[idx].y < y {
            idx += 1;
            debug_assert!(idx < head.y_offsets.len());
        }

        let yoff = head.y_offsets[idx];
        Some((&head.data[yoff.offset as usize..], self.bounds.y() + yoff.y))
    }

    /// The run of `data` (the runs of a row) that covers `x`, and how many pixels of it remain
    /// from `x` on (`findX`).
    // Port of: src/core/SkAAClip.cpp#L1592-L1607 (chrome/m156)
    fn find_x<'d>(&self, data: &'d [u8], x: i32) -> (&'d [u8], i32) {
        debug_assert!(x >= self.bounds.left && x < self.bounds.right);
        let mut x = x - self.bounds.x();

        // first skip up to X
        let mut data = data;
        loop {
            let n = i32::from(data[0]);
            if x < n {
                return (data, n - x);
            }
            data = &data[2..];
            x -= n;
        }
    }

    // Port of: src/core/SkAAClip.cpp#L1213-L1226 (chrome/m156)
    fn trim_left_right(&mut self) -> bool {
        if self.is_empty() {
            return false;
        }

        self.validate();

        let width = self.bounds.width();
        let mut left_zeros = width;
        let mut rite_zeros = width;
        {
            let head = self.run_head.as_ref().expect("not empty");
            // After this loop, 'leftZeros' & 'rightZeros' will contain the minimum
            // number of zeros on the left and right of the clip. This information
            // can be used to shrink the bounding box.
            for yoff in &head.y_offsets {
                let (l, r) = count_left_right_zeros(&head.data[yoff.offset as usize..], width);
                debug_assert!(l + r < width || (l == width && r == width));
                if l < left_zeros {
                    left_zeros = l;
                }
                if r < rite_zeros {
                    rite_zeros = r;
                }
                if 0 == (left_zeros | rite_zeros) {
                    // no trimming to do
                    return true;
                }
            }
        }

        debug_assert!(left_zeros != 0 || rite_zeros != 0);
        if width == left_zeros {
            debug_assert_eq!(width, rite_zeros);
            return self.set_empty();
        }

        self.validate();

        self.bounds.left += left_zeros;
        self.bounds.right -= rite_zeros;
        debug_assert!(!self.bounds.is_empty());

        // For now we don't realloc the storage (for time), we just shrink in place
        // This means we don't have to do any memmoves either, since we can just
        // play tricks with the yoff->fOffset for each row
        let head = Arc::get_mut(self.run_head.as_mut().expect("not empty"))
            .expect("the runs are unique while trimming");
        for yoff in &mut head.y_offsets {
            let row = &mut head.data[yoff.offset as usize..];
            debug_assert!(compute_row_length(row, width) > 0);
            yoff.offset += trim_row_left_right(row, width, left_zeros, rite_zeros);
            debug_assert!(
                compute_row_length(
                    &head.data[yoff.offset as usize..],
                    width - left_zeros - rite_zeros
                ) > 0
            );
        }
        true
    }

    // Port of: src/core/SkAAClip.cpp#L1137-L1211 (chrome/m156)
    fn trim_top_bottom(&mut self) -> bool {
        if self.is_empty() {
            return false;
        }

        self.validate();

        let width = self.bounds.width();
        let head = Arc::get_mut(self.run_head.as_mut().expect("not empty"))
            .expect("the runs are unique while trimming");

        //  Look to trim away empty rows from the top.
        //
        let row_count = head.y_offsets.len();
        let mut skip = 0usize;
        while skip < row_count {
            let data = &head.data[head.y_offsets[skip].offset as usize..];
            if !row_is_all_zeros(data, width) {
                break;
            }
            skip += 1;
        }
        debug_assert!(skip <= row_count);
        if skip == row_count {
            return self.set_empty();
        }
        if skip > 0 {
            // adjust fRowCount and fBounds.fTop, and slide all the data up
            // as we remove [skip] number of YOffset entries
            let dy = head.y_offsets[skip - 1].y + 1;
            for yoff in &mut head.y_offsets[skip..] {
                debug_assert!(yoff.y >= dy);
                yoff.y -= dy;
            }
            head.y_offsets.drain(..skip);

            self.bounds.top += dy;
            debug_assert!(!self.bounds.is_empty());
            debug_assert!(!head.y_offsets.is_empty());
        }

        //  Look to trim away empty rows from the bottom.
        //  We know that we have at least one non-zero row, so we can just walk
        //  backwards without checking for running past the start.
        //
        let len = head.y_offsets.len();
        let mut idx = len;
        loop {
            idx -= 1;
            if !row_is_all_zeros(&head.data[head.y_offsets[idx].offset as usize..], width) {
                break;
            }
        }
        let skip = len - idx - 1;
        debug_assert!(skip < len);
        if skip > 0 {
            // removing from the bottom is easier than from the top, as we don't
            // have to adjust any of the Y values, we just have to trim the array
            head.y_offsets.truncate(len - skip);

            self.bounds.bottom = self.bounds.top + head.y_offsets[idx].y + 1;
            debug_assert!(!self.bounds.is_empty());
            debug_assert!(!head.y_offsets.is_empty());
        }
        self.validate();

        true
    }

    // can't validate before we're done, since trimming is part of the process of
    // making us valid after the Builder. Since we build from top to bottom, its
    // possible our fBounds.fBottom is bigger than our last scanline of data, so
    // we trim fBounds.fBottom back up.
    //
    // TODO: check for duplicates in X and Y to further compress our data
    //
    // Port of: src/core/SkAAClip.cpp#L1237-L1255 (chrome/m156)
    fn trim_bounds(&mut self) -> bool {
        let Some(head) = &self.run_head else {
            return false;
        };

        debug_assert!(!head.y_offsets.is_empty());
        let last_y = head.y_offsets[head.y_offsets.len() - 1];
        debug_assert!(last_y.y < self.bounds.height());
        self.bounds.bottom = self.bounds.top + last_y.y + 1;
        debug_assert_eq!(last_y.y + 1, self.bounds.height());
        debug_assert!(!self.bounds.is_empty());

        self.trim_top_bottom() && self.trim_left_right()
    }
}

///////////////////////////////////////////////////////////////////////////////

// we don't read our initial n from data, since the caller may have had to
// clip it, hence the initialCount parameter.
// Port of: src/core/SkAAClip.cpp#L1640-L1667 (chrome/m156)
fn expand_to_runs(data: &[u8], initial_count: i32, width: i32, runs: &mut [i16], aa: &mut [Alpha]) {
    let mut n = initial_count;
    let mut width = width;
    let mut data = data;
    let mut r = 0usize;
    loop {
        if n > width {
            n = width;
        }
        debug_assert!(n > 0);
        runs[r] = i16::try_from(n).expect("SkToS16");
        aa[r] = data[1];
        r += usize::try_from(n).expect("positive");

        data = &data[2..];
        width -= n;
        if 0 == width {
            break;
        }
        // load the next count
        n = i32::from(data[0]);
    }
    runs[r] = 0; // sentinel
}

// Port of: src/core/SkAAClip.cpp#L1708-L1749 (chrome/m156)
fn merge(
    row: &[u8],
    row_n: i32,
    src_aa: &[Alpha],
    src_runs: &[i16],
    dst_aa: &mut [Alpha],
    dst_runs: &mut [i16],
    width: i32,
) {
    let mut accumulated = 0;
    let mut src_n = i32::from(src_runs[0]);
    // do we need this check?
    if 0 == src_n {
        return;
    }

    let mut row_n = row_n;
    let (mut sp, mut dp, mut rp) = (0usize, 0usize, 0usize);
    loop {
        debug_assert!(row_n > 0);
        debug_assert!(src_n > 0);

        let new_alpha = mul_div_255_round(u32::from(src_aa[sp]), u32::from(row[rp + 1]));
        let min_n = src_n.min(row_n);
        let step = usize::try_from(min_n).expect("positive");
        dst_runs[dp] = i16::try_from(min_n).expect("SkToS16");
        dst_aa[dp] = u8::try_from(new_alpha).expect("a product of two alphas");
        dp += step;

        src_n -= min_n;
        if 0 == src_n {
            src_n = i32::from(src_runs[sp]); // refresh
            sp += usize::try_from(src_n).expect("positive");
            src_n = i32::from(src_runs[sp]); // reload
            if 0 == src_n {
                break;
            }
        }
        row_n -= min_n;
        if 0 == row_n {
            rp += 2;
            row_n = i32::from(row[rp]); // reload
        }

        accumulated += min_n;
        debug_assert!(accumulated <= width);
    }
    dst_runs[dp] = 0;
}

// `SkMulDiv255Round(value, alpha)` as a byte.
// Port of: src/core/SkAAClip.cpp#L1782-L1784 (chrome/m156)
fn merge_one_u8(value: u8, alpha: u32) -> u8 {
    u8::try_from(mul_div_255_round(u32::from(value), alpha) & 0xFF).expect("masked")
}

// Port of: src/core/SkAAClip.cpp#L1786-L1794 (chrome/m156)
fn merge_one_u16(value: u16, alpha: u32) -> u16 {
    let value = u32::from(value);
    let r = get_packed_r16(value);
    let g = get_packed_g16(value);
    let b = get_packed_b16(value);
    pack_rgb16(
        mul_div_255_round(r, alpha),
        mul_div_255_round(g, alpha),
        mul_div_255_round(b, alpha),
    )
}

/// `mergeT<T>`: `src` and `dst` hold `T`s of `size` bytes each, in native byte order.
// Port of: src/core/SkAAClip.cpp#L1796-L1827 (chrome/m156)
fn merge_t(src: &[u8], src_n: i32, row: &[u8], row_n: i32, dst: &mut [u8], size: usize) {
    let mut src_n = src_n;
    let mut row_n = row_n;
    let mut s = 0usize;
    let mut d = 0usize;
    let mut rp = 0usize;
    loop {
        debug_assert!(row_n > 0);
        debug_assert!(src_n > 0);

        let n = usize::try_from(row_n.min(src_n)).expect("positive");
        let row_a = u32::from(row[rp + 1]);
        if 0xFF == row_a {
            dst[d..d + n * size].copy_from_slice(&src[s..s + n * size]);
        } else if 0 == row_a {
            dst[d..d + n * size].fill(0);
        } else {
            for i in 0..n {
                let at = i * size;
                if size == 1 {
                    dst[d + at] = merge_one_u8(src[s + at], row_a);
                } else {
                    let v = u16::from_ne_bytes([src[s + at], src[s + at + 1]]);
                    dst[d + at..d + at + 2].copy_from_slice(&merge_one_u16(v, row_a).to_ne_bytes());
                }
            }
        }

        src_n -= i32::try_from(n).expect("fits");
        if 0 == src_n {
            break;
        }

        s += n * size;
        d += n * size;

        debug_assert_eq!(row_n, i32::try_from(n).expect("fits"));
        rp += 2;
        row_n = i32::from(row[rp]);
    }
}

/// The bytes per pixel of the `mergeT` instantiation that handles `format` (`find_merge_aa_proc`).
// Port of: src/core/SkAAClip.cpp#L1829-L1845 (chrome/m156)
fn find_merge_aa_proc(format: MaskFormat) -> usize {
    match format {
        MaskFormat::A8 | MaskFormat::ThreeD => 1,
        MaskFormat::Lcd16 => 2,
        _ => unreachable!("unsupported"),
    }
}

// negation turns any non-zero into 0xFFFFFF??, so we just shift down
// some value >= 8 to get a full FF value
// Port of: src/core/SkAAClip.cpp#L1847-L1852 (chrome/m156)
fn bit2byte(bit_in_a_byte: i32) -> u8 {
    debug_assert!(bit_in_a_byte <= 0xFF);
    // `-bitInAByte >> 8` is an arithmetic shift; the byte store keeps the low 8 bits.
    u8::try_from((-bit_in_a_byte >> 8) & 0xFF).expect("masked")
}

// Port of: src/core/SkAAClip.cpp#L1854-L1892 (chrome/m156)
fn upscale_bw2a8(dst_mask: &mut MaskBuilder, src_mask: &Mask<'_>) {
    debug_assert_eq!(MaskFormat::BW, src_mask.format);
    debug_assert_eq!(MaskFormat::A8, dst_mask.format);

    let width = src_mask.bounds.width();
    let height = src_mask.bounds.height();

    let src = src_mask.image;
    let src_rb = src_mask.row_bytes as usize;
    let dst_rb = dst_mask.row_bytes as usize;

    let whole_bytes = usize::try_from(width >> 3).expect("positive");
    let left_over_bits = width & 7;

    let mut s = 0usize;
    let mut dst = 0usize;
    for _ in 0..height {
        let mut d = dst;
        for i in 0..whole_bytes {
            let src_byte = i32::from(src[s + i]);
            for bit in 0..8 {
                dst_mask.image[d + bit] = bit2byte(src_byte & (1 << (7 - bit)));
            }
            d += 8;
        }
        if left_over_bits != 0 {
            let mut src_byte = i32::from(src[s + whole_bytes]);
            for _ in 0..left_over_bits {
                dst_mask.image[d] = bit2byte(src_byte & 0x80);
                d += 1;
                src_byte <<= 1;
            }
        }
        s += src_rb;
        dst += dst_rb;
    }
}

/// Wraps a blitter so that everything it draws is modulated by the coverage of an [`AAClip`]
/// (`SkAAClipBlitter`).
// Port of: src/core/SkAAClip.h#L90-L124 (chrome/m156)
#[doc(alias = "SkAAClipBlitter")]
pub struct AAClipBlitter<'a> {
    blitter: &'a mut dyn Blitter,
    aa_clip: &'a AAClip,
    aa_clip_bounds: IRect,

    // fRuns and fAA: the runs and alphas of a scanline.
    runs: Vec<i16>,
    aa: Vec<Alpha>,
    // The scanline of a mask (up to 32 bits deep) that `blit_mask` builds.
    row_mask: Vec<u8>,
    memory: BlitMemory,
}

impl std::fmt::Debug for AAClipBlitter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AAClipBlitter")
            .field("aa_clip_bounds", &self.aa_clip_bounds)
            .finish_non_exhaustive()
    }
}

impl<'a> AAClipBlitter<'a> {
    /// Wraps `blitter`; `aaclip` must not be empty (`SkAAClipBlitter::init`).
    // Port of: src/core/SkAAClip.h#L96-L100 (chrome/m156)
    #[must_use]
    pub fn new(blitter: &'a mut dyn Blitter, aaclip: &'a AAClip) -> Self {
        // skia-rust: Skia asserts `!aaclip->isEmpty()`, but the hairline scan converters wrap
        // an empty AA raster clip (which draws nothing) without checking, in release builds.
        AAClipBlitter {
            blitter,
            aa_clip: aaclip,
            aa_clip_bounds: aaclip.bounds,
            runs: Vec::new(),
            aa: Vec::new(),
            row_mask: Vec::new(),
            memory: BlitMemory::default(),
        }
    }

    // Port of: src/core/SkAAClip.cpp#L1675-L1684 (chrome/m156)
    fn ensure_runs_and_aa(&mut self) {
        if self.runs.is_empty() {
            // add 1 so we can store the terminating run count of 0
            let count = usize::try_from(self.aa_clip_bounds.width()).expect("positive") + 1;
            // we use this either for fRuns + fAA, or a scaline of a mask
            // which may be as deep as 32bits
            self.runs = vec![0; count];
            self.aa = vec![0; count];
            self.row_mask = vec![0; count * 4];
        }
    }
}

impl Blitter for AAClipBlitter<'_> {
    // Port of: src/core/SkAAClip.cpp#L1686-L1706 (chrome/m156)
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        debug_assert!(width > 0);
        debug_assert!(
            self.aa_clip_bounds
                .contains(skia_rust_core::point::IPoint::new(x, y))
        );
        debug_assert!(
            self.aa_clip_bounds
                .contains(skia_rust_core::point::IPoint::new(x + width - 1, y))
        );

        let aa_clip = self.aa_clip;
        let (row, _) = aa_clip.find_row(y).expect("y is inside the clip");
        let (row, initial_count) = aa_clip.find_x(row, x);

        if initial_count >= width {
            let alpha = row[1];
            if 0 == alpha {
                return;
            }
            if 0xFF == alpha {
                self.blitter.blit_h(x, y, width);
                return;
            }
        }

        self.ensure_runs_and_aa();
        expand_to_runs(row, initial_count, width, &mut self.runs, &mut self.aa);

        self.blitter.blit_anti_h(x, y, &mut self.aa, &mut self.runs);
    }

    // Port of: src/core/SkAAClip.cpp#L1751-L1762 (chrome/m156)
    fn blit_anti_h(&mut self, x: i32, y: i32, aa: &mut [Alpha], runs: &mut [i16]) {
        let aa_clip = self.aa_clip;
        let (row, _) = aa_clip.find_row(y).expect("y is inside the clip");
        let (row, initial_count) = aa_clip.find_x(row, x);

        self.ensure_runs_and_aa();

        merge(
            row,
            initial_count,
            aa,
            runs,
            &mut self.aa,
            &mut self.runs,
            self.aa_clip_bounds.width(),
        );
        self.blitter.blit_anti_h(x, y, &mut self.aa, &mut self.runs);
    }

    // Port of: src/core/SkAAClip.cpp#L1764-L1791 (chrome/m156)
    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: Alpha) {
        let aa_clip = self.aa_clip;
        if aa_clip.quick_contains_ltrb(x, y, x + 1, y + height) {
            self.blitter.blit_v(x, y, height, alpha);
            return;
        }

        let mut y = y;
        let mut height = height;
        loop {
            let (row, last_y) = aa_clip.find_row(y).expect("y is inside the clip");
            let mut dy = last_y - y + 1;
            if dy > height {
                dy = height;
            }
            height -= dy;

            let (row, _) = aa_clip.find_x(row, x);
            let new_alpha = mul_div_255_round(u32::from(alpha), u32::from(row[1]));
            if new_alpha != 0 {
                self.blitter
                    .blit_v(x, y, dy, u8::try_from(new_alpha & 0xFF).expect("masked"));
            }
            debug_assert!(height >= 0);
            if height <= 0 {
                break;
            }
            y = last_y + 1;
        }
    }

    // Port of: src/core/SkAAClip.cpp#L1793-L1804 (chrome/m156)
    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        if self
            .aa_clip
            .quick_contains_ltrb(x, y, x + width, y + height)
        {
            self.blitter.blit_rect(x, y, width, height);
            return;
        }

        let mut y = y;
        let mut height = height;
        loop {
            height -= 1;
            if height < 0 {
                break;
            }
            self.blit_h(x, y, width);
            y += 1;
        }
    }

    // Port of: src/core/SkAAClip.cpp#L1894-L1966 (chrome/m156)
    fn blit_mask(&mut self, orig_mask: &Mask<'_>, clip: &IRect) {
        let aa_clip = self.aa_clip;
        debug_assert!(aa_clip.bounds.contains(clip));

        if aa_clip.quick_contains(clip) {
            self.blitter.blit_mask(orig_mask, clip);
            return;
        }

        // if we're BW, we need to upscale to A8 (ugh)
        let mut gray_mask = MaskBuilder::default();
        let gray_view;
        let mask: &Mask<'_> = if MaskFormat::BW == orig_mask.format {
            gray_mask.format = MaskFormat::A8;
            gray_mask.bounds = orig_mask.bounds;
            gray_mask.row_bytes = u32::try_from(orig_mask.bounds.width()).expect("positive");
            let size = gray_mask.compute_image_size();
            gray_mask.image = vec![0; size];

            upscale_bw2a8(&mut gray_mask, orig_mask);
            gray_view = gray_mask.as_mask();
            &gray_view
        } else {
            orig_mask
        };

        self.ensure_runs_and_aa();

        // HACK -- we are devolving 3D into A8, need to copy the rest of the 3D
        // data into a temp block to support it better (ugh)

        let src = mask.get_addr(clip.left, clip.top);
        let src_rb = mask.row_bytes as usize;
        let width = clip.width();
        let size = find_merge_aa_proc(mask.format);

        let row_format = if MaskFormat::ThreeD == mask.format {
            MaskFormat::A8
        } else {
            mask.format
        };
        let mut row_bounds = IRect::new(clip.left, 0, clip.right, 0);

        let mut y = clip.top;
        let stop_y = y + clip.height();
        let mut src_off = 0usize;

        loop {
            let (row, last_y) = aa_clip.find_row(y).expect("y is inside the clip");
            // findRow returns last Y, not stop, so we add 1
            let local_stop_y = (last_y + 1).min(stop_y);

            let (row, initial_count) = aa_clip.find_x(row, clip.left);
            loop {
                merge_t(
                    &src[src_off..],
                    width,
                    row,
                    initial_count,
                    &mut self.row_mask,
                    size,
                );
                row_bounds.top = y;
                row_bounds.bottom = y + 1;
                let row_mask = Mask::new(&self.row_mask, row_bounds, mask.row_bytes, row_format);
                self.blitter.blit_mask(&row_mask, &row_bounds);
                src_off += src_rb;
                y += 1;
                if y >= local_stop_y {
                    break;
                }
            }
            if y >= stop_y {
                break;
            }
        }
    }

    fn blit_memory(&mut self) -> &mut BlitMemory {
        &mut self.memory
    }
}
