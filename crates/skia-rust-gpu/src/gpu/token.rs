// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/Token.h

//! `skgpu::Token` and `skgpu::TokenTracker`: sequence numbers that order draws and flushes.

// Port of: src/gpu/Token.h#L24-L63 (chrome/m156)
/// A token: a sequence number that orders work in the order it was issued.
#[doc(alias = "skgpu::Token")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Token {
    sequence_number: u64,
}

impl Token {
    // Port of: src/gpu/Token.h#L26 (chrome/m156)
    /// `Token::InvalidToken()`: the token before any work has been issued.
    #[must_use]
    pub const fn invalid_token() -> Token {
        Token { sequence_number: 0 }
    }

    // Port of: src/gpu/Token.h#L59 (chrome/m156)
    // Private constructor, visible to the tracker (and, through it, to the recorder).
    const fn from_sequence_number(sequence_number: u64) -> Token {
        Token { sequence_number }
    }

    // Port of: src/gpu/Token.h#L38-L41 (chrome/m156)
    /// `operator++()`: advances this token in place.
    #[doc(alias = "operator++")]
    pub fn increment(&mut self) {
        self.sequence_number += 1;
    }

    // Port of: src/gpu/Token.h#L42-L47 (chrome/m156)
    /// `operator++(int)`: returns the token before the increment, advancing this one.
    #[doc(alias = "operator++(int)")]
    #[must_use]
    pub fn post_increment(&mut self) -> Token {
        let old = self.sequence_number;
        self.sequence_number += 1;
        Token::from_sequence_number(old)
    }

    // Port of: src/gpu/Token.h#L48 (chrome/m156)
    /// `next()`: the token after this one.
    #[must_use]
    pub const fn next(&self) -> Token {
        Token::from_sequence_number(self.sequence_number + 1)
    }

    // Port of: src/gpu/Token.h#L51 (chrome/m156)
    /// `value()`: the raw value, for debugging and comparison.
    #[must_use]
    pub const fn value(&self) -> u64 {
        self.sequence_number
    }

    // Port of: src/gpu/Token.h#L54-L57 (chrome/m156)
    /// `inInterval(start, end)`: is this token in the `[start, end]` inclusive interval?
    #[must_use]
    pub fn in_interval(&self, start: &Token, end: &Token) -> bool {
        *self >= *start && *self <= *end
    }
}

// Port of: src/gpu/Token.h#L68-L98 (chrome/m156)
/// Issues draw and flush tokens. Only the recorder and the flush code advance the counters.
#[doc(alias = "skgpu::TokenTracker")]
#[derive(Clone, Copy, Debug)]
pub struct TokenTracker {
    current_draw_token: Token,
    current_flush_token: Token,
}

impl Default for TokenTracker {
    // Port of: src/gpu/Token.h#L96 (chrome/m156)
    /// Both counters start at `Token::InvalidToken()`.
    fn default() -> Self {
        TokenTracker {
            current_draw_token: Token::invalid_token(),
            current_flush_token: Token::invalid_token(),
        }
    }
}

impl TokenTracker {
    // Port of: src/gpu/Token.h#L74 (chrome/m156)
    /// Gets the token one beyond the last token that has been flushed. This represents the ID
    /// for the *current* batch of work being recorded.
    #[must_use]
    pub const fn next_flush_token(&self) -> Token {
        self.current_flush_token.next()
    }

    // Port of: src/gpu/Token.h#L80 (chrome/m156)
    /// Gets the token that was *just issued*. This represents the ID of the flush that was most
    /// recently completed.
    #[must_use]
    pub const fn current_flush_token(&self) -> Token {
        self.current_flush_token
    }

    // Port of: src/gpu/Token.h#L85 (chrome/m156)
    /// Gets the next draw token.
    #[must_use]
    pub const fn next_draw_token(&self) -> Token {
        self.current_draw_token.next()
    }

    // Port of: src/gpu/Token.h#L93 (chrome/m156)
    /// `issueDrawToken()`: only the recorder advances the draw counter.
    #[allow(dead_code)] // used by the Graphite recorder, which is not ported yet
    pub(crate) fn issue_draw_token(&mut self) -> Token {
        self.current_draw_token.increment();
        self.current_draw_token
    }

    // Port of: src/gpu/Token.h#L94 (chrome/m156)
    /// `issueFlushToken()`: only the flush code advances the flush counter.
    #[allow(dead_code)] // used by the flush code, which is not ported yet
    pub(crate) fn issue_flush_token(&mut self) -> Token {
        self.current_flush_token.increment();
        self.current_flush_token
    }
}
