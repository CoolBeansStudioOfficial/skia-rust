// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLType.{h,cpp}: the data of every `Type` subclass and its
// virtual accessors. Coercion, `toCompound`, qualifiers, literal range checks, array-size
// conversion, `clone` and the checked `MakeStructType`/`MakeArrayType` come with task S6.

//! [`Type`]: `SkSL` types, and [`TypeRef`], which answers Skia's `Type` queries through the pool.

use std::borrow::Cow;

use super::{IrPool, Layout, ModifierFlags, ids::TypeId};
use crate::position::Position;

/// `Type::TypeKind`.
#[doc(alias = "Type::TypeKind")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TypeKind {
    Array,
    Atomic,
    Generic,
    Literal,
    Matrix,
    Other,
    Sampler,
    SeparateSampler,
    Scalar,
    Struct,
    Texture,
    Vector,
    Void,
    ColorFilter,
    Shader,
    Blender,
}

/// `Type::NumberKind`.
#[doc(alias = "Type::NumberKind")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NumberKind {
    Float,
    Signed,
    Unsigned,
    Boolean,
    Nonnumeric,
}

/// `Type::TextureAccess`.
#[doc(alias = "Type::TextureAccess")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureAccess {
    /// Allows both sampling and reading.
    Sample,
    Read,
    Write,
    ReadWrite,
}

/// `SpvDim_`: the texture dimensions, with SPIR-V's values.
#[doc(alias = "SpvDim_")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpvDim {
    Dim1D = 0,
    Dim2D = 1,
    Dim3D = 2,
    Cube = 3,
    Rect = 4,
    Buffer = 5,
    SubpassData = 6,
}

/// `SkSL::CoercionCost`: how expensive an implicit conversion is.
// Port of: src/sksl/ir/SkSLType.h#L36-L69 (chrome/m156)
#[doc(alias = "SkSL::CoercionCost")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoercionCost {
    /// `fNormalCost`.
    pub normal_cost: i32,
    /// `fNarrowingCost`.
    pub narrowing_cost: i32,
    /// `fImpossible`.
    pub impossible: bool,
}

impl CoercionCost {
    /// `Free()`.
    #[must_use]
    pub const fn free() -> Self {
        Self {
            normal_cost: 0,
            narrowing_cost: 0,
            impossible: false,
        }
    }

    /// `Normal(cost)`.
    #[must_use]
    pub const fn normal(cost: i32) -> Self {
        Self {
            normal_cost: cost,
            narrowing_cost: 0,
            impossible: false,
        }
    }

    /// `Narrowing(cost)`.
    #[must_use]
    pub const fn narrowing(cost: i32) -> Self {
        Self {
            normal_cost: 0,
            narrowing_cost: cost,
            impossible: false,
        }
    }

    /// `Impossible()`.
    #[must_use]
    pub const fn impossible() -> Self {
        Self {
            normal_cost: 0,
            narrowing_cost: 0,
            impossible: true,
        }
    }

    /// `isPossible(allowNarrowing)`.
    #[must_use]
    pub fn is_possible(self, allow_narrowing: bool) -> bool {
        !self.impossible && (self.narrowing_cost == 0 || allow_narrowing)
    }

    /// The `std::tie(fImpossible, fNarrowingCost, fNormalCost)` key Skia orders costs by.
    fn key(self) -> (bool, i32, i32) {
        (self.impossible, self.narrowing_cost, self.normal_cost)
    }
}

impl std::ops::Add for CoercionCost {
    type Output = Self;

    /// `operator+`: impossible if either is, otherwise the costs add up.
    fn add(self, rhs: Self) -> Self {
        if self.impossible || rhs.impossible {
            return Self::impossible();
        }
        Self {
            normal_cost: self.normal_cost + rhs.normal_cost,
            narrowing_cost: self.narrowing_cost + rhs.narrowing_cost,
            impossible: false,
        }
    }
}

impl PartialOrd for CoercionCost {
    /// Skia's `operator<` / `operator<=`: by impossibility, then narrowing, then normal cost.
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.key().cmp(&other.key()))
    }
}

/// `SkSL::Field`: one field of a struct or interface block.
// Port of: src/sksl/ir/SkSLType.h#L74-L92 (chrome/m156)
#[doc(alias = "SkSL::Field")]
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// `fPosition`.
    pub position: Position,
    /// `fLayout`.
    pub layout: Layout,
    /// `fModifierFlags`.
    pub modifier_flags: ModifierFlags,
    /// `fName`.
    pub name: Box<str>,
    /// `fType`.
    pub ty: TypeId,
}

impl Field {
    /// `Field::description`: `layout (…) flags type name;`.
    // Port of: src/sksl/ir/SkSLType.cpp#L1410-L1413 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        format!(
            "{}{}{} {};",
            self.layout.padded_description(),
            self.modifier_flags.padded_description(),
            pool.ty(self.ty).display_name(),
            self.name
        )
    }
}

/// `StructType`'s data: the fields and the facts its constructor derives from them.
// Port of: src/sksl/ir/SkSLType.cpp#L622-L733 (chrome/m156)
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // Mirrors Skia's `StructType` fields one for one.
pub struct StructType {
    /// `fFields`.
    pub fields: Vec<Field>,
    /// `fSlotCount` (0 when the struct contains an unsized array).
    pub slot_count: usize,
    /// `fNestingDepth`.
    pub nesting_depth: i32,
    /// `fUniformErrorPosition`: the first field that may not appear in a uniform.
    pub uniform_error_position: Position,
    /// `fInterfaceBlock`.
    pub interface_block: bool,
    /// `fContainsArray`.
    pub contains_array: bool,
    /// `fContainsUnsizedArray`.
    pub contains_unsized_array: bool,
    /// `fContainsAtomic`.
    pub contains_atomic: bool,
    /// `fContainsBool`.
    pub contains_bool: bool,
    /// `fIsBuiltin`.
    pub is_builtin: bool,
    /// `fIsAllowedInES2`.
    pub is_allowed_in_es2: bool,
}

impl StructType {
    /// The `StructType` constructor: derives the cached facts from the fields' types.
    #[must_use]
    pub fn new(
        pool: &IrPool,
        fields: Vec<Field>,
        nesting_depth: i32,
        interface_block: bool,
        is_builtin: bool,
    ) -> Self {
        let mut contains_array = false;
        let mut contains_unsized_array = false;
        let mut contains_atomic = false;
        let mut contains_bool = false;
        let mut is_allowed_in_es2 = true;
        for f in &fields {
            let ty = pool.ty(f.ty);
            contains_array = contains_array || ty.is_or_contains_array();
            contains_unsized_array = contains_unsized_array || ty.is_or_contains_unsized_array();
            contains_atomic = contains_atomic || ty.is_or_contains_atomic();
            contains_bool = contains_bool || ty.is_or_contains_bool();
            is_allowed_in_es2 = is_allowed_in_es2 && ty.is_allowed_in_es2();
        }
        let mut uniform_error_position = Position::default();
        for f in &fields {
            let mut error_position = f.position;
            if !pool
                .ty(f.ty)
                .is_allowed_in_uniform(Some(&mut error_position))
            {
                uniform_error_position = error_position;
                break;
            }
        }
        let mut slot_count = 0;
        if !contains_unsized_array {
            for f in &fields {
                slot_count += pool.ty(f.ty).slot_count();
            }
        }
        Self {
            fields,
            slot_count,
            nesting_depth,
            uniform_error_position,
            interface_block,
            contains_array,
            contains_unsized_array,
            contains_atomic,
            contains_bool,
            is_builtin,
            is_allowed_in_es2,
        }
    }
}

/// The subclass of a [`Type`] and its data (Skia's `AliasType`, `ArrayType`, … and plain `Type`
/// for `MakeSpecialType`).
#[derive(Clone, Debug, PartialEq)]
pub enum TypeClass {
    /// A plain `Type` (`MakeSpecialType`): `void`, `<INVALID>`, `shader`, …
    Special,
    /// `AliasType`: `vec2` for `float2`, …
    Alias {
        /// `fTargetType`.
        target: TypeId,
    },
    /// `ArrayType`.
    Array {
        /// `fComponentType`.
        component: TypeId,
        /// `fCount`, or [`Type::UNSIZED_ARRAY`].
        count: i32,
        /// `fIsBuiltin`.
        is_builtin: bool,
    },
    /// `GenericType`: `$genType`, …
    Generic {
        /// `fCoercibleTypes`.
        coercible_types: &'static [TypeId],
        /// `fSlotType`.
        slot_type: TypeId,
    },
    /// `LiteralType`: `$floatLiteral`, `$intLiteral`.
    Literal {
        /// `fScalarType`.
        scalar_type: TypeId,
        /// `fPriority`.
        priority: i8,
    },
    /// `ScalarType`.
    Scalar {
        /// `fNumberKind`.
        number_kind: NumberKind,
        /// `fPriority`.
        priority: i8,
        /// `fBitWidth`.
        bit_width: i8,
    },
    /// `AtomicType`.
    Atomic,
    /// `MatrixType`.
    Matrix {
        /// `fComponentType` (a scalar type).
        component: TypeId,
        /// `fColumns`.
        columns: i8,
        /// `fRows`.
        rows: i8,
    },
    /// `TextureType`.
    Texture {
        /// `fDimensions`.
        dimensions: SpvDim,
        /// `fIsDepth`.
        is_depth: bool,
        /// `fIsArrayed`.
        is_arrayed: bool,
        /// `fIsMultisampled`.
        is_multisampled: bool,
        /// `fTextureAccess`.
        access: TextureAccess,
    },
    /// `SamplerType`.
    Sampler {
        /// `fTextureType` (a texture type).
        texture: TypeId,
    },
    /// `StructType`.
    Struct(Box<StructType>),
    /// `VectorType`.
    Vector {
        /// `fComponentType` (a scalar type).
        component: TypeId,
        /// `fColumns`.
        columns: i8,
    },
}

/// `SkSL::Type`: a symbol that names a type.
///
/// Types are compared by id ([`TypeRef::matches`] resolves aliases first). Built-in types live
/// in a fixed table ([`crate::builtin_types`]), so `TypeId::FLOAT` is a constant; struct and
/// array types are allocated in the pool of the code that declares them.
// Port of: src/sksl/ir/SkSLType.h#L98-L558 (chrome/m156)
#[doc(alias = "SkSL::Type")]
#[derive(Clone, Debug, PartialEq)]
pub struct Type {
    /// `fPosition`.
    pub position: Position,
    /// The symbol name (`Symbol::name()`).
    pub name: Cow<'static, str>,
    /// `abbreviatedName()`: a short name used to mangle function names (at most 3 characters).
    pub abbreviated_name: &'static str,
    /// `typeKind()`.
    pub type_kind: TypeKind,
    /// The subclass data.
    pub class: TypeClass,
}

impl Type {
    /// `kMaxAbbrevLength`.
    pub const MAX_ABBREV_LENGTH: usize = 3;
    /// `kUnsizedArray`: the `count` of an unsized array.
    pub const UNSIZED_ARRAY: i32 = -1;

    /// The `Type` constructor; the `Make*Type` factories pick `class`.
    const fn with_class(
        name: Cow<'static, str>,
        abbrev: &'static str,
        type_kind: TypeKind,
        class: TypeClass,
    ) -> Self {
        Self {
            position: Position::INVALID,
            name,
            abbreviated_name: abbrev,
            type_kind,
            class,
        }
    }

    /// `MakeScalarType`.
    #[must_use]
    pub const fn make_scalar_type(
        name: &'static str,
        abbrev: &'static str,
        number_kind: NumberKind,
        priority: i8,
        bit_width: i8,
    ) -> Self {
        Self::with_class(
            Cow::Borrowed(name),
            abbrev,
            TypeKind::Scalar,
            TypeClass::Scalar {
                number_kind,
                priority,
                bit_width,
            },
        )
    }

    /// `MakeVectorType`. `component` must be a scalar type.
    #[must_use]
    pub const fn make_vector_type(
        name: &'static str,
        abbrev: &'static str,
        component: TypeId,
        columns: i8,
    ) -> Self {
        Self::with_class(
            Cow::Borrowed(name),
            abbrev,
            TypeKind::Vector,
            TypeClass::Vector { component, columns },
        )
    }

    /// `MakeMatrixType`. `component` must be a scalar type.
    #[must_use]
    pub const fn make_matrix_type(
        name: &'static str,
        abbrev: &'static str,
        component: TypeId,
        columns: i8,
        rows: i8,
    ) -> Self {
        Self::with_class(
            Cow::Borrowed(name),
            abbrev,
            TypeKind::Matrix,
            TypeClass::Matrix {
                component,
                columns,
                rows,
            },
        )
    }

    /// `MakeSpecialType`.
    #[must_use]
    pub const fn make_special_type(
        name: &'static str,
        abbrev: &'static str,
        type_kind: TypeKind,
    ) -> Self {
        Self::with_class(Cow::Borrowed(name), abbrev, type_kind, TypeClass::Special)
    }

    /// `MakeLiteralType`.
    #[must_use]
    pub const fn make_literal_type(name: &'static str, scalar_type: TypeId, priority: i8) -> Self {
        Self::with_class(
            Cow::Borrowed(name),
            "L",
            TypeKind::Literal,
            TypeClass::Literal {
                scalar_type,
                priority,
            },
        )
    }

    /// `MakeAliasType`. An alias copies its target's abbreviation and type kind; they are passed
    /// in because a `const` table cannot look the target up.
    #[must_use]
    pub const fn make_alias_type(
        name: &'static str,
        target: TypeId,
        target_abbrev: &'static str,
        target_kind: TypeKind,
    ) -> Self {
        Self::with_class(
            Cow::Borrowed(name),
            target_abbrev,
            target_kind,
            TypeClass::Alias { target },
        )
    }

    /// `MakeTextureType`.
    #[must_use]
    pub const fn make_texture_type(
        name: &'static str,
        dimensions: SpvDim,
        is_depth: bool,
        is_arrayed: bool,
        is_multisampled: bool,
        access: TextureAccess,
    ) -> Self {
        Self::with_class(
            Cow::Borrowed(name),
            "T",
            TypeKind::Texture,
            TypeClass::Texture {
                dimensions,
                is_depth,
                is_arrayed,
                is_multisampled,
                access,
            },
        )
    }

    /// `MakeSamplerType`. `texture` must be a texture type.
    #[must_use]
    pub const fn make_sampler_type(name: &'static str, texture: TypeId) -> Self {
        Self::with_class(
            Cow::Borrowed(name),
            "Z",
            TypeKind::Sampler,
            TypeClass::Sampler { texture },
        )
    }

    /// `MakeGenericType`.
    #[must_use]
    pub const fn make_generic_type(
        name: &'static str,
        coercible_types: &'static [TypeId],
        slot_type: TypeId,
    ) -> Self {
        Self::with_class(
            Cow::Borrowed(name),
            "G",
            TypeKind::Generic,
            TypeClass::Generic {
                coercible_types,
                slot_type,
            },
        )
    }

    /// `MakeAtomicType`.
    #[must_use]
    pub const fn make_atomic_type(name: &'static str, abbrev: &'static str) -> Self {
        Self::with_class(
            Cow::Borrowed(name),
            abbrev,
            TypeKind::Atomic,
            TypeClass::Atomic,
        )
    }

    /// An `ArrayType` without `MakeArrayType`'s context: `name` is the component's
    /// [`TypeRef::array_name`], `abbrev` the component's abbreviation.
    #[must_use]
    pub fn new_array_type(
        name: String,
        abbrev: &'static str,
        component: TypeId,
        count: i32,
        is_builtin: bool,
    ) -> Self {
        Self::with_class(
            Cow::Owned(name),
            abbrev,
            TypeKind::Array,
            TypeClass::Array {
                component,
                count,
                is_builtin,
            },
        )
    }

    /// A `StructType` without `MakeStructType`'s error checks (Skia's `Type::clone` builds
    /// struct types this way).
    #[must_use]
    pub fn new_struct_type(position: Position, name: String, data: StructType) -> Self {
        let mut ty = Self::with_class(
            Cow::Owned(name),
            "S",
            TypeKind::Struct,
            TypeClass::Struct(Box::new(data)),
        );
        ty.position = position;
        ty
    }
}

/// A [`Type`] together with the pool that resolves the ids it refers to. It answers Skia's
/// virtual `Type` queries (`componentType()`, `columns()`, `isScalar()`, …) with each
/// subclass's override.
#[derive(Clone, Copy, Debug)]
pub struct TypeRef<'a> {
    pool: &'a IrPool,
    id: TypeId,
    ty: &'a Type,
}

impl std::ops::Deref for TypeRef<'_> {
    type Target = Type;

    fn deref(&self) -> &Type {
        self.ty
    }
}

/// `SkHalfToFloat(SK_HalfMax)`: the largest finite half, 65504.
const HALF_MAX: f64 = 65504.0;

// Port of: src/sksl/ir/SkSLType.h#L98-L558 and src/sksl/ir/SkSLType.cpp#L43-L797 (chrome/m156):
// `Type`'s virtual accessors and the overrides of each subclass.
impl<'a> TypeRef<'a> {
    /// The type `id` of `pool`.
    #[must_use]
    pub fn new(pool: &'a IrPool, id: TypeId) -> Self {
        Self {
            pool,
            id,
            ty: pool.type_node(id),
        }
    }

    /// This type's id.
    #[must_use]
    pub fn id(self) -> TypeId {
        self.id
    }

    /// The pool this type is looked up in.
    #[must_use]
    pub fn pool(self) -> &'a IrPool {
        self.pool
    }

    /// The underlying [`Type`] node, with the pool's lifetime.
    #[must_use]
    pub fn node(self) -> &'a Type {
        self.ty
    }

    /// Another type of the same pool.
    fn other(self, id: TypeId) -> Self {
        Self::new(self.pool, id)
    }

    /// `name()`.
    #[must_use]
    pub fn name(self) -> &'a str {
        &self.ty.name
    }

    /// `resolve()`: the target of an alias, otherwise this type.
    #[must_use]
    pub fn resolve(self) -> Self {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target),
            _ => self,
        }
    }

    /// `matches(that)`: equal after alias resolution. Arrays match structurally (same size,
    /// matching component types).
    #[must_use]
    pub fn matches(self, that: TypeId) -> bool {
        if let TypeClass::Array { .. } = self.ty.class {
            let that = self.other(that).resolve();
            return that.is_array()
                && self.columns() == that.columns()
                && self.component_type().matches(that.component_type().id);
        }
        self.resolve().id == self.other(that).resolve().id
    }

    /// `isBuiltin()`: true except for arrays and structs declared in program code.
    #[must_use]
    pub fn is_builtin(self) -> bool {
        match &self.ty.class {
            TypeClass::Array { is_builtin, .. } => *is_builtin,
            TypeClass::Struct(s) => s.is_builtin,
            _ => true,
        }
    }

    /// `scalarTypeForLiteral()`: the scalar type behind `$floatLiteral`/`$intLiteral`.
    #[must_use]
    pub fn scalar_type_for_literal(self) -> Self {
        match self.ty.class {
            TypeClass::Literal { scalar_type, .. } => self.other(scalar_type),
            _ => self,
        }
    }

    /// `displayName()`: the name shown in descriptions and errors (`float` for a float
    /// literal).
    #[must_use]
    pub fn display_name(self) -> &'a str {
        self.scalar_type_for_literal().name()
    }

    /// `description()` (the same as [`TypeRef::display_name`]).
    #[must_use]
    pub fn description(self) -> String {
        self.display_name().to_owned()
    }

    /// `numberKind()`.
    #[must_use]
    pub fn number_kind(self) -> NumberKind {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).number_kind(),
            TypeClass::Literal { scalar_type, .. } => self.other(scalar_type).number_kind(),
            TypeClass::Scalar { number_kind, .. } => number_kind,
            _ => NumberKind::Nonnumeric,
        }
    }

    /// `isBoolean()`.
    #[must_use]
    pub fn is_boolean(self) -> bool {
        self.number_kind() == NumberKind::Boolean
    }

    /// `isNumber()`.
    #[must_use]
    pub fn is_number(self) -> bool {
        matches!(
            self.number_kind(),
            NumberKind::Float | NumberKind::Signed | NumberKind::Unsigned
        )
    }

    /// `isFloat()`.
    #[must_use]
    pub fn is_float(self) -> bool {
        self.number_kind() == NumberKind::Float
    }

    /// `isSigned()`.
    #[must_use]
    pub fn is_signed(self) -> bool {
        self.number_kind() == NumberKind::Signed
    }

    /// `isUnsigned()`.
    #[must_use]
    pub fn is_unsigned(self) -> bool {
        self.number_kind() == NumberKind::Unsigned
    }

    /// `isInteger()`.
    #[must_use]
    pub fn is_integer(self) -> bool {
        matches!(
            self.number_kind(),
            NumberKind::Signed | NumberKind::Unsigned
        )
    }

    /// `isOpaque()`.
    #[must_use]
    pub fn is_opaque(self) -> bool {
        matches!(
            self.ty.type_kind,
            TypeKind::Atomic
                | TypeKind::Blender
                | TypeKind::ColorFilter
                | TypeKind::Sampler
                | TypeKind::SeparateSampler
                | TypeKind::Shader
                | TypeKind::Texture
        )
    }

    /// `isStorageTexture()`.
    #[must_use]
    pub fn is_storage_texture(self) -> bool {
        self.ty.type_kind == TypeKind::Texture
            && self.dimensions() != SpvDim::SubpassData
            && matches!(
                self.texture_access(),
                TextureAccess::Write | TextureAccess::ReadWrite
            )
    }

    /// `isReadOnlyTexture()`.
    #[must_use]
    pub fn is_read_only_texture(self) -> bool {
        self.ty.type_kind == TypeKind::Texture
            && self.dimensions() != SpvDim::SubpassData
            && self.texture_access() == TextureAccess::Read
    }

    /// `priority()`: the rank used to pick the wider of two number types.
    ///
    /// # Panics
    ///
    /// In debug builds, for a type that is not a number (Skia: `SkDEBUGFAIL`).
    #[must_use]
    pub fn priority(self) -> i32 {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).priority(),
            TypeClass::Literal { priority, .. } | TypeClass::Scalar { priority, .. } => {
                i32::from(priority)
            }
            _ => {
                debug_assert!(false, "not a number type");
                -1
            }
        }
    }

    /// `componentType()`: the element of an array, the scalar of a vector or matrix, otherwise
    /// the type itself.
    #[must_use]
    pub fn component_type(self) -> Self {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).component_type(),
            TypeClass::Array { component, .. }
            | TypeClass::Matrix { component, .. }
            | TypeClass::Vector { component, .. } => self.other(component),
            _ => self,
        }
    }

    /// `textureType()`: the texture behind a sampler.
    ///
    /// # Panics
    ///
    /// In debug builds, for a type that is not a sampler.
    #[must_use]
    pub fn texture_type(self) -> Self {
        if let TypeClass::Sampler { texture } = self.ty.class {
            self.other(texture)
        } else {
            debug_assert!(false, "not a sampler type");
            self
        }
    }

    /// `columns()`: vector width, matrix columns, array size (or -1 when unsized), 1 for
    /// scalars.
    ///
    /// # Panics
    ///
    /// In debug builds, for a type that has no columns.
    #[must_use]
    pub fn columns(self) -> i32 {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).columns(),
            TypeClass::Array { count, .. } => count,
            TypeClass::Literal { .. } | TypeClass::Scalar { .. } => 1,
            TypeClass::Matrix { columns, .. } | TypeClass::Vector { columns, .. } => {
                i32::from(columns)
            }
            _ => {
                debug_assert!(false, "type does not have columns");
                -1
            }
        }
    }

    /// `rows()`: matrix rows, 1 for scalars and vectors.
    ///
    /// # Panics
    ///
    /// In debug builds, for a type that has no rows.
    #[must_use]
    pub fn rows(self) -> i32 {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).rows(),
            TypeClass::Literal { .. } | TypeClass::Scalar { .. } | TypeClass::Vector { .. } => 1,
            TypeClass::Matrix { rows, .. } => i32::from(rows),
            _ => {
                debug_assert!(false, "type does not have rows");
                -1
            }
        }
    }

    /// `minimumValue()`.
    ///
    /// # Panics
    ///
    /// In debug builds, for a type that has no minimum value.
    #[must_use]
    pub fn minimum_value(self) -> f64 {
        match self.ty.class {
            TypeClass::Literal { scalar_type, .. } => self.other(scalar_type).minimum_value(),
            TypeClass::Scalar { number_kind, .. } => match number_kind {
                NumberKind::Signed => {
                    if self.high_precision() {
                        f64::from(i32::MIN)
                    } else {
                        f64::from(i16::MIN)
                    }
                }
                NumberKind::Unsigned => 0.0,
                _ => {
                    if self.high_precision() {
                        f64::from(f32::MIN)
                    } else {
                        -HALF_MAX
                    }
                }
            },
            _ => {
                debug_assert!(false, "type does not have a minimum value");
                f64::NEG_INFINITY
            }
        }
    }

    /// `maximumValue()`.
    ///
    /// # Panics
    ///
    /// In debug builds, for a type that has no maximum value.
    #[must_use]
    pub fn maximum_value(self) -> f64 {
        match self.ty.class {
            TypeClass::Literal { scalar_type, .. } => self.other(scalar_type).maximum_value(),
            TypeClass::Scalar { number_kind, .. } => match number_kind {
                NumberKind::Signed => {
                    if self.high_precision() {
                        f64::from(i32::MAX)
                    } else {
                        f64::from(i16::MAX)
                    }
                }
                NumberKind::Unsigned => {
                    if self.high_precision() {
                        f64::from(u32::MAX)
                    } else {
                        f64::from(u16::MAX)
                    }
                }
                _ => {
                    if self.high_precision() {
                        f64::from(f32::MAX)
                    } else {
                        HALF_MAX
                    }
                }
            },
            _ => {
                debug_assert!(false, "type does not have a maximum value");
                f64::INFINITY
            }
        }
    }

    /// `slotCount()`: how many scalar slots a value of this type occupies.
    #[must_use]
    pub fn slot_count(self) -> usize {
        match &self.ty.class {
            TypeClass::Alias { target } => self.other(*target).slot_count(),
            TypeClass::Array {
                component, count, ..
            } => {
                debug_assert!(*count > 0, "slotCount of an unsized array");
                usize::try_from(*count).unwrap_or(0) * self.other(*component).slot_count()
            }
            TypeClass::Literal { .. } | TypeClass::Scalar { .. } => 1,
            TypeClass::Matrix { columns, rows, .. } => {
                usize::try_from(i32::from(*columns) * i32::from(*rows)).unwrap_or(0)
            }
            TypeClass::Vector { columns, .. } => usize::try_from(*columns).unwrap_or(0),
            TypeClass::Struct(s) => {
                debug_assert!(!s.contains_unsized_array, "slotCount of an unsized struct");
                s.slot_count
            }
            _ => 0,
        }
    }

    /// `slotType(n)`: the scalar type of slot `n`.
    #[must_use]
    pub fn slot_type(self, n: usize) -> Self {
        match &self.ty.class {
            TypeClass::Alias { target } => self.other(*target).slot_type(n),
            TypeClass::Array { component, .. } => {
                let component = self.other(*component);
                component.slot_type(n % component.slot_count())
            }
            TypeClass::Generic { slot_type, .. } => self.other(*slot_type),
            TypeClass::Literal { scalar_type, .. } => {
                debug_assert_eq!(n, 0);
                self.other(*scalar_type)
            }
            TypeClass::Matrix { component, .. } | TypeClass::Vector { component, .. } => {
                debug_assert!(n < self.slot_count());
                self.other(*component)
            }
            TypeClass::Struct(s) => {
                let mut n = n;
                for field in &s.fields {
                    let field_type = self.other(field.ty);
                    let field_slots = field_type.slot_count();
                    if n < field_slots {
                        return field_type.slot_type(n);
                    }
                    n -= field_slots;
                }
                debug_assert!(false, "slot index out of range");
                self
            }
            _ => self,
        }
    }

    /// `fields()`: the fields of a struct or interface block.
    ///
    /// # Panics
    ///
    /// For a type that is not a struct (Skia: `SK_ABORT`).
    #[must_use]
    pub fn fields(self) -> &'a [Field] {
        match &self.ty.class {
            TypeClass::Struct(s) => &s.fields,
            _ => panic!("Internal error: not a struct"),
        }
    }

    /// `coercibleTypes()`: the types a generic type stands for.
    #[must_use]
    pub fn coercible_types(self) -> &'static [TypeId] {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).coercible_types(),
            TypeClass::Generic {
                coercible_types, ..
            } => coercible_types,
            _ => {
                debug_assert!(false, "Internal error: not a generic type");
                &[]
            }
        }
    }

    /// `dimensions()`.
    #[must_use]
    pub fn dimensions(self) -> SpvDim {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).dimensions(),
            TypeClass::Texture { dimensions, .. } => dimensions,
            TypeClass::Sampler { texture } => self.other(texture).dimensions(),
            _ => {
                debug_assert!(false, "Internal error: not a texture type");
                SpvDim::Dim1D
            }
        }
    }

    /// `isDepth()`.
    #[must_use]
    pub fn is_depth(self) -> bool {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).is_depth(),
            TypeClass::Texture { is_depth, .. } => is_depth,
            TypeClass::Sampler { texture } => self.other(texture).is_depth(),
            _ => {
                debug_assert!(false, "Internal error: not a texture type");
                false
            }
        }
    }

    /// `isArrayedTexture()`.
    #[must_use]
    pub fn is_arrayed_texture(self) -> bool {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).is_arrayed_texture(),
            TypeClass::Texture { is_arrayed, .. } => is_arrayed,
            TypeClass::Sampler { texture } => self.other(texture).is_arrayed_texture(),
            _ => {
                debug_assert!(false, "Internal error: not a texture type");
                false
            }
        }
    }

    /// `isMultisampled()`.
    #[must_use]
    pub fn is_multisampled(self) -> bool {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).is_multisampled(),
            TypeClass::Texture {
                is_multisampled, ..
            } => is_multisampled,
            TypeClass::Sampler { texture } => self.other(texture).is_multisampled(),
            _ => {
                debug_assert!(false, "not a texture type");
                false
            }
        }
    }

    /// `textureAccess()`.
    #[must_use]
    pub fn texture_access(self) -> TextureAccess {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).texture_access(),
            TypeClass::Texture { access, .. } => access,
            TypeClass::Sampler { texture } => self.other(texture).texture_access(),
            _ => {
                debug_assert!(false, "not a texture type");
                TextureAccess::Sample
            }
        }
    }

    /// `isVoid()`.
    #[must_use]
    pub fn is_void(self) -> bool {
        self.ty.type_kind == TypeKind::Void
    }

    /// `isGeneric()`.
    #[must_use]
    pub fn is_generic(self) -> bool {
        self.ty.type_kind == TypeKind::Generic
    }

    /// `isSampler()`.
    #[must_use]
    pub fn is_sampler(self) -> bool {
        self.ty.type_kind == TypeKind::Sampler
    }

    /// `isAtomic()`.
    #[must_use]
    pub fn is_atomic(self) -> bool {
        self.ty.type_kind == TypeKind::Atomic
    }

    /// `isScalar()`.
    #[must_use]
    pub fn is_scalar(self) -> bool {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).is_scalar(),
            TypeClass::Literal { .. } | TypeClass::Scalar { .. } => true,
            _ => false,
        }
    }

    /// `isLiteral()`.
    #[must_use]
    pub fn is_literal(self) -> bool {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).is_literal(),
            TypeClass::Literal { .. } => true,
            _ => false,
        }
    }

    /// `isVector()`.
    #[must_use]
    pub fn is_vector(self) -> bool {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).is_vector(),
            TypeClass::Vector { .. } => true,
            _ => false,
        }
    }

    /// `isMatrix()`.
    #[must_use]
    pub fn is_matrix(self) -> bool {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).is_matrix(),
            TypeClass::Matrix { .. } => true,
            _ => false,
        }
    }

    /// `isArray()`.
    #[must_use]
    pub fn is_array(self) -> bool {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).is_array(),
            TypeClass::Array { .. } => true,
            _ => false,
        }
    }

    /// `isUnsizedArray()`.
    #[must_use]
    pub fn is_unsized_array(self) -> bool {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).is_unsized_array(),
            TypeClass::Array { count, .. } => count == Type::UNSIZED_ARRAY,
            _ => false,
        }
    }

    /// `isStruct()`.
    #[must_use]
    pub fn is_struct(self) -> bool {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).is_struct(),
            TypeClass::Struct(_) => true,
            _ => false,
        }
    }

    /// `isInterfaceBlock()`.
    #[must_use]
    pub fn is_interface_block(self) -> bool {
        match &self.ty.class {
            TypeClass::Alias { target } => self.other(*target).is_interface_block(),
            TypeClass::Struct(s) => s.interface_block,
            _ => false,
        }
    }

    /// `isEffectChild()`: `colorFilter`, `shader` or `blender`.
    #[must_use]
    pub fn is_effect_child(self) -> bool {
        matches!(
            self.ty.type_kind,
            TypeKind::ColorFilter | TypeKind::Shader | TypeKind::Blender
        )
    }

    /// `hasPrecision()`.
    #[must_use]
    pub fn has_precision(self) -> bool {
        self.component_type().is_number() || self.is_sampler()
    }

    /// `highPrecision()`.
    #[must_use]
    pub fn high_precision(self) -> bool {
        self.bit_width() >= 32
    }

    /// `bitWidth()`.
    #[must_use]
    pub fn bit_width(self) -> i32 {
        match self.ty.class {
            TypeClass::Alias { target } => self.other(target).bit_width(),
            TypeClass::Array { component, .. }
            | TypeClass::Matrix { component, .. }
            | TypeClass::Vector { component, .. } => self.other(component).bit_width(),
            TypeClass::Literal { scalar_type, .. } => self.other(scalar_type).bit_width(),
            TypeClass::Scalar { bit_width, .. } => i32::from(bit_width),
            _ => 0,
        }
    }

    /// `isAllowedInES2()`: whether a strict-ES2 program may use this type.
    #[must_use]
    pub fn is_allowed_in_es2(self) -> bool {
        match &self.ty.class {
            TypeClass::Alias { target } => self.other(*target).is_allowed_in_es2(),
            TypeClass::Array { component, .. } | TypeClass::Vector { component, .. } => {
                self.other(*component).is_allowed_in_es2()
            }
            TypeClass::Scalar { number_kind, .. } => *number_kind != NumberKind::Unsigned,
            TypeClass::Atomic => false,
            TypeClass::Matrix { columns, rows, .. } => columns == rows,
            TypeClass::Struct(s) => s.is_allowed_in_es2,
            _ => true,
        }
    }

    /// `isAllowedInUniform(errorPosition)`: whether a uniform may have this type. Structs
    /// report the first offending field through `error_position`.
    #[must_use]
    pub fn is_allowed_in_uniform(self, error_position: Option<&mut Position>) -> bool {
        match &self.ty.class {
            TypeClass::Array { component, .. } | TypeClass::Vector { component, .. } => {
                self.other(*component).is_allowed_in_uniform(error_position)
            }
            TypeClass::Scalar { number_kind, .. } => *number_kind != NumberKind::Boolean,
            TypeClass::Atomic => false,
            TypeClass::Struct(s) => {
                if let Some(error_position) = error_position {
                    *error_position = s.uniform_error_position;
                }
                !s.uniform_error_position.valid()
            }
            _ => !self.is_opaque(),
        }
    }

    /// `isOrContainsArray()`.
    #[must_use]
    pub fn is_or_contains_array(self) -> bool {
        match &self.ty.class {
            TypeClass::Array { .. } => true,
            TypeClass::Struct(s) => s.contains_array,
            _ => false,
        }
    }

    /// `isOrContainsUnsizedArray()`.
    #[must_use]
    pub fn is_or_contains_unsized_array(self) -> bool {
        match &self.ty.class {
            TypeClass::Array { component, .. } => {
                self.is_unsized_array() || self.other(*component).is_or_contains_unsized_array()
            }
            TypeClass::Struct(s) => s.contains_unsized_array,
            _ => false,
        }
    }

    /// `isOrContainsAtomic()`.
    #[must_use]
    pub fn is_or_contains_atomic(self) -> bool {
        match &self.ty.class {
            TypeClass::Alias { target } => self.other(*target).is_or_contains_atomic(),
            TypeClass::Array { component, .. } => self.other(*component).is_or_contains_atomic(),
            TypeClass::Atomic => true,
            TypeClass::Struct(s) => s.contains_atomic,
            _ => false,
        }
    }

    /// `isOrContainsBool()`.
    #[must_use]
    pub fn is_or_contains_bool(self) -> bool {
        match &self.ty.class {
            TypeClass::Alias { target } => self.other(*target).is_or_contains_bool(),
            TypeClass::Array { component, .. } | TypeClass::Vector { component, .. } => {
                self.other(*component).is_or_contains_bool()
            }
            TypeClass::Literal { scalar_type, .. } => self.other(*scalar_type).is_boolean(),
            TypeClass::Scalar { number_kind, .. } => *number_kind == NumberKind::Boolean,
            TypeClass::Struct(s) => s.contains_bool,
            _ => false,
        }
    }

    /// `isInRootSymbolTable()`: only structs and arrays are created by code.
    #[must_use]
    pub fn is_in_root_symbol_table(self) -> bool {
        !(self.is_array() || self.is_struct())
    }

    /// `structNestingDepth()`.
    #[must_use]
    pub fn struct_nesting_depth(self) -> i32 {
        match &self.ty.class {
            TypeClass::Struct(s) => s.nesting_depth,
            _ => 0,
        }
    }

    /// `getArrayName(arraySize)`: `float[10]`, or `float[]` for [`Type::UNSIZED_ARRAY`].
    // Port of: src/sksl/ir/SkSLType.cpp#L795-L801 (chrome/m156)
    #[must_use]
    pub fn array_name(self, array_size: i32) -> String {
        let name = self.name();
        if array_size == Type::UNSIZED_ARRAY {
            return format!("{name}[]");
        }
        format!("{name}[{array_size}]")
    }
}

#[cfg(test)]
mod tests {
    use super::{CoercionCost, NumberKind, StructType, Type, TypeKind};
    use crate::ir::{Field, IrPool, Layout, ModifierFlags, TypeId};
    use crate::position::Position;

    #[test]
    fn builtin_queries() {
        let pool = IrPool::new();
        let float4 = pool.ty(TypeId::FLOAT4);
        assert_eq!(float4.name(), "float4");
        assert_eq!(float4.component_type().id(), TypeId::FLOAT);
        assert_eq!(float4.columns(), 4);
        assert_eq!(float4.rows(), 1);
        assert_eq!(float4.slot_count(), 4);
        // Vectors have no number kind of their own (Skia: only scalars and literals do).
        assert!(float4.is_vector() && !float4.is_float() && float4.component_type().is_float());

        let vec4 = pool.ty(TypeId::VEC4);
        assert_eq!(vec4.resolve().id(), TypeId::FLOAT4);
        assert!(vec4.matches(TypeId::FLOAT4));
        assert_eq!(vec4.type_kind, TypeKind::Vector);
        assert_eq!(vec4.abbreviated_name, "f4");
        assert_eq!(vec4.display_name(), "vec4");

        let lit = pool.ty(TypeId::FLOAT_LITERAL);
        assert_eq!(lit.display_name(), "float");
        assert_eq!(lit.number_kind(), NumberKind::Float);
        assert_eq!(lit.priority(), 8);

        let half3x2 = pool.ty(TypeId::HALF3X2);
        assert_eq!((half3x2.columns(), half3x2.rows()), (3, 2));
        assert!(!half3x2.is_allowed_in_es2());
        assert_eq!(half3x2.slot_count(), 6);

        assert_eq!(pool.ty(TypeId::SHORT).maximum_value(), 32767.0);
        assert_eq!(pool.ty(TypeId::HALF).minimum_value(), -65504.0);
        assert_eq!(pool.ty(TypeId::UINT).maximum_value(), 4_294_967_295.0);
        assert_eq!(
            pool.ty(TypeId::SAMPLER2D).texture_type().name(),
            "$texture2D_sample"
        );
        assert_eq!(pool.ty(TypeId::POISON).name(), "<POISON>");
    }

    #[test]
    fn arrays_and_structs_live_in_the_pool() {
        let mut pool = IrPool::new();
        let name = pool.ty(TypeId::INT).array_name(2);
        assert_eq!(name, "int[2]");
        let int2_array = pool.add_type(Type::new_array_type(name, "i", TypeId::INT, 2, false));
        let other = pool.add_type(Type::new_array_type(
            "int[2]".into(),
            "i",
            TypeId::INT,
            2,
            false,
        ));
        // Arrays match structurally even when they are different symbols.
        assert!(pool.ty(int2_array).matches(other));
        assert_eq!(pool.ty(int2_array).slot_count(), 2);
        assert!(!pool.ty(int2_array).is_builtin());

        let fields = vec![
            Field {
                position: Position::default(),
                layout: Layout::new(),
                modifier_flags: ModifierFlags::empty(),
                name: "a".into(),
                ty: int2_array,
            },
            Field {
                position: Position::range(3, 4),
                layout: Layout::new(),
                modifier_flags: ModifierFlags::empty(),
                name: "b".into(),
                ty: TypeId::BOOL,
            },
        ];
        let data = StructType::new(&pool, fields, 1, false, false);
        let s = pool.add_type(Type::new_struct_type(Position::default(), "S".into(), data));
        let s = pool.ty(s);
        assert_eq!(s.slot_count(), 3);
        assert!(s.is_or_contains_array() && s.is_or_contains_bool());
        assert_eq!(s.slot_type(2).id(), TypeId::BOOL);
        let mut error = Position::default();
        assert!(!s.is_allowed_in_uniform(Some(&mut error)));
        assert_eq!(error, Position::range(3, 4));
        assert_eq!(s.fields()[1].description(&pool), "bool b;");
    }

    #[test]
    fn coercion_costs_order_like_skia() {
        assert!(CoercionCost::normal(5) < CoercionCost::narrowing(1));
        assert!(CoercionCost::narrowing(1) < CoercionCost::impossible());
        assert!(CoercionCost::free() <= CoercionCost::free());
        assert!(!CoercionCost::narrowing(1).is_possible(false));
        assert_eq!(
            CoercionCost::normal(1) + CoercionCost::normal(2),
            CoercionCost::normal(3)
        );
    }
}
