// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLType.{h,cpp}: the data of every `Type` subclass, its virtual
// accessors, and (with task S6) coercion costs, `toCompound`, qualifiers, literal range checks,
// array-size conversion, `clone` and the checked `MakeStructType`/`MakeArrayType`.
// `coerceExpression` and the expression forms of the range and array-size checks wait for S7a/S8.

//! [`Type`]: `SkSL` types, and [`TypeRef`], which answers Skia's `Type` queries through the pool.

use std::borrow::Cow;
use std::collections::HashSet;

use super::constructor::{constant_value_for_variable, get_constant_int};
use super::symbol_table::{add_array_dimension, add_symbol};
use super::{
    ConstructorArrayCast, ConstructorCompoundCast, ConstructorScalarCast, Expression, IrPool,
    Layout, LayoutFlags, ModifierFlags, SymbolId,
    ids::{ExprId, SymTabId, TypeId},
};
use crate::context::Context;
use crate::defines::{SkslInt, VARIABLE_SLOT_LIMIT};
use crate::position::Position;
use crate::program_settings::ProgramConfig;
use crate::string::{Arg, printf};

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

/// `kMaxStructDepth`.
const MAX_STRUCT_DEPTH: i32 = 8;

/// `toCompound` tables: the vector or matrix of each shape, indexed `[rows - 1][columns - 1]`.
/// `INVALID` marks shapes the type does not have (a vector family has only its first row).
const FLOAT_COMPOUNDS: [[TypeId; 4]; 4] = [
    [
        TypeId::FLOAT,
        TypeId::FLOAT2,
        TypeId::FLOAT3,
        TypeId::FLOAT4,
    ],
    [
        TypeId::INVALID,
        TypeId::FLOAT2X2,
        TypeId::FLOAT3X2,
        TypeId::FLOAT4X2,
    ],
    [
        TypeId::INVALID,
        TypeId::FLOAT2X3,
        TypeId::FLOAT3X3,
        TypeId::FLOAT4X3,
    ],
    [
        TypeId::INVALID,
        TypeId::FLOAT2X4,
        TypeId::FLOAT3X4,
        TypeId::FLOAT4X4,
    ],
];
const HALF_COMPOUNDS: [[TypeId; 4]; 4] = [
    [TypeId::HALF, TypeId::HALF2, TypeId::HALF3, TypeId::HALF4],
    [
        TypeId::INVALID,
        TypeId::HALF2X2,
        TypeId::HALF3X2,
        TypeId::HALF4X2,
    ],
    [
        TypeId::INVALID,
        TypeId::HALF2X3,
        TypeId::HALF3X3,
        TypeId::HALF4X3,
    ],
    [
        TypeId::INVALID,
        TypeId::HALF2X4,
        TypeId::HALF3X4,
        TypeId::HALF4X4,
    ],
];
const INT_COMPOUNDS: [[TypeId; 4]; 4] = [
    [TypeId::INT, TypeId::INT2, TypeId::INT3, TypeId::INT4],
    [TypeId::INVALID; 4],
    [TypeId::INVALID; 4],
    [TypeId::INVALID; 4],
];
const SHORT_COMPOUNDS: [[TypeId; 4]; 4] = [
    [
        TypeId::SHORT,
        TypeId::SHORT2,
        TypeId::SHORT3,
        TypeId::SHORT4,
    ],
    [TypeId::INVALID; 4],
    [TypeId::INVALID; 4],
    [TypeId::INVALID; 4],
];
const UINT_COMPOUNDS: [[TypeId; 4]; 4] = [
    [TypeId::UINT, TypeId::UINT2, TypeId::UINT3, TypeId::UINT4],
    [TypeId::INVALID; 4],
    [TypeId::INVALID; 4],
    [TypeId::INVALID; 4],
];
const USHORT_COMPOUNDS: [[TypeId; 4]; 4] = [
    [
        TypeId::USHORT,
        TypeId::USHORT2,
        TypeId::USHORT3,
        TypeId::USHORT4,
    ],
    [TypeId::INVALID; 4],
    [TypeId::INVALID; 4],
    [TypeId::INVALID; 4],
];
const BOOL_COMPOUNDS: [[TypeId; 4]; 4] = [
    [TypeId::BOOL, TypeId::BOOL2, TypeId::BOOL3, TypeId::BOOL4],
    [TypeId::INVALID; 4],
    [TypeId::INVALID; 4],
    [TypeId::INVALID; 4],
];

// Port of: src/sksl/ir/SkSLType.h#L98-L558 and src/sksl/ir/SkSLType.cpp#L795-L1415 (chrome/m156):
// the rest of `Type`. Coercion costs, `toCompound`, qualifiers, `clone`, the checked factories,
// and the scalar range and array-size checks are here. The expression forms of coercion, range
// and array-size checks (`coerce_expression`, `check_for_out_of_range_literal`,
// `convert_array_size`) follow `Type`'s value forms, at the end of this file (S7a).
impl TypeRef<'_> {
    /// `coercionCost(other)`: how expensive it is to coerce this type to `other`.
    // Port of: src/sksl/ir/SkSLType.cpp#L943-L986 (chrome/m156)
    #[must_use]
    pub fn coercion_cost(self, other: TypeId) -> CoercionCost {
        let target = self.other(other);
        if self.matches(other) {
            return CoercionCost::free();
        }
        if self.ty.type_kind == target.ty.type_kind
            && (self.is_vector() || self.is_matrix() || self.is_array())
        {
            // Vectors, matrices and arrays of the same size can be coerced if their component
            // type can be.
            if self.is_matrix() && self.rows() != target.rows() {
                return CoercionCost::impossible();
            }
            if self.columns() != target.columns() {
                return CoercionCost::impossible();
            }
            return self
                .component_type()
                .coercion_cost(target.component_type().id);
        }

        if self.is_number() && target.is_number() {
            if self.is_literal()
                && (self.is_integer() || self.number_kind() == target.number_kind())
            {
                // `${intLiteral}` and `${floatLiteral}` coerce freely to `float` and `half`.
                // Without this, `${floatLiteral}`'s priority would make conversion to `half`
                // cheaper than conversion to `float`. Overload selection then follows the
                // non-literal arguments, and a call with only literals needs a cast.
                return CoercionCost::free();
            } else if self.number_kind() != target.number_kind() {
                return CoercionCost::impossible();
            } else if target.priority() >= self.priority() {
                return CoercionCost::normal(target.priority() - self.priority());
            }
            return CoercionCost::narrowing(self.priority() - target.priority());
        }
        if self.ty.type_kind == TypeKind::Generic {
            for (i, candidate) in self.coercible_types().iter().enumerate() {
                if self.other(*candidate).matches(other) {
                    // A generic type has only a few coercible types, so the index fits.
                    let index = i32::try_from(i).unwrap_or(i32::MAX);
                    return CoercionCost::normal(index + 1);
                }
            }
        }
        CoercionCost::impossible()
    }

    /// `canCoerceTo(other, allowNarrowing)`.
    #[must_use]
    pub fn can_coerce_to(self, other: TypeId, allow_narrowing: bool) -> bool {
        self.coercion_cost(other).is_possible(allow_narrowing)
    }

    /// `isAllowedInES2(context)`: a strict-ES2 program may use only ES2 types.
    #[must_use]
    pub fn is_allowed_in_es2_for(self, ctx: &Context) -> bool {
        !ctx.config().strict_es2_mode() || self.is_allowed_in_es2()
    }

    /// `toCompound(context, columns, rows)`: the vector or matrix of this scalar's type with the
    /// given shape. A `columns`/`rows` of 1/1 returns this type unchanged.
    ///
    /// # Panics
    ///
    /// For a shape the scalar type has no vector or matrix for (Skia: `SK_ABORT`).
    // Port of: src/sksl/ir/SkSLType.cpp#L1101-L1235 (chrome/m156)
    #[must_use]
    pub fn to_compound(self, columns: i32, rows: i32) -> TypeId {
        debug_assert!(self.is_scalar(), "toCompound of a non-scalar type");
        if columns == 1 && rows == 1 {
            return self.id;
        }
        let table = if self.matches(TypeId::FLOAT) || self.matches(TypeId::FLOAT_LITERAL) {
            &FLOAT_COMPOUNDS
        } else if self.matches(TypeId::HALF) {
            &HALF_COMPOUNDS
        } else if self.matches(TypeId::INT) || self.matches(TypeId::INT_LITERAL) {
            &INT_COMPOUNDS
        } else if self.matches(TypeId::SHORT) {
            &SHORT_COMPOUNDS
        } else if self.matches(TypeId::UINT) {
            &UINT_COMPOUNDS
        } else if self.matches(TypeId::USHORT) {
            &USHORT_COMPOUNDS
        } else if self.matches(TypeId::BOOL) {
            &BOOL_COMPOUNDS
        } else {
            debug_assert!(false, "unsupported toCompound type {}", self.description());
            return TypeId::VOID;
        };
        let row = usize::try_from(rows)
            .ok()
            .filter(|r| (1..=4).contains(r))
            .unwrap_or_else(|| panic!("unsupported row count ({rows})"));
        let shape = if rows == 1 { "vector" } else { "matrix" };
        let column = usize::try_from(columns)
            .ok()
            .filter(|c| (1..=4).contains(c))
            .unwrap_or_else(|| panic!("unsupported {shape} column count ({columns})"));
        let compound = table[row - 1][column - 1];
        assert!(
            compound != TypeId::INVALID,
            "unsupported shape ({columns} columns, {rows} rows)"
        );
        compound
    }
}

impl Type {
    /// `MakeArrayType(context, name, componentType, columns)`: an array of `component`. Skia's
    /// factory has no error checks of its own; its invariants are debug assertions here.
    // Port of: src/sksl/ir/SkSLType.cpp#L807-L813 (chrome/m156)
    #[must_use]
    pub fn make_array_type(ctx: &Context, name: &str, component: TypeId, columns: i32) -> Self {
        let comp = ctx.pool.ty(component);
        debug_assert!(
            columns > 0 || columns == Type::UNSIZED_ARRAY,
            "array count must be positive or unsized"
        );
        debug_assert!(!comp.is_array(), "multi-dimensional arrays are disallowed");
        Self::new_array_type(
            name.to_owned(),
            comp.abbreviated_name,
            component,
            columns,
            ctx.config().is_builtin_code(),
        )
    }

    /// `MakeStructType(context, pos, name, fields, interfaceBlock)`: a struct or interface block,
    /// with Skia's checks on its fields and nesting. Errors go to `ctx`, and the struct is still
    /// returned so that the caller can carry on.
    // Port of: src/sksl/ir/SkSLType.cpp#L850-L929 (chrome/m156)
    // Skia's `MakeStructType` is one function of this length, and its checks run in its order.
    #[allow(clippy::too_many_lines)]
    pub fn make_struct_type(
        ctx: &mut Context,
        pos: Position,
        name: &str,
        fields: Vec<Field>,
        interface_block: bool,
    ) -> Self {
        let (struct_or_ib, a_struct_or_ib) = if interface_block {
            ("interface block", "an interface block")
        } else {
            ("struct", "a struct")
        };
        let builtin_code = ctx.config().is_builtin_code();

        if fields.is_empty() {
            ctx.errors.error(
                pos,
                &format!("{struct_or_ib} '{name}' must contain at least one field"),
            );
        }
        let mut slots: usize = 0;
        let limit = usize::try_from(VARIABLE_SLOT_LIMIT).unwrap_or(usize::MAX);
        let mut field_names: HashSet<&str> = HashSet::new();
        for field in &fields {
            // A repeated name is a duplicate: the set does not grow.
            if !field_names.insert(&field.name) {
                ctx.errors.error(
                    field.position,
                    &format!(
                        "field '{}' was already defined in the same {struct_or_ib} ('{name}')",
                        field.name
                    ),
                );
            }
            if field.modifier_flags != ModifierFlags::empty() {
                let desc = field.modifier_flags.description();
                ctx.errors.error(
                    field.position,
                    &format!("modifier '{desc}' is not permitted on {a_struct_or_ib} field"),
                );
            }
            if field.layout.flags.contains(LayoutFlags::BINDING) {
                ctx.errors.error(
                    field.position,
                    &format!(
                        "layout qualifier 'binding' is not permitted on {a_struct_or_ib} field"
                    ),
                );
            }
            if field.layout.flags.contains(LayoutFlags::SET) {
                ctx.errors.error(
                    field.position,
                    &format!("layout qualifier 'set' is not permitted on {a_struct_or_ib} field"),
                );
            }

            let (is_void, is_opaque_not_atomic, display, contains_bool, unsized_array, slot_count) = {
                let ty = ctx.pool.ty(field.ty);
                (
                    ty.is_void(),
                    ty.is_opaque() && !ty.is_atomic(),
                    ty.display_name().to_owned(),
                    ty.is_or_contains_bool(),
                    ty.is_or_contains_unsized_array(),
                    // Only read for sized fields below (Skia asserts on unsized slot counts).
                    if ty.is_or_contains_unsized_array() {
                        0
                    } else {
                        ty.slot_count()
                    },
                )
            };
            if is_void {
                ctx.errors.error(
                    field.position,
                    &format!("type 'void' is not permitted in {a_struct_or_ib}"),
                );
            }
            if is_opaque_not_atomic {
                ctx.errors.error(
                    field.position,
                    &format!("opaque type '{display}' is not permitted in {a_struct_or_ib}"),
                );
            }
            if interface_block && contains_bool {
                // Reject booleans anywhere in an interface block.
                ctx.errors.error(
                    field.position,
                    "type 'bool' is not permitted in an interface block",
                );
            }
            if unsized_array {
                if !interface_block {
                    // Reject unsized arrays anywhere in structs.
                    ctx.errors
                        .error(field.position, "unsized arrays are not permitted here");
                }
            } else if slots < limit {
                // If we haven't already exceeded the struct size limit, see whether this field
                // causes us to exceed it.
                slots = slots.saturating_add(slot_count);
                if slots >= limit {
                    ctx.errors
                        .error(pos, &format!("{struct_or_ib} is too large"));
                }
            }
        }

        let mut nesting_depth = 0;
        for field in &fields {
            nesting_depth = nesting_depth.max(ctx.pool.ty(field.ty).struct_nesting_depth());
        }
        if nesting_depth >= MAX_STRUCT_DEPTH {
            ctx.errors.error(
                pos,
                &format!("{struct_or_ib} '{name}' is too deeply nested"),
            );
        }
        let data = StructType::new(
            &ctx.pool,
            fields,
            nesting_depth + 1,
            interface_block,
            builtin_code,
        );
        Self::new_struct_type(pos, name.to_owned(), data)
    }
}

impl TypeId {
    /// `applyQualifiers(context, modifierFlags, pos)`: the type a declaration with these
    /// precision and access qualifiers has. Consumed qualifiers are removed from `flags`.
    // Port of: src/sksl/ir/SkSLType.cpp#L988-L995 (chrome/m156)
    #[must_use]
    pub fn apply_qualifiers(
        self,
        ctx: &mut Context,
        flags: &mut ModifierFlags,
        pos: Position,
    ) -> Self {
        let ty = self.apply_precision_qualifiers(ctx, flags, pos);
        ty.apply_access_qualifiers(ctx, flags, pos)
    }

    /// `applyPrecisionQualifiers`.
    // Port of: src/sksl/ir/SkSLType.cpp#L997-L1064 (chrome/m156)
    fn apply_precision_qualifiers(
        self,
        ctx: &mut Context,
        flags: &mut ModifierFlags,
        pos: Position,
    ) -> Self {
        let precision =
            *flags & (ModifierFlags::HIGHP | ModifierFlags::MEDIUMP | ModifierFlags::LOWP);
        if precision.is_empty() {
            // No precision qualifiers here. Return the type as-is.
            return self;
        }
        if !ProgramConfig::is_runtime_effect(ctx.config().kind) {
            // Precision modifiers are discouraged internally: use the type for the precision
            // you need (`half` vs `float`, `short` vs `int`).
            ctx.errors
                .error(pos, "precision qualifiers are not allowed");
            return TypeId::POISON;
        }
        if precision.bits().count_ones() > 1 {
            ctx.errors
                .error(pos, "only one precision qualifier can be used");
            return TypeId::POISON;
        }

        // We're going to return a whole new type, so the modifier bits can be cleared out.
        *flags &= !(ModifierFlags::HIGHP | ModifierFlags::MEDIUMP | ModifierFlags::LOWP);

        let (component_high_precision, number_kind) = {
            let component = ctx.pool.ty(self).component_type();
            (component.high_precision(), component.number_kind())
        };
        if component_high_precision {
            if precision.contains(ModifierFlags::HIGHP) {
                // Type is already high precision, and we are requesting high precision.
                return self;
            }

            // SkSL doesn't support low precision, so `lowp` is interpreted as medium precision.
            // Skia's `mediumpType` falls back to `fPoison`, which is never null, so this branch
            // always returns.
            let mediump = match number_kind {
                NumberKind::Float => TypeId::HALF,
                NumberKind::Signed => TypeId::SHORT,
                NumberKind::Unsigned => TypeId::USHORT,
                _ => TypeId::POISON,
            };
            // Convert the mediump component type into the final vector/matrix/array type.
            let (is_array, columns, rows) = {
                let ty = ctx.pool.ty(self);
                if ty.is_array() {
                    (true, ty.columns(), 0)
                } else {
                    (false, ty.columns(), ty.rows())
                }
            };
            if is_array {
                let table = ctx
                    .symbol_table
                    .expect("applyPrecisionQualifiers: no current symbol table");
                return add_array_dimension(ctx, table, mediump, columns);
            }
            return ctx.pool.ty(mediump).to_compound(columns, rows);
        }

        let display = ctx.pool.ty(self).display_name().to_owned();
        ctx.errors.error(
            pos,
            &format!("type '{display}' does not support precision qualifiers"),
        );
        TypeId::POISON
    }

    /// `applyAccessQualifiers`.
    // Port of: src/sksl/ir/SkSLType.cpp#L1066-L1099 (chrome/m156)
    fn apply_access_qualifiers(
        self,
        ctx: &mut Context,
        flags: &mut ModifierFlags,
        pos: Position,
    ) -> Self {
        let access = *flags & (ModifierFlags::READ_ONLY | ModifierFlags::WRITE_ONLY);

        // We're going to return a whole new type, so the modifier bits must be cleared out.
        *flags &= !(ModifierFlags::READ_ONLY | ModifierFlags::WRITE_ONLY);

        if ctx.pool.ty(self).matches(TypeId::TEXTURE2D) {
            // Every texture2D must be qualified with `readonly` or `writeonly`. (Read-write
            // textures are not yet supported in WGSL.)
            if access == ModifierFlags::READ_ONLY {
                return TypeId::READ_ONLY_TEXTURE2D;
            }
            if access == ModifierFlags::WRITE_ONLY {
                return TypeId::WRITE_ONLY_TEXTURE2D;
            }
            ctx.errors.error(
                pos,
                if access.is_empty() {
                    "'texture2D' requires a 'readonly' or 'writeonly' access qualifier"
                } else {
                    "'readonly' and 'writeonly' qualifiers cannot be combined"
                },
            );
            return self;
        }

        if !access.is_empty() {
            let display = ctx.pool.ty(self).display_name().to_owned();
            let desc = access.description();
            ctx.errors.error(
                pos,
                &format!("type '{display}' does not support qualifier '{desc}'"),
            );
        }
        self
    }

    /// `clone(context, symbolTable)`: this type as a symbol of `table`. Built-in and root types
    /// are returned as they are. A program's array or struct type is added to `table` (or found
    /// there by name), and the id of the symbol in `table` is returned.
    ///
    /// # Panics
    ///
    /// If `table` already holds a non-type symbol under this type's name (Skia reinterprets it).
    // Port of: src/sksl/ir/SkSLType.cpp#L1237-L1278 (chrome/m156)
    pub fn clone_in(self, ctx: &mut Context, table: SymTabId) -> Option<Self> {
        let (kind, name, position) = {
            let ty = ctx.pool.ty(self);
            // Many types are built-ins, and exist in every SymbolTable by default.
            if ty.is_in_root_symbol_table() {
                return Some(self);
            }
            (ty.type_kind, ty.name().to_owned(), ty.position)
        };
        let builtin_code = ctx.config().is_builtin_code();
        // If the type comes from a module, it is in scope anywhere in the program.
        if !builtin_code && ctx.pool.ty(self).is_builtin() {
            return Some(self);
        }
        // Even if the type isn't a built-in, it might already exist in the table. Search by name.
        if let Some(existing) = ctx.pool.find_symbol(table, &name) {
            let SymbolId::Type(existing) = existing else {
                panic!("clone: symbol '{name}' is not a type");
            };
            debug_assert_eq!(ctx.pool.ty(existing).type_kind, kind);
            return Some(existing);
        }
        // This type needs to be cloned into the table.
        match kind {
            TypeKind::Array => {
                let (component, columns) = {
                    let ty = ctx.pool.ty(self);
                    (ty.component_type().id, ty.columns())
                };
                Some(add_array_dimension(ctx, table, component, columns))
            }
            TypeKind::Struct => {
                // We are cloning an existing struct, so there's no need to check it again.
                let (fields, depth, interface_block) = {
                    let ty = ctx.pool.ty(self);
                    (
                        ty.fields().to_vec(),
                        ty.struct_nesting_depth(),
                        ty.is_interface_block(),
                    )
                };
                let data = StructType::new(&ctx.pool, fields, depth, interface_block, builtin_code);
                let id = ctx
                    .pool
                    .add_type(Type::new_struct_type(position, name, data));
                add_symbol(ctx, table, SymbolId::Type(id));
                Some(id)
            }
            _ => {
                debug_assert!(false, "don't know how to clone type '{name}'");
                None
            }
        }
    }

    /// The value check of `checkForOutOfRangeLiteral(context, value, pos)`: reports `value` if it
    /// does not fit in this scalar type. Floats and booleans accept any value.
    // Port of: src/sksl/ir/SkSLType.cpp#L1339-L1352 (chrome/m156)
    pub fn check_for_out_of_range_literal_value(
        self,
        ctx: &mut Context,
        value: f64,
        pos: Position,
    ) -> bool {
        debug_assert!(ctx.pool.ty(self).is_scalar());
        let (is_number, min, max, display) = {
            let ty = ctx.pool.ty(self);
            if ty.is_number() {
                (
                    true,
                    ty.minimum_value(),
                    ty.maximum_value(),
                    ty.display_name().to_owned(),
                )
            } else {
                (false, 0.0, 0.0, String::new())
            }
        };
        if !is_number {
            return false;
        }
        if value >= min && value <= max {
            return false;
        }
        // We found a value that can't fit in our type. Flag it as an error.
        let msg = printf(
            "value is out of range for type '%s': %.0f",
            &[Arg::Str(&display), Arg::Float(value)],
        );
        ctx.errors.error(pos, &msg);
        true
    }

    /// `checkIfUsableInArray(context, arrayPos)`: whether an array may have this element type.
    // Port of: src/sksl/ir/SkSLType.cpp#L1354-L1369 (chrome/m156)
    pub fn check_if_usable_in_array(self, ctx: &mut Context, array_pos: Position) -> bool {
        let (is_array, is_void, opaque_not_atomic, name) = {
            let ty = ctx.pool.ty(self);
            (
                ty.is_array(),
                ty.is_void(),
                ty.is_opaque() && !ty.is_atomic(),
                ty.name().to_owned(),
            )
        };
        if is_array {
            ctx.errors
                .error(array_pos, "multi-dimensional arrays are not supported");
            return false;
        }
        if is_void {
            ctx.errors
                .error(array_pos, "type 'void' may not be used in an array");
            return false;
        }
        if opaque_not_atomic {
            ctx.errors.error(
                array_pos,
                &format!("opaque type '{name}' may not be used in an array"),
            );
            return false;
        }
        true
    }

    /// `convertArraySize(context, arrayPos, sizePos, size)` for an array size that is already a
    /// constant integer. Returns `size`, or 0 after an error.
    // Port of: src/sksl/ir/SkSLType.cpp#L1386-L1408 (chrome/m156)
    pub fn convert_array_size_value(
        self,
        ctx: &mut Context,
        array_pos: Position,
        size_pos: Position,
        size: SkslInt,
    ) -> SkslInt {
        if !self.check_if_usable_in_array(ctx, array_pos) {
            // `checkIfUsableInArray` has reported the error.
            return 0;
        }
        if size <= 0 {
            ctx.errors.error(size_pos, "array size must be positive");
            return 0;
        }
        // An interior type with an unsized array has no slot count. Such types are never valid
        // in a runtime effect.
        let too_large = {
            let ty = ctx.pool.ty(self);
            let limit = usize::try_from(VARIABLE_SLOT_LIMIT).unwrap_or(usize::MAX);
            !ty.is_or_contains_unsized_array()
                && ty
                    .slot_count()
                    .saturating_mul(usize::try_from(size).unwrap_or(usize::MAX))
                    > limit
        };
        if too_large {
            ctx.errors.error(size_pos, "array size is too large");
            return 0;
        }
        size
    }

    /// `Type::coerceExpression(expr, context)`: converts `expr` to this type, as an implicit
    /// conversion. Reports an error and returns `None` when the conversion is impossible.
    // Port of: src/sksl/ir/SkSLType.cpp#L1280-L1312 (chrome/m156)
    pub fn coerce_expression(self, ctx: &mut Context, expr: ExprId) -> Option<ExprId> {
        if Expression::is_incomplete(ctx, expr) {
            return None;
        }
        let expr_ty = ctx.pool.expression(expr).ty;
        if ctx.pool.ty(expr_ty).matches(self) {
            return Some(expr);
        }

        let pos = ctx.pool.expression(expr).position;
        let allow_narrowing = ctx.config().settings.allow_narrowing_conversions;
        if !ctx
            .pool
            .ty(expr_ty)
            .coercion_cost(self)
            .is_possible(allow_narrowing)
        {
            let msg = format!(
                "expected '{}', but found '{}'",
                ctx.pool.ty(self).display_name(),
                ctx.pool.ty(expr_ty).display_name()
            );
            ctx.errors.error(pos, &msg);
            return None;
        }

        let (is_scalar, is_compound, is_array) = {
            let t = ctx.pool.ty(self);
            (t.is_scalar(), t.is_vector() || t.is_matrix(), t.is_array())
        };
        if is_scalar {
            return Some(ConstructorScalarCast::make(ctx, pos, self, expr));
        }
        if is_compound {
            return Some(ConstructorCompoundCast::make(ctx, pos, self, expr));
        }
        if is_array {
            return Some(ConstructorArrayCast::make(ctx, pos, self, expr));
        }
        let name = ctx.pool.ty(self).display_name().to_owned();
        ctx.errors.error(pos, &format!("cannot construct '{name}'"));
        None
    }

    /// `Type::checkForOutOfRangeLiteral(context, expr)`: checks every constant slot of `expr`
    /// against this type's range. Returns true if any slot is out of range (after reporting it).
    // Port of: src/sksl/ir/SkSLType.cpp#L1314-L1337 (chrome/m156)
    pub fn check_for_out_of_range_literal(self, ctx: &mut Context, expr: ExprId) -> bool {
        let base_type = ctx.pool.ty(self).component_type().id();
        let mut found_error = false;
        // We don't need range checks for floats or booleans; any matched-type value is acceptable.
        if !ctx.pool.ty(base_type).is_number() {
            return false;
        }
        // Replace constant expressions with their corresponding values.
        let value_expr = constant_value_for_variable(&ctx.pool, expr);
        let (supports, unsized_array, num_slots, value_pos) = {
            let value = ctx.pool.expression(value_expr);
            let value_ty = ctx.pool.ty(value.ty);
            (
                value.supports_constant_values(),
                value_ty.is_unsized_array(),
                value_ty.slot_count(),
                value.position,
            )
        };
        // Unsized arrays can't have constants and fail to get a slot count.
        if supports && !unsized_array {
            // Iterate over every constant subexpression in the value.
            for slot in 0..num_slots {
                let slot_val = ctx
                    .pool
                    .expression(value_expr)
                    .get_constant_value(&ctx.pool, slot);
                // Check for Literal values that are out of range for the base type.
                if let Some(slot_val) = slot_val {
                    found_error |=
                        base_type.check_for_out_of_range_literal_value(ctx, slot_val, value_pos);
                }
            }
        }
        found_error
    }

    /// `Type::convertArraySize(context, arrayPos, size)`: the array length of `size`, which must
    /// be an integer constant. Returns 0 after an error.
    // Port of: src/sksl/ir/SkSLType.cpp#L1371-L1384 (chrome/m156)
    pub fn convert_array_size(
        self,
        ctx: &mut Context,
        array_pos: Position,
        size: ExprId,
    ) -> SkslInt {
        let Some(size) = TypeId::INT.coerce_expression(ctx, size) else {
            return 0;
        };
        let size_pos = ctx.pool.expression(size).position;
        let Some(count) = get_constant_int(&ctx.pool, size) else {
            ctx.errors.error(size_pos, "array size must be an integer");
            return 0;
        };
        self.convert_array_size_value(ctx, array_pos, size_pos, count)
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
