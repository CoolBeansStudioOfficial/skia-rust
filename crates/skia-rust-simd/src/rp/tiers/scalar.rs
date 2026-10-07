// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The stages and interpreter of the `Scalar` tier (`SKRP_CPU_SCALAR`): the shared highp stage
//! sources stamped with `rp::lanes::scalar`, without target features. Scalar has no lowp
//! pipeline (Skia's lowp stages are null there), and it is its own model.

use crate::rp::lanes::scalar as lanes;

/// No target features.
macro_rules! tier_fn {
    ($($item:item)*) => { $( $item )* };
}

/// Skia's `SI`.
macro_rules! si {
    ($($item:item)*) => { $( #[inline] $item )* };
}

#[allow(clippy::duplicate_mod)] // the same stage sources, stamped per tier (design §2.5)
#[path = "highp/mod.rs"]
pub(crate) mod highp;
