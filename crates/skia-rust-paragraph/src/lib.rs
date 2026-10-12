// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph (include/, src/, utils/)
//
// Milestones M8 (the font collections), M9 (the model) and M10 (layout and paint) of
// docs/design/modules.md §9.

// Skia's names (`SkParagraph`, `HarfBuzz`, `Skia`) are proper names, not code.
#![allow(clippy::doc_markdown)]
// The port keeps Skia's arithmetic and control flow function by function (CLAUDE.md rule 2), so
// the following lints, which ask to restructure it, are allowed crate-wide:
// - the casts mirror the implicit C++ conversions between `int`, `size_t`, `float` and `double`;
// - long functions, argument lists, bool fields, nesting and `if (!x) {...} else {...}` mirror the
//   C++ functions and structs they port;
// - `debug_assert!(a == b)` mirrors `SkASSERT(a == b)`, and the `# Panics` of indexing mirror the
//   C++ preconditions;
// - float comparisons, `(a + b) / 2` and the other arithmetic idioms must keep their exact value.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::collapsible_if,
    clippy::bool_to_int_with_if,
    clippy::explicit_counter_loop,
    clippy::field_reassign_with_default,
    clippy::float_cmp,
    clippy::if_not_else,
    clippy::if_same_then_else,
    clippy::items_after_statements,
    clippy::manual_assert_eq,
    clippy::manual_midpoint,
    clippy::missing_panics_doc,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::struct_field_names,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity
)]

//! The `modules/skparagraph` module of Skia, as `skia_safe::textlayout`: paragraphs of styled
//! text, shaped, broken into lines and painted.
//!
//! - [`ParagraphBuilder`] collects text and [`TextStyle`]s under a [`ParagraphStyle`] and builds a
//!   [`Paragraph`].
//! - [`FontCollection`] resolves the typefaces of the styles from font managers, with
//!   [`TypefaceFontProvider`] serving registered typefaces.
//! - [`Paragraph::layout`] shapes the text ([`one_line_shaper`]), breaks it into lines
//!   ([`text_wrapper`]) and aligns them ([`text_line`]); [`Paragraph::paint`] draws it.
//!
//! The types of the layout (`Run`, `Cluster`, `TextLine`, ...) are public because Skia's own tests
//! look into them; the names and the module layout follow Skia's files.

pub mod dart_types;
pub mod decorations;
pub mod font_arguments;
pub mod font_collection;
pub mod iterators;
pub mod metrics;
pub mod one_line_shaper;
pub mod paragraph_builder;
pub mod paragraph_cache;
pub mod paragraph_impl;
pub mod paragraph_painter;
pub mod paragraph_style;
pub mod run;
pub mod text_line;
pub mod text_shadow;
pub mod text_style;
pub mod text_wrapper;
pub mod typeface_font_provider;
pub mod utils;

pub use dart_types::{
    Affinity, EMPTY_INDEX, EMPTY_RANGE, LineMetricStyle, PositionWithAffinity, RangeExtensions,
    RectHeightStyle, RectWidthStyle, TextAlign, TextBaseline, TextBox, TextDirection,
    TextHeightBehavior,
};
pub use font_arguments::FontArguments;
pub use font_collection::FontCollection;
pub use metrics::{IndexedStyleMetrics, LineMetrics, StyleMetrics};
pub use paragraph_builder::{ParagraphBuilder, ParagraphBuilderImpl};
pub use paragraph_cache::ParagraphCache;
pub use paragraph_impl::{
    ExtendedVisitorInfo, FontInfo, GlyphClusterInfo, GlyphInfo, InternalState, Paragraph,
    ParagraphImpl, ResolvedFontDescriptor, VisitorFlags, VisitorInfo,
};
pub use paragraph_painter::{
    CanvasParagraphPainter, DashPathEffect, DecorationStyle, PaintId, PaintOrId, ParagraphPainter,
};
pub use paragraph_style::{ParagraphStyle, StrutStyle};
pub use run::{Cluster, InternalLineMetrics, Run};
pub use text_line::{ClipContext, TextLine};
pub use text_shadow::TextShadow;
pub use text_style::{
    ALL_TEXT_DECORATIONS, Block, BlockIndex, BlockRange, DEFAULT_FONT_FAMILY, Decoration,
    EMPTY_BLOCK, EMPTY_BLOCKS, EMPTY_TEXT, FontFeature, Placeholder, PlaceholderAlignment,
    PlaceholderStyle, StyleType, TextDecoration, TextDecorationMode, TextDecorationStyle,
    TextIndex, TextRange, TextStyle,
};
pub use typeface_font_provider::{TypefaceFontProvider, TypefaceFontStyleSet};
