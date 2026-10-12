// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/include/ParagraphBuilder.h,
// modules/skparagraph/src/ParagraphBuilderImpl.h, modules/skparagraph/src/ParagraphBuilderImpl.cpp

//! `ParagraphBuilder`: builds a [`Paragraph`] from styles and text.
//!
//! The client-supplied Unicode data of `SK_UNICODE_CLIENT_IMPLEMENTATION` builds
//! (`setWordsUtf8` and the like) is not part of this port: the default builds use ICU.

use std::sync::Arc;

use skia_rust_unicode::unicode::{Unicode, convert_utf16_to_utf8};

use crate::font_collection::FontCollection;
use crate::paragraph_impl::Paragraph;
use crate::paragraph_style::ParagraphStyle;
use crate::text_style::{Block, Placeholder, PlaceholderStyle, TextIndex, TextStyle};

// Port of: modules/skparagraph/src/ParagraphBuilderImpl.h#L21-L103 (chrome/m156)
/// `ParagraphBuilder` (and `ParagraphBuilderImpl`, its only implementation).
#[doc(alias = "ParagraphBuilderImpl")]
pub struct ParagraphBuilder {
    utf8: Vec<u8>,
    text_styles: Vec<TextStyle>,
    styled_blocks: Vec<Block>,
    placeholders: Vec<Placeholder>,
    font_collection: FontCollection,
    paragraph_style: ParagraphStyle,
    unicode: Arc<dyn Unicode>,
}

impl std::fmt::Debug for ParagraphBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParagraphBuilder")
            .field("text", &String::from_utf8_lossy(&self.utf8))
            .finish_non_exhaustive()
    }
}

/// `ParagraphBuilderImpl`, the implementation of `ParagraphBuilder`: the same type.
pub type ParagraphBuilderImpl = ParagraphBuilder;

impl ParagraphBuilder {
    // Port of: modules/skparagraph/src/ParagraphBuilderImpl.cpp#L22-L47 (chrome/m156)
    /// `ParagraphBuilder::make(style, fontCollection, unicode)` and
    /// `ParagraphBuilderImpl(style, fontCollection, unicode)`.
    #[must_use]
    pub fn new(
        style: &ParagraphStyle,
        font_collection: impl Into<FontCollection>,
        unicode: Arc<dyn Unicode>,
    ) -> Self {
        let mut builder = Self {
            utf8: Vec::new(),
            text_styles: Vec::new(),
            styled_blocks: Vec::new(),
            placeholders: Vec::new(),
            font_collection: font_collection.into(),
            paragraph_style: style.clone(),
            unicode,
        };
        builder.start_styled_block();
        builder
    }

    /// `ParagraphBuilder::make(style, fontCollection, unicode)`.
    #[must_use]
    pub fn make(
        style: &ParagraphStyle,
        font_collection: impl Into<FontCollection>,
        unicode: Arc<dyn Unicode>,
    ) -> Self {
        Self::new(style, font_collection, unicode)
    }

    // Port of: modules/skparagraph/src/ParagraphBuilderImpl.cpp#L51-L62 (chrome/m156)
    /// Push a style to the stack. The corresponding text added with AddText will use the
    /// top-most style.
    #[doc(alias = "pushStyle")]
    pub fn push_style(&mut self, style: &TextStyle) -> &mut Self {
        self.text_styles.push(style.clone());
        if !self.styled_blocks.is_empty()
            && self.styled_blocks.last().expect("a block").range.end == self.utf8.len()
            && self.styled_blocks.last().expect("a block").style == *style
        {
            // Just continue with the same style
        } else {
            // Go with the new style
            self.start_styled_block();
        }
        self
    }

    // Port of: modules/skparagraph/src/ParagraphBuilderImpl.cpp#L64-L72 (chrome/m156)
    /// Remove a style from the stack. Useful to apply different styles to chunks of text such as
    /// bolding.
    ///
    /// Example:
    ///   builder.push_style(normal_style);
    ///   builder.add_text("Hello this is normal. ");
    ///
    ///   builder.push_style(bold_style);
    ///   builder.add_text("And this is BOLD. ");
    ///
    ///   builder.pop();
    ///   builder.add_text(" Back to normal again.");
    pub fn pop(&mut self) -> &mut Self {
        if !self.text_styles.is_empty() {
            self.text_styles.pop();
        } else {
            // In this case we use paragraph style and skip Pop operation
            // `SkDEBUGF("SkParagraphBuilder.Pop() called too many times.\n")`
        }

        self.start_styled_block();
        self
    }

    // Port of: modules/skparagraph/src/ParagraphBuilderImpl.cpp#L74-L80 (chrome/m156)
    fn internal_peek_style(&self) -> &TextStyle {
        match self.text_styles.last() {
            None => self.paragraph_style.text_style(),
            Some(style) => style,
        }
    }

    // Port of: modules/skparagraph/src/ParagraphBuilderImpl.cpp#L82-L84 (chrome/m156)
    /// `peekStyle()`.
    #[must_use]
    #[doc(alias = "peekStyle")]
    pub fn peek_style(&mut self) -> TextStyle {
        self.internal_peek_style().clone()
    }

    // Port of: modules/skparagraph/src/ParagraphBuilderImpl.cpp#L86-L99 (chrome/m156)
    /// `addText(const std::u16string& text)`: adds UTF16-encoded text to the builder. Forms the
    /// proper runs to use the upper-most style on the style_stack.
    #[doc(alias = "addText")]
    pub fn add_text_utf16(&mut self, text: &[u16]) -> &mut Self {
        let utf8 = convert_utf16_to_utf8(text);
        self.utf8.extend_from_slice(utf8.as_bytes());
        self
    }

    /// `addText(const char* text)` and `addText(const char* text, size_t len)`: adds UTF8-encoded
    /// text to the builder, using the top-most style on the style_stack.
    #[doc(alias = "addText")]
    pub fn add_text(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.utf8.extend_from_slice(text.as_ref().as_bytes());
        self
    }

    /// `addText(const char* text, size_t len)` for bytes that need not be valid UTF-8.
    #[doc(alias = "addText")]
    pub fn add_text_bytes(&mut self, text: &[u8]) -> &mut Self {
        self.utf8.extend_from_slice(text);
        self
    }

    // Port of: modules/skparagraph/src/ParagraphBuilderImpl.cpp#L101-L116 (chrome/m156)
    /// Pushes the information required to leave an open space, where Flutter may draw a custom
    /// placeholder into. Internally, this method adds a single object replacement character
    /// (0xFFFC).
    #[doc(alias = "addPlaceholder")]
    pub fn add_placeholder(&mut self, placeholder_style: &PlaceholderStyle) -> &mut Self {
        self.add_placeholder_last(placeholder_style, false);
        self
    }

    // Port of: modules/skparagraph/src/ParagraphBuilderImpl.cpp#L118-L146 (chrome/m156)
    fn add_placeholder_last(&mut self, placeholder_style: &PlaceholderStyle, last_one: bool) {
        if !self.utf8.is_empty() && !last_one {
            // We keep the very last text style
            self.end_run_if_needed();
        }

        let styles_before: std::ops::Range<usize> = (if self.placeholders.is_empty() {
            0
        } else {
            self.placeholders
                .last()
                .expect("a placeholder")
                .blocks_before
                .end
                + 1
        })..self.styled_blocks.len();
        let text_before: std::ops::Range<usize> = (if self.placeholders.is_empty() {
            0
        } else {
            self.placeholders.last().expect("a placeholder").range.end
        })..self.utf8.len();

        let start = self.utf8.len();
        let top_style = self.internal_peek_style().clone();
        if !last_one {
            let placeholder_text_style = top_style.clone_for_placeholder();
            self.push_style(&placeholder_text_style);
            self.add_text_utf16(&[0xFFFC]);
            self.pop();
        }
        let end = self.utf8.len();
        self.placeholders.push(Placeholder::new(
            start..end,
            *placeholder_style,
            top_style,
            styles_before,
            text_before,
        ));
    }

    // Port of: modules/skparagraph/src/ParagraphBuilderImpl.cpp#L148-L158 (chrome/m156)
    fn end_run_if_needed(&mut self) {
        let Some(last) = self.styled_blocks.last_mut() else {
            return;
        };

        if last.range.start == self.utf8.len() {
            self.styled_blocks.pop();
        } else {
            last.range.end = self.utf8.len();
        }
    }

    // Port of: modules/skparagraph/src/ParagraphBuilderImpl.cpp#L160-L163 (chrome/m156)
    fn start_styled_block(&mut self) {
        self.end_run_if_needed();
        let style = self.internal_peek_style().clone();
        self.styled_blocks
            .push(Block::new(self.utf8.len()..self.utf8.len(), style));
    }

    // Port of: modules/skparagraph/src/ParagraphBuilderImpl.cpp#L165-L177 (chrome/m156)
    fn finalize(&mut self) {
        if !self.utf8.is_empty() {
            self.end_run_if_needed();
        }
    }

    // Port of: modules/skparagraph/src/ParagraphBuilderImpl.cpp#L179-L192 (chrome/m156)
    /// `Build()`: constructs a paragraph that can be used to layout and paint the text.
    #[doc(alias = "Build")]
    pub fn build(&mut self) -> Paragraph {
        self.finalize();
        // Add one fake placeholder with the rest of the text
        self.add_placeholder_last(&PlaceholderStyle::default(), true);
        Paragraph::new(
            self.utf8.clone(),
            self.paragraph_style.clone(),
            self.styled_blocks.clone(),
            self.placeholders.clone(),
            self.font_collection.clone(),
            self.unicode.clone(),
        )
    }

    // Port of: modules/skparagraph/src/ParagraphBuilderImpl.cpp#L194-L197 (chrome/m156)
    /// `getText()`: the text of the paragraph so far.
    #[doc(alias = "getText")]
    pub fn get_text(&mut self) -> &[u8] {
        self.finalize();
        &self.utf8
    }

    // Port of: modules/skparagraph/src/ParagraphBuilderImpl.cpp#L199-L201 (chrome/m156)
    /// `getParagraphStyle()`.
    #[must_use]
    #[doc(alias = "getParagraphStyle")]
    pub fn get_paragraph_style(&self) -> ParagraphStyle {
        self.paragraph_style.clone()
    }

    // Port of: modules/skparagraph/src/ParagraphBuilderImpl.cpp#L252-L263 (chrome/m156)
    /// `Reset()`: resets this builder to its initial state, discarding any text, styles,
    /// placeholders that have been added, but keeping the initial ParagraphStyle.
    #[doc(alias = "Reset")]
    pub fn reset(&mut self) {
        self.text_styles.clear();
        self.utf8.clear();
        self.styled_blocks.clear();
        self.placeholders.clear();

        self.start_styled_block();
    }

    /// `RequiresClientICU()`: false, the client Unicode is not part of this port.
    #[must_use]
    #[doc(alias = "RequiresClientICU")]
    pub fn requires_client_icu() -> bool {
        false
    }
}

/// The text index type of the paragraph API.
#[allow(dead_code)]
type _TextIndex = TextIndex;
