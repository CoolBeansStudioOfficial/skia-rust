// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/TextureProxyView.h

//! `TextureProxyView`: a texture proxy with the swizzle and origin it is read with.

use std::sync::Arc;

use skia_rust_core::size::ISize;

use crate::gpu::gpu_types::{Mipmapped, Origin, Protected};
use crate::gpu::swizzle::Swizzle;
use crate::graphite::texture_proxy::TextureProxy;

/// A proxy plus its read swizzle and origin.
// Port of: src/gpu/graphite/TextureProxyView.h#L23-L101 (chrome/m156)
#[doc(alias = "skgpu::graphite::TextureProxyView")]
#[derive(Clone, Debug, Default)]
pub struct TextureProxyView {
    proxy: Option<Arc<TextureProxy>>,
    swizzle: Swizzle,
    origin: Origin,
}

impl PartialEq for TextureProxyView {
    // Port of: src/gpu/graphite/TextureProxyView.h#L45-L49 (chrome/m156)
    fn eq(&self, view: &Self) -> bool {
        let same_proxy = match (&self.proxy, &view.proxy) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        same_proxy && self.swizzle == view.swizzle && self.origin == view.origin
    }
}

impl TextureProxyView {
    /// `TextureProxyView(proxy, swizzle)`.
    // Port of: src/gpu/graphite/TextureProxyView.h#L27-L28 (chrome/m156)
    #[must_use]
    pub fn new(proxy: Option<Arc<TextureProxy>>, swizzle: Swizzle) -> Self {
        Self {
            proxy,
            swizzle,
            origin: Origin::TopLeft,
        }
    }

    /// `TextureProxyView(proxy, swizzle, origin)`.
    // Port of: src/gpu/graphite/TextureProxyView.h#L30-L31 (chrome/m156)
    #[must_use]
    pub fn new_with_origin(
        proxy: Option<Arc<TextureProxy>>,
        swizzle: Swizzle,
        origin: Origin,
    ) -> Self {
        Self {
            proxy,
            swizzle,
            origin,
        }
    }

    /// `TextureProxyView(proxy)`: RGBA swizzle, top-left origin.
    // Port of: src/gpu/graphite/TextureProxyView.h#L34-L35 (chrome/m156)
    #[must_use]
    pub fn from_proxy(proxy: Option<Arc<TextureProxy>>) -> Self {
        Self {
            proxy,
            ..Self::default()
        }
    }

    /// `operator bool`: true if the view has a proxy.
    // Port of: src/gpu/graphite/TextureProxyView.h#L40 (chrome/m156)
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.proxy.is_some()
    }

    fn proxy_ref(&self) -> &TextureProxy {
        self.proxy.as_deref().expect("TextureProxyView has a proxy")
    }

    /// `width()`.
    #[must_use]
    pub fn width(&self) -> i32 {
        self.proxy_ref().dimensions().width
    }

    /// `height()`.
    #[must_use]
    pub fn height(&self) -> i32 {
        self.proxy_ref().dimensions().height
    }

    /// `dimensions()`.
    #[must_use]
    pub fn dimensions(&self) -> ISize {
        self.proxy_ref().dimensions()
    }

    /// `isProtected()`.
    // Port of: src/gpu/graphite/TextureProxyView.h#L56-L58 (chrome/m156)
    #[must_use]
    pub fn is_protected(&self) -> Protected {
        self.proxy
            .as_ref()
            .map_or(Protected::No, |p| p.is_protected())
    }

    /// `mipmapped()`.
    // Port of: src/gpu/graphite/TextureProxyView.h#L59-L61 (chrome/m156)
    #[must_use]
    pub fn mipmapped(&self) -> Mipmapped {
        self.proxy.as_ref().map_or(Mipmapped::No, |p| p.mipmapped())
    }

    /// `proxy()`.
    #[must_use]
    pub fn proxy(&self) -> Option<&Arc<TextureProxy>> {
        self.proxy.as_ref()
    }

    /// `refProxy()`.
    #[doc(alias = "refProxy")]
    #[must_use]
    pub fn ref_proxy(&self) -> Option<Arc<TextureProxy>> {
        self.proxy.clone()
    }

    /// `swizzle()`.
    #[must_use]
    pub fn swizzle(&self) -> Swizzle {
        self.swizzle
    }

    /// `origin()`.
    #[must_use]
    pub fn origin(&self) -> Origin {
        self.origin
    }

    /// `concatSwizzle()`.
    // Port of: src/gpu/graphite/TextureProxyView.h#L69-L71 (chrome/m156)
    #[doc(alias = "concatSwizzle")]
    pub fn concat_swizzle(&mut self, swizzle: Swizzle) {
        self.swizzle = Swizzle::concat(&self.swizzle, &swizzle);
    }

    /// `makeSwizzle()`: a new view with `swizzle` composed onto this view's swizzle.
    // Port of: src/gpu/graphite/TextureProxyView.h#L74-L80 (chrome/m156)
    #[doc(alias = "makeSwizzle")]
    #[must_use]
    pub fn make_swizzle(&self, swizzle: Swizzle) -> Self {
        Self::new_with_origin(
            self.proxy.clone(),
            Swizzle::concat(&self.swizzle, &swizzle),
            self.origin,
        )
    }

    /// `replaceSwizzle()`: a new view that uses `swizzle` instead of this view's swizzle.
    // Port of: src/gpu/graphite/TextureProxyView.h#L83-L85 (chrome/m156)
    #[doc(alias = "replaceSwizzle")]
    #[must_use]
    pub fn replace_swizzle(&self, swizzle: Swizzle) -> Self {
        Self::new_with_origin(self.proxy.clone(), swizzle, self.origin)
    }

    /// `reset()`.
    // Port of: src/gpu/graphite/TextureProxyView.h#L87-L89 (chrome/m156)
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// `detachProxy()`: takes the proxy, keeping the swizzle and origin.
    // Port of: src/gpu/graphite/TextureProxyView.h#L93-L95 (chrome/m156)
    #[doc(alias = "detachProxy")]
    pub fn detach_proxy(&mut self) -> Option<Arc<TextureProxy>> {
        self.proxy.take()
    }
}
