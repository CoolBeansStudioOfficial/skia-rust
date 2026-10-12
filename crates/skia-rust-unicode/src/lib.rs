// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skunicode/include/SkUnicode.h, modules/skunicode/src/
// SkUnicode.cpp, SkUnicode_hardcoded.{h,cpp}, SkUnicode_client.cpp.
//
// Milestone M1 of docs/design/modules.md, and the bidi engine of M3 (`icu_bidi`, from
// modules/skunicode/src/SkUnicode_icu_bidi.cpp, SkBidiFactory_icu_full.cpp and ICU ubidiln.cpp).
// The ICU-backed implementation (M2) and the Chromium break data (M4) are not here yet.

//! `SkUnicode`: the Unicode services Skia's text stack uses (code-unit flags, break and bidi
//! iterators, UTF conversions), and the two implementations that need no Unicode data.
//!
//! - [`unicode::Unicode`] is the abstract `SkUnicode` class.
//! - [`unicode_client::make`] is `SkUnicodes::Client::Make`: the client supplies words, grapheme
//!   and line breaks itself.
//! - The hardcoded character properties are the default methods of [`unicode::Unicode`].

pub mod icu_bidi;
pub mod unicode;
pub mod unicode_client;
pub(crate) mod unicode_hardcoded;
