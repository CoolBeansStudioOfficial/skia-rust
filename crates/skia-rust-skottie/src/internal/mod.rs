// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src (chrome/m156), the `skottie::internal` namespace.
//
// The animation builder and everything it builds: animators and keyframes, composition and layer
// builders, transforms, the camera, and the layer types. Public so that the tests of Skia's
// `skottie::internal` classes can be ported.

pub mod animator;
pub mod blend_modes;
pub mod camera;
pub mod composition;
pub mod effects;
pub mod layer;
pub mod layers;
pub mod path;
pub mod skottie_priv;
pub mod transform;
