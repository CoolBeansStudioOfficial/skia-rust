// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp (the anonymous-namespace helpers).

//! WGSL type names, reserved words and builtin tables.

use std::collections::HashSet;
use std::sync::LazyLock;

use crate::context::Context;
use crate::ir::{Layout, LayoutFlags, NumberKind, TypeId, TypeKind, TypeRef, Variable};
use crate::operator::{Operator, OperatorKind};
use crate::program_settings::{ProgramConfig, ProgramKind};

/// `kSamplerSuffix`.
pub(super) const SAMPLER_SUFFIX: &str = "_Sampler";
/// `kTextureSuffix`.
pub(super) const TEXTURE_SUFFIX: &str = "_Texture";

/// `SK_POSITION_BUILTIN` (`SkSLCompiler.h`).
pub(super) const SK_POSITION_BUILTIN: i32 = 0;
/// `SK_POINTSIZE_BUILTIN`.
pub(super) const SK_POINTSIZE_BUILTIN: i32 = 1;
/// `SK_FRAGCOORD_BUILTIN`.
const SK_FRAGCOORD_BUILTIN: i32 = 15;
/// `SK_CLOCKWISE_BUILTIN`.
const SK_CLOCKWISE_BUILTIN: i32 = 17;
/// `SK_SAMPLEMASKIN_BUILTIN`.
const SK_SAMPLEMASKIN_BUILTIN: i32 = 20;
/// `SK_NUMWORKGROUPS_BUILTIN`.
const SK_NUMWORKGROUPS_BUILTIN: i32 = 24;
/// `SK_WORKGROUPID_BUILTIN`.
const SK_WORKGROUPID_BUILTIN: i32 = 26;
/// `SK_LOCALINVOCATIONID_BUILTIN`.
const SK_LOCALINVOCATIONID_BUILTIN: i32 = 27;
/// `SK_GLOBALINVOCATIONID_BUILTIN`.
const SK_GLOBALINVOCATIONID_BUILTIN: i32 = 28;
/// `SK_LOCALINVOCATIONINDEX_BUILTIN`.
const SK_LOCALINVOCATIONINDEX_BUILTIN: i32 = 29;
/// `SK_VERTEXID_BUILTIN`.
const SK_VERTEXID_BUILTIN: i32 = 42;
/// `SK_INSTANCEID_BUILTIN`.
const SK_INSTANCEID_BUILTIN: i32 = 43;
/// `SK_LASTFRAGCOLOR_BUILTIN`.
const SK_LASTFRAGCOLOR_BUILTIN: i32 = 10008;
/// `SK_SAMPLEMASK_BUILTIN`.
const SK_SAMPLEMASK_BUILTIN: i32 = 10020;

/// `WGSLCodeGenerator::Builtin`: see <https://www.w3.org/TR/WGSL/#builtin-values>.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Builtin {
    // Vertex stage:
    VertexIndex,
    InstanceIndex,
    Position,
    // Fragment stage:
    LastFragColor,
    FrontFacing,
    #[allow(dead_code)] // Skia declares it; `builtin_from_sksl_name` never returns it.
    SampleIndex,
    #[allow(dead_code)] // Likewise.
    FragDepth,
    SampleMaskIn,
    SampleMask,
    // Compute stage:
    LocalInvocationId,
    LocalInvocationIndex,
    GlobalInvocationId,
    WorkgroupId,
    NumWorkgroups,
}

/// `WGSLCodeGenerator::Delimiter`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Delimiter {
    Comma,
    #[allow(dead_code)] // Skia declares it; no caller passes it.
    Semicolon,
    #[allow(dead_code)] // Skia declares it; no caller passes it.
    None,
}

/// `PtrAddressSpace`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PtrAddressSpace {
    Function,
    #[allow(dead_code)] // Skia declares these too; no caller passes them.
    Private,
    #[allow(dead_code)]
    Storage,
}

/// `operator_name`: `LOGICALXOR` is `!=` in WGSL.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L477-L482 (chrome/m156)
pub(super) fn operator_name(op: Operator) -> &'static str {
    match op.kind() {
        OperatorKind::LogicalXor => " != ",
        _ => op.operator_name(),
    }
}

/// The words `is_reserved_word` rejects.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L484-L731 (chrome/m156)
const RESERVED_WORDS: &[&str] = &[
    // Used by SkSL:
    "FSIn",
    "FSOut",
    "VSIn",
    "VSOut",
    "CSIn",
    "_globalUniforms",
    "_GlobalUniforms",
    "_return",
    "_stageIn",
    "_stageOut",
    // Keywords: https://www.w3.org/TR/WGSL/#keyword-summary
    "alias",
    "break",
    "case",
    "const",
    "const_assert",
    "continue",
    "continuing",
    "default",
    "diagnostic",
    "discard",
    "else",
    "enable",
    "false",
    "fn",
    "for",
    "if",
    "let",
    "loop",
    "override",
    "requires",
    "return",
    "struct",
    "switch",
    "true",
    "var",
    "while",
    // Pre-declared types: https://www.w3.org/TR/WGSL/#predeclared-types
    "bool",
    "f16",
    "f32",
    "i32",
    "u32",
    // ... and pre-declared type generators:
    "array",
    "atomic",
    "mat2x2",
    "mat2x3",
    "mat2x4",
    "mat3x2",
    "mat3x3",
    "mat3x4",
    "mat4x2",
    "mat4x3",
    "mat4x4",
    "ptr",
    "texture_1d",
    "texture_2d",
    "texture_2d_array",
    "texture_3d",
    "texture_cube",
    "texture_cube_array",
    "texture_multisampled_2d",
    "texture_storage_1d",
    "texture_storage_2d",
    "texture_storage_2d_array",
    "texture_storage_3d",
    "vec2",
    "vec3",
    "vec4",
    // Pre-declared enumerants: https://www.w3.org/TR/WGSL/#predeclared-enumerants
    "read",
    "write",
    "read_write",
    "function",
    "private",
    "workgroup",
    "uniform",
    "storage",
    "perspective",
    "linear",
    "flat",
    "center",
    "centroid",
    "sample",
    "vertex_index",
    "instance_index",
    "position",
    "front_facing",
    "frag_depth",
    "local_invocation_id",
    "local_invocation_index",
    "global_invocation_id",
    "workgroup_id",
    "num_workgroups",
    "sample_index",
    "sample_mask",
    "rgba8unorm",
    "rgba8snorm",
    "rgba8uint",
    "rgba8sint",
    "rgba16uint",
    "rgba16sint",
    "rgba16float",
    "r32uint",
    "r32sint",
    "r32float",
    "rg32uint",
    "rg32sint",
    "rg32float",
    "rgba32uint",
    "rgba32sint",
    "rgba32float",
    "bgra8unorm",
    // Reserved words: https://www.w3.org/TR/WGSL/#reserved-words
    "_",
    "NULL",
    "Self",
    "abstract",
    "active",
    "alignas",
    "alignof",
    "as",
    "asm",
    "asm_fragment",
    "async",
    "attribute",
    "auto",
    "await",
    "become",
    "binding_array",
    "cast",
    "catch",
    "class",
    "co_await",
    "co_return",
    "co_yield",
    "coherent",
    "column_major",
    "common",
    "compile",
    "compile_fragment",
    "concept",
    "const_cast",
    "consteval",
    "constexpr",
    "constinit",
    "crate",
    "debugger",
    "decltype",
    "delete",
    "demote",
    "demote_to_helper",
    "do",
    "dynamic_cast",
    "enum",
    "explicit",
    "export",
    "extends",
    "extern",
    "external",
    "fallthrough",
    "filter",
    "final",
    "finally",
    "friend",
    "from",
    "fxgroup",
    "get",
    "goto",
    "groupshared",
    "highp",
    "impl",
    "implements",
    "import",
    "inline",
    "instanceof",
    "interface",
    "layout",
    "lowp",
    "macro",
    "macro_rules",
    "match",
    "mediump",
    "meta",
    "mod",
    "module",
    "move",
    "mut",
    "mutable",
    "namespace",
    "new",
    "nil",
    "noexcept",
    "noinline",
    "nointerpolation",
    "noperspective",
    "null",
    "nullptr",
    "of",
    "operator",
    "package",
    "packoffset",
    "partition",
    "pass",
    "patch",
    "pixelfragment",
    "precise",
    "precision",
    "premerge",
    "priv",
    "protected",
    "pub",
    "public",
    "readonly",
    "ref",
    "regardless",
    "register",
    "reinterpret_cast",
    "require",
    "resource",
    "restrict",
    "self",
    "set",
    "shared",
    "sizeof",
    "smooth",
    "snorm",
    "static",
    "static_assert",
    "static_cast",
    "std",
    "subroutine",
    "super",
    "target",
    "template",
    "this",
    "thread_local",
    "throw",
    "trait",
    "try",
    "type",
    "typedef",
    "typeid",
    "typename",
    "typeof",
    "union",
    "unless",
    "unorm",
    "unsafe",
    "unsized",
    "use",
    "using",
    "varying",
    "virtual",
    "volatile",
    "wgsl",
    "where",
    "with",
    "writeonly",
    "yield",
];

/// `is_reserved_word`. Skia's `THashSet` is only queried, never iterated.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L484-L731 (chrome/m156)
pub(super) fn is_reserved_word(word: &str) -> bool {
    static SET: LazyLock<HashSet<&'static str>> =
        LazyLock::new(|| RESERVED_WORDS.iter().copied().collect());
    SET.contains(word)
}

/// `pipeline_struct_prefix`.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L733-L745 (chrome/m156)
pub(super) fn pipeline_struct_prefix(kind: ProgramKind) -> &'static str {
    if ProgramConfig::is_vertex(kind) {
        return "VS";
    }
    if ProgramConfig::is_fragment(kind) {
        return "FS";
    }
    if ProgramConfig::is_compute(kind) {
        return "CS";
    }
    // Compute programs don't have stage-in/stage-out pipeline structs.
    ""
}

/// `address_space_to_str`.
pub(super) fn address_space_to_str(space: PtrAddressSpace) -> &'static str {
    match space {
        PtrAddressSpace::Function => "function",
        PtrAddressSpace::Private => "private",
        PtrAddressSpace::Storage => "storage",
    }
}

/// `type_is_low_precision(type, f16Available)`: for now only half vs. float, not short vs int.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L763-L766 (chrome/m156)
pub(super) fn type_is_low_precision_with(ty: TypeRef<'_>, f16_available: bool) -> bool {
    f16_available && (ty.component_type().is_float() && !ty.high_precision())
}

/// `type_is_low_precision(type, context)`.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L768-L770 (chrome/m156)
pub(super) fn type_is_low_precision(ctx: &Context, ty: TypeId) -> bool {
    type_is_low_precision_with(ctx.pool.ty(ty), !ctx.config().settings.force_high_precision)
}

/// `to_scalar_type`.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L772-L794 (chrome/m156)
fn to_scalar_type(ty: TypeRef<'_>, f16_available: bool) -> String {
    debug_assert!(ty.type_kind == TypeKind::Scalar);
    match ty.number_kind() {
        NumberKind::Float => {
            // Floating point numbers can map to f32 or f16 depending on whether or not the
            // shader-f16 wgsl extension is supported.
            if type_is_low_precision_with(ty, f16_available) {
                "f16"
            } else {
                "f32"
            }
            .to_owned()
        }
        NumberKind::Signed => "i32".to_owned(),
        NumberKind::Unsigned => "u32".to_owned(),
        NumberKind::Boolean => "bool".to_owned(),
        NumberKind::Nonnumeric => ty.name().to_owned(),
    }
}

/// `to_wgsl_type`: converts a `SkSL` type to a WGSL type. Handles all plain types except structure
/// types (see <https://www.w3.org/TR/WGSL/#plain-types-section>).
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L796-L870 (chrome/m156)
pub(super) fn to_wgsl_type(
    ctx: &Context,
    raw: TypeId,
    layout: Option<&Layout>,
    force_high_precision: bool,
) -> String {
    let ty = ctx.pool.ty(raw).resolve().scalar_type_for_literal();
    let f16_available = !ctx.config().settings.force_high_precision && !force_high_precision;

    match ty.type_kind {
        TypeKind::Scalar => return to_scalar_type(ty, f16_available),
        TypeKind::Atomic => {
            debug_assert!(ty.matches(TypeId::ATOMIC_UINT));
            return "atomic<u32>".to_owned();
        }
        TypeKind::Vector => {
            let ct = to_scalar_type(ty.component_type(), f16_available);
            return format!("vec{}<{}>", ty.columns(), ct);
        }
        TypeKind::Matrix => {
            let ct = to_scalar_type(ty.component_type(), f16_available);
            return format!("mat{}x{}<{}>", ty.columns(), ty.rows(), ct);
        }
        TypeKind::Array => {
            let mut result = String::from("array<");
            result.push_str(&to_wgsl_type(ctx, ty.component_type().id(), layout, false));
            if !ty.is_unsized_array() {
                result.push_str(", ");
                result.push_str(&ty.columns().to_string());
            }
            result.push('>');
            return result;
        }
        TypeKind::Texture => {
            if ty.matches(TypeId::WRITE_ONLY_TEXTURE2D) {
                let result = "texture_storage_2d<";
                // Write-only storage texture types require a pixel format, which is in the layout.
                let flags = layout.map_or(LayoutFlags::empty(), |l| l.flags);
                let pixel_format = flags & LayoutFlags::ALL_PIXEL_FORMATS;
                return if pixel_format == LayoutFlags::RGBA8 {
                    format!("{result}rgba8unorm, write>")
                } else if pixel_format == LayoutFlags::RGBA32F {
                    format!("{result}rgba32float, write>")
                } else if pixel_format == LayoutFlags::R32F {
                    format!("{result}r32float, write>")
                } else {
                    // The front-end should have rejected this.
                    format!("{result}write>")
                };
            }
            if ty.matches(TypeId::READ_ONLY_TEXTURE2D) {
                return "texture_2d<f32>".to_owned();
            }
        }
        _ => {}
    }
    ty.name().to_owned()
}

/// `to_wgsl_type(context, type)` with no layout and no forced precision.
pub(super) fn to_wgsl_type_simple(ctx: &Context, ty: TypeId) -> String {
    to_wgsl_type(ctx, ty, None, false)
}

/// `to_ptr_type`.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L872-L878 (chrome/m156)
pub(super) fn to_ptr_type(
    ctx: &Context,
    ty: TypeId,
    layout: Option<&Layout>,
    address_space: PtrAddressSpace,
) -> String {
    format!(
        "ptr<{}, {}>",
        address_space_to_str(address_space),
        to_wgsl_type(ctx, ty, layout, false)
    )
}

/// `wgsl_builtin_name`.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L880-L913 (chrome/m156)
pub(super) fn wgsl_builtin_name(builtin: Builtin) -> &'static str {
    match builtin {
        Builtin::VertexIndex => "@builtin(vertex_index)",
        Builtin::InstanceIndex => "@builtin(instance_index)",
        Builtin::Position => "@builtin(position)",
        Builtin::LastFragColor => "@color(0)",
        Builtin::FrontFacing => "@builtin(front_facing)",
        Builtin::SampleIndex => "@builtin(sample_index)",
        Builtin::FragDepth => "@builtin(frag_depth)",
        Builtin::SampleMask | Builtin::SampleMaskIn => "@builtin(sample_mask)",
        Builtin::LocalInvocationId => "@builtin(local_invocation_id)",
        Builtin::LocalInvocationIndex => "@builtin(local_invocation_index)",
        Builtin::GlobalInvocationId => "@builtin(global_invocation_id)",
        Builtin::WorkgroupId => "@builtin(workgroup_id)",
        Builtin::NumWorkgroups => "@builtin(num_workgroups)",
    }
}

/// `wgsl_builtin_type`.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L915-L952 (chrome/m156)
pub(super) fn wgsl_builtin_type(builtin: Builtin, ctx: &Context) -> &'static str {
    match builtin {
        Builtin::VertexIndex
        | Builtin::InstanceIndex
        | Builtin::SampleIndex
        | Builtin::SampleMask
        | Builtin::SampleMaskIn
        | Builtin::LocalInvocationIndex => "u32",
        Builtin::Position => "vec4<f32>",
        Builtin::LastFragColor => {
            // The last frag color is declared as a half4 in SkSL, so its type depends on f16
            // being available or not.
            if ctx.config().settings.force_high_precision {
                "vec4<f32>"
            } else {
                "vec4<f16>"
            }
        }
        Builtin::FrontFacing => "bool",
        Builtin::FragDepth => "f32",
        Builtin::LocalInvocationId
        | Builtin::GlobalInvocationId
        | Builtin::WorkgroupId
        | Builtin::NumWorkgroups => "vec3<u32>",
    }
}

/// `needs_builtin_type_conversion`: some built-in variables have a type that differs from their
/// `SkSL` counterpart (e.g. signed vs unsigned integer). The WGSL type of the conversion target, if
/// conversion is needed.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L954-L964 (chrome/m156)
pub(super) fn needs_builtin_type_conversion(v: &Variable) -> Option<&'static str> {
    match v.layout.builtin {
        SK_VERTEXID_BUILTIN | SK_INSTANCEID_BUILTIN => Some("i32"),
        _ => None,
    }
}

/// `builtin_from_sksl_name`: maps a `SkSL` builtin flag to a WGSL builtin kind. `None` if `builtin`
/// is not supported for WGSL.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L966-L1006 (chrome/m156)
pub(super) fn builtin_from_sksl_name(builtin: i32) -> Option<Builtin> {
    Some(match builtin {
        SK_POSITION_BUILTIN | SK_FRAGCOORD_BUILTIN => Builtin::Position,
        SK_VERTEXID_BUILTIN => Builtin::VertexIndex,
        SK_INSTANCEID_BUILTIN => Builtin::InstanceIndex,
        SK_LASTFRAGCOLOR_BUILTIN => Builtin::LastFragColor,
        SK_CLOCKWISE_BUILTIN => Builtin::FrontFacing,
        SK_SAMPLEMASKIN_BUILTIN => Builtin::SampleMaskIn,
        SK_SAMPLEMASK_BUILTIN => Builtin::SampleMask,
        SK_NUMWORKGROUPS_BUILTIN => Builtin::NumWorkgroups,
        SK_WORKGROUPID_BUILTIN => Builtin::WorkgroupId,
        SK_LOCALINVOCATIONID_BUILTIN => Builtin::LocalInvocationId,
        SK_GLOBALINVOCATIONID_BUILTIN => Builtin::GlobalInvocationId,
        SK_LOCALINVOCATIONINDEX_BUILTIN => Builtin::LocalInvocationIndex,
        _ => return None,
    })
}

/// `delimiter_to_str`.
pub(super) fn delimiter_to_str(delimiter: Delimiter) -> &'static str {
    match delimiter {
        Delimiter::Comma => ",",
        Delimiter::Semicolon => ";",
        Delimiter::None => "",
    }
}
