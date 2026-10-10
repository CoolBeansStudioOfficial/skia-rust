// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/layers (chrome/m156)
//
// The layer types: audio, footage (image and video), null, precomp, shape, solid. The text layer
// is M22; `text_layer` is its boundary.

pub mod audio_layer;
pub mod footage_layer;
pub mod null_layer;
pub mod precomp_layer;
pub mod shapelayer;
pub mod solid_layer;
pub mod text_layer;
