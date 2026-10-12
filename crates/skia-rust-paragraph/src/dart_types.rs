// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/include/DartTypes.h

//! `DartTypes.h`: the enums and small structs of the paragraph API.
//!
//! `SkRange<size_t>` is a `Range<usize>` (the shape of `skia_safe::textlayout`), with the methods
//! of `SkRange` in the [`RangeExtensions`] trait.

use std::cmp::{max, min};
use std::ops::Range;

use skia_rust_core::rect::Rect;

// Port of: modules/skparagraph/include/DartTypes.h#L18 (chrome/m156)
/// `Affinity`: which side of a position a caret belongs to.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum Affinity {
    /// `kUpstream`.
    #[doc(alias = "kUpstream")]
    Upstream,
    /// `kDownstream`.
    #[doc(alias = "kDownstream")]
    #[default]
    Downstream,
}

// Port of: modules/skparagraph/include/DartTypes.h#L20-L48 (chrome/m156)
/// `RectHeightStyle`: how tall the boxes of `getRectsForRange` are.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum RectHeightStyle {
    /// Provide tight bounding boxes that fit heights per run.
    #[default]
    Tight,
    /// The height of the boxes will be the maximum height of all runs in the line. All rects in
    /// the same line will be the same height.
    Max,
    /// Extends the top and/or bottom edge of the bounds to fully cover any line spacing. The top
    /// edge of each line should be the same as the bottom edge of the line above. There should be
    /// no gaps in vertical coverage given any ParagraphStyle line_height.
    ///
    /// The top and bottom of each rect will cover half of the space above and half of the space
    /// below the line.
    IncludeLineSpacingMiddle,
    /// The line spacing will be added to the top of the rect.
    IncludeLineSpacingTop,
    /// The line spacing will be added to the bottom of the rect.
    IncludeLineSpacingBottom,
    /// `kStrut`.
    Strut,
}

// Port of: modules/skparagraph/include/DartTypes.h#L50-L58 (chrome/m156)
/// `RectWidthStyle`: how wide the boxes of `getRectsForRange` are.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum RectWidthStyle {
    /// Provide tight bounding boxes that fit widths to the runs of each line independently.
    #[default]
    Tight,
    /// Extends the width of the last rect of each line to match the position of the widest rect
    /// over all the lines.
    Max,
}

// Port of: modules/skparagraph/include/DartTypes.h#L60-L67 (chrome/m156)
/// `TextAlign`.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum TextAlign {
    /// `kLeft`.
    Left,
    /// `kRight`.
    Right,
    /// `kCenter`.
    Center,
    /// `kJustify`.
    Justify,
    /// `kStart`.
    #[default]
    Start,
    /// `kEnd`.
    End,
}

// Port of: modules/skparagraph/include/DartTypes.h#L69-L72 (chrome/m156)
/// `TextDirection`.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
#[repr(i32)]
#[allow(clippy::upper_case_acronyms)] // the names of skia_safe::textlayout::TextDirection
pub enum TextDirection {
    /// `kRtl`.
    RTL,
    /// `kLtr`.
    LTR,
}

// Port of: modules/skparagraph/include/DartTypes.h#L74-L79 (chrome/m156)
/// `PositionWithAffinity`: a text position and the side of it.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PositionWithAffinity {
    /// `position`.
    pub position: i32,
    /// `affinity`.
    pub affinity: Affinity,
}

impl Default for PositionWithAffinity {
    // Port of: modules/skparagraph/include/DartTypes.h#L77 (chrome/m156)
    fn default() -> Self {
        Self {
            position: 0,
            affinity: Affinity::Downstream,
        }
    }
}

// Port of: modules/skparagraph/include/DartTypes.h#L81-L86 (chrome/m156)
/// `TextBox`: a rectangle of text and its direction.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct TextBox {
    /// `rect`.
    pub rect: Rect,
    /// `direction`.
    pub direct: TextDirection,
}

impl TextBox {
    // Port of: modules/skparagraph/include/DartTypes.h#L84 (chrome/m156)
    /// `TextBox(SkRect r, TextDirection d)`.
    #[must_use]
    pub fn new(rect: Rect, direct: TextDirection) -> Self {
        Self { rect, direct }
    }
}

// Port of: modules/skparagraph/include/DartTypes.h#L103 (chrome/m156)
/// `EMPTY_INDEX`: `std::numeric_limits<size_t>::max()`.
pub const EMPTY_INDEX: usize = usize::MAX;

// Port of: modules/skparagraph/include/DartTypes.h#L105-L134 (chrome/m156)
/// The methods of `SkRange<size_t>` that `std::ops::Range` has no counterpart for.
#[doc(alias = "SkRange")]
pub trait RangeExtensions {
    /// `width()`: `end - start`.
    fn width(&self) -> usize;
    /// `Shift(delta)`.
    fn shift(&mut self, d: isize);
    /// `contains(other)`.
    fn contains(&self, other: &Self) -> bool;
    /// `intersects(other)`.
    fn intersects(&self, other: &Self) -> bool;
    /// `intersection(other)`.
    #[must_use]
    fn intersection(&self, other: &Self) -> Self;
    /// `empty()`: both ends are [`EMPTY_INDEX`].
    fn empty(&self) -> bool;
}

impl RangeExtensions for Range<usize> {
    // Port of: modules/skparagraph/include/DartTypes.h#L113 (chrome/m156)
    fn width(&self) -> usize {
        self.end.wrapping_sub(self.start)
    }

    // Port of: modules/skparagraph/include/DartTypes.h#L114-L117 (chrome/m156)
    fn shift(&mut self, d: isize) {
        // `start += delta` with an unsigned start and a signed delta wraps, as in C++.
        self.start = self.start.wrapping_add_signed(d);
        self.end = self.end.wrapping_add_signed(d);
    }

    // Port of: modules/skparagraph/include/DartTypes.h#L118-L120 (chrome/m156)
    fn contains(&self, other: &Self) -> bool {
        self.start <= other.start && self.end >= other.end
    }

    // Port of: modules/skparagraph/include/DartTypes.h#L121-L123 (chrome/m156)
    fn intersects(&self, other: &Self) -> bool {
        max(self.start, other.start) <= min(self.end, other.end)
    }

    // Port of: modules/skparagraph/include/DartTypes.h#L124-L126 (chrome/m156)
    fn intersection(&self, other: &Self) -> Self {
        Self {
            start: max(self.start, other.start),
            end: min(self.end, other.end),
        }
    }

    // Port of: modules/skparagraph/include/DartTypes.h#L127-L129 (chrome/m156)
    fn empty(&self) -> bool {
        self.start == EMPTY_INDEX && self.end == EMPTY_INDEX
    }
}

// Port of: modules/skparagraph/include/DartTypes.h#L131 (chrome/m156)
/// `EMPTY_RANGE`.
#[allow(clippy::reversed_empty_ranges)] // start == end == EMPTY_INDEX, as in C++
pub const EMPTY_RANGE: Range<usize> = Range {
    start: EMPTY_INDEX,
    end: EMPTY_INDEX,
};

// Port of: modules/skparagraph/include/DartTypes.h#L133-L136 (chrome/m156)
/// `TextBaseline`.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum TextBaseline {
    /// `kAlphabetic`.
    #[default]
    Alphabetic,
    /// `kIdeographic`.
    Ideographic,
}

// Port of: modules/skparagraph/include/DartTypes.h#L138-L143 (chrome/m156)
/// `TextHeightBehavior`: a set of flags (`kAll`, `kDisableFirstAscent`, `kDisableLastDescent`,
/// `kDisableAll`), used as an enum in `skia_safe` and as bits by the layout.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum TextHeightBehavior {
    /// `kAll`.
    #[default]
    All = 0x0,
    /// `kDisableFirstAscent`.
    DisableFirstAscent = 0x1,
    /// `kDisableLastDescent`.
    DisableLastDescent = 0x2,
    /// `kDisableAll`.
    DisableAll = 0x1 | 0x2,
}

impl TextHeightBehavior {
    /// The bits of the C++ enum.
    #[must_use]
    pub fn bits(self) -> i32 {
        self as i32
    }

    /// `behavior & kDisableFirstAscent` (non-zero).
    #[must_use]
    pub fn disable_first_ascent(self) -> bool {
        self.bits() & Self::DisableFirstAscent.bits() != 0
    }

    /// `behavior & kDisableLastDescent` (non-zero).
    #[must_use]
    pub fn disable_last_descent(self) -> bool {
        self.bits() & Self::DisableLastDescent.bits() != 0
    }
}

// Port of: modules/skparagraph/include/DartTypes.h#L145-L150 (chrome/m156)
/// `LineMetricStyle`.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
#[repr(u8)]
pub enum LineMetricStyle {
    /// Use ascent, descent, etc from a fixed baseline.
    Typographic,
    /// Use ascent, descent, etc like css with the leading split and with height adjustments.
    CSS,
}
