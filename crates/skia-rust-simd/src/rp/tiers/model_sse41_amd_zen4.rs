// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The `Sse41` tier's model twin with the oracle host's (`AmdZen4`) estimates (design §2.8): the shared stage sources
//! stamped with `rp::lanes::model_sse41::amd_zen4`, without target features.

use crate::rp::lanes::model_sse41::amd_zen4 as lanes;

/// No target features (a model runs on any host).
macro_rules! tier_fn {
    ($($item:item)*) => { $( $item )* };
}

/// Skia's `SI`, without target features.
macro_rules! si {
    ($($item:item)*) => { $( #[inline] $item )* };
}

#[allow(clippy::duplicate_mod)] // the same stage sources, stamped per tier (design §2.5)
#[path = "highp/mod.rs"]
pub(crate) mod highp;
#[allow(clippy::duplicate_mod)]
#[path = "lowp/mod.rs"]
pub(crate) mod lowp;
