// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/fonts/TestEmptyTypeface.h (chrome/m156)

//! [`TestEmptyTypeface`]: a typeface with no glyphs and no data, unlike the shared empty
//! typeface, which tests use to check identity and caching.

use std::sync::Arc;

use skia_rust_core::descriptor::Descriptor;
use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_arguments::variation_position::Coordinate;
use skia_rust_core::font_descriptor::FontDescriptor;
use skia_rust_core::font_parameters::variation::Axis;
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::FourByteTag;
use skia_rust_core::scaler_context::{ScalerContext, ScalerContextEffects, ScalerContextRec};
use skia_rust_core::stream::StreamAsset;
use skia_rust_core::typeface::{
    LocalizedStrings, Typeface, TypefaceBase, TypefaceCore, VecLocalizedStrings,
};

/// `TestEmptyTypeface`: no glyphs, no data, a fixed pitch and the default style.
// Port of: tools/fonts/TestEmptyTypeface.h#L16-L63 (chrome/m156)
#[derive(Debug)]
pub struct TestEmptyTypeface {
    core: TypefaceCore,
}

impl TestEmptyTypeface {
    /// `TestEmptyTypeface::Make()`: a new typeface, with a new unique id.
    // Port of: tools/fonts/TestEmptyTypeface.h#L18 (chrome/m156)
    #[must_use]
    pub fn make() -> Typeface {
        Typeface::new(Arc::new(Self {
            core: TypefaceCore::new(FontStyle::default(), true),
        }))
    }
}

impl TypefaceBase for TestEmptyTypeface {
    fn core(&self) -> &TypefaceCore {
        &self.core
    }

    // Port of: tools/fonts/TestEmptyTypeface.h#L36 (chrome/m156)
    fn on_get_font_descriptor(&self) -> (FontDescriptor, bool) {
        (FontDescriptor::new(), false)
    }

    // Port of: tools/fonts/TestEmptyTypeface.h#L23 (chrome/m156)
    fn on_open_stream(&self) -> Option<(Box<dyn StreamAsset>, i32)> {
        None
    }

    // Port of: tools/fonts/TestEmptyTypeface.h#L48 (chrome/m156)
    fn on_get_family_name(&self) -> String {
        String::new()
    }

    // Port of: tools/fonts/TestEmptyTypeface.h#L54-L62 (chrome/m156)
    fn on_get_variation_design_position(&self) -> Option<Vec<Coordinate>> {
        Some(Vec::new())
    }

    // Port of: tools/fonts/TestEmptyTypeface.h#L58-L60 (chrome/m156)
    fn on_get_variation_design_parameters(&self) -> Option<Vec<Axis>> {
        Some(Vec::new())
    }

    // Port of: tools/fonts/TestEmptyTypeface.h#L43 (chrome/m156)
    fn on_get_upem(&self) -> i32 {
        0
    }

    // Port of: tools/fonts/TestEmptyTypeface.h#L49 (chrome/m156)
    fn on_get_postscript_name(&self) -> Option<String> {
        None
    }

    // Port of: tools/fonts/TestEmptyTypeface.h#L44-L47 (chrome/m156)
    fn on_create_family_name_iterator(&self) -> Box<dyn LocalizedStrings> {
        Box::new(VecLocalizedStrings::default())
    }

    // Port of: tools/fonts/TestEmptyTypeface.h#L61 (chrome/m156)
    fn on_get_table_tags(&self) -> Vec<FourByteTag> {
        Vec::new()
    }

    // Port of: tools/fonts/TestEmptyTypeface.h#L62 (chrome/m156)
    fn on_get_table_data(
        &self,
        _tag: FourByteTag,
        _offset: usize,
        _length: usize,
        _data: &mut [u8],
    ) -> usize {
        0
    }

    // Port of: tools/fonts/TestEmptyTypeface.h#L24-L26 (chrome/m156)
    fn on_make_clone(&self, this: Typeface, _args: &FontArguments<'_, '_>) -> Typeface {
        this
    }

    // Port of: tools/fonts/TestEmptyTypeface.h#L27-L31 (chrome/m156)
    fn on_create_scaler_context(
        &self,
        this: Typeface,
        effects: &ScalerContextEffects,
        desc: &Descriptor,
    ) -> ScalerContext {
        ScalerContext::make_empty(this, effects, desc)
    }

    // Port of: tools/fonts/TestEmptyTypeface.h#L32 (chrome/m156)
    fn on_filter_rec(&self, _rec: &mut ScalerContextRec) {}

    // Port of: tools/fonts/TestEmptyTypeface.h#L53 (chrome/m156)
    fn on_glyph_mask_needs_current_color(&self) -> bool {
        false
    }
}
