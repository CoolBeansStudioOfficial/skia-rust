// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/TypefaceTest.cpp (chrome/m156), the cases that need only the typeface core

#![cfg(test)]

use skia_rust_core::font_arguments::variation_position::Coordinate;
use skia_rust_core::font_descriptor::FontDescriptor;
use skia_rust_core::font_style::{FontStyle, Slant, Weight, Width};
use skia_rust_core::stream::DynamicMemoryWStream;
use skia_rust_core::typeface::Typeface;
use skia_rust_core::typeface_cache::TypefaceCache;
use skia_rust_tools::font_tool_utils::{create_test_typeface, default_typeface};
use skia_rust_tools::fonts::test_empty_typeface::TestEmptyTypeface;

use crate::{Reporter, def_font_test, def_test, errorf, reporter_assert};

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

/// `SkTypeface::Equal(a, b)`: two nulls are equal, a null and a typeface are not.
// Port of: src/core/SkTypeface.cpp#L148-L156 (chrome/m156)
fn typeface_equal(a: Option<&Typeface>, b: Option<&Typeface>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

// Port of: tests/TypefaceTest.cpp#L502-L514 (chrome/m156)
def_font_test!(Typeface, |reporter| {
    let t1 = create_test_typeface(None, FontStyle::default());
    let t2 = default_typeface();

    reporter_assert!(reporter, typeface_equal(Some(&t1), Some(&t2)));
    reporter_assert!(reporter, typeface_equal(None, None));

    reporter_assert!(reporter, !typeface_equal(None, Some(&t1)));
    reporter_assert!(reporter, !typeface_equal(None, Some(&t2)));
    reporter_assert!(reporter, !typeface_equal(Some(&t1), None));
    reporter_assert!(reporter, !typeface_equal(Some(&t2), None));
});
