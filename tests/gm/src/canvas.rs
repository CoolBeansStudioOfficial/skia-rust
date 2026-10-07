// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The drawing seam GMs are written against: the real raster `Canvas`/`Surface` (task D6 replaced
//! the stub this module used to define; GM ports did not change).

pub use skia_rust_core::blend_mode::BlendMode;
pub use skia_rust_core::canvas::Canvas;
pub use skia_rust_core::surface_props::{PixelGeometry, SurfaceProps, SurfacePropsFlags};
pub use skia_rust_raster::surface::Surface;
