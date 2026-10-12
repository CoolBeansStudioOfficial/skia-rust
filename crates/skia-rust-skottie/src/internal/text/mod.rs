// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/text (chrome/m156)
//
// Text layers: the custom (embedded glyph) fonts, the text value parser, the range selectors and
// text animators, the adapter that builds the scene graph of a text layer, and the Unicode glue.
// The shaper they use is [`crate::text_shaper`] (`modules/skottie/include/TextShaper.h`).

pub mod font;
pub mod range_selector;
pub mod text_adapter;
pub mod text_animator;
pub mod text_value;
pub mod unicode;
