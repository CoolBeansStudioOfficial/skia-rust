// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/include/ExternalLayer.h (chrome/m156)

use std::rc::Rc;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::size::Size;

/// Content that an animation renders in place of a precomp layer (`ExternalLayer`).
// Port of: modules/skottie/include/ExternalLayer.h#L16-L26 (chrome/m156) (`class ExternalLayer`)
#[doc(alias = "skottie::ExternalLayer")]
pub trait ExternalLayer {
    /// Renders the layer content into the given canvas. `t` is the time in seconds, relative to
    /// the layer in-point (start time) (`render`).
    fn render(&self, canvas: &Canvas, t: f64);
}

/// Substitutes precomp layers with custom/externally managed content (`PrecompInterceptor`).
// Port of: modules/skottie/include/ExternalLayer.h#L28-L45 (chrome/m156) (`class PrecompInterceptor`)
#[doc(alias = "skottie::PrecompInterceptor")]
pub trait PrecompInterceptor {
    /// Invoked at animation build time, for each precomp layer.
    ///
    /// `id` is the target composition ID (usually assigned automatically by BM: `comp_0`, ...),
    /// `name` the name of the precomp layer (by default it matches the target comp name, but can
    /// be changed in AE) and `size` the Lottie-specified precomp layer size. Returns an
    /// `ExternalLayer` implementation (to be used instead of the actual Lottie file content), or
    /// `None` (to use the Lottie file content).
    #[doc(alias = "onLoadPrecomp")]
    fn on_load_precomp(&self, id: &str, name: &str, size: Size) -> Option<Rc<dyn ExternalLayer>>;
}
