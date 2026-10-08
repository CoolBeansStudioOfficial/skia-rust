// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/sksl-minify/SkSLMinify.cpp, with the rules of gn/minify_sksl.py and
// gn/minify_sksl_tests.py that decide what each run is given (chrome/m156).

//! `sksl-minify` as a library: compiles a program's module chain, optimizes each module for
//! minifying (`optimizeModuleBeforeMinifying` with `shrinkSymbols`), prints the last module's
//! elements, and strips the whitespace with the lexer.
//!
//! Skia's tool reads a worklist and writes a file. [`minify`] takes the same inputs and returns the
//! bytes the tool writes. The `--stringify` mode (the C++ string-literal wrapping) and the
//! `--unoptimized` mode are not ported: the goldens and the embedded modules are the raw text.

// Port of: tools/sksl-minify/SkSLMinify.cpp (chrome/m156)

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use skia_rust_sksl::compiler::{Compiler, ModuleParts};
use skia_rust_sksl::flavor::Flavor;
use skia_rust_sksl::ir::ProgramElementKind;
use skia_rust_sksl::lexer::{Lexer, TokenKind};
use skia_rust_sksl::module_loader::{ModuleLoader, add_public_type_aliases};
use skia_rust_sksl::modules::{Module, ModuleType};
use skia_rust_sksl::program_settings::{ProgramConfig, ProgramKind};

/// The ways `sksl-minify` fails. Each one is a message the tool prints before it returns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MinifyError {
    /// `error reading '<path>'`.
    Read(PathBuf),
    /// A module did not compile or optimize; carries the compiler's error text.
    Compile(String),
    /// `unable to parse '<text>' at offset <n>`: the lexer found text it cannot tokenize.
    Unparsable { offset: i32, text: String },
}

impl fmt::Display for MinifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(path) => write!(f, "error reading '{}'", path.display()),
            Self::Compile(text) => f.write_str(text),
            Self::Unparsable { offset, text } => {
                write!(f, "unable to parse '{text}' at offset {offset}")
            }
        }
    }
}

impl std::error::Error for MinifyError {}

/// `module_type_for_path`: the module type of a file, by its name in `SKSL_MODULE_LIST`.
// Port of: tools/sksl-minify/SkSLMinify.cpp#L60-L67 (chrome/m156)
fn module_type_for_path(path: &Path) -> ModuleType {
    let file = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    ModuleType::ALL
        .into_iter()
        .find(|module| file == format!("{}.sksl", module.name()))
        .unwrap_or(ModuleType::Unknown)
}

/// The error text of the compiler, as a `String`.
fn error_text(compiler: &mut Compiler) -> String {
    String::from_utf8_lossy(&compiler.error_text_bytes(true)).into_owned()
}

/// Compiles one module of `kind` atop `parent` and optimizes it for minifying, with shrinking on
/// (`process_command` does not pass `--unoptimized`).
// Port of: tools/sksl-minify/SkSLMinify.cpp#L107-L131 (the body of `compile_module_list`'s loop).
fn compile_one(
    compiler: &mut Compiler,
    kind: ProgramKind,
    path: &Path,
    parent: &Arc<Module>,
) -> Result<ModuleParts, MinifyError> {
    let source = std::fs::read(path).map_err(|_| MinifyError::Read(path.to_path_buf()))?;
    let mut parts = compiler
        .compile_module_parts(kind, module_type_for_path(path), &source, parent)
        .ok_or_else(|| MinifyError::Compile(error_text(compiler)))?;
    // We need to optimize every module in the chain. We rename private functions at global scope,
    // and we need to make sure there are no name collisions between nested modules.
    if !compiler.optimize_module_before_minifying(kind, &mut parts, parent, true) {
        return Err(MinifyError::Compile(error_text(compiler)));
    }
    Ok(parts)
}

/// Compiles `paths` from the last to the first, each module atop the one after it, the last one
/// atop `base`. Returns the first path's module, unfrozen, and the frozen module it inherits from.
// Port of: tools/sksl-minify/SkSLMinify.cpp#L107-L131 (`compile_module_list`, chrome/m156)
fn compile_chain(
    paths: &[PathBuf],
    kind: ProgramKind,
    base: &Arc<Module>,
) -> Result<(ModuleParts, Arc<Module>), MinifyError> {
    assert!(!paths.is_empty(), "compile_chain needs at least one module");
    let mut compiler = Compiler::with_flavor(Flavor::Standalone);
    let mut parent = Arc::clone(base);
    // Load in each input as a module, from right to left. Each module inherits the symbols from
    // its parent module.
    for (index, path) in paths.iter().enumerate().rev() {
        let parts = compile_one(&mut compiler, kind, path, &parent)?;
        if index == 0 {
            return Ok((parts, parent));
        }
        parent = parts.freeze(parent);
    }
    unreachable!("the loop returns at index 0")
}

/// `compile_module_list`: the chain of modules a run compiles, and the module that is printed (the
/// first path's, the input). A runtime effect's parent modules are compiled as fragment programs,
/// and the public type aliases go on the module that the input inherits from.
// Port of: tools/sksl-minify/SkSLMinify.cpp#L104-L152 (chrome/m156)
fn compile_module_list(
    paths: &[PathBuf],
    kind: ProgramKind,
    module_dir: &Path,
) -> Result<ModuleParts, MinifyError> {
    let root = ModuleLoader::for_flavor(Flavor::Standalone).root();
    if !ProgramConfig::is_runtime_effect(kind) {
        return Ok(compile_chain(paths, kind, &root)?.0);
    }

    // If we are compiling a Runtime Effect, the parent modules still need to be compiled as
    // Fragment programs. If no modules are listed, the built-in modules for runtime effects
    // (sksl_shared, sksl_public) come from the tool's own directory.
    let defaults = [
        module_dir.join("sksl_public.sksl"),
        module_dir.join("sksl_shared.sksl"),
    ];
    let parents: &[PathBuf] = if paths.len() == 1 {
        &defaults
    } else {
        &paths[1..]
    };
    let (mut front, front_parent) = compile_chain(parents, ProgramKind::Fragment, &root)?;
    // Set up the public type aliases so that Runtime Shader code with GLSL types works as-is.
    add_public_type_aliases(&mut front);
    let base = front.freeze(front_parent);
    Ok(compile_chain(&paths[..1], kind, &base)?.0)
}

/// `generate_minified_text`: the lexer's tokens, with comments and whitespace dropped, joined with
/// a space only where two tokens would otherwise run together, and float literals shortened
/// (`3.0` is `3.`, `0.5` is `.5`).
// Port of: tools/sksl-minify/SkSLMinify.cpp#L152-L205 (chrome/m156)
fn generate_minified_text(text: &[u8], out: &mut Vec<u8>) -> Result<(), MinifyError> {
    let maybe_identifier = |c: u8| c.is_ascii_alphanumeric() || c == b'$' || c == b'_';
    let is_plus_or_minus = |c: u8| c == b'+' || c == b'-';

    let mut lexer = Lexer::new(text);
    let mut last_token_text: &[u8] = b" ";
    loop {
        let token = lexer.next_token();
        if token.kind == Some(TokenKind::EndOfFile) {
            break;
        }
        if matches!(
            token.kind,
            Some(TokenKind::LineComment | TokenKind::BlockComment | TokenKind::Whitespace)
        ) {
            continue;
        }
        let start = usize::try_from(token.offset).unwrap_or(0);
        let end = start + usize::try_from(token.length).unwrap_or(0);
        let mut this_token_text: &[u8] = &text[start..end];
        // Skia compares the raw kind: an unaccepted lexer state (no kind at all) never reaches
        // the output, so it is reported with the invalid tokens.
        let Some(kind) = token.kind.filter(|&kind| kind != TokenKind::Invalid) else {
            return Err(MinifyError::Unparsable {
                offset: token.offset,
                text: String::from_utf8_lossy(this_token_text).into_owned(),
            });
        };
        if this_token_text.is_empty() {
            continue;
        }
        if kind == TokenKind::FloatLiteral {
            // We can reduce `3.0` to `3.` safely.
            if this_token_text.contains(&b'.') {
                while this_token_text.last() == Some(&b'0') && this_token_text.len() >= 3 {
                    this_token_text = &this_token_text[..this_token_text.len() - 1];
                }
            }
            // We can reduce `0.5` to `.5` safely.
            if this_token_text.starts_with(b"0.") && this_token_text.len() >= 3 {
                this_token_text = &this_token_text[1..];
            }
        }

        let last_back = last_token_text.last().copied().unwrap_or(b' ');
        let first = this_token_text[0];
        // Detect tokens with abutting alphanumeric characters side-by-side.
        let adjacent_identifiers = maybe_identifier(last_back) && maybe_identifier(first);
        // Detect potentially ambiguous preincrement/postincrement operators. For instance,
        // `x + ++y` and `x++ + y` require whitespace for differentiation.
        let adjacent_plus_or_minus = is_plus_or_minus(last_back) && is_plus_or_minus(first);
        // Insert whitespace when it is necessary for program correctness.
        if adjacent_identifiers || adjacent_plus_or_minus {
            out.push(b' ');
        }
        out.extend_from_slice(this_token_text);
        last_token_text = this_token_text;
    }
    Ok(())
}

/// Whether `element` is the `Attributes` or `Varyings` struct of a mesh program. The minified output
/// leaves those out: they are synthesized from the `SkMeshSpecification`.
// Port of: tools/sksl-minify/SkSLMinify.cpp#L268-L276 (chrome/m156), the mesh filter of the loop.
fn is_mesh_io_struct(
    pool: &skia_rust_sksl::ir::IrPool,
    element: skia_rust_sksl::ir::ElemId,
) -> bool {
    match &pool.element(element).kind {
        ProgramElementKind::StructDefinition(def) => {
            matches!(pool.ty(def.ty).name(), "Attributes" | "Varyings")
        }
        _ => false,
    }
}

/// `sksl-minify <output> <input> [flags] [dependencies…]`, as a function: the text the tool writes
/// for the module `paths[0]`, which inherits from `paths[1..]` (the same order as the command
/// line). `module_dir` is the directory the tool finds its default runtime modules in, which a
/// runtime effect with no listed module uses.
///
/// # Errors
///
/// Returns the message the tool prints for the first failure: an unreadable input, a module that
/// does not compile, or text the lexer cannot tokenize.
///
/// # Panics
///
/// If `paths` is empty.
// Port of: tools/sksl-minify/SkSLMinify.cpp#L219-L318 (`process_command`, chrome/m156), without
// the `--stringify` and `--unoptimized` flags.
pub fn minify(
    kind: ProgramKind,
    paths: &[PathBuf],
    module_dir: &Path,
) -> Result<Vec<u8>, MinifyError> {
    let module = compile_module_list(paths, kind, module_dir)?;
    let is_mesh = matches!(kind, ProgramKind::MeshFragment | ProgramKind::MeshVertex);

    // Generate the program text by getting the program's description.
    let mut text = Vec::new();
    for &element in &module.elements {
        if is_mesh && is_mesh_io_struct(&module.pool, element) {
            continue;
        }
        text.extend_from_slice(module.pool.element_description(element).as_bytes());
    }

    // Eliminate whitespace and perform other basic simplifications via a lexer pass.
    let mut out = Vec::new();
    generate_minified_text(&text, &mut out)?;
    out.push(b'\n');
    Ok(out)
}

/// The program kind and the modules after the input that `gn/minify_sksl_tests.py` passes for a
/// `tests/sksl` input, by its extension. `None` for inputs that are not minified.
// Port of: gn/minify_sksl_tests.py#L27-L66 (the worklist), chrome/m156
#[must_use]
#[allow(clippy::case_sensitive_file_extension_comparisons)] // Skia compares suffixes case-sensitively.
pub fn test_input_rule(input: &str) -> Option<(ProgramKind, &'static [&'static str])> {
    const RUNTIME_DEPENDENCIES: &[&str] = &["sksl_public", "sksl_shared"];
    const PRIVATE_DEPENDENCIES: &[&str] = &["sksl_rt_shader", "sksl_public", "sksl_shared"];
    Some(if input.ends_with(".rts") {
        (ProgramKind::RuntimeShader, RUNTIME_DEPENDENCIES)
    } else if input.ends_with(".privrts") {
        (ProgramKind::PrivateRuntimeShader, PRIVATE_DEPENDENCIES)
    } else if input.ends_with(".rtcf") {
        (ProgramKind::RuntimeColorFilter, RUNTIME_DEPENDENCIES)
    } else if input.ends_with(".rtb") {
        (ProgramKind::RuntimeBlender, RUNTIME_DEPENDENCIES)
    } else if input.ends_with(".mfrag") {
        (ProgramKind::MeshFragment, RUNTIME_DEPENDENCIES)
    } else if input.ends_with(".mvert") {
        (ProgramKind::MeshVertex, RUNTIME_DEPENDENCIES)
    } else {
        return None;
    })
}

/// The program kind and dependencies that `gn/minify_sksl.py` gives a built-in module, by its
/// name: the minified text of `module` is `minify` of its original source with these.
// Port of: gn/minify_sksl.py#L11-L40 (`dependencies` and the `--compute`/`--vert`/`--frag` choice),
// chrome/m156.
#[must_use]
pub fn module_rule(module: ModuleType) -> Option<(ProgramKind, &'static [&'static str])> {
    Some(match module {
        ModuleType::SkslCompute => (ProgramKind::Compute, &["sksl_gpu", "sksl_shared"]),
        ModuleType::SkslFrag => (ProgramKind::Fragment, &["sksl_gpu", "sksl_shared"]),
        ModuleType::SkslVert => (ProgramKind::Vertex, &["sksl_gpu", "sksl_shared"]),
        ModuleType::SkslGraphiteFrag => (
            ProgramKind::Fragment,
            &["sksl_frag", "sksl_gpu", "sksl_shared"],
        ),
        ModuleType::SkslGraphiteVert => (
            ProgramKind::Vertex,
            &["sksl_vert", "sksl_gpu", "sksl_shared"],
        ),
        ModuleType::SkslGpu | ModuleType::SkslPublic => (ProgramKind::Fragment, &["sksl_shared"]),
        ModuleType::SkslRtShader => (ProgramKind::Fragment, &["sksl_public", "sksl_shared"]),
        ModuleType::SkslShared => (ProgramKind::Fragment, &[]),
        ModuleType::Program | ModuleType::Unknown => return None,
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use skia_rust_sksl::flavor::ModuleSource;
    use skia_rust_sksl::modules::ModuleType;

    use super::{minify, module_rule};

    /// Every built-in module that `gn/minify_sksl.py` minifies, regenerated from the original text
    /// in the Skia checkout, must equal the minified text the crate embeds, byte for byte. This is
    /// the check Skia's `minify_sksl` build step performs (`docs/design/sksl.md` §3). Skipped
    /// without a Skia checkout.
    #[test]
    fn embedded_minified_modules_regenerate_byte_for_byte() {
        let skia_sksl = Path::new(env!("CARGO_MANIFEST_DIR")).join("../third_party/skia/src/sksl");
        if !skia_sksl.join("sksl_shared.sksl").exists() {
            eprintln!("todo: skipping, missing Skia checkout");
            return;
        }
        let mut failures = Vec::new();
        let mut checked = 0;
        for module in ModuleType::ALL {
            let Some((kind, dependencies)) = module_rule(module) else {
                continue;
            };
            let mut paths: Vec<PathBuf> = vec![skia_sksl.join(format!("{}.sksl", module.name()))];
            paths.extend(
                dependencies
                    .iter()
                    .map(|dependency| skia_sksl.join(format!("{dependency}.sksl"))),
            );
            let regenerated = minify(kind, &paths, &skia_sksl);
            let embedded = module.text(ModuleSource::Minified);
            checked += 1;
            // The tool writes a newline after the text. The embedded module is the text of the C++
            // string literal that holds it, and that literal ends before the newline (`";`).
            match regenerated {
                Ok(text) if text.strip_suffix(b"\n") == Some(embedded.as_bytes()) => {}
                Ok(_) => failures.push(format!("{}: the text differs", module.name())),
                Err(e) => failures.push(format!("{}: {e}", module.name())),
            }
        }
        assert_eq!(checked, 9, "the nine built-in modules");
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
