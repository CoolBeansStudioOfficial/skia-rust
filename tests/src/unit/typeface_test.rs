// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/TypefaceTest.cpp (chrome/m156), the cases that need only the typeface core

#![cfg(test)]

use std::sync::Arc;

use skia_rust_core::descriptor::Descriptor;
use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_arguments::variation_position::Coordinate;
use skia_rust_core::font_descriptor::FontDescriptor;
use skia_rust_core::font_style::{FontStyle, Slant, Weight, Width};
use skia_rust_core::scaler_context::{ScalerContext, ScalerContextEffects, ScalerContextRec};
use skia_rust_core::stream::{DynamicMemoryWStream, StreamAsset};
use skia_rust_core::typeface::{Typeface, TypefaceBase, TypefaceCore};
use skia_rust_core::typeface_cache::TypefaceCache;

use crate::{Reporter, def_test, errorf, reporter_assert};

/// The test-only typeface with no glyphs and no data (`TestEmptyTypeface`).
///
/// Port of: tools/fonts/TestEmptyTypeface.h (chrome/m156). It lives here until the test tools
/// crate exists (docs/design/text.md T10), and then moves there.
// Port of: tools/fonts/TestEmptyTypeface.h#L16-L63 (chrome/m156)
#[derive(Debug)]
struct TestEmptyTypeface {
    core: TypefaceCore,
}

impl TestEmptyTypeface {
    /// `TestEmptyTypeface::Make()`: a new typeface, with a new unique id.
    // Port of: tools/fonts/TestEmptyTypeface.h#L18 (chrome/m156)
    fn make() -> Typeface {
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

/// `count_proc` and `count`: the number of typefaces the cache holds, counted with a search that
/// never matches.
// Port of: tests/TypefaceTest.cpp#L655-L665 (chrome/m156)
fn count(reporter: &mut Reporter, cache: &TypefaceCache) -> i32 {
    let mut count = 0;
    let none = cache.find_by_proc_and_ref(|_| {
        count += 1;
        false
    });
    reporter_assert!(reporter, none.is_none());
    count
}

// Port of: tests/TypefaceTest.cpp#L667-L686 (chrome/m156)
def_test!(TypefaceCache, |reporter| {
    let t1 = TestEmptyTypeface::make();
    {
        let mut cache = TypefaceCache::new();
        reporter_assert!(reporter, count(reporter, &cache) == 0);
        {
            let t0 = TestEmptyTypeface::make();
            cache.add(t0.clone());
            reporter_assert!(reporter, count(reporter, &cache) == 1);
            cache.add(t1.clone());
            reporter_assert!(reporter, count(reporter, &cache) == 2);
            cache.purge_all();
            reporter_assert!(reporter, count(reporter, &cache) == 2);
        }
        reporter_assert!(reporter, count(reporter, &cache) == 2);
        cache.purge_all();
        reporter_assert!(reporter, count(reporter, &cache) == 1);
    }
    reporter_assert!(reporter, t1.is_unique());
});

// Port of: tests/TypefaceTest.cpp#L309-L331 (chrome/m156)
// The C++ test compares the float with `==`; the value is exact, so the comparison is too.
def_test!(
    #[allow(clippy::float_cmp)]
    FontDescriptorNegativeVariationSerialize,
    |reporter| {
        let mut desc = FontDescriptor::new();
        let style = FontStyle::new(Weight::from(2), Width::from(9), Slant::Oblique);
        desc.set_style(style);
        let postscript_name = "postscript";
        desc.set_postscript_name(postscript_name);
        let variation = desc.set_variation_coordinates(1);
        variation[0] = Coordinate {
            axis: 0,
            value: -1.0,
        };

        let mut stream = DynamicMemoryWStream::new();
        reporter_assert!(reporter, desc.serialize(&mut stream));
        let mut read_back = stream.detach_as_stream();
        // C++ leaves the output descriptor default-constructed when Deserialize fails, and the
        // assertions below then fail; `unwrap_or_default` keeps that behavior.
        let desc_d = FontDescriptor::deserialize(read_back.as_mut(), None).unwrap_or_default();

        reporter_assert!(reporter, desc_d.style() == style);
        reporter_assert!(reporter, desc.postscript_name() == postscript_name);
        if desc_d.variation().len() == 1 {
            reporter_assert!(reporter, desc_d.variation()[0].value == -1.0);
        } else {
            errorf!(reporter, "descD.getVariationCoordinateCount() != 1");
        }
    }
);
