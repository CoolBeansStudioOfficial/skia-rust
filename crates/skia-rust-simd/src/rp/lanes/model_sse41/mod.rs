// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The `Sse41` tier's model twin (design §2.8): `model_sse2` with the SSE4.1 primitives
//! (`roundps` floor/ceil, saturating `packusdw`) and the raw estimates for `rcp_fast`/`rsqrt`.
//!
//! Instantiated once per estimate source, like [`model_sse2`](super::model_sse2):
//! [`host`] and [`amd_zen4`].

pub mod host;

pub mod amd_zen4;
