// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLMemoryLayout.h (chrome/m156).

//! [`MemoryLayout`]: the size, alignment and stride of a type under a memory layout standard.

use crate::ir::{TypeKind, TypeRef};

/// `MemoryLayout`: a memory layout standard and the rules it applies.
// Port of: src/sksl/SkSLMemoryLayout.h#L16-L243 (chrome/m156)
#[doc(alias = "SkSL::MemoryLayout")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryLayout {
    std: Standard,
}

/// `MemoryLayout::Standard`.
#[doc(alias = "SkSL::MemoryLayout::Standard")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Standard {
    /// `k140`: std140.
    Std140,
    /// `k430`: std430.
    Std430,
    /// `kMetal`.
    Metal,
    /// `kWGSLUniform_Base`: treats `f16` as a full 32-bit float.
    WgslUniformBase,
    /// `kWGSLUniform_EnableF16`: treats `f16` as a 16-bit half float.
    WgslUniformEnableF16,
    /// `kWGSLStorage_Base`.
    WgslStorageBase,
    /// `kWGSLStorage_EnableF16`.
    WgslStorageEnableF16,
}

impl MemoryLayout {
    /// The `MemoryLayout` constructor.
    #[must_use]
    pub const fn new(std: Standard) -> Self {
        Self { std }
    }

    /// `isWGSL_Base()`.
    #[must_use]
    pub fn is_wgsl_base(self) -> bool {
        matches!(
            self.std,
            Standard::WgslUniformBase | Standard::WgslStorageBase
        )
    }

    /// `isWGSL_F16()`.
    #[must_use]
    pub fn is_wgsl_f16(self) -> bool {
        matches!(
            self.std,
            Standard::WgslUniformEnableF16 | Standard::WgslStorageEnableF16
        )
    }

    /// `isWGSL_Uniform()`.
    #[must_use]
    pub fn is_wgsl_uniform(self) -> bool {
        matches!(
            self.std,
            Standard::WgslUniformBase | Standard::WgslUniformEnableF16
        )
    }

    /// `isWGSL()`.
    #[must_use]
    pub fn is_wgsl(self) -> bool {
        self.is_wgsl_base() || self.is_wgsl_f16()
    }

    /// `isMetal()`.
    #[must_use]
    pub fn is_metal(self) -> bool {
        self.std == Standard::Metal
    }

    /// `roundUpIfNeeded(raw, type)`: std140 and WGSL uniform layouts round up everything but
    /// matrices to a multiple of 16.
    // Port of: src/sksl/SkSLMemoryLayout.h#L51-L60 (chrome/m156)
    #[must_use]
    pub fn round_up_if_needed(self, raw: usize, type_kind: TypeKind) -> usize {
        if self.std == Standard::Std140 {
            return Self::round_up_16(raw);
        }
        if self.is_wgsl_uniform() && type_kind != TypeKind::Matrix {
            return Self::round_up_16(raw);
        }
        raw
    }

    /// `roundUp16(n)`: the smallest multiple of 16 greater than or equal to `n`.
    // Port of: src/sksl/SkSLMemoryLayout.h#L63-L65 (chrome/m156)
    #[must_use]
    pub const fn round_up_16(n: usize) -> usize {
        (n + 15) & !15
    }

    /// `GetVectorAlignment(componentSize, columns)`.
    // Port of: src/sksl/SkSLMemoryLayout.h#L236-L238 (chrome/m156)
    const fn vector_alignment(component_size: usize, columns: usize) -> usize {
        component_size * (columns + columns % 2)
    }

    /// `alignment(type)`: the alignment a standalone variable of this type needs.
    ///
    /// # Panics
    ///
    /// For a type kind with no alignment rule (Skia: `SK_ABORT`).
    // Port of: src/sksl/SkSLMemoryLayout.h#L72-L101 (chrome/m156)
    #[must_use]
    pub fn alignment(self, ty: TypeRef<'_>) -> usize {
        match ty.type_kind {
            TypeKind::Scalar | TypeKind::Atomic => self.size(ty),
            TypeKind::Vector => Self::vector_alignment(
                self.size(ty.component_type()),
                usize::try_from(ty.columns()).expect("a vector has columns"),
            ),
            TypeKind::Matrix => self.round_up_if_needed(
                Self::vector_alignment(
                    self.size(ty.component_type()),
                    usize::try_from(ty.rows()).expect("a matrix has rows"),
                ),
                ty.type_kind,
            ),
            TypeKind::Array => {
                self.round_up_if_needed(self.alignment(ty.component_type()), ty.type_kind)
            }
            TypeKind::Struct => {
                let mut result = 0;
                for field in ty.fields() {
                    let alignment = self.alignment(ty.pool().ty(field.ty));
                    if alignment > result {
                        result = alignment;
                    }
                }
                self.round_up_if_needed(result, ty.type_kind)
            }
            _ => panic!("cannot determine alignment of type '{}'", ty.display_name()),
        }
    }

    /// `stride(type)`: for a matrix or array, the number of bytes from the start of one entry (a
    /// row, for a matrix) to the start of the next.
    ///
    /// # Panics
    ///
    /// For a type that has no stride (Skia: `SK_ABORT`).
    // Port of: src/sksl/SkSLMemoryLayout.h#L104-L122 (chrome/m156)
    #[must_use]
    pub fn stride(self, ty: TypeRef<'_>) -> usize {
        match ty.type_kind {
            TypeKind::Matrix => self.alignment(ty),
            TypeKind::Array => {
                let mut stride = self.size(ty.component_type());
                if stride > 0 {
                    let align = self.alignment(ty.component_type());
                    stride += align - 1;
                    stride -= stride % align;
                    stride = self.round_up_if_needed(stride, ty.type_kind);
                }
                stride
            }
            _ => panic!("type '{}' does not have a stride", ty.display_name()),
        }
    }

    /// `size(type)`: the size of a type in bytes, or 0 for an unsized array.
    ///
    /// # Panics
    ///
    /// For a type kind with no size rule (Skia: `SK_ABORT`).
    // Port of: src/sksl/SkSLMemoryLayout.h#L125-L166 (chrome/m156)
    #[must_use]
    pub fn size(self, ty: TypeRef<'_>) -> usize {
        match ty.type_kind {
            TypeKind::Scalar => {
                if ty.is_boolean() {
                    return usize::from(!self.is_wgsl());
                }
                if self.is_metal() && !ty.high_precision() && ty.is_number() {
                    return 2;
                }
                if self.is_wgsl_f16() && !ty.high_precision() && ty.is_float() {
                    return 2;
                }
                4
            }
            TypeKind::Atomic => 4,
            TypeKind::Vector => {
                let columns = usize::try_from(ty.columns()).expect("a vector has columns");
                if self.is_metal() && columns == 3 {
                    return 4 * self.size(ty.component_type());
                }
                columns * self.size(ty.component_type())
            }
            TypeKind::Matrix | TypeKind::Array => {
                if ty.is_unsized_array() {
                    0
                } else {
                    let columns = usize::try_from(ty.columns()).expect("a count is positive");
                    columns * self.stride(ty)
                }
            }
            TypeKind::Struct => {
                let mut total: usize = 0;
                for field in ty.fields() {
                    let field_ty = ty.pool().ty(field.ty);
                    let alignment = self.alignment(field_ty);
                    if !total.is_multiple_of(alignment) {
                        total += alignment - total % alignment;
                    }
                    debug_assert!(total.is_multiple_of(alignment));
                    total += self.size(field_ty);
                }
                let alignment = self.alignment(ty);
                debug_assert!(
                    ty.fields().is_empty()
                        || alignment
                            .is_multiple_of(self.alignment(ty.pool().ty(ty.fields()[0].ty)))
                );
                // Skia computes this in `size_t`, so the arithmetic wraps.
                total.wrapping_add(alignment).wrapping_sub(1) & !alignment.wrapping_sub(1)
            }
            _ => panic!("cannot determine size of type '{}'", ty.display_name()),
        }
    }

    /// `isSupported(type)`: whether this memory layout can hold a value of this type.
    // Port of: src/sksl/SkSLMemoryLayout.h#L169-L192 (chrome/m156)
    #[must_use]
    pub fn is_supported(self, ty: TypeRef<'_>) -> bool {
        match ty.type_kind {
            TypeKind::Atomic => true,
            TypeKind::Scalar => !self.is_wgsl() || !ty.is_boolean(),
            TypeKind::Vector | TypeKind::Matrix | TypeKind::Array => {
                self.is_supported(ty.component_type())
            }
            TypeKind::Struct => ty
                .fields()
                .iter()
                .all(|field| self.is_supported(ty.pool().ty(field.ty))),
            _ => false,
        }
    }

    /// `getStandard()`.
    #[must_use]
    pub const fn standard(self) -> Standard {
        self.std
    }
}
