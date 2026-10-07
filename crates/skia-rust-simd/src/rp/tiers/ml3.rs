// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The stages and interpreters of the `Ml3` tier: the shared stage sources stamped with
//! `rp::lanes::ml3` and the tier's target features (`Ml3Token::FEATURES`).

use crate::rp::lanes::ml3 as lanes;

/// The tier's target features on a function (the interpreter entry points).
macro_rules! tier_fn {
    ($($item:item)*) => { $( #[target_feature(enable = "sse2,ssse3,sse4.1,sse4.2,avx,avx2,bmi1,bmi2,f16c,fma")] $item )* };
}

/// Skia's `SI`: the tier's target features, inlined.
macro_rules! si {
    ($($item:item)*) => { $( #[target_feature(enable = "sse2,ssse3,sse4.1,sse4.2,avx,avx2,bmi1,bmi2,f16c,fma")] #[inline] $item )* };
}

#[allow(clippy::duplicate_mod)] // the same stage sources, stamped per tier (design §2.5)
#[path = "highp/mod.rs"]
pub(crate) mod highp;
#[allow(clippy::duplicate_mod)]
#[path = "lowp/mod.rs"]
pub(crate) mod lowp;
