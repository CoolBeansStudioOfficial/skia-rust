// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The `Sse41` tier's model twin (design §2.8): `model_sse2` with the SSE4.1 primitives
//! (`roundps` floor/ceil, saturating `packusdw`) and the raw estimates for `rcp_fast`/`rsqrt`.
//!
//! Instantiated once per estimate source, like [`model_sse2`](super::model_sse2):
//! [`host`] and [`amd_zen4`].

/// `model_sse41` with [`Estimates::Host`](crate::Estimates::Host).
pub mod host {
    /// The `Sse2` model with the same estimates, which this one extends.
    use crate::rp::lanes::model_sse2::host as base;

    #[path = "../imp.rs"]
    mod imp;
    pub use imp::*;
}

/// `model_sse41` with [`Estimates::AmdZen4`](crate::Estimates::AmdZen4).
pub mod amd_zen4 {
    /// The `Sse2` model with the same estimates, which this one extends.
    use crate::rp::lanes::model_sse2::amd_zen4 as base;

    // The same body as `host`, mounted again with another `base` (design §2.8).
    #[allow(clippy::duplicate_mod)]
    #[path = "../imp.rs"]
    mod imp;
    pub use imp::*;
}
