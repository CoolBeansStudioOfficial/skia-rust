// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLPosition.{h,cpp}.

//! [`Position`]: where a construct sits in the source text, as a byte range.

// Port of: src/sksl/SkSLPosition.h#L16-L100 and src/sksl/SkSLPosition.cpp#L11-L31 (chrome/m156)

/// `SkSL::Position`: a start offset and a length, packed as Skia packs them: the start is a
/// 24-bit signed field (so offsets are at most [`Position::MAX_OFFSET`]) and the length is an
/// 8-bit field (longer ranges are capped at 255).
#[doc(alias = "SkSL::Position")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    start_offset: i32,
    length: u8,
}

impl Default for Position {
    /// An invalid position: start -1, length 0.
    fn default() -> Self {
        Self {
            start_offset: -1,
            length: 0,
        }
    }
}

impl Position {
    /// `kMaxOffset`: the largest start offset a 24-bit field holds.
    pub const MAX_OFFSET: i32 = 0x7F_FFFF;

    /// `Position::Range(startOffset, endOffset)`. Offsets must satisfy `start <= end` and
    /// `start <= MAX_OFFSET`.
    #[must_use]
    // The length cast keeps the low byte on purpose, as the bit-field store does.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn range(start_offset: i32, end_offset: i32) -> Self {
        debug_assert!(
            start_offset <= end_offset,
            "Position::range: start after end"
        );
        debug_assert!(
            start_offset <= Self::MAX_OFFSET,
            "Position::range: offset too large"
        );
        let length = end_offset - start_offset;
        Self {
            // The 24-bit signed field keeps the low 24 bits, sign-extended.
            start_offset: (start_offset << 8) >> 8,
            // A length beyond the 8-bit field is capped at 255. Negative lengths cannot occur
            // (asserted above); they keep their low byte, as a bit-field does.
            // The cast keeps the low byte on purpose, as the bit-field store does.
            length: if length <= 0xFF { length as u8 } else { 0xFF },
        }
    }

    /// `valid`: whether this position names a source range.
    #[must_use]
    pub fn valid(&self) -> bool {
        self.start_offset != -1
    }

    /// `line(source)`: the 1-based line of the start offset, or -1 for an invalid position. The
    /// offset may equal the length of `source` (where the end-of-file token is).
    #[must_use]
    pub fn line(&self, source: &[u8]) -> i32 {
        if self.start_offset == -1 {
            return -1;
        }
        let offset = usize::try_from(self.start_offset)
            .unwrap_or(0)
            .min(source.len());
        // A plain count: this is for diagnostics, not a hot path.
        #[allow(clippy::naive_bytecount)]
        let newlines = source[..offset].iter().filter(|&&b| b == b'\n').count();
        i32::try_from(newlines).unwrap_or(i32::MAX) + 1
    }

    /// `startOffset()`: the first byte. Only valid for a valid position.
    #[must_use]
    pub fn start_offset(&self) -> i32 {
        debug_assert!(self.valid(), "startOffset() of an invalid position");
        self.start_offset
    }

    /// `endOffset()`: one past the last byte. Only valid for a valid position.
    #[must_use]
    pub fn end_offset(&self) -> i32 {
        debug_assert!(self.valid(), "endOffset() of an invalid position");
        self.start_offset + i32::from(self.length)
    }

    /// `rangeThrough(end)`: the range from this position through all of `end`. If either
    /// position is invalid, this one is returned unchanged.
    #[must_use]
    pub fn range_through(&self, end: Position) -> Self {
        if self.start_offset == -1 || end.start_offset == -1 {
            return *self;
        }
        debug_assert!(
            self.start_offset() <= end.start_offset() && self.end_offset() <= end.end_offset(),
            "Invalid range: ({}-{}) - ({}-{})",
            self.start_offset(),
            self.end_offset(),
            end.start_offset(),
            end.end_offset()
        );
        Self::range(self.start_offset(), end.end_offset())
    }

    /// `after()`: the single character immediately after this position.
    #[must_use]
    pub fn after(&self) -> Self {
        let end = self.end_offset();
        Self::range(end, end + 1)
    }

    /// `operator<` and the other ordering operators compare start offsets only.
    #[must_use]
    pub fn starts_before(&self, other: &Self) -> bool {
        self.start_offset < other.start_offset
    }
}

/// `SkSL::ForLoopPositions`: the positions of the three clauses of a `for` header.
#[doc(alias = "SkSL::ForLoopPositions")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ForLoopPositions {
    /// `initPosition`.
    pub init_position: Position,
    /// `conditionPosition`.
    pub condition_position: Position,
    /// `nextPosition`.
    pub next_position: Position,
}

#[cfg(test)]
mod tests {
    use super::Position;

    #[test]
    fn default_is_invalid() {
        let p = Position::default();
        assert!(!p.valid());
        assert_eq!(p.line(b"abc"), -1);
    }

    #[test]
    fn range_and_ends() {
        let p = Position::range(3, 7);
        assert!(p.valid());
        assert_eq!(p.start_offset(), 3);
        assert_eq!(p.end_offset(), 7);
        assert_eq!(p.after(), Position::range(7, 8));
        assert_eq!(
            p.range_through(Position::range(5, 9)),
            Position::range(3, 9)
        );
    }

    #[test]
    fn length_is_capped_at_one_byte() {
        let p = Position::range(0, 1000);
        assert_eq!(p.end_offset(), 255);
    }

    #[test]
    fn line_counts_newlines_before_the_offset() {
        let source = b"a\nbc\nd";
        assert_eq!(Position::range(0, 1).line(source), 1);
        assert_eq!(Position::range(2, 3).line(source), 2);
        assert_eq!(Position::range(6, 7).line(source), 3);
        // An offset at the end of the text is still a position.
        assert_eq!(Position::range(99, 100).line(source), 3);
    }
}
