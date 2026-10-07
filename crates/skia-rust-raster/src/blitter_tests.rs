// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

// Tests for the blitter base. There is no upstream unit test for these types (the only Skia tests
// that touch them, `BlitMaskClip`/`CoreBlittersTest`, need the real blitters of task D4), so the
// expectations are derived by hand from the C++ default implementations in
// src/core/SkBlitter.cpp (chrome/m156); each test says how.

use skia_rust_core::color::Alpha;
use skia_rust_core::mask::{Mask, MaskFormat};
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::{Op, Region};

use crate::blitter::{
    BlitMemory, Blitter, BlitterClipper, NullBlitter, RectClipBlitter, RgnClipBlitter,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    H(i32, i32, i32),
    // (x, y, [(run length, alpha)])
    AntiH(i32, i32, Vec<(i32, u8)>),
    V(i32, i32, i32, u8),
    Rect(i32, i32, i32, i32),
    AntiRect(i32, i32, i32, i32, u8, u8),
    Mask(IRect),
}

// Decodes the sparse run encoding into (run length, alpha) pairs.
fn decode(aa: &[Alpha], runs: &[i16]) -> Vec<(i32, u8)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while runs[i] != 0 {
        out.push((i32::from(runs[i]), aa[i]));
        i += usize::try_from(runs[i]).unwrap();
    }
    out
}

// Implements only the two required methods: everything else is the Skia default.
#[derive(Default)]
struct Basic {
    calls: Vec<Call>,
    mem: BlitMemory,
}

impl Blitter for Basic {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        self.calls.push(Call::H(x, y, width));
    }
    fn blit_anti_h(&mut self, x: i32, y: i32, aa: &mut [Alpha], runs: &mut [i16]) {
        self.calls.push(Call::AntiH(x, y, decode(aa, runs)));
    }
    fn blit_memory(&mut self) -> &mut BlitMemory {
        &mut self.mem
    }
}

// Overrides every virtual method, so clip blitters' output is observed unmodified.
#[derive(Default)]
struct Full {
    calls: Vec<Call>,
    mem: BlitMemory,
    rows: i32,
}

impl Blitter for Full {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        self.calls.push(Call::H(x, y, width));
    }
    fn blit_anti_h(&mut self, x: i32, y: i32, aa: &mut [Alpha], runs: &mut [i16]) {
        self.calls.push(Call::AntiH(x, y, decode(aa, runs)));
    }
    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: Alpha) {
        self.calls.push(Call::V(x, y, height, alpha));
    }
    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.calls.push(Call::Rect(x, y, width, height));
    }
    fn blit_anti_rect(&mut self, x: i32, y: i32, w: i32, h: i32, la: Alpha, ra: Alpha) {
        self.calls.push(Call::AntiRect(x, y, w, h, la, ra));
    }
    fn blit_mask(&mut self, _mask: &Mask<'_>, clip: &IRect) {
        self.calls.push(Call::Mask(*clip));
    }
    fn request_rows_preserved(&self) -> i32 {
        self.rows
    }
    fn blit_memory(&mut self) -> &mut BlitMemory {
        &mut self.mem
    }
}

#[test]
fn default_blit_v_opaque_is_blit_rect() {
    // alpha == 255: blitRect(x, y, 1, height) -> `height` blitH calls of width 1.
    let mut b = Basic::default();
    b.blit_v(3, 7, 3, 255);
    assert_eq!(
        b.calls,
        [Call::H(3, 7, 1), Call::H(3, 8, 1), Call::H(3, 9, 1)]
    );
}

#[test]
fn default_blit_v_translucent_is_one_anti_h_per_row() {
    let mut b = Basic::default();
    b.blit_v(3, 7, 3, 0x80);
    assert_eq!(
        b.calls,
        [
            Call::AntiH(3, 7, vec![(1, 0x80)]),
            Call::AntiH(3, 8, vec![(1, 0x80)]),
            Call::AntiH(3, 9, vec![(1, 0x80)]),
        ]
    );
    // height 0: nothing.
    let mut b = Basic::default();
    b.blit_v(3, 7, 0, 0x80);
    assert_eq!(b.calls, []);
}

#[test]
fn default_blit_rect() {
    let mut b = Basic::default();
    b.blit_rect(1, 2, 5, 2);
    assert_eq!(b.calls, [Call::H(1, 2, 5), Call::H(1, 3, 5)]);
    let mut b = Basic::default();
    b.blit_rect(1, 2, 5, 0);
    assert_eq!(b.calls, []);
}

#[test]
fn default_blit_anti_rect() {
    // Left column at x=5 (alpha 100), 3 opaque columns at x=6..9, right column at x=9 (alpha 50).
    let mut b = Basic::default();
    b.blit_anti_rect(5, 10, 3, 2, 100, 50);
    assert_eq!(
        b.calls,
        [
            Call::AntiH(5, 10, vec![(1, 100)]),
            Call::AntiH(5, 11, vec![(1, 100)]),
            Call::H(6, 10, 3),
            Call::H(6, 11, 3),
            Call::AntiH(9, 10, vec![(1, 50)]),
            Call::AntiH(9, 11, vec![(1, 50)]),
        ]
    );

    // leftAlpha == 0 skips the left column; width == 0 skips the middle; x still advances by 1.
    let mut b = Basic::default();
    b.blit_anti_rect(-1, 0, 0, 1, 0, 255);
    assert_eq!(b.calls, [Call::H(0, 0, 1)]);
}

#[test]
fn default_blit_anti_h2_and_v2() {
    let mut b = Basic::default();
    b.blit_anti_h2(4, 5, 0x10, 0x20);
    assert_eq!(b.calls, [Call::AntiH(4, 5, vec![(1, 0x10), (1, 0x20)])]);

    let mut b = Basic::default();
    b.blit_anti_v2(4, 5, 0x10, 0x20);
    assert_eq!(
        b.calls,
        [
            Call::AntiH(4, 5, vec![(1, 0x10)]),
            Call::AntiH(4, 6, vec![(1, 0x20)])
        ]
    );
}

#[test]
fn request_rows_preserved_and_memory() {
    let mut b = Basic::default();
    assert_eq!(b.request_rows_preserved(), 1);
    assert_eq!(b.alloc_blit_memory(16).len(), 16);
    b.alloc_blit_memory(4)[0] = 7;
    assert_eq!(b.alloc_blit_memory(4).len(), 4);
    assert_eq!(b.alloc_blit_memory(0).len(), 0);

    // The wrappers forward both to the wrapped blitter.
    let mut full = Full {
        rows: 4,
        ..Full::default()
    };
    let mut clip = RectClipBlitter::new(&mut full, IRect::new(0, 0, 4, 4));
    assert_eq!(clip.request_rows_preserved(), 4);
    assert_eq!(clip.alloc_blit_memory(8).len(), 8);
}

#[test]
fn null_blitter_does_nothing() {
    let mut n = NullBlitter::new();
    n.blit_h(0, 0, 5);
    n.blit_v(0, 0, 5, 3);
    n.blit_rect(0, 0, 5, 5);
    n.blit_anti_rect(0, 0, 5, 5, 1, 2);
    let mut aa = [1u8];
    let mut runs = [1i16, 0];
    n.blit_anti_h(0, 0, &mut aa, &mut runs);
    n.blit_anti_h2(0, 0, 1, 2);
    n.blit_anti_v2(0, 0, 1, 2);
}

fn a8_mask(image: &[u8]) -> Mask<'_> {
    Mask::new(image, IRect::new(10, 20, 14, 22), 4, MaskFormat::A8)
}

#[test]
fn default_blit_mask_a8() {
    let image = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let mask = a8_mask(&image);

    // Full clip: one blitAntiH per row with every run of length 1.
    let mut b = Basic::default();
    b.blit_mask(&mask, &IRect::new(10, 20, 14, 22));
    assert_eq!(
        b.calls,
        [
            Call::AntiH(10, 20, vec![(1, 1), (1, 2), (1, 3), (1, 4)]),
            Call::AntiH(10, 21, vec![(1, 5), (1, 6), (1, 7), (1, 8)]),
        ]
    );

    // Sub-rect clip: starts at getAddr8(11, 21) = image[5].
    let mut b = Basic::default();
    b.blit_mask(&mask, &IRect::new(11, 21, 13, 22));
    assert_eq!(b.calls, [Call::AntiH(11, 21, vec![(1, 6), (1, 7)])]);
}

#[test]
fn default_blit_mask_lcd16_is_left_to_subclass() {
    let image = [0u8; 16];
    let mask = Mask::new(&image, IRect::new(0, 0, 4, 2), 8, MaskFormat::Lcd16);
    let mut b = Basic::default();
    b.blit_mask(&mask, &IRect::new(0, 0, 4, 2));
    assert_eq!(b.calls, []);
}

#[test]
fn default_blit_mask_bw() {
    // 10x2 mask, two bytes per row. Row 0 = 1100_1111 11xx_xxxx, row 1 = 0000_0000 01xx_xxxx
    // (the low 6 bits of the second byte are padding and must be ignored).
    let image = [0b1100_1111u8, 0b1111_1111, 0b0000_0000, 0b0111_1111];
    let mask = Mask::new(&image, IRect::new(0, 0, 10, 2), 2, MaskFormat::BW);

    // Whole width: rightMask = (0xFF00 >> 2) & 0xFF = 0xC0 keeps x = 8, 9 of the second byte.
    // Row 0: x = 0, 1 set; 2, 3 clear; 4..9 set. Row 1: x = 9 only.
    let mut b = Basic::default();
    b.blit_mask(&mask, &IRect::new(0, 0, 10, 2));
    assert_eq!(
        b.calls,
        [Call::H(0, 0, 2), Call::H(4, 0, 6), Call::H(9, 1, 1)]
    );

    // Clip (3, 0, 9, 1): bitsLeft = 0, leftMask = 0xFF >> 3 = 0x1F, affectedRightBit = 8,
    // rightMask = (0xFF00 >> 1) & 0xFF = 0x80, 2 bytes. Row 0 byte 0 & 0x1F = 0b0000_1111 -> x 4..8
    // and byte 1 & 0x80 -> x 8: one run 4..9.
    let mut b = Basic::default();
    b.blit_mask(&mask, &IRect::new(3, 0, 9, 1));
    assert_eq!(b.calls, [Call::H(4, 0, 5)]);

    // A clip that ends on a byte boundary and starts inside the first byte of row 1.
    // bitsLeft = 2 - 2 = 0; leftMask = 0xFF >> 2 = 0x3F; affectedRightBit = 7; rightMask = 0xFF;
    // rowBytes = 1. Row 1 byte 0 is 0 -> nothing.
    let mut b = Basic::default();
    b.blit_mask(&mask, &IRect::new(2, 1, 8, 2));
    assert_eq!(b.calls, []);
}

#[test]
fn blit_fat_anti_rect() {
    // rect (0.5, 0.5, 4.5, 4.5) rounds out to (0, 0, 5, 5). partialL = partialR = partialT =
    // partialB = 0.5. scalar_to_alpha(0.25) = (u8)63.75 = 63, scalar_to_alpha(0.5) = (u8)127.5 =
    // 127. Both are in [8, 247], so they pass through unchanged.
    let mut b = Full::default();
    b.blit_fat_anti_rect(&Rect::new(0.5, 0.5, 4.5, 4.5));
    assert_eq!(
        b.calls,
        [
            Call::AntiH(0, 0, vec![(1, 63), (3, 127), (1, 63)]),
            Call::AntiRect(0, 1, 3, 3, 127, 127),
            Call::AntiH(0, 4, vec![(1, 63), (3, 127), (1, 63)]),
        ]
    );
}

#[test]
fn blit_fat_anti_rect_alpha_thresholds_and_single_row() {
    // rect (0.1, 0.0, 3.0, 0.5) -> bounds (0, 0, 3, 1): height 1, so partialT = bottom - top =
    // 0.5. partialL = 0 + 1 - 0.1 = 0.9, partialR = 3.0 - (3 - 1) = 1.0.
    // alphas[0] = (u8)(0.9 * 0.5 * 255 = 114.75) = 114; alphas[1] = (u8)127.5 = 127;
    // alphas[2] = (u8)(1.0 * 0.5 * 255) = 127. Only the top row is blitted.
    let mut b = Full::default();
    b.blit_fat_anti_rect(&Rect::new(0.1, 0.0, 3.0, 0.5));
    assert_eq!(
        b.calls,
        [Call::AntiH(0, 0, vec![(1, 114), (1, 127), (1, 127)])]
    );

    // Alphas above 247 snap to 0xFF and below 8 snap to 0: rect (0.99, 0, 3.0, 1.0) has
    // partialT = 1 (>247 -> 255) and partialL = 0.01 -> 2 -> 0.
    let mut b = Full::default();
    b.blit_fat_anti_rect(&Rect::new(0.99, 0.0, 3.0, 1.0));
    assert_eq!(
        b.calls,
        [Call::AntiH(0, 0, vec![(1, 0), (1, 255), (1, 255)])]
    );
}

fn two_rect_region() -> Region {
    // (0,0,4,2) U (6,0,10,2)
    let mut r = Region::from_rect(IRect::new(0, 0, 4, 2));
    assert!(r.op_rect(IRect::new(6, 0, 10, 2), Op::Union));
    r
}

#[test]
fn region_helpers() {
    let r = two_rect_region();

    let mut b = Full::default();
    b.blit_region(&r);
    // SkRegionPriv::VisitSpans visits every scanline separately (each span has height 1), in
    // Y -> X order.
    assert_eq!(
        b.calls,
        [
            Call::Rect(0, 0, 4, 1),
            Call::Rect(6, 0, 4, 1),
            Call::Rect(0, 1, 4, 1),
            Call::Rect(6, 1, 4, 1)
        ]
    );

    let mut b = Full::default();
    b.blit_rect_region(&IRect::new(2, 1, 8, 5), &r);
    assert_eq!(b.calls, [Call::Rect(2, 1, 2, 1), Call::Rect(6, 1, 2, 1)]);

    let image = [0u8; 40];
    let mask = Mask::new(&image, IRect::new(0, 0, 10, 4), 10, MaskFormat::A8);
    let mut b = Full::default();
    b.blit_mask_region(&mask, &r);
    assert_eq!(
        b.calls,
        [
            Call::Mask(IRect::new(0, 0, 4, 2)),
            Call::Mask(IRect::new(6, 0, 10, 2))
        ]
    );

    // quickReject: a mask outside the region's bounds is not blitted at all.
    let far = Mask::new(&image, IRect::new(20, 20, 24, 21), 4, MaskFormat::A8);
    let mut b = Full::default();
    b.blit_mask_region(&far, &r);
    assert_eq!(b.calls, []);
}

#[test]
fn rect_clip_blit_h_v_rect() {
    let mut full = Full::default();
    {
        let mut c = RectClipBlitter::new(&mut full, IRect::new(2, 3, 8, 6));
        c.blit_h(0, 3, 4); // x clipped to [2, 4)
        c.blit_h(0, 2, 4); // row above the clip
        c.blit_h(7, 5, 10); // right edge: [7, 8)
        c.blit_h(8, 5, 2); // entirely right of the clip
        c.blit_v(3, 1, 4, 9); // y clipped to [3, 5)
        c.blit_v(8, 3, 2, 9); // column outside
        c.blit_rect(0, 0, 4, 4); // intersects: (2,3)-(4,4)
        c.blit_rect(10, 10, 4, 4); // misses
    }
    assert_eq!(
        full.calls,
        [
            Call::H(2, 3, 2),
            Call::H(7, 5, 1),
            Call::V(3, 3, 2, 9),
            Call::Rect(2, 3, 2, 1),
        ]
    );
}

#[test]
fn rect_clip_blit_anti_h() {
    // Clip x range [2, 6). Width 8 at x = 0: runs 3@0 (alpha 10), 2@3 (20), 3@5 (30).
    let mut aa = [0u8; 9];
    let mut runs = [0i16; 9];
    runs[0] = 3;
    aa[0] = 10;
    runs[3] = 2;
    aa[3] = 20;
    runs[5] = 3;
    aa[5] = 30;
    runs[8] = 0;

    let mut full = Full::default();
    {
        let mut c = RectClipBlitter::new(&mut full, IRect::new(2, 0, 6, 10));
        c.blit_anti_h(0, 4, &mut aa, &mut runs);
        // y outside, and x entirely outside either side: nothing.
        c.blit_anti_h(0, 10, &mut aa, &mut runs);
    }
    // Derivation in the C++ code: BreakAt(dx = 2) trims the first run to (1, 10); the right edge
    // breaks at x1 - x0 = 4: [1@10][2@20][1@30] and a zero terminator.
    assert_eq!(
        full.calls,
        [Call::AntiH(2, 4, vec![(1, 10), (2, 20), (1, 30)])]
    );

    // Entirely left / right of the clip.
    let mut aa = [7u8, 0, 0];
    let mut runs = [2i16, 0, 0];
    let mut full = Full::default();
    {
        let mut c = RectClipBlitter::new(&mut full, IRect::new(2, 0, 6, 10));
        c.blit_anti_h(0, 0, &mut aa, &mut runs); // x1 = 2 <= clip.left
        c.blit_anti_h(6, 0, &mut aa, &mut runs); // left >= clip.right
    }
    assert_eq!(full.calls, []);
}

#[test]
fn rect_clip_blit_anti_rect() {
    // Anti rect at x = 2: left column x=2, middle x = 3..7, right column x = 7; true width 6.
    let clip = IRect::new(0, 0, 100, 3);
    let mut full = Full::default();
    {
        let mut c = RectClipBlitter::new(&mut full, clip);
        // Fully inside horizontally, height clipped from 5 to 3: forwarded as an anti rect.
        c.blit_anti_rect(2, 0, 4, 5, 10, 20);
    }
    assert_eq!(full.calls, [Call::AntiRect(2, 0, 4, 3, 10, 20)]);

    // Clip cuts off the left column (r.left != left): leftAlpha becomes 255; the right alpha is
    // kept. Result is 5 wide so it stays an anti rect.
    let mut full = Full::default();
    {
        let mut c = RectClipBlitter::new(&mut full, IRect::new(3, 0, 100, 3));
        c.blit_anti_rect(2, 0, 4, 3, 10, 20);
    }
    assert_eq!(full.calls, [Call::AntiRect(3, 0, 3, 3, 255, 20)]);

    // Both columns cut off: both opaque -> blitRect.
    let mut full = Full::default();
    {
        let mut c = RectClipBlitter::new(&mut full, IRect::new(3, 0, 6, 3));
        c.blit_anti_rect(2, 0, 4, 3, 10, 20);
    }
    assert_eq!(full.calls, [Call::Rect(3, 0, 3, 3)]);

    // Only the right column (x = 7) remains (width 1): blitV with the right alpha.
    let mut full = Full::default();
    {
        let mut c = RectClipBlitter::new(&mut full, IRect::new(7, 0, 8, 3));
        c.blit_anti_rect(2, 0, 4, 3, 10, 20);
    }
    assert_eq!(full.calls, [Call::V(7, 0, 3, 20)]);

    // Only the left column remains.
    let mut full = Full::default();
    {
        let mut c = RectClipBlitter::new(&mut full, IRect::new(0, 0, 3, 3));
        c.blit_anti_rect(2, 0, 4, 3, 10, 20);
    }
    // r = (2,0,3,3): right edge cut -> rightAlpha 255; width 1 -> blitV(left alpha).
    assert_eq!(full.calls, [Call::V(2, 0, 3, 10)]);

    // Missing the clip entirely.
    let mut full = Full::default();
    {
        let mut c = RectClipBlitter::new(&mut full, IRect::new(50, 50, 60, 60));
        c.blit_anti_rect(2, 0, 4, 3, 10, 20);
    }
    assert_eq!(full.calls, []);
}

#[test]
fn rect_clip_blit_mask() {
    let image = [0u8; 40];
    let mask = Mask::new(&image, IRect::new(0, 0, 10, 4), 10, MaskFormat::A8);
    let mut full = Full::default();
    {
        let mut c = RectClipBlitter::new(&mut full, IRect::new(2, 1, 6, 3));
        c.blit_mask(&mask, &IRect::new(0, 0, 10, 4));
        c.blit_mask(&mask, &IRect::new(8, 0, 10, 4)); // outside
    }
    assert_eq!(full.calls, [Call::Mask(IRect::new(2, 1, 6, 3))]);
}

#[test]
fn rgn_clip_blit_h_v_rect() {
    let r = two_rect_region();
    let mut full = Full::default();
    {
        let mut c = RgnClipBlitter::new(&mut full, &r);
        c.blit_h(0, 0, 10); // two spans
        c.blit_h(2, 1, 6); // [2, 4) and [6, 8)
        c.blit_h(0, 2, 10); // below the region
        c.blit_v(7, -3, 10, 99); // x = 7 is in the right rect: rows [0, 2)
        c.blit_v(5, 0, 2, 99); // x = 5 is in the gap
        c.blit_rect(3, 0, 5, 5); // [3, 4) and [6, 8) x [0, 2)
    }
    assert_eq!(
        full.calls,
        [
            Call::H(0, 0, 4),
            Call::H(6, 0, 4),
            Call::H(2, 1, 2),
            Call::H(6, 1, 2),
            Call::V(7, 0, 2, 99),
            Call::Rect(3, 0, 1, 2),
            Call::Rect(6, 0, 2, 2),
        ]
    );
}

#[test]
fn rgn_clip_blit_anti_h() {
    // Width 10 all alpha 0x80; the region keeps [0, 4) and [6, 10). The C++ trace: Break at
    // (0, 4) then (6, 4), then zero the gap: runs 4@0x80, 2@0, 4@0x80.
    let r = two_rect_region();
    let mut aa = [0u8; 11];
    let mut runs = [0i16; 11];
    runs[0] = 10;
    aa[0] = 0x80;
    runs[10] = 0;
    let mut full = Full::default();
    {
        let mut c = RgnClipBlitter::new(&mut full, &r);
        c.blit_anti_h(0, 0, &mut aa, &mut runs);
        // A row outside the region produces no spans at all.
        let mut aa = [0u8; 11];
        let mut runs = [0i16; 11];
        runs[0] = 10;
        aa[0] = 0x80;
        c.blit_anti_h(0, 5, &mut aa, &mut runs);
    }
    assert_eq!(
        full.calls,
        [Call::AntiH(0, 0, vec![(4, 0x80), (2, 0), (4, 0x80)])]
    );
}

#[test]
fn rgn_clip_blit_anti_h_negative_x() {
    // Starting at x = -2, width 6 (alpha 0x40): pixels -2..4. The region's left rect (0, 4)
    // contains [0, 4): Break(left - x = 2, count 4); no gap. prevRite = 4 > x so runs[6] = 0;
    // x < 0, so skip = runs[0] = 2: the blitter is called at x = 0 with the remaining 4.
    let r = two_rect_region();
    let mut aa = [0u8; 7];
    let mut runs = [0i16; 7];
    runs[0] = 6;
    aa[0] = 0x40;
    let mut full = Full::default();
    {
        let mut c = RgnClipBlitter::new(&mut full, &r);
        c.blit_anti_h(-2, 0, &mut aa, &mut runs);
    }
    assert_eq!(full.calls, [Call::AntiH(0, 0, vec![(4, 0x40)])]);
}

#[test]
fn rgn_clip_blit_anti_rect_and_mask() {
    let r = two_rect_region();
    let mut full = Full::default();
    {
        let mut c = RgnClipBlitter::new(&mut full, &r);
        // True width 6 at x=1: [1, 7). Intersection pieces: [1, 4) and [6, 7), rows [0, 2).
        // First piece: left == x -> leftAlpha 10; right edge 4 != 7 -> 255; width 3 -> anti rect
        // (1, 0, 1, 2, 10, 255). Second: left 6 != x -> 255; right == x + width + 2 = 7 -> 20;
        // width 1 -> blitV(6, 0, 2, 20).
        c.blit_anti_rect(1, 0, 4, 2, 10, 20);
    }
    assert_eq!(
        full.calls,
        [Call::AntiRect(1, 0, 1, 2, 10, 255), Call::V(6, 0, 2, 20)]
    );

    let image = [0u8; 40];
    let mask = Mask::new(&image, IRect::new(0, 0, 10, 4), 10, MaskFormat::A8);
    let mut full = Full::default();
    {
        let mut c = RgnClipBlitter::new(&mut full, &r);
        c.blit_mask(&mask, &IRect::new(2, 0, 8, 4));
    }
    assert_eq!(
        full.calls,
        [
            Call::Mask(IRect::new(2, 0, 4, 2)),
            Call::Mask(IRect::new(6, 0, 8, 2))
        ]
    );
}

#[test]
fn blitter_clipper_selection() {
    let ir = IRect::new(2, 2, 6, 6);

    // No clip: the original blitter.
    let mut full = Full::default();
    {
        let mut clipper = BlitterClipper::new();
        let b = clipper.apply(&mut full, None, Some(&ir));
        b.blit_rect(0, 0, 100, 1);
    }
    assert_eq!(full.calls, [Call::Rect(0, 0, 100, 1)]);

    // Empty clip, or clip that misses `ir`: null blitter.
    let empty = Region::new();
    let mut full = Full::default();
    {
        let mut clipper = BlitterClipper::new();
        let b = clipper.apply(&mut full, Some(&empty), None);
        b.blit_rect(0, 0, 100, 1);
    }
    assert_eq!(full.calls, []);

    let rect_clip = Region::from_rect(IRect::new(10, 10, 20, 20));
    let mut full = Full::default();
    {
        let mut clipper = BlitterClipper::new();
        let b = clipper.apply(&mut full, Some(&rect_clip), Some(&ir));
        b.blit_rect(0, 0, 100, 100);
    }
    assert_eq!(full.calls, []);

    // Rect clip containing `ir`: the original blitter (no clipping wrapper).
    let big = Region::from_rect(IRect::new(0, 0, 8, 8));
    let mut full = Full::default();
    {
        let mut clipper = BlitterClipper::new();
        let b = clipper.apply(&mut full, Some(&big), Some(&ir));
        b.blit_rect(0, 0, 100, 100);
    }
    assert_eq!(full.calls, [Call::Rect(0, 0, 100, 100)]);

    // Rect clip not containing `ir` (or with no `ir`): a RectClipBlitter.
    let small = Region::from_rect(IRect::new(3, 3, 5, 5));
    let mut full = Full::default();
    {
        let mut clipper = BlitterClipper::new();
        let b = clipper.apply(&mut full, Some(&small), Some(&ir));
        b.blit_rect(0, 0, 100, 100);
    }
    assert_eq!(full.calls, [Call::Rect(3, 3, 2, 2)]);
    let mut full = Full::default();
    {
        let mut clipper = BlitterClipper::new();
        let b = clipper.apply(&mut full, Some(&small), None);
        b.blit_rect(0, 0, 100, 100);
    }
    assert_eq!(full.calls, [Call::Rect(3, 3, 2, 2)]);

    // Complex clip: a RgnClipBlitter.
    let complex = two_rect_region();
    let mut full = Full::default();
    {
        let mut clipper = BlitterClipper::new();
        let b = clipper.apply(&mut full, Some(&complex), None);
        b.blit_h(0, 0, 10);
    }
    assert_eq!(full.calls, [Call::H(0, 0, 4), Call::H(6, 0, 4)]);
}
