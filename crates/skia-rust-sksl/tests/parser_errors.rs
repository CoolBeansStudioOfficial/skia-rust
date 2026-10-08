// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Parse-and-convert tests of the `SkSL` parser (task S10): each case is an input from
//! `resources/sksl/errors` and the golden from `tests/sksl/errors` that `skslc` wrote for it.
//! The goldens list the exact error text, so the parser, the checks of the IR conversions it
//! drives and the error formatting are compared byte for byte.
//!
//! The harness is the part of Skia's `Compiler::initializeContext`/`ModuleLoader` that parsing
//! needs: the root symbol table, the built-in modules (compiled with the parser itself, from the
//! original sources `skslc` reads) and a program context on top of them. Task S11 replaces it with
//! the real driver.
//!
//! A golden can also contain errors from `Compiler::finalize` or `optimize`, which run after a
//! successful parse. Those cases are listed in `DEFERRED_TO_FINALIZATION`; the test checks that
//! the parser accepted them (reported no error), so the remaining errors can only come from the
//! driver (S11).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use skia_rust_sksl::ModuleSource;
use skia_rust_sksl::compiler::Compiler;
use skia_rust_sksl::ir::{
    IrPool, Layout, ModifierFlags, SymTabId, SymbolId, SymbolTable, Type, TypeId, TypeKind,
    Variable, VariableStorage,
};
use skia_rust_sksl::modules::{Module, ModuleType};
use skia_rust_sksl::parser::Parser;
use skia_rust_sksl::position::Position;
use skia_rust_sksl::program_settings::{ProgramConfig, ProgramKind, ProgramSettings};

/// `kRootTypes` of `SkSLModuleLoader.cpp`.
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

/// `kPrivateTypes` of `SkSLModuleLoader.cpp`, with the names `addPublicTypeAliases` hides.
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

/// The aliases `addPublicTypeAliases` adds to the runtime-effect modules.
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

/// `ModuleLoader::Impl::makeRootSymbolTable`.
fn root_module() -> Arc<Module> {
    let mut pool = IrPool::new();
    let table = pool.add_symbol_table(SymbolTable::new(None, true));
    for &ty in ROOT_TYPES {
        pool.inject_symbol(table, SymbolId::Type(ty));
    }
    for &(ty, _) in PRIVATE_TYPES {
        pool.inject_symbol(table, SymbolId::Type(ty));
    }
    // sk_Caps is "builtin", but all references to it are resolved to Settings.
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
    })
}

fn settings() -> ProgramSettings {
    ProgramSettings {
        use_memory_pool: false,
        ..ProgramSettings::default()
    }
}

/// `Compiler::compileModule` for the original module text, without the post-load inlining.
fn compile_module(
    kind: ProgramKind,
    module_type: ModuleType,
    parent: &Arc<Module>,
    public_aliases: bool,
) -> Arc<Module> {
    let text = module_type.text(ModuleSource::Original);
    let mut compiler = Compiler::new();
    let ctx = compiler.context_mut();
    ctx.pool = IrPool::extend(parent.pool.clone());
    ctx.config = Some(ProgramConfig::new(module_type, kind, settings()));
    ctx.module = Some(parent.clone());
    ctx.errors.set_source(Arc::from(text));
    let mut globals = SymbolTable::new(Some(parent.symbols), true);
    globals.mark_module_boundary();
    let table = ctx.pool.add_symbol_table(globals);
    ctx.symbol_table = Some(table);
    let elements = Parser::new(ctx, settings(), kind, text.as_bytes()).module_inheriting_from();
    if public_aliases {
        add_public_type_aliases(&mut compiler, table);
    }
    assert_eq!(
        compiler.error_count(),
        0,
        "unexpected errors compiling {}:\n{}",
        module_type.name(),
        compiler.error_text(true)
    );
    let pool = std::mem::take(&mut compiler.context_mut().pool).freeze();
    Arc::new(Module {
        parent: Some(parent.clone()),
        pool,
        symbols: table,
        elements,
        module_type,
    })
}

/// `ModuleLoader::addPublicTypeAliases`.
fn add_public_type_aliases(compiler: &mut Compiler, table: SymTabId) {
    let pool = &mut compiler.context_mut().pool;
    for &ty in PUBLIC_ALIASES {
        pool.inject_symbol(table, SymbolId::Type(ty));
    }
    // Hide all the private symbols by aliasing them all to "invalid".
    for &(_, name) in PRIVATE_TYPES {
        let hidden = pool.add_type(Type::make_alias_type(
            // The names are 'static in this table.
            name_static(name),
            TypeId::INVALID,
            "O",
            TypeKind::Other,
        ));
        pool.inject_symbol(table, SymbolId::Type(hidden));
    }
}

fn name_static(name: &str) -> &'static str {
    PRIVATE_TYPES
        .iter()
        .find(|(_, n)| *n == name)
        .map(|(_, n)| *n)
        .expect("a private type name")
}

macro_rules! cached_module {
    ($name:ident, $parent:ident, $kind:expr, $ty:expr, $aliases:expr) => {
        fn $name() -> Arc<Module> {
            static CELL: OnceLock<Arc<Module>> = OnceLock::new();
            CELL.get_or_init(|| compile_module($kind, $ty, &$parent(), $aliases))
                .clone()
        }
    };
}

fn root() -> Arc<Module> {
    static CELL: OnceLock<Arc<Module>> = OnceLock::new();
    CELL.get_or_init(root_module).clone()
}

cached_module!(
    shared,
    root,
    ProgramKind::Fragment,
    ModuleType::SkslShared,
    false
);
cached_module!(
    gpu,
    shared,
    ProgramKind::Fragment,
    ModuleType::SkslGpu,
    false
);
cached_module!(
    frag,
    gpu,
    ProgramKind::Fragment,
    ModuleType::SkslFrag,
    false
);
cached_module!(vert, gpu, ProgramKind::Vertex, ModuleType::SkslVert, false);
cached_module!(
    compute,
    gpu,
    ProgramKind::Compute,
    ModuleType::SkslCompute,
    false
);
cached_module!(
    public,
    shared,
    ProgramKind::Fragment,
    ModuleType::SkslPublic,
    true
);

/// `Compiler::moduleForProgramKind`.
fn module_for_kind(kind: ProgramKind) -> Arc<Module> {
    match kind {
        ProgramKind::Fragment => frag(),
        ProgramKind::Vertex => vert(),
        ProgramKind::Compute => compute(),
        ProgramKind::RuntimeColorFilter
        | ProgramKind::RuntimeShader
        | ProgramKind::RuntimeBlender
        | ProgramKind::MeshVertex
        | ProgramKind::MeshFragment => public(),
        other => panic!("no module for {other:?} in this harness"),
    }
}

/// `Compiler::convertProgram` up to the parse (the part before `releaseProgram`). Returns the
/// compiler, which holds the error text, and whether the parse succeeded.
fn parse_with(kind: ProgramKind, text: &[u8]) -> (Compiler, bool) {
    let module = module_for_kind(kind);
    let mut compiler = Compiler::new();
    // The values `skslc` gives so the RT-flip checks pass.
    let mut program_settings = ProgramSettings {
        rt_flip_offset: 16384,
        rt_flip_set: 0,
        rt_flip_binding: 0,
        ..ProgramSettings::default()
    };
    // `Compiler::FinalizeSettings`: runtime effects always allow narrowing conversions.
    if ProgramConfig::is_runtime_effect(kind) {
        program_settings.allow_narrowing_conversions = true;
    }
    let ctx = compiler.context_mut();
    ctx.pool = IrPool::extend(module.pool.clone());
    ctx.config = Some(ProgramConfig::new(
        ModuleType::Program,
        kind,
        program_settings,
    ));
    ctx.module = Some(module.clone());
    ctx.errors.set_source_bytes(Arc::from(text));
    let mut globals = SymbolTable::new(Some(module.symbols), false);
    globals.mark_module_boundary();
    let table = ctx.pool.add_symbol_table(globals);
    ctx.symbol_table = Some(table);
    let elements = Parser::new(ctx, program_settings, kind, text).program_inheriting_from();
    (compiler, elements.is_some())
}

/// The result of parsing a program: whether it parsed, and the compiler's error text.
struct Parsed {
    accepted: bool,
    error_text: Vec<u8>,
}

fn parse_program(kind: ProgramKind, text: &[u8]) -> Parsed {
    let (mut compiler, accepted) = parse_with(kind, text);
    Parsed {
        accepted,
        error_text: compiler.error_text_bytes(true),
    }
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/errors")
}

/// The program kind `skslc` picks from the input's extension.
fn kind_for_extension(ext: &str) -> ProgramKind {
    match ext {
        "vert" => ProgramKind::Vertex,
        "frag" | "sksl" => ProgramKind::Fragment,
        "mvert" => ProgramKind::MeshVertex,
        "mfrag" => ProgramKind::MeshFragment,
        "compute" => ProgramKind::Compute,
        "rtb" => ProgramKind::RuntimeBlender,
        "rtcf" => ProgramKind::RuntimeColorFilter,
        "rts" => ProgramKind::RuntimeShader,
        other => panic!("unknown extension {other}"),
    }
}

/// Goldens whose errors come (at least in part) from `Compiler::finalize` or `optimize`, which
/// run after a successful parse (task S11). The parser must accept each of these.
const DEFERRED_TO_FINALIZATION: &[&str] = &[
    // The inliner's constant evaluation (S12) reports these.
    "ArrayInlinedIndexOutOfRange",
    "MatrixInlinedIndexOutOfRange",
    "VectorInlinedIndexOutOfRange",
    "OverflowInlinedLiteral",
    // `Compiler::finalize` (S11): program-level checks.
    "DuplicateBinding",
    "DuplicateWorkgroupSize",
    "MissingWorkgroupSize",
    "IllegalRecursionComplex",
    "IllegalRecursionMutual",
    "IllegalRecursionSimple",
    "ProgramTooLarge_Globals",
    "SamplerExternalOES",
    "UnassignedOutParameter",
    "UndefinedFunction",
];

/// Goldens whose message quotes a source byte that is not valid UTF-8. Skia's messages are bytes;
/// the error reporter's are `&str`, so the byte comes out as U+FFFD.
const NON_UTF8_MESSAGE: &[&str] = &["Ossfuzz519154489"];

#[test]
fn builtin_modules_parse_with_the_parser() {
    // The harness compiles every module the error goldens need; this checks the rest.
    let _ = (shared(), gpu(), frag(), vert(), compute(), public());
}

#[test]
fn error_goldens_match() {
    let dir = data_dir();
    let mut cases: Vec<(String, PathBuf)> = Vec::new();
    let mut goldens = 0;
    for entry in fs::read_dir(&dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display())) {
        let path = entry.expect("directory entry").path();
        if path.extension().is_some_and(|ext| ext == "glsl") {
            goldens += 1;
            continue;
        }
        let name = path
            .file_stem()
            .expect("file stem")
            .to_string_lossy()
            .into_owned();
        // An input without a golden is a test that Skia's `dm` runs but `skslc` does not.
        if path.with_extension("glsl").exists() {
            cases.push((name, path));
        }
    }
    assert!(goldens >= cases.len());
    cases.sort();
    assert!(cases.len() > 100, "only {} cases", cases.len());
    // `PARSER_CASE=Name cargo test ...` runs one case.
    if let Ok(only) = std::env::var("PARSER_CASE") {
        cases.retain(|(name, _)| *name == only);
    }

    let mut failures = Vec::new();
    for (name, path) in &cases {
        let ext = path.extension().expect("extension").to_string_lossy();
        // Skia's strings are bytes: a fuzzer's input and its golden need not be UTF-8.
        let text = fs::read(path).expect("input");
        let golden = fs::read(path.with_extension("glsl")).expect("golden");
        let expected = golden
            .strip_prefix(b"### Compilation failed:\n\n")
            .unwrap_or_else(|| panic!("{name}: not an error golden"));
        let kind = kind_for_extension(&ext);
        let Ok(parsed) =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parse_program(kind, &text)))
        else {
            failures.push(format!("{name}: the parser panicked"));
            continue;
        };
        if DEFERRED_TO_FINALIZATION.contains(&name.as_str()) {
            if !parsed.accepted {
                failures.push(format!(
                    "{name}: deferred to finalization, but the parser reported:\n{}",
                    String::from_utf8_lossy(&parsed.error_text)
                ));
            }
        } else if NON_UTF8_MESSAGE.contains(&name.as_str()) {
            // The message quotes a byte that is not UTF-8, which `&str` messages cannot hold: the
            // text is compared after lossy conversion of both sides.
            if String::from_utf8_lossy(&parsed.error_text) != String::from_utf8_lossy(expected) {
                failures.push(format!("{name}: error text differs (lossy)"));
            }
        } else if parsed.error_text != expected {
            failures.push(format!(
                "{name}: error text differs\n--- expected\n{}--- got\n{}",
                String::from_utf8_lossy(expected),
                String::from_utf8_lossy(&parsed.error_text)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases failed:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

/// The messages `SkSLErrorTest` expects, from the `/*%%* ... *%%*/` comment of a test input.
fn expected_messages(text: &str) -> Vec<String> {
    let start = text.find("/*%%*").expect("an expectation comment") + "/*%%*".len();
    let end = text
        .find("*%%*/")
        .expect("the end of the expectation comment");
    text[start..end]
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `check_expected_errors` of `tests/SkSLErrorTest.cpp`: every expected message must appear in the
/// reported text, in order. The list is not necessarily exhaustive.
fn missing_expected_errors(expected: &[String], reported: &str) -> Vec<String> {
    let mut remaining = reported;
    let mut missing = Vec::new();
    for message in expected {
        match remaining.find(message.as_str()) {
            Some(pos) => remaining = &remaining[pos + message.len()..],
            None => missing.push(message.clone()),
        }
    }
    missing
}

/// The error tests of `resources/sksl/runtime_errors` (`SkSLRuntimeShaderErrorTest` and its
/// siblings): each file lists messages that the compiler must report. Messages from checks that
/// run after the parse (finalization, the inliner, `SkRuntimeEffect`'s validation) cannot appear
/// yet; the files that expect them are listed in `DEFERRED_RUNTIME_ERRORS` with the messages the
/// parser is not responsible for, and every other expected message must still be reported.
#[test]
fn runtime_error_expectations_match() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/runtime_errors");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    files.sort();
    assert!(files.len() > 30, "only {} files", files.len());
    let mut failures = Vec::new();
    for path in &files {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let ext = path.extension().unwrap().to_string_lossy().into_owned();
        let text = fs::read_to_string(path).expect("input");
        let expected = expected_messages(&text);
        let parsed = parse_program(kind_for_extension(&ext), text.as_bytes());
        let reported = String::from_utf8_lossy(&parsed.error_text).into_owned();
        let missing = missing_expected_errors(&expected, &reported);
        let deferred: &[&str] = DEFERRED_RUNTIME_ERRORS
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(&[], |(_, messages)| *messages);
        if !deferred.is_empty() && !parsed.accepted {
            failures.push(format!(
                "{name}: deferred, but the parser reported errors\n{reported}"
            ));
        }
        if missing != deferred {
            failures.push(format!(
                "{name}: missing {missing:?}, deferred {deferred:?}\n{reported}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} files failed:\n{}",
        failures.len(),
        files.len(),
        failures.join("\n")
    );
}

/// Expected messages that come from phases after the parse, by test.
const DEFERRED_RUNTIME_ERRORS: &[(&str, &[&str])] = &[
    // `Analysis::ValidateIndexingForES2`, which `Compiler::finalize` runs (S11).
    (
        "IllegalIndexing",
        &[
            "error: 23: index expression must be constant",
            "error: 24: index expression must be constant",
        ],
    ),
    // `Analysis::CheckProgramStructure`, which `Compiler::finalize` runs (S11): the whole
    // multi-line message.
    ("ProgramTooLarge_StackDepth", STACK_DEPTH_MESSAGE),
];

/// The expected lines of `ProgramTooLarge_StackDepth`.
const STACK_DEPTH_MESSAGE: &[&str] = &[
    "exceeded max function call depth:",
    "\thalf4 main(float2 xy)",
    "\tvoid f1()",
    "\tvoid f2()",
    "\tvoid f3()",
    "\tvoid f4()",
    "\tvoid f5()",
    "\tvoid f6()",
    "\tvoid f7()",
    "\tvoid f8()",
    "\tvoid f9()",
    "\tvoid f10()",
    "\tvoid f11()",
    "\tvoid f12()",
    "\tvoid f13()",
    "\tvoid f14()",
    "\tvoid f15()",
    "\tvoid f16()",
    "\tvoid f17()",
    "\tvoid f18()",
    "\tvoid f19()",
    "\tvoid f20()",
    "\tvoid f21()",
    "\tvoid f22()",
    "\tvoid f23()",
    "\tvoid f24()",
    "\tvoid f25()",
    "\tvoid f26()",
    "\tvoid f27()",
    "\tvoid f28()",
    "\tvoid f29()",
    "\tvoid f30()",
    "\tvoid f31()",
    "\tvoid f32()",
    "\tvoid f33()",
    "\tvoid f34()",
    "\tvoid f35()",
    "\tvoid f36()",
    "\tvoid f37()",
    "\tvoid f38()",
    "\tvoid f39()",
    "\tvoid f40()",
    "\tvoid f41()",
    "\tvoid f42()",
    "\tvoid f43()",
    "\tvoid f44()",
    "\tvoid f45()",
    "\tvoid f46()",
    "\tvoid f47()",
    "\tvoid f48()",
    "\tvoid f49()",
    "\tvoid f50()",
];
