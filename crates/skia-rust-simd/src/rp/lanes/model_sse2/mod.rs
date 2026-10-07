// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The `Sse2` tier's model twin (design §2.8): the same names as `sse2`, implemented per lane in
//! plain Rust with the x86 instructions' exact semantics ([`x86_model`](super::x86_model)), no
//! target features. Runs on any host and under Miri (except the estimate primitives with
//! [`Estimates::Host`](crate::Estimates::Host)).
//!
//! The model is instantiated once per estimate source, as a submodule:
//!
//! - [`host`]: `rcpps`/`rsqrtps` executed on this host (x86 only).
//! - [`amd_zen4`]: the oracle host's estimates (`estimates::amd_zen4`), on any host.

pub mod host;

pub mod amd_zen4;
