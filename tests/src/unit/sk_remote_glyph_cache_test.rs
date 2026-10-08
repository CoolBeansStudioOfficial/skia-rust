// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkRemoteGlyphCacheTest.cpp (chrome/m156), the `SkGraphics_Limits` test only;
// the other tests in the file need the remote glyph cache (T23).

#![cfg(test)]

use skia_rust_core::graphics::{set_typeface_cache_count_limit, typeface_cache_count_limit};

use crate::{def_test, reporter_assert};

// Port of: tests/SkRemoteGlyphCacheTest.cpp#L1403-L1412 (chrome/m156)
def_test!(SkGraphics_Limits, |reporter| {
    let prev1 = typeface_cache_count_limit();

    let mut prev2 = set_typeface_cache_count_limit(prev1 + 1);
    reporter_assert!(reporter, prev1 == prev2);
    prev2 = typeface_cache_count_limit();
    reporter_assert!(reporter, prev2 == prev1 + 1);

    let _ = set_typeface_cache_count_limit(prev1); // restore orig
});
