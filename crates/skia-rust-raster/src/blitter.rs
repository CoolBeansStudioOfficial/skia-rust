// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkBlitter.h, src/core/SkBlitter.cpp

//! The blitter interface (`SkBlitter`) and its clipping wrappers.
//!
//! [`Blitter`] and its implementors are responsible for actually writing pixels into memory.
//! Besides efficiency, they handle clipping and antialiasing. A blitter contains all the context
//! needed to generate pixels for the destination and how src/generated pixels map to the
//! destination. The coordinates passed to the `blit_*` calls are in destination pixel space.
//!
//! Not ported here (they belong to the legacy-blitter task): `SkBlitter::Choose`,
//! `ChooseSprite`, `UseLegacyBlitter` and `gSkForceRasterPipelineBlitter`. The debug-only
//! `SkRectClipCheckBlitter` is not ported either. `canDirectBlit` is [`Blitter::can_direct_blit`]
//! (implemented by the raster pipeline blitter, task D3).

use skia_rust_core::color::Alpha;
use skia_rust_core::mask::{Mask, MaskFormat};
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::{IRect, Rect, RoundOut};
use skia_rust_core::region::{Cliperator, Region, Spanerator, region_priv};

use crate::alpha_runs::AlphaRuns;

/// Memory owned by a blitter and handed out by [`Blitter::alloc_blit_memory`]: the safe stand-in
/// for `SkBlitter::fBlitMemory` (an `SkAutoMalloc`).
// Port of: src/core/SkBlitter.h#L162-L163 (chrome/m156)
#[derive(Clone, Debug, Default)]
pub struct BlitMemory {
    buf: Vec<u8>,
}

impl BlitMemory {
    /// Returns `sz` bytes of scratch memory, reusing the existing allocation when it is large
    /// enough (`SkAutoMalloc::kReuse_OnShrink`). The contents are unspecified (here: whatever the
    /// previous use left, or zero).
    pub fn reset(&mut self, sz: usize) -> &mut [u8] {
        if sz > self.buf.len() {
            self.buf = vec![0; sz];
        }
        &mut self.buf[..sz]
    }
}

// `SkToU8` (checked only in debug builds in Skia).
#[allow(clippy::cast_possible_truncation)] // mirrors SkToU8, which is only asserted
fn to_u8(v: u32) -> u8 {
    debug_assert!(v <= 0xFF);
    v as u8
}

// `SkToS16`.
#[allow(clippy::cast_possible_truncation)] // mirrors SkToS16, which is only asserted
fn to_s16(v: i32) -> i16 {
    debug_assert!(i16::try_from(v).is_ok());
    v as i16
}

// Port of: src/core/SkBlitter.cpp#L49-L52 (chrome/m156)
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors `(SkAlpha)(a * 255)`
fn scalar_to_alpha(a: f32) -> Alpha {
    let alpha = (a * 255.0) as Alpha;
    if alpha > 247 {
        0xFF
    } else if alpha < 8 {
        0
    } else {
        alpha
    }
}

/// What [`Blitter::can_direct_blit`] returns: a solid fill the caller may write straight into the
/// destination pixels.
///
/// `SkBlitter::DirectBlit` holds an `SkPixmap` copy (a shared view of the destination's pixels);
/// here the pixmap is a reborrow of the blitter's own, so it lives as long as the borrow of the
/// blitter.
// Port of: src/core/SkBlitter.h#L125-L128 (chrome/m156)
#[doc(alias = "SkBlitter::DirectBlit")]
#[derive(Debug)]
pub struct DirectBlit<'p> {
    /// `pm`: the destination.
    pub pm: Pixmap<'p>,
    /// `value`: the pixel to write; the low bits match the pixmap's bit depth.
    pub value: u64,
}

/// `SkBlitter`: writes pixels into memory. See the module documentation.
///
/// `blit_h` and `blit_anti_h` are the only methods without a default.
///
/// Unlike C++, `blit_anti_h` takes mutable slices: the clip blitters rewrite the run arrays in
/// place (`SkAlphaRuns::Break`) behind `const_cast`s in Skia, and `blitAntiV2` relies on that.
// Port of: src/core/SkBlitter.h#L35-L168 (chrome/m156)
#[doc(alias = "SkBlitter")]
pub trait Blitter {
    /// Blit a horizontal run of one or more pixels.
    #[doc(alias = "blitH")]
    fn blit_h(&mut self, x: i32, y: i32, width: i32);

    /// Blit a horizontal run of antialiased pixels; `runs` is a *sparse* zero-terminated
    /// run-length encoding of spans of constant alpha values.
    ///
    /// The `runs` and `antialias` arrays work together to represent long runs of pixels with the
    /// same alphas. `runs` contains the number of pixels with the same alpha, and `antialias`
    /// contains the coverage value for that number of pixels. The runs array is zero terminated,
    /// and has enough entries for each pixel plus one; in most cases some of the entries will not
    /// contain valid data. An entry in the runs array contains the number of pixels (`np`) that
    /// have the same alpha value. The next `np` value is found `np` entries away. For example, if
    /// `runs[0] = 7`, then the next valid entry will be at `runs[7]`. The two arrays are coupled
    /// by index: if the `np` entry is at `runs[45] = 12`, then the alpha value can be found at
    /// `antialias[45] = 0x88`, meaning an alpha of `0x88` for the next 12 pixels starting at pixel
    /// 45.
    #[doc(alias = "blitAntiH")]
    fn blit_anti_h(&mut self, x: i32, y: i32, antialias: &mut [Alpha], runs: &mut [i16]);

    /// Blit a vertical run of pixels with a constant alpha value.
    // Port of: src/core/SkBlitter.cpp#L106-L118 (chrome/m156)
    #[doc(alias = "blitV")]
    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: Alpha) {
        if alpha == 255 {
            self.blit_rect(x, y, 1, height);
        } else {
            let mut runs = [0i16; 2];
            runs[0] = 1;
            runs[1] = 0;

            let mut alpha = alpha;
            let mut y = y;
            let mut height = height;
            loop {
                height -= 1;
                if height < 0 {
                    break;
                }
                self.blit_anti_h(x, y, std::slice::from_mut(&mut alpha), &mut runs);
                y += 1;
            }
        }
    }

    /// Blit a solid rectangle one or more pixels wide.
    // Port of: src/core/SkBlitter.cpp#L120-L125 (chrome/m156)
    #[doc(alias = "blitRect")]
    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        debug_assert!(width > 0);
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

    /// Blit a rectangle with one alpha-blended column on the left, `width` (zero or more) opaque
    /// pixels, and one alpha-blended column on the right. The result will always be at least two
    /// pixels wide.
    ///
    /// The default implementation doesn't check for easy optimizations such as `alpha == 255`;
    /// it also uses [`Self::blit_v`], which some implementors may not support.
    // Port of: src/core/SkBlitter.cpp#L127-L143 (chrome/m156)
    #[doc(alias = "blitAntiRect")]
    fn blit_anti_rect(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        left_alpha: Alpha,
        right_alpha: Alpha,
    ) {
        let mut x = x;
        if left_alpha > 0 {
            // we may send in x = -1 with leftAlpha = 0
            self.blit_v(x, y, height, left_alpha);
        }
        x += 1;
        if width > 0 {
            self.blit_rect(x, y, width, height);
            x += width;
        }
        if right_alpha > 0 {
            self.blit_v(x, y, height, right_alpha);
        }
    }

    /// Blit a rect in AA with size at least 3 x 3 (small rect has too many edge cases...).
    // Port of: src/core/SkBlitter.cpp#L54-L104 (chrome/m156)
    #[doc(alias = "blitFatAntiRect")]
    // The int -> float conversions mirror the implicit C++ ones.
    #[allow(clippy::cast_precision_loss)]
    fn blit_fat_anti_rect(&mut self, rect: &Rect) {
        let bounds: IRect = rect.round_out();
        debug_assert!(bounds.width() >= 3);

        // skbug.com/40039068
        // To ensure consistency of the threaded backend (a rect that's considered fat in the
        // init-once phase must also be considered fat in the draw phase), we have to deal with
        // rects with small heights because the horizontal tiling in the threaded backend may
        // change the height.
        //
        // This also implies that we cannot do vertical tiling unless we can blit any rect (not
        // just the fat one.)
        if bounds.height() == 0 {
            return;
        }

        // Skia carves the runs/alphas out of `allocBlitMemory`; a borrow of `self`'s memory could
        // not be held across the `blit_anti_h` calls, so local buffers are used instead.
        let width = usize::try_from(bounds.width()).expect("width >= 3");
        let run_size = width + 1; // +1 so we can set runs[bounds.width()] = 0
        let mut runs = vec![0i16; run_size];
        let mut alphas: Vec<Alpha> = vec![0; run_size];

        runs[0] = 1;
        runs[1] = to_s16(bounds.width() - 2);
        runs[width - 1] = 1;
        runs[width] = 0;

        // `bounds.fLeft + 1 - rect.fLeft`: the int sum is converted to float for the subtraction.
        // (Mirrors the C++ implicit int -> float conversions; device coordinates are small.)
        let partial_l = (bounds.left + 1) as f32 - rect.left;
        let partial_r = rect.right - (bounds.right - 1) as f32;
        let mut partial_t = (bounds.top + 1) as f32 - rect.top;
        let partial_b = rect.bottom - (bounds.bottom - 1) as f32;

        if bounds.height() == 1 {
            partial_t = rect.bottom - rect.top;
        }

        alphas[0] = scalar_to_alpha(partial_l * partial_t);
        alphas[1] = scalar_to_alpha(partial_t);
        alphas[width - 1] = scalar_to_alpha(partial_r * partial_t);
        self.blit_anti_h(bounds.left, bounds.top, &mut alphas, &mut runs);

        if bounds.height() > 2 {
            self.blit_anti_rect(
                bounds.left,
                bounds.top + 1,
                bounds.width() - 2,
                bounds.height() - 2,
                scalar_to_alpha(partial_l),
                scalar_to_alpha(partial_r),
            );
        }

        if bounds.height() > 1 {
            alphas[0] = scalar_to_alpha(partial_l * partial_b);
            alphas[1] = scalar_to_alpha(partial_b);
            alphas[width - 1] = scalar_to_alpha(partial_r * partial_b);
            self.blit_anti_h(bounds.left, bounds.bottom - 1, &mut alphas, &mut runs);
        }
    }

    /// Blit a pattern of pixels defined by a rectangle-clipped mask; typically used for text.
    ///
    /// The default implementation handles every format except [`MaskFormat::Lcd16`], which "needs
    /// to be handled by subclass" (it does nothing).
    // Port of: src/core/SkBlitter.cpp#L188-L267 (chrome/m156)
    #[doc(alias = "blitMask")]
    fn blit_mask(&mut self, mask: &Mask<'_>, clip: &IRect) {
        blit_mask_default(self, mask, clip);
    }

    /// Blit `(x, y)` and `(x + 1, y)`.
    // Port of: src/core/SkBlitter.h#L99-L108 (chrome/m156)
    #[doc(alias = "blitAntiH2")]
    fn blit_anti_h2(&mut self, x: i32, y: i32, a0: u32, a1: u32) {
        let mut runs = [1i16, 1, 0];
        let mut aa = [to_u8(a0), to_u8(a1)];
        self.blit_anti_h(x, y, &mut aa, &mut runs);
    }

    /// Blit `(x, y)` and `(x, y + 1)`.
    // Port of: src/core/SkBlitter.h#L111-L124 (chrome/m156)
    #[doc(alias = "blitAntiV2")]
    fn blit_anti_v2(&mut self, x: i32, y: i32, a0: u32, a1: u32) {
        let mut runs = [1i16, 0];
        let mut aa = [to_u8(a0)];
        self.blit_anti_h(x, y, &mut aa, &mut runs);
        // reset in case the clipping blitter modified runs
        runs[0] = 1;
        runs[1] = 0;
        aa[0] = to_u8(a1);
        self.blit_anti_h(x, y + 1, &mut aa, &mut runs);
    }

    /// Special method for blitters that can blit more than one row at a time. Returns the number
    /// of rows that this blitter could optimally process at a time. It is still required to
    /// support blitting one scanline at a time.
    // Port of: src/core/SkBlitter.h#L126-L132 (chrome/m156)
    #[doc(alias = "requestRowsPreserved")]
    fn request_rows_preserved(&self) -> i32 {
        1
    }

    /// If the blitter would fill the whole destination with one constant pixel value for a solid
    /// blit (and the blit may skip the pipeline), the destination and that value
    /// (`canDirectBlit`). Wrappers return `None`, as the default does.
    // Port of: src/core/SkBlitter.h#L129 (chrome/m156)
    #[doc(alias = "canDirectBlit")]
    fn can_direct_blit(&mut self) -> Option<DirectBlit<'_>> {
        None
    }

    /// The memory owned by this blitter (`fBlitMemory`); wrappers forward to the wrapped blitter.
    fn blit_memory(&mut self) -> &mut BlitMemory;

    /// Allocates memory that the blitter owns. The memory can be used by the calling function at
    /// will, but it is released when the blitter is dropped (or the next call reuses it).
    // Port of: src/core/SkBlitter.h#L145-L148 (chrome/m156)
    #[doc(alias = "allocBlitMemory")]
    fn alloc_blit_memory(&mut self, sz: usize) -> &mut [u8] {
        self.blit_memory().reset(sz)
    }

    /// Blits `mask` through each rectangle of `clip` that intersects it (non-virtual in Skia).
    // Port of: src/core/SkBlitter.cpp#L270-L282 (chrome/m156)
    #[doc(alias = "blitMaskRegion")]
    fn blit_mask_region(&mut self, mask: &Mask<'_>, clip: &Region) {
        if clip.quick_reject_rect(mask.bounds) {
            return;
        }

        for cr in Cliperator::new(clip, mask.bounds) {
            self.blit_mask(mask, &cr);
        }
    }

    /// Blits `rect` through each rectangle of `clip` (non-virtual in Skia).
    // Port of: src/core/SkBlitter.cpp#L284-L292 (chrome/m156)
    #[doc(alias = "blitRectRegion")]
    fn blit_rect_region(&mut self, rect: &IRect, clip: &Region) {
        for cr in Cliperator::new(clip, rect) {
            self.blit_rect(cr.left, cr.top, cr.width(), cr.height());
        }
    }

    /// Blits every span of `clip` (non-virtual in Skia).
    // Port of: src/core/SkBlitter.cpp#L294-L298 (chrome/m156)
    #[doc(alias = "blitRegion")]
    fn blit_region(&mut self, clip: &Region) {
        region_priv::visit_spans(clip, &mut |r: &IRect| {
            self.blit_rect(r.left, r.top, r.width(), r.height());
        });
    }
}

/// The default body of [`Blitter::blit_mask`], for blitters that override it and fall back to it
/// (`SkBlitter::blitMask` called as `INHERITED::blitMask`).
///
/// # Panics
/// If `clip` is not inside `mask.bounds` (a debug assertion in Skia) or the mask image is too
/// small for its bounds and row bytes.
// Port of: src/core/SkBlitter.cpp#L188-L267 (chrome/m156)
pub fn blit_mask_default<B: Blitter + ?Sized>(blitter: &mut B, mask: &Mask<'_>, clip: &IRect) {
    debug_assert!(
        mask.bounds.left <= clip.left
            && mask.bounds.top <= clip.top
            && clip.right <= mask.bounds.right
            && clip.bottom <= mask.bounds.bottom
    );

    if mask.format == MaskFormat::Lcd16 {
        return; // needs to be handled by subclass
    }

    if mask.format == MaskFormat::BW {
        let cx = clip.left;
        let mut cy = clip.top;
        let mask_left = mask.bounds.left;
        let mask_row_bytes = mask.row_bytes as usize;
        let mut height = clip.height();

        let bits = mask.get_addr1(cx, cy);
        // Offset (in bytes) of the current row, relative to `bits`.
        let mut row = 0usize;

        if cx == mask_left && clip.right == mask.bounds.right {
            loop {
                height -= 1;
                if height < 0 {
                    break;
                }
                let affected_right_bit = mask.bounds.width() - 1;
                let row_bytes = (affected_right_bit >> 3) + 1;
                let right_mask = generate_right_mask((affected_right_bit & 7) + 1);
                bits_to_runs(
                    blitter,
                    cx,
                    cy,
                    &bits[row..],
                    0xFF,
                    row_bytes as isize,
                    right_mask,
                );
                row += mask_row_bytes;
                cy += 1;
            }
        } else {
            // Bits is calculated as the offset into the mask at the point {cx, cy} therefore,
            // all addressing into the bit mask is relative to that point. Since this is an
            // address calculated from a arbitrary bit in that byte, calculate the left most
            // bit.
            let bits_left = cx - ((cx - mask_left) & 7);

            // Everything is relative to the bitsLeft.
            let left_edge = cx - bits_left;
            debug_assert!(left_edge >= 0);
            let right_edge = clip.right - bits_left;
            debug_assert!(right_edge > left_edge);

            // Calculate left byte and mask
            let left_mask = 0xFFu32 >> (left_edge & 7);

            // Calculate right byte and mask
            let affected_right_bit = right_edge - 1;
            let right_mask = generate_right_mask((affected_right_bit & 7) + 1);

            // leftByte and rightByte are byte locations therefore, to get a count of bytes the
            // code must add one.
            let row_bytes = (affected_right_bit >> 3) + 1;

            loop {
                height -= 1;
                if height < 0 {
                    break;
                }
                bits_to_runs(
                    blitter,
                    bits_left,
                    cy,
                    &bits[row..],
                    to_u8(left_mask),
                    row_bytes as isize,
                    right_mask,
                );
                row += mask_row_bytes;
                cy += 1;
            }
        }
    } else {
        let width = clip.width();
        let w = usize::try_from(width).expect("clip is non-empty");
        let mut runs = vec![0i16; w + 1];
        let mut row_aa = vec![0 as Alpha; w];
        let mut aa = mask.get_addr8(clip.left, clip.top);

        runs[..w].fill(1);
        runs[w] = 0;

        let mut height = clip.height();
        let mut y = clip.top;
        loop {
            height -= 1;
            if height < 0 {
                break;
            }
            // `aa` is const in C++ and may be rewritten by clipping blitters; hand out a copy.
            row_aa.copy_from_slice(&aa[..w]);
            blitter.blit_anti_h(clip.left, y, &mut row_aa, &mut runs);
            if height > 0 {
                aa = &aa[mask.row_bytes as usize..];
            }
            y += 1;
        }
    }
}

// Port of: src/core/SkBlitter.cpp#L147-L181 (chrome/m156)
fn bits_to_runs<B: Blitter + ?Sized>(
    blitter: &mut B,
    x: i32,
    y: i32,
    bits: &[u8],
    left_mask: u8,
    row_bytes: isize,
    right_mask: u8,
) {
    let mut x = x;
    let mut left_mask = left_mask;
    let mut row_bytes = row_bytes;
    let mut in_fill = false;
    let mut pos = 0;
    let mut bits = bits.iter();

    loop {
        row_bytes -= 1;
        if row_bytes < 0 {
            break;
        }
        let mut b = *bits.next().expect("mask row is long enough") & left_mask;
        if row_bytes == 0 {
            b &= right_mask;
        }

        let mut test = 0x80u8;
        while test != 0 {
            if b & test != 0 {
                if !in_fill {
                    pos = x;
                    in_fill = true;
                }
            } else if in_fill {
                blitter.blit_h(pos, y, x - pos);
                in_fill = false;
            }
            x += 1;
            test >>= 1;
        }
        left_mask = 0xFF;
    }

    // final cleanup
    if in_fill {
        blitter.blit_h(pos, y, x - pos);
    }
}

// maskBitCount is the number of 1's to place in the mask. It must be in the range between 1 and 8.
// Port of: src/core/SkBlitter.cpp#L184-L186 (chrome/m156)
fn generate_right_mask(mask_bit_count: i32) -> u8 {
    // The result of `& 0xFF` always fits.
    to_u8((0xFF00u32 >> mask_bit_count) & 0xFF)
}

// Port of: src/core/SkBlitter.cpp#L302-L316 (chrome/m156)
fn compute_anti_width(runs: &[i16]) -> i32 {
    let mut width = 0;
    let mut i = 0usize;

    loop {
        let count = i32::from(runs[i]);

        debug_assert!(count >= 0);
        if count == 0 {
            break;
        }
        width += count;
        i += usize::try_from(count).expect("non-negative");
    }
    width
}

// `(unsigned)(y - rect.fTop) < (unsigned)rect.height()`
// Port of: src/core/SkBlitter.cpp#L318-L320 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // mirrors the C++ unsigned range trick
fn y_in_rect(y: i32, rect: &IRect) -> bool {
    (y.wrapping_sub(rect.top) as u32) < (rect.height() as u32)
}

// Port of: src/core/SkBlitter.cpp#L322-L324 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // mirrors the C++ unsigned range trick
fn x_in_rect(x: i32, rect: &IRect) -> bool {
    (x.wrapping_sub(rect.left) as u32) < (rect.width() as u32)
}

/// This blitter silently never draws anything.
// Port of: src/core/SkBlitter.h#L170-L178 (chrome/m156)
#[doc(alias = "SkNullBlitter")]
#[derive(Clone, Debug, Default)]
pub struct NullBlitter {
    memory: BlitMemory,
}

impl NullBlitter {
    /// Creates a null blitter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Blitter for NullBlitter {
    fn blit_h(&mut self, _x: i32, _y: i32, _width: i32) {}
    fn blit_anti_h(&mut self, _x: i32, _y: i32, _aa: &mut [Alpha], _runs: &mut [i16]) {}
    fn blit_v(&mut self, _x: i32, _y: i32, _height: i32, _alpha: Alpha) {}
    fn blit_rect(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) {}
    fn blit_mask(&mut self, _mask: &Mask<'_>, _clip: &IRect) {}
    fn blit_memory(&mut self) -> &mut BlitMemory {
        &mut self.memory
    }
}

/// Wraps another (real) blitter, and ensures that the real blitter is only called with
/// coordinates that have been clipped by the specified clip rect. This means the caller need not
/// perform the clipping ahead of time.
// Port of: src/core/SkBlitter.h#L180-L215 (chrome/m156)
#[doc(alias = "SkRectClipBlitter")]
#[derive(Debug)]
pub struct RectClipBlitter<'a> {
    blitter: &'a mut dyn Blitter,
    clip_rect: IRect,
}

impl<'a> RectClipBlitter<'a> {
    /// `SkRectClipBlitter::init`: `clip_rect` must not be empty.
    #[must_use]
    pub fn new(blitter: &'a mut dyn Blitter, clip_rect: IRect) -> Self {
        debug_assert!(!clip_rect.is_empty());
        RectClipBlitter { blitter, clip_rect }
    }
}

impl std::fmt::Debug for dyn Blitter + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("dyn Blitter")
    }
}

impl Blitter for RectClipBlitter<'_> {
    // Port of: src/core/SkBlitter.cpp#L326-L346 (chrome/m156)
    fn blit_h(&mut self, left: i32, y: i32, width: i32) {
        debug_assert!(width > 0);
        let clip = self.clip_rect;

        if !y_in_rect(y, &clip) {
            return;
        }

        let mut left = left;
        let mut right = left + width;

        if left < clip.left {
            left = clip.left;
        }
        if right > clip.right {
            right = clip.right;
        }

        let width = right - left;
        if width > 0 {
            self.blitter.blit_h(left, y, width);
        }
    }

    // Port of: src/core/SkBlitter.cpp#L348-L384 (chrome/m156)
    fn blit_anti_h(&mut self, left: i32, y: i32, aa: &mut [Alpha], runs: &mut [i16]) {
        let clip = self.clip_rect;
        let mut aa: &mut [Alpha] = aa;
        let mut runs: &mut [i16] = runs;

        if !y_in_rect(y, &clip) || left >= clip.right {
            return;
        }

        let mut x0 = left;
        let mut x1 = left + compute_anti_width(runs);

        if x1 <= clip.left {
            return;
        }

        debug_assert!(x0 < x1);
        if x0 < clip.left {
            let dx = clip.left - x0;
            AlphaRuns::break_at(runs, aa, dx);
            let dxu = usize::try_from(dx).expect("positive");
            runs = &mut runs[dxu..];
            aa = &mut aa[dxu..];
            x0 = clip.left;
        }

        debug_assert!(x0 < x1 && runs[usize::try_from(x1 - x0).expect("positive")] == 0);
        if x1 > clip.right {
            x1 = clip.right;
            AlphaRuns::break_at(runs, aa, x1 - x0);
            runs[usize::try_from(x1 - x0).expect("positive")] = 0;
        }

        debug_assert!(x0 < x1 && runs[usize::try_from(x1 - x0).expect("positive")] == 0);
        debug_assert_eq!(compute_anti_width(runs), x1 - x0);

        self.blitter.blit_anti_h(x0, y, aa, runs);
    }

    // Port of: src/core/SkBlitter.cpp#L386-L406 (chrome/m156)
    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: Alpha) {
        debug_assert!(height > 0);
        let clip = self.clip_rect;

        if !x_in_rect(x, &clip) {
            return;
        }

        let mut y0 = y;
        let mut y1 = y + height;

        if y0 < clip.top {
            y0 = clip.top;
        }
        if y1 > clip.bottom {
            y1 = clip.bottom;
        }

        if y0 < y1 {
            self.blitter.blit_v(x, y0, y1 - y0, alpha);
        }
    }

    // Port of: src/core/SkBlitter.cpp#L408-L415 (chrome/m156)
    fn blit_rect(&mut self, left: i32, y: i32, width: i32, height: i32) {
        let r = IRect::new(left, y, left + width, y + height);
        if let Some(r) = IRect::intersect(&r, &self.clip_rect) {
            self.blitter.blit_rect(r.left, r.top, r.width(), r.height());
        }
    }

    // Port of: src/core/SkBlitter.cpp#L417-L446 (chrome/m156)
    fn blit_anti_rect(
        &mut self,
        left: i32,
        y: i32,
        width: i32,
        height: i32,
        left_alpha: Alpha,
        right_alpha: Alpha,
    ) {
        let mut left_alpha = left_alpha;
        let mut right_alpha = right_alpha;

        // The *true* width of the rectangle blitted is width+2:
        let r = IRect::new(left, y, left + width + 2, y + height);
        if let Some(r) = IRect::intersect(&r, &self.clip_rect) {
            if r.left != left {
                debug_assert!(r.left > left);
                left_alpha = 255;
            }
            if r.right != left + width + 2 {
                debug_assert!(r.right < left + width + 2);
                right_alpha = 255;
            }
            if 255 == left_alpha && 255 == right_alpha {
                self.blitter.blit_rect(r.left, r.top, r.width(), r.height());
            } else if 1 == r.width() {
                if r.left == left {
                    self.blitter.blit_v(r.left, r.top, r.height(), left_alpha);
                } else {
                    debug_assert_eq!(r.left, left + width + 1);
                    self.blitter.blit_v(r.left, r.top, r.height(), right_alpha);
                }
            } else {
                self.blitter.blit_anti_rect(
                    r.left,
                    r.top,
                    r.width() - 2,
                    r.height(),
                    left_alpha,
                    right_alpha,
                );
            }
        }
    }

    // Port of: src/core/SkBlitter.cpp#L448-L456 (chrome/m156)
    fn blit_mask(&mut self, mask: &Mask<'_>, clip: &IRect) {
        debug_assert!(
            mask.bounds.left <= clip.left
                && mask.bounds.top <= clip.top
                && clip.right <= mask.bounds.right
                && clip.bottom <= mask.bounds.bottom
        );

        if let Some(r) = IRect::intersect(clip, &self.clip_rect) {
            self.blitter.blit_mask(mask, &r);
        }
    }

    fn request_rows_preserved(&self) -> i32 {
        self.blitter.request_rows_preserved()
    }

    fn blit_memory(&mut self) -> &mut BlitMemory {
        self.blitter.blit_memory()
    }
}

/// Wraps another (real) blitter, and ensures that the real blitter is only called with
/// coordinates that have been clipped by the specified region. This means the caller need not
/// perform the clipping ahead of time.
// Port of: src/core/SkBlitter.h#L217-L248 (chrome/m156)
#[doc(alias = "SkRgnClipBlitter")]
#[derive(Debug)]
pub struct RgnClipBlitter<'a> {
    blitter: &'a mut dyn Blitter,
    rgn: &'a Region,
}

impl<'a> RgnClipBlitter<'a> {
    /// `SkRgnClipBlitter::init`: `clip_rgn` must not be empty.
    #[must_use]
    pub fn new(blitter: &'a mut dyn Blitter, clip_rgn: &'a Region) -> Self {
        debug_assert!(!clip_rgn.is_empty());
        RgnClipBlitter {
            blitter,
            rgn: clip_rgn,
        }
    }
}

impl Blitter for RgnClipBlitter<'_> {
    // Port of: src/core/SkBlitter.cpp#L460-L468 (chrome/m156)
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        for (left, right) in Spanerator::new(self.rgn, y, x, x + width) {
            debug_assert!(left < right);
            self.blitter.blit_h(left, y, right - left);
        }
    }

    // Port of: src/core/SkBlitter.cpp#L470-L510 (chrome/m156)
    fn blit_anti_h(&mut self, x: i32, y: i32, aa: &mut [Alpha], runs: &mut [i16]) {
        let mut aa: &mut [Alpha] = aa;
        let mut runs: &mut [i16] = runs;
        let mut x = x;

        let width = compute_anti_width(runs);
        let bounds = *self.rgn.bounds();

        let mut prev_rite = x;
        for (left, right) in Spanerator::new(self.rgn, y, x, x + width) {
            debug_assert!(x <= left);
            debug_assert!(left < right);
            debug_assert!(left >= bounds.left && right <= bounds.right);

            AlphaRuns::break_runs(runs, aa, left - x, right - left);

            // now zero before left
            if left > prev_rite {
                let index = usize::try_from(prev_rite - x).expect("non-negative");
                aa[index] = 0; // skip runs after right
                runs[index] = to_s16(left - prev_rite);
            }

            prev_rite = right;
        }

        if prev_rite > x {
            runs[usize::try_from(prev_rite - x).expect("positive")] = 0;

            if x < 0 {
                let skip = i32::from(runs[0]);
                debug_assert!(skip >= -x);
                let skipu = usize::try_from(skip).expect("positive");
                aa = &mut aa[skipu..];
                runs = &mut runs[skipu..];
                x += skip;
            }
            self.blitter.blit_anti_h(x, y, aa, runs);
        }
    }

    // Port of: src/core/SkBlitter.cpp#L512-L525 (chrome/m156)
    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: Alpha) {
        let mut bounds = IRect::new(0, 0, 0, 0);
        bounds.set_xywh(x, y, 1, height);

        for r in Cliperator::new(self.rgn, bounds) {
            debug_assert!(contains(&bounds, &r));

            self.blitter.blit_v(x, r.top, r.height(), alpha);
        }
    }

    // Port of: src/core/SkBlitter.cpp#L527-L540 (chrome/m156)
    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        let mut bounds = IRect::new(0, 0, 0, 0);
        bounds.set_xywh(x, y, width, height);

        for r in Cliperator::new(self.rgn, bounds) {
            debug_assert!(contains(&bounds, &r));

            self.blitter.blit_rect(r.left, r.top, r.width(), r.height());
        }
    }

    // Port of: src/core/SkBlitter.cpp#L542-L577 (chrome/m156)
    fn blit_anti_rect(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        left_alpha: Alpha,
        right_alpha: Alpha,
    ) {
        // The *true* width of the rectangle to blit is width + 2
        let mut bounds = IRect::new(0, 0, 0, 0);
        bounds.set_xywh(x, y, width + 2, height);

        for r in Cliperator::new(self.rgn, bounds) {
            debug_assert!(contains(&bounds, &r));
            debug_assert!(r.left >= x);
            debug_assert!(r.right <= x + width + 2);

            let effective_left_alpha = if r.left == x { left_alpha } else { 255 };
            let effective_right_alpha = if r.right == x + width + 2 {
                right_alpha
            } else {
                255
            };

            if 255 == effective_left_alpha && 255 == effective_right_alpha {
                self.blitter.blit_rect(r.left, r.top, r.width(), r.height());
            } else if 1 == r.width() {
                if r.left == x {
                    self.blitter
                        .blit_v(r.left, r.top, r.height(), effective_left_alpha);
                } else {
                    debug_assert_eq!(r.left, x + width + 1);
                    self.blitter
                        .blit_v(r.left, r.top, r.height(), effective_right_alpha);
                }
            } else {
                self.blitter.blit_anti_rect(
                    r.left,
                    r.top,
                    r.width() - 2,
                    r.height(),
                    effective_left_alpha,
                    effective_right_alpha,
                );
            }
        }
    }

    // Port of: src/core/SkBlitter.cpp#L580-L591 (chrome/m156)
    fn blit_mask(&mut self, mask: &Mask<'_>, clip: &IRect) {
        debug_assert!(
            mask.bounds.left <= clip.left
                && mask.bounds.top <= clip.top
                && clip.right <= mask.bounds.right
                && clip.bottom <= mask.bounds.bottom
        );

        for r in Cliperator::new(self.rgn, clip) {
            self.blitter.blit_mask(mask, &r);
        }
    }

    fn request_rows_preserved(&self) -> i32 {
        self.blitter.request_rows_preserved()
    }

    fn blit_memory(&mut self) -> &mut BlitMemory {
        self.blitter.blit_memory()
    }
}

// `SkIRect::contains(const SkIRect&)` for non-empty rects (used only in assertions).
fn contains(outer: &IRect, inner: &IRect) -> bool {
    outer.left <= inner.left
        && outer.top <= inner.top
        && outer.right >= inner.right
        && outer.bottom >= inner.bottom
}

/// Factory to set up the appropriate most-efficient wrapper blitter to apply a clip. Returns a
/// reference to a member, so lifetime must be managed carefully: the returned blitter borrows the
/// clipper (and, through it, the blitter and region passed to [`Self::apply`]).
// Port of: src/core/SkBlitter.h#L283-L294 (chrome/m156)
#[doc(alias = "SkBlitterClipper")]
#[derive(Debug, Default)]
pub struct BlitterClipper<'a> {
    null: NullBlitter,
    rect: Option<RectClipBlitter<'a>>,
    rgn: Option<RgnClipBlitter<'a>>,
}

impl<'a> BlitterClipper<'a> {
    /// Creates a clipper with no blitter set up yet.
    #[must_use]
    pub fn new() -> Self {
        BlitterClipper {
            null: NullBlitter::new(),
            rect: None,
            rgn: None,
        }
    }

    /// Returns the most efficient blitter that applies `clip` to `blitter`: `blitter` itself when
    /// there is no clip (or a rect clip that contains `ir`), a [`NullBlitter`] when the clip is
    /// empty or misses `ir`, otherwise a [`RectClipBlitter`] or [`RgnClipBlitter`] owned by
    /// `self`. `ir` is the bounds of what will be drawn, if known.
    // Port of: src/core/SkBlitter.cpp#L595-L613 (chrome/m156)
    pub fn apply<'b>(
        &'b mut self,
        blitter: &'a mut dyn Blitter,
        clip: Option<&'a Region>,
        ir: Option<&IRect>,
    ) -> &'b mut dyn Blitter
    where
        'a: 'b,
    {
        let Some(clip) = clip else {
            return blitter;
        };
        let clip_r = *clip.bounds();

        if clip.is_empty() || ir.is_some_and(|ir| !IRect::intersects(&clip_r, ir)) {
            &mut self.null
        } else if clip.is_rect() {
            if ir.is_none_or(|ir| !contains(&clip_r, ir)) {
                self.rect.insert(RectClipBlitter::new(blitter, clip_r))
            } else {
                blitter
            }
        } else {
            self.rgn.insert(RgnClipBlitter::new(blitter, clip))
        }
    }
}
