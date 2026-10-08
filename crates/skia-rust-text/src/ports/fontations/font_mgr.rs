// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file.
// Port of: src/ports/SkFontMgr_fontations_empty.cpp#L1-L83 (chrome/m156)

//! The empty Fontations font manager (`SkFontMgr_New_Fontations_Empty`): no families, and
//! typefaces made from data and streams. It is the test manager of the Fontations configuration
//! (docs/design/text.md §8).

use std::sync::Arc;

use skia_rust_core::data::Data;
use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_mgr::{FontMgr, FontMgrBase, FontStyleSet};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::stream::{MemoryStream, StreamAsset};
use skia_rust_core::typeface::Typeface;
use skia_rust_core::utf::Unichar;

use super::typeface::make_from_stream;

/// `SkFontMgr_Fontations_Empty`: a manager with no families, which makes typefaces from data.
// Port of: src/ports/SkFontMgr_fontations_empty.cpp#L19-L72 (chrome/m156)
#[derive(Debug, Default)]
struct FontMgrFontationsEmpty;

impl FontMgrBase for FontMgrFontationsEmpty {
    // Port of: src/ports/SkFontMgr_fontations_empty.cpp#L30 (chrome/m156)
    fn on_count_families(&self) -> usize {
        0
    }

    // Port of: src/ports/SkFontMgr_fontations_empty.cpp#L31 (chrome/m156)
    fn on_get_family_name(&self, _index: usize) -> String {
        String::new()
    }

    // Port of: src/ports/SkFontMgr_fontations_empty.cpp#L32-L34 (chrome/m156)
    fn on_create_style_set(&self, _index: usize) -> Option<FontStyleSet> {
        Some(FontStyleSet::create_empty())
    }

    // Port of: src/ports/SkFontMgr_fontations_empty.cpp#L35-L37 (chrome/m156)
    fn on_match_family(&self, _family_name: Option<&str>) -> Option<FontStyleSet> {
        Some(FontStyleSet::create_empty())
    }

    // Port of: src/ports/SkFontMgr_fontations_empty.cpp#L38-L41 (chrome/m156)
    fn on_match_family_style(
        &self,
        _family_name: Option<&str>,
        _style: &FontStyle,
    ) -> Option<Typeface> {
        None
    }

    // Port of: src/ports/SkFontMgr_fontations_empty.cpp#L42-L49 (chrome/m156)
    fn on_match_family_style_character(
        &self,
        _family_name: Option<&str>,
        _style: &FontStyle,
        _bcp47: &[&str],
        _character: Unichar,
    ) -> Option<Typeface> {
        None
    }

    // Port of: src/ports/SkFontMgr_fontations_empty.cpp#L50-L53 (chrome/m156)
    fn on_make_from_data(&self, data: &Data, tt_index: i32) -> Option<Typeface> {
        let stream: Box<dyn StreamAsset> = Box::new(MemoryStream::from_data(Some(data.clone())));
        make_from_stream(stream, &index_arguments(tt_index))
    }

    // Port of: src/ports/SkFontMgr_fontations_empty.cpp#L54-L57 (chrome/m156)
    fn on_make_from_stream_index(
        &self,
        stream: Box<dyn StreamAsset>,
        tt_index: i32,
    ) -> Option<Typeface> {
        make_from_stream(stream, &index_arguments(tt_index))
    }

    // Port of: src/ports/SkFontMgr_fontations_empty.cpp#L58-L60 (chrome/m156)
    fn on_make_from_stream_args(
        &self,
        stream: Box<dyn StreamAsset>,
        args: &FontArguments<'_, '_>,
    ) -> Option<Typeface> {
        make_from_stream(stream, args)
    }

    // Port of: src/ports/SkFontMgr_fontations_empty.cpp#L61-L65 (chrome/m156)
    fn on_make_from_file(&self, path: &str, tt_index: i32) -> Option<Typeface> {
        let stream = skia_rust_core::stream::make_from_file(path)?;
        make_from_stream(stream, &index_arguments(tt_index))
    }

    // Port of: src/ports/SkFontMgr_fontations_empty.cpp#L66-L68 (chrome/m156)
    fn on_legacy_make_typeface(
        &self,
        _family_name: Option<&str>,
        _style: FontStyle,
    ) -> Option<Typeface> {
        None
    }
}

/// `SkFontArguments().setCollectionIndex(ttcIndex)`. C++ stores the `int` index; the typeface
/// keeps its low 32 bits, so the same bits are kept here.
// Port of: src/ports/SkFontMgr_fontations_empty.cpp#L55-L56 (chrome/m156)
fn index_arguments(tt_index: i32) -> FontArguments<'static, 'static> {
    let mut args = FontArguments::new();
    #[allow(clippy::cast_sign_loss)] // the same bits as the C++ `int` index
    args.set_collection_index(tt_index as usize);
    args
}

/// `SkFontMgr_New_Fontations_Empty()`: the empty Fontations font manager.
// Port of: src/ports/SkFontMgr_fontations_empty.cpp#L76-L78 (chrome/m156)
#[must_use]
pub fn new_fontations_empty() -> FontMgr {
    FontMgr::new(Arc::new(FontMgrFontationsEmpty))
}
