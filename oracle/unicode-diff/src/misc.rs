// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The `misc` section: `u_errorName` over the codes -128..=40, `uloc_forLanguageTag` over the tags of
//! the driver, and `uloc_getDefault`, through `skia_rust_unicode::icu_shim` (M2).

use std::fmt::Write as _;

use skia_rust_unicode::icu_shim::{u_error_name, uloc_for_language_tag, uloc_get_default};

/// The tags of `run_misc` in `cpp/unicode_diff.cc`.
const TAGS: [&str; 24] = [
    "en",
    "en-US",
    "en-us",
    "EN-us",
    "tr",
    "zh-Hans-CN",
    "zh-Hant-TW",
    "ja-JP",
    "und",
    "ru",
    "ar-EG",
    "sr-Latn-RS",
    "de-DE-u-co-phonebk",
    "en-u-lb-strict",
    "x-private",
    "",
    "!!",
    "en_US",
    "th-TH-u-nu-thai",
    "pt-BR",
    "he",
    "fa-IR",
    "hi-IN",
    "ko-KR",
];

/// Prints the section.
#[must_use]
pub fn run() -> String {
    let mut out = String::new();
    for code in -128..=40 {
        let _ = writeln!(out, "u_errorName {code} {}", u_error_name(code));
    }
    for tag in TAGS {
        let r = uloc_for_language_tag(tag);
        // The shim's conversion does not fail: the status is always `U_ZERO_ERROR`, as ICU's for
        // these tags.
        let _ = writeln!(
            out,
            "uloc_forLanguageTag \"{tag}\" -> \"{}\" len={} parsed={} status=U_ZERO_ERROR",
            r.locale_id,
            r.locale_id.len(),
            r.parsed_length
        );
    }
    let _ = writeln!(out, "uloc_getDefault {}", uloc_get_default());
    out
}
