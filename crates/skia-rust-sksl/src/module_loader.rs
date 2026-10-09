// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLModuleLoader.{h,cpp} (chrome/m156).

//! [`ModuleLoader`]: the built-in module chain, compiled on first use.
//!
//! The chain is root (types and `sk_Caps`), shared, then gpu (which every stage module extends),
//! with frag, vert and compute on top of gpu, and public (shared minus the private types, with
//! the runtime-effect intrinsics), with the private runtime-shader module on top of public. Each
//! module is compiled from the text of the [`Flavor`]'s [`ModuleSource`](crate::flavor::ModuleSource),
//! once per process and flavour. The memoization never changes a result (`docs/design/sksl.md`
//! R9).

use std::sync::{Arc, OnceLock};

use crate::compiler::Compiler;
use crate::flavor::Flavor;
use crate::ir::{
    Layout, ModifierFlags, ProgramElementKind, SymTabId, SymbolId, SymbolTable, Type, TypeId,
    TypeKind, Variable, VariableStorage,
};
use crate::modules::{Module, ModuleType};
use crate::position::Position;
use crate::program_settings::ProgramKind;

/// `kRootTypes` of `SkSLModuleLoader.cpp`: the types every module sees.
const ROOT_TYPES: &[TypeId] = &[
    TypeId::VOID,
    TypeId::FLOAT,
    TypeId::FLOAT2,
    TypeId::FLOAT3,
    TypeId::FLOAT4,
    TypeId::HALF,
    TypeId::HALF2,
    TypeId::HALF3,
    TypeId::HALF4,
    TypeId::INT,
    TypeId::INT2,
    TypeId::INT3,
    TypeId::INT4,
    TypeId::UINT,
    TypeId::UINT2,
    TypeId::UINT3,
    TypeId::UINT4,
    TypeId::SHORT,
    TypeId::SHORT2,
    TypeId::SHORT3,
    TypeId::SHORT4,
    TypeId::USHORT,
    TypeId::USHORT2,
    TypeId::USHORT3,
    TypeId::USHORT4,
    TypeId::BOOL,
    TypeId::BOOL2,
    TypeId::BOOL3,
    TypeId::BOOL4,
    TypeId::FLOAT2X2,
    TypeId::FLOAT2X3,
    TypeId::FLOAT2X4,
    TypeId::FLOAT3X2,
    TypeId::FLOAT3X3,
    TypeId::FLOAT3X4,
    TypeId::FLOAT4X2,
    TypeId::FLOAT4X3,
    TypeId::FLOAT4X4,
    TypeId::HALF2X2,
    TypeId::HALF2X3,
    TypeId::HALF2X4,
    TypeId::HALF3X2,
    TypeId::HALF3X3,
    TypeId::HALF3X4,
    TypeId::HALF4X2,
    TypeId::HALF4X3,
    TypeId::HALF4X4,
    TypeId::SQUARE_MAT,
    TypeId::SQUARE_HMAT,
    TypeId::MAT,
    TypeId::HMAT,
    // TODO(skbug.com/40043431): generic short/ushort
    TypeId::GEN_TYPE,
    TypeId::GEN_ITYPE,
    TypeId::GEN_UTYPE,
    TypeId::GEN_HTYPE,
    TypeId::GEN_BTYPE,
    TypeId::INT_LITERAL,
    TypeId::FLOAT_LITERAL,
    TypeId::VEC,
    TypeId::IVEC,
    TypeId::UVEC,
    TypeId::HVEC,
    TypeId::SVEC,
    TypeId::USVEC,
    TypeId::BVEC,
    TypeId::COLOR_FILTER,
    TypeId::SHADER,
    TypeId::BLENDER,
];

/// `kPrivateTypes` of `SkSLModuleLoader.cpp`, with the names that `addPublicTypeAliases` hides.
const PRIVATE_TYPES: &[(TypeId, &str)] = &[
    (TypeId::SAMPLER2D, "sampler2D"),
    (TypeId::SAMPLER_EXTERNAL_OES, "samplerExternalOES"),
    (TypeId::SAMPLER2D_RECT, "sampler2DRect"),
    (TypeId::SUBPASS_INPUT, "subpassInput"),
    (TypeId::SUBPASS_INPUT_MS, "subpassInputMS"),
    (TypeId::SAMPLER, "sampler"),
    (TypeId::TEXTURE2D_SAMPLE, "$texture2D_sample"),
    (TypeId::TEXTURE2D, "texture2D"),
    (TypeId::READ_ONLY_TEXTURE2D, "readonlyTexture2D"),
    (TypeId::WRITE_ONLY_TEXTURE2D, "writeonlyTexture2D"),
    (TypeId::GEN_TEXTURE2D, "$genTexture2D"),
    (TypeId::READABLE_TEXTURE2D, "$readableTexture2D"),
    (TypeId::WRITABLE_TEXTURE2D, "$writableTexture2D"),
    (TypeId::ATOMIC_UINT, "atomicUint"),
    (TypeId::ATOMIC_UINT_ALIAS, "atomic_uint"),
];

/// The types `addPublicTypeAliases` adds to the runtime-effect modules, so that they read more
/// like GLSL.
const PUBLIC_ALIASES: &[TypeId] = &[
    TypeId::VEC2,
    TypeId::VEC3,
    TypeId::VEC4,
    TypeId::IVEC2,
    TypeId::IVEC3,
    TypeId::IVEC4,
    TypeId::UVEC2,
    TypeId::UVEC3,
    TypeId::UVEC4,
    TypeId::BVEC2,
    TypeId::BVEC3,
    TypeId::BVEC4,
    TypeId::MAT2,
    TypeId::MAT3,
    TypeId::MAT4,
    TypeId::MAT2X2,
    TypeId::MAT2X3,
    TypeId::MAT2X4,
    TypeId::MAT3X2,
    TypeId::MAT3X3,
    TypeId::MAT3X4,
    TypeId::MAT4X2,
    TypeId::MAT4X3,
    TypeId::MAT4X4,
];

/// `ModuleLoader::Impl::makeRootSymbolTable`: the root module, with the built-in types and
/// `sk_Caps`.
// Port of: src/sksl/SkSLModuleLoader.cpp#L223-L255 (chrome/m156)
fn make_root_module() -> Arc<Module> {
    let mut pool = crate::ir::IrPool::new();
    let table = pool.add_symbol_table(SymbolTable::new(None, true));
    for &ty in ROOT_TYPES {
        pool.inject_symbol(table, SymbolId::Type(ty));
    }
    for &(ty, _) in PRIVATE_TYPES {
        pool.inject_symbol(table, SymbolId::Type(ty));
    }
    // sk_Caps is "builtin", but all references to it are resolved to Settings, so we don't need
    // to treat it as builtin (no need to clone it into the Program).
    let sk_caps = Variable::make(
        &mut pool,
        Position::default(),
        Position::default(),
        Layout::new(),
        ModifierFlags::empty(),
        TypeId::SK_CAPS,
        "sk_Caps",
        String::new(),
        false,
        VariableStorage::Global,
    );
    pool.inject_symbol(table, SymbolId::Variable(sk_caps));
    Arc::new(Module {
        parent: None,
        pool: pool.freeze(),
        symbols: table,
        elements: Vec::new(),
        module_type: ModuleType::Unknown,
        source: Arc::from(&b""[..]),
    })
}

/// `ModuleLoader::addPublicTypeAliases`: the GLSL-style aliases, and the private type names
/// aliased to `invalid` so that code cannot use them as variable names.
// Port of: src/sksl/SkSLModuleLoader.cpp#L192-L219 (chrome/m156)
pub fn add_public_type_aliases(parts: &mut crate::compiler::ModuleParts) {
    inject_public_type_aliases(&mut parts.pool, parts.symbols);
}

/// The body of [`add_public_type_aliases`], on a module's pool and symbol table.
fn inject_public_type_aliases(pool: &mut crate::ir::IrPool, table: SymTabId) {
    for &ty in PUBLIC_ALIASES {
        pool.inject_symbol(table, SymbolId::Type(ty));
    }
    // Hide all the private symbols by aliasing them all to "invalid". This will prevent code from
    // using built-in names like `sampler2D` as variable names.
    for &(_, name) in PRIVATE_TYPES {
        let hidden = pool.add_type(Type::make_alias_type(
            name,
            TypeId::INVALID,
            "O",
            TypeKind::Other,
        ));
        pool.inject_symbol(table, SymbolId::Type(hidden));
    }
}

/// `compile_and_shrink`: compiles a module, drops the function prototypes (they are in the symbol
/// table already, and only matter for recreating the input verbatim), and freezes it. A module
/// that does not compile is a bug in the embedded text, so this panics, as Skia aborts.
// Port of: src/sksl/SkSLModuleLoader.cpp#L95-L120 (chrome/m156)
fn compile_and_shrink(
    flavor: Flavor,
    kind: ProgramKind,
    module_type: ModuleType,
    parent: &Arc<Module>,
    public_aliases: bool,
) -> Arc<Module> {
    let mut compiler = Compiler::with_flavor(flavor);
    let source = module_type.text(flavor.module_source()).as_bytes();
    let Some(mut parts) = compiler.compile_module_parts(kind, module_type, source, parent) else {
        panic!(
            "Unable to load module {}:\n{}",
            module_type.name(),
            String::from_utf8_lossy(&compiler.error_text_bytes(true))
        );
    };
    assert!(
        compiler.optimize_module_after_loading(kind, &mut parts, parent),
        "Unable to optimize module {}",
        module_type.name()
    );
    // We need to preserve functions, globals, interface blocks and structs; the prototypes go.
    parts.elements.retain(|&element| {
        match parts.pool.element(element).kind {
            ProgramElementKind::Function(_)
            | ProgramElementKind::GlobalVar(_)
            | ProgramElementKind::InterfaceBlock(_)
            | ProgramElementKind::StructDefinition(_) => true,
            // Prototypes are already in the symbol table, so the element isn't needed anymore.
            // No other kind is expected in a module (Skia's SkDEBUGFAILF), and any is dropped.
            _ => false,
        }
    });
    if public_aliases {
        inject_public_type_aliases(&mut parts.pool, parts.symbols);
    }
    parts.freeze(parent.clone())
}

/// `SkSL::ModuleLoader`: the built-in modules of one flavour, each loaded on first use.
///
/// Skia's loader is a process-wide singleton behind a mutex. Here each flavour has one loader,
/// and every module is a `OnceLock`, so the loads are memoized and never run twice.
// Port of: src/sksl/SkSLModuleLoader.h#L22-L56 (chrome/m156)
#[doc(alias = "SkSL::ModuleLoader")]
#[derive(Debug)]
pub struct ModuleLoader {
    flavor: Flavor,
    root: OnceLock<Arc<Module>>,
    shared: OnceLock<Arc<Module>>,
    gpu: OnceLock<Arc<Module>>,
    fragment: OnceLock<Arc<Module>>,
    vertex: OnceLock<Arc<Module>>,
    compute: OnceLock<Arc<Module>>,
    public: OnceLock<Arc<Module>>,
    private_rt_shader: OnceLock<Arc<Module>>,
}

static LIBRARY: ModuleLoader = ModuleLoader::new(Flavor::Library);
static STANDALONE: ModuleLoader = ModuleLoader::new(Flavor::Standalone);

impl ModuleLoader {
    const fn new(flavor: Flavor) -> Self {
        Self {
            flavor,
            root: OnceLock::new(),
            shared: OnceLock::new(),
            gpu: OnceLock::new(),
            fragment: OnceLock::new(),
            vertex: OnceLock::new(),
            compute: OnceLock::new(),
            public: OnceLock::new(),
            private_rt_shader: OnceLock::new(),
        }
    }

    /// The loader of `flavor`.
    #[must_use]
    pub fn for_flavor(flavor: Flavor) -> &'static Self {
        match flavor {
            Flavor::Library => &LIBRARY,
            Flavor::Standalone => &STANDALONE,
        }
    }

    /// `rootModule()`.
    #[must_use]
    pub fn root(&self) -> Arc<Module> {
        self.root.get_or_init(make_root_module).clone()
    }

    /// `loadSharedModule`: root intrinsics (`sksl_shared`).
    // Port of: src/sksl/SkSLModuleLoader.cpp#L302-L310 (chrome/m156)
    #[must_use]
    pub fn shared(&self) -> Arc<Module> {
        self.shared
            .get_or_init(|| {
                compile_and_shrink(
                    self.flavor,
                    ProgramKind::Fragment,
                    ModuleType::SkslShared,
                    &self.root(),
                    false,
                )
            })
            .clone()
    }

    /// `loadGPUModule`: the GPU-only intrinsics (`sksl_gpu`).
    // Port of: src/sksl/SkSLModuleLoader.cpp#L312-L319 (chrome/m156)
    #[must_use]
    pub fn gpu(&self) -> Arc<Module> {
        self.gpu
            .get_or_init(|| {
                compile_and_shrink(
                    self.flavor,
                    ProgramKind::Fragment,
                    ModuleType::SkslGpu,
                    &self.shared(),
                    false,
                )
            })
            .clone()
    }

    /// `loadFragmentModule`: the fragment stage (`sksl_frag`).
    // Port of: src/sksl/SkSLModuleLoader.cpp#L321-L328 (chrome/m156)
    #[must_use]
    pub fn fragment(&self) -> Arc<Module> {
        self.fragment
            .get_or_init(|| {
                compile_and_shrink(
                    self.flavor,
                    ProgramKind::Fragment,
                    ModuleType::SkslFrag,
                    &self.gpu(),
                    false,
                )
            })
            .clone()
    }

    /// `loadVertexModule`: the vertex stage (`sksl_vert`).
    // Port of: src/sksl/SkSLModuleLoader.cpp#L330-L337 (chrome/m156)
    #[must_use]
    pub fn vertex(&self) -> Arc<Module> {
        self.vertex
            .get_or_init(|| {
                compile_and_shrink(
                    self.flavor,
                    ProgramKind::Vertex,
                    ModuleType::SkslVert,
                    &self.gpu(),
                    false,
                )
            })
            .clone()
    }

    /// `loadComputeModule`: the compute stage (`sksl_compute`).
    // Port of: src/sksl/SkSLModuleLoader.cpp#L339-L346 (chrome/m156)
    #[must_use]
    pub fn compute(&self) -> Arc<Module> {
        self.compute
            .get_or_init(|| {
                compile_and_shrink(
                    self.flavor,
                    ProgramKind::Compute,
                    ModuleType::SkslCompute,
                    &self.gpu(),
                    false,
                )
            })
            .clone()
    }

    /// `loadPublicModule`: the runtime-effect intrinsics (`sksl_public`), with the public type
    /// aliases.
    // Port of: src/sksl/SkSLModuleLoader.cpp#L281-L292 (chrome/m156)
    #[must_use]
    pub fn public(&self) -> Arc<Module> {
        self.public
            .get_or_init(|| {
                compile_and_shrink(
                    self.flavor,
                    ProgramKind::Fragment,
                    ModuleType::SkslPublic,
                    &self.shared(),
                    true,
                )
            })
            .clone()
    }

    /// `loadPrivateRTShaderModule`: the runtime-shader helpers (`sksl_rt_shader`), for the private
    /// runtime-effect kinds.
    // Port of: src/sksl/SkSLModuleLoader.cpp#L294-L300 (chrome/m156)
    #[must_use]
    pub fn private_rt_shader(&self) -> Arc<Module> {
        self.private_rt_shader
            .get_or_init(|| {
                compile_and_shrink(
                    self.flavor,
                    ProgramKind::Fragment,
                    ModuleType::SkslRtShader,
                    &self.public(),
                    false,
                )
            })
            .clone()
    }
}
