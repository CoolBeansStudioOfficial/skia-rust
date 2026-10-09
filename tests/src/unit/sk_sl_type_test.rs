// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkSLTypeTest.cpp (chrome/m156)

//! The numeric limits of the built-in types.
//!
//! Skia's `BuiltinTypes` object is the static table in `skia_rust_sksl::builtin_types`, so there is
//! nothing to construct: `types.fInt` is `TypeId::INT`, looked up in an empty pool.
//!
//! The limits are compared exactly, as Skia compares them (`==`), so the float lint is allowed.

use skia_rust_sksl::ir::{IrPool, TypeId};

use crate::{def_test, reporter_assert};

// Port of: tests/SkSLTypeTest.cpp#L16-L38 (chrome/m156)
def_test!(
    #[allow(clippy::float_cmp)]
    SkSLTypeLimits,
    |r| {
        let pool = IrPool::new();

        reporter_assert!(
            r,
            pool.ty(TypeId::INT).minimum_value() == f64::from(i32::MIN)
        );
        reporter_assert!(
            r,
            pool.ty(TypeId::INT).maximum_value() == f64::from(i32::MAX)
        );

        reporter_assert!(
            r,
            pool.ty(TypeId::SHORT).minimum_value() == f64::from(i16::MIN)
        );
        reporter_assert!(
            r,
            pool.ty(TypeId::SHORT).maximum_value() == f64::from(i16::MAX)
        );

        reporter_assert!(
            r,
            pool.ty(TypeId::UINT).minimum_value() == f64::from(u32::MIN)
        );
        reporter_assert!(
            r,
            pool.ty(TypeId::UINT).maximum_value() == f64::from(u32::MAX)
        );

        reporter_assert!(
            r,
            pool.ty(TypeId::USHORT).minimum_value() == f64::from(u16::MIN)
        );
        reporter_assert!(
            r,
            pool.ty(TypeId::USHORT).maximum_value() == f64::from(u16::MAX)
        );

        reporter_assert!(
            r,
            pool.ty(TypeId::FLOAT).minimum_value() == f64::from(f32::MIN)
        );
        reporter_assert!(
            r,
            pool.ty(TypeId::FLOAT).maximum_value() == f64::from(f32::MAX)
        );
    }
);
