// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skunicode/include/SkUnicode.h, modules/skunicode/src/
// SkUnicode.cpp, SkUnicode_hardcoded.{h,cpp}, SkUnicode_client.cpp.
//
// Milestone M1 of docs/design/modules.md, and the bidi engine of M3 (`icu_bidi`, from
// modules/skunicode/src/SkUnicode_icu_bidi.cpp, SkBidiFactory_icu_full.cpp and ICU ubidiln.cpp).
// Milestone M2: the ICU shim (`icu_shim`, the SKICU_EMIT_FUNCS functions on ICU4X) and the ICU
// implementation (`unicode_icu`, from modules/skunicode/src/SkUnicode_icu.cpp). The Chromium break
// data (M4) is not here yet.

//! `SkUnicode`: the Unicode services Skia's text stack uses (code-unit flags, break and bidi
//! iterators, UTF conversions), and its implementations.
//!
//! - [`unicode::Unicode`] is the abstract `SkUnicode` class.
//! - [`unicode_client::make`] is `SkUnicodes::Client::Make`: the client supplies words, grapheme
//!   and line breaks itself.
//! - [`unicode_icu::make`] is `SkUnicodes::ICU::Make`, over [`icu_shim`] and [`icu_bidi`].
//! - The hardcoded character properties are the default methods of [`unicode::Unicode`].

pub mod icu_bidi;
pub mod icu_shim;
pub mod unicode;
pub mod unicode_client;
pub(crate) mod unicode_hardcoded;
pub mod unicode_icu;
