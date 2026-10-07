// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The `Ml4` tier's model twin (design §2.8): the same names as `ml4`, implemented per lane in
//! plain Rust with the AVX-512/FMA/F16C instructions' exact semantics
//! ([`x86_model`](super::x86_model)), no target features. Runs on any host and under Miri
//! (except the estimate primitives with [`Estimates::Host`](crate::Estimates::Host)).
//!
//! The model is instantiated once per estimate source, as a submodule:
//!
//! - [`host`]: `vrcp14ps`/`vrsqrt14ps` executed on this host (AVX-512 hosts only).
//! - [`amd_zen4`]: the oracle host's `rcp14`/`rsqrt14` (`estimates::amd_zen4`, Intel's
//!   reference algorithm), on any host. This is what lets hosts without AVX-512 check `Ml4`.

pub mod host;

pub mod amd_zen4;
