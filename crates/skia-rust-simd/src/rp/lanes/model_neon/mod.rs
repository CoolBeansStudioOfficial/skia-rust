// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The `Neon` tier's model twin (design §2.8): the same names as `neon`, implemented per lane in
//! plain Rust with the A64 instructions' exact semantics ([`neon_model`](super::neon_model)), no
//! target features. Runs on any host and under Miri (except the estimate primitives with
//! [`Estimates::Host`](crate::Estimates::Host)).
//!
//! The model is instantiated once per estimate source, as a submodule:
//!
//! - [`host`]: `FRECPE`/`FRSQRTE` executed on this host (`aarch64` only).
//! - [`arm`]: the Arm ARM's architectural `RecipEstimate`/`RecipSqrtEstimate`
//!   (`estimates::arm`), on any host. Since the estimates are architectural, this is the
//!   `Neon` tier exactly, on every Arm core.

pub mod host;

pub mod arm;
