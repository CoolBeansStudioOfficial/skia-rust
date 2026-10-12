// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/src/Iterators.h

//! `LangIterator`: the language run iterator of the text styles of a paragraph.

use skia_rust_shaper::run_iterator::{LanguageRunIterator, RunIterator};

use crate::text_style::{Block, TextStyle};

// Port of: modules/skparagraph/src/Iterators.h#L18-L56 (chrome/m156)
/// `LangIterator`: runs of text of one locale, taken from the blocks of a paragraph.
///
/// Positions are byte offsets from the start of the text span the iterator is given. The ranges
/// of the blocks are added to that start as they are (they are offsets in the whole paragraph
/// text), exactly as `fText.data() + fCurrentStyle->fRange.end` does in C++.
#[derive(Debug)]
pub struct LangIterator {
    text_len: usize,
    text_styles: Vec<Block>,
    current_char: usize,
    current_style: usize,
    current_locale: String,
}

impl LangIterator {
    /// `LangIterator(utf8, styles, defaultStyle)`: `text_len` is the size of the text.
    #[must_use]
    pub fn new(text_len: usize, styles: &[Block], default_style: &TextStyle) -> Self {
        Self {
            text_len,
            text_styles: styles.to_vec(),
            current_char: 0,
            current_style: 0,
            current_locale: default_style.locale().to_owned(),
        }
    }
}

impl RunIterator for LangIterator {
    // Port of: modules/skparagraph/src/Iterators.h#L27-L43 (chrome/m156)
    fn consume(&mut self) {
        let text_end = self.text_len;
        let styles_end = self.text_styles.len();
        debug_assert!(self.current_char < text_end);
        if self.current_style == styles_end {
            self.current_char = text_end;
            return;
        }
        self.current_char = self.text_styles[self.current_style].range.end;
        self.current_locale = self.text_styles[self.current_style]
            .style
            .locale()
            .to_owned();
        self.current_style += 1;
        while self.current_style != styles_end
            && !self.text_styles[self.current_style].style.is_placeholder()
        {
            if self.text_styles[self.current_style].style.locale() != self.current_locale {
                break;
            }
            self.current_char = self.text_styles[self.current_style].range.end;
            self.current_style += 1;
        }
    }

    // Port of: modules/skparagraph/src/Iterators.h#L44 (chrome/m156)
    fn end_of_current_run(&self) -> usize {
        self.current_char
    }

    // Port of: modules/skparagraph/src/Iterators.h#L45 (chrome/m156)
    fn at_end(&self) -> bool {
        self.current_char >= self.text_len
    }
}

impl LanguageRunIterator for LangIterator {
    // Port of: modules/skparagraph/src/Iterators.h#L46 (chrome/m156)
    fn current_language(&self) -> &str {
        &self.current_locale
    }
}
