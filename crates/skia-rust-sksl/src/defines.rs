// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLDefines.h.

//! The compiler's scalar types and limits.

// Port of: src/sksl/SkSLDefines.h#L18-L47 (chrome/m156)

/// `SKSL_INT`: the type of integer values in the IR (literals, array sizes, switch cases).
#[doc(alias = "SKSL_INT")]
pub type SkslInt = i64;

/// `SKSL_FLOAT`: the precision of `Literal::floatValue()`. Literals store a `double`, and a float
/// literal reads back through this type.
#[doc(alias = "SKSL_FLOAT")]
pub type SkslFloat = f32;

/// `kDefaultInlineThreshold`: functions larger than this (in IR nodes, times the number of
/// calls) are not inlined.
#[doc(alias = "kDefaultInlineThreshold")]
pub const DEFAULT_INLINE_THRESHOLD: i32 = 50;

/// `kVariableSlotLimit`: the most variable slots allowed in a function or global scope.
#[doc(alias = "kVariableSlotLimit")]
pub const VARIABLE_SLOT_LIMIT: i32 = 100_000;
