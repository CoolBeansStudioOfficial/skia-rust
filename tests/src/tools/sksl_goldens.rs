// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: gn/sksl_tests.gni and the `compile_sksl` rule of BUILD.gn (which says which
// input produces which golden, with which flags).

//! The `tests/sksl` goldens: which input each golden comes from, and how it is checked.
//!
//! `gn/sksl_tests.gni` lists the inputs of each golden group. `BUILD.gn` pairs each group with an
//! output extension and a `skslc` language flag, and names the golden as the input's path under
//! `tests/sksl`, with the input's extension replaced. [`plan`] repeats that mapping for the
//! groups whose outputs skia-rust checks. [`run_job`] then runs `skslc` on each input and
//! compares the result with the golden.

// Port of: gn/sksl_tests.gni (chrome/m156) and BUILD.gn#L775-L872 (compile_sksl).

use std::collections::BTreeMap;
use std::path::Path;

use crate::tools::skslc::{SkslcError, skslc};

/// A parsed `gni` file: each variable's list of strings (`sksl_blend_tests`, …), with `+`
/// concatenations already expanded.
pub type GniLists = BTreeMap<String, Vec<String>>;

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Ident(String),
    Str(String),
    Punct(char),
}

fn tokenize(text: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            c if c.is_whitespace() => {
                chars.next();
            }
            '#' => {
                // A comment runs to the end of the line.
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '"' => {
                chars.next();
                let mut s = String::new();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some(c) => s.push(c),
                        None => return Err("unterminated string in gni file".to_owned()),
                    }
                }
                tokens.push(Token::Str(s));
            }
            '=' | '[' | ']' | '+' | ',' => {
                chars.next();
                tokens.push(Token::Punct(c));
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        s.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Ident(s));
            }
            other => return Err(format!("unexpected character {other:?} in gni file")),
        }
    }
    Ok(tokens)
}

/// Parses the subset of GN syntax that `sksl_tests.gni` uses: `name = [ "…", … ]` and
/// `name = a + b + …`.
///
/// # Errors
///
/// Returns a message for syntax this parser does not accept, or for a reference to an undefined
/// variable.
pub fn parse_gni(text: &str) -> Result<GniLists, String> {
    let tokens = tokenize(text)?;
    let mut lists = GniLists::new();
    let mut tokens = tokens.into_iter().peekable();
    while let Some(token) = tokens.next() {
        let Token::Ident(name) = token else {
            return Err(format!("expected a variable name, got {token:?}"));
        };
        if tokens.next() != Some(Token::Punct('=')) {
            return Err(format!("expected '=' after {name}"));
        }
        let mut value = Vec::new();
        if tokens.peek() == Some(&Token::Punct('[')) {
            tokens.next();
            loop {
                match tokens.next() {
                    Some(Token::Str(s)) => value.push(s),
                    Some(Token::Punct(',')) => {}
                    Some(Token::Punct(']')) => break,
                    other => return Err(format!("unexpected {other:?} in the list {name}")),
                }
            }
        } else {
            loop {
                let Some(Token::Ident(reference)) = tokens.next() else {
                    return Err(format!("expected a list reference in {name}"));
                };
                let items = lists
                    .get(&reference)
                    .ok_or_else(|| format!("{name} refers to undefined {reference}"))?;
                value.extend(items.iter().cloned());
                if tokens.peek() == Some(&Token::Punct('+')) {
                    tokens.next();
                } else {
                    break;
                }
            }
        }
        lists.insert(name, value);
    }
    Ok(lists)
}

/// One `BUILD.gn` `compile_sksl` target whose outputs skia-rust checks.
struct Target {
    /// The `gni` variables whose inputs the target compiles (`sksl_skrp_tests_sources`, …).
    sources: &'static [&'static str],
    /// The output extension (`.skrp`, `.minified.sksl`, …).
    out_ext: &'static str,
    /// Only inputs with one of these extensions are compiled (`minify_sksl_tests.py`).
    input_exts: Option<&'static [&'static str]>,
}

/// The targets in scope: Raster Pipeline dumps, pipeline stages, WGSL, the front end's error
/// text (`errors/*.glsl`, which fail before any GLSL generator runs) and minified modules.
/// `skslc` is run with `--settings` for all of them.
const TARGETS: &[Target] = &[
    Target {
        // `sksl_skrp_tests` is not in `sksl_skrp_tests_sources`, but its one golden
        // (`tests/sksl/skrp/ImmediateOpsOpt.skrp`) is in the pinned tree, so it is included.
        sources: &[
            "sksl_folding_tests",
            "sksl_rte_tests",
            "sksl_shared_tests",
            "sksl_skrp_tests",
        ],
        out_ext: ".skrp",
        input_exts: None,
    },
    Target {
        sources: &["sksl_rte_tests", "sksl_mesh_tests", "sksl_mesh_error_tests"],
        out_ext: ".stage",
        input_exts: None,
    },
    Target {
        sources: &[
            "sksl_blend_tests",
            "sksl_compute_tests",
            "sksl_folding_tests",
            "sksl_shared_tests",
            "sksl_wgsl_tests",
        ],
        out_ext: ".wgsl",
        input_exts: None,
    },
    Target {
        sources: &["sksl_error_tests"],
        out_ext: ".glsl",
        input_exts: None,
    },
    Target {
        sources: &["sksl_folding_tests", "sksl_mesh_tests", "sksl_rte_tests"],
        out_ext: ".minified.sksl",
        input_exts: Some(&["rts", "privrts", "rtcf", "rtb", "mfrag", "mvert"]),
    },
];

/// Inputs whose `.wgsl` golden is in the pinned tree but which no `gni` list names. The
/// `compute/Atomic*` inputs are missing from `sksl_compute_tests`, while their `.wgsl`, `.metal`
/// and `.asm.comp` goldens exist. They are compiled with the WGSL rule, as the manifest scan
/// (`xtask`'s `scan_sksl_goldens`) already treats them.
const UNLISTED_WGSL_INPUTS: &[&str] = &[
    "compute/AtomicDeclarations.compute",
    "compute/AtomicOperationsOverArrayAndStruct.compute",
];

/// One golden and the input that produces it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoldenJob {
    /// The manifest id: the golden's path, `tests/sksl/<dir>/<name><ext>`.
    pub id: String,
    /// The input's path under `resources/sksl`, e.g. `blend/BlendClear.sksl`.
    pub input: String,
}

/// Maps every golden of the in-scope targets to its input, as `BUILD.gn` does, sorted by id.
///
/// # Errors
///
/// Returns a message if the `gni` lists lack a variable a target needs, or if two inputs claim
/// the same golden.
pub fn plan(lists: &GniLists) -> Result<Vec<GoldenJob>, String> {
    let mut jobs: BTreeMap<String, GoldenJob> = BTreeMap::new();
    for target in TARGETS {
        for source in target.sources {
            let inputs = lists
                .get(*source)
                .ok_or_else(|| format!("sksl_tests.gni has no list {source}"))?;
            for input in inputs {
                let (stem_path, input_ext) = input
                    .rsplit_once('.')
                    .ok_or_else(|| format!("input without an extension: {input}"))?;
                if target
                    .input_exts
                    .is_some_and(|exts| !exts.contains(&input_ext))
                {
                    continue;
                }
                let id = format!("tests/sksl/{stem_path}{}", target.out_ext);
                let job = GoldenJob {
                    id: id.clone(),
                    input: input.clone(),
                };
                if jobs.insert(id.clone(), job).is_some() {
                    return Err(format!("two inputs produce {id}"));
                }
            }
        }
    }
    for input in UNLISTED_WGSL_INPUTS {
        let (stem_path, _) = input
            .rsplit_once('.')
            .ok_or_else(|| format!("input without an extension: {input}"))?;
        let id = format!("tests/sksl/{stem_path}.wgsl");
        let job = GoldenJob {
            id: id.clone(),
            input: (*input).to_owned(),
        };
        if jobs.insert(id.clone(), job).is_some() {
            return Err(format!("two inputs produce {id}"));
        }
    }
    Ok(jobs.into_values().collect())
}

/// The verdict for one golden.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The output is byte-for-byte the golden.
    Ok,
    /// The output differs from the golden, or the compiler reported an error for it.
    Failed(String),
    /// The golden was not checked, and the reason.
    Ignored(String),
}

/// Runs `skslc` on one job's input under `skia` (the Skia checkout) and compares the output with
/// the golden in `skia/tests/sksl`.
#[must_use]
pub fn run_job(job: &GoldenJob, skia: &Path) -> Verdict {
    if job.id.ends_with(".minified.sksl") {
        // Made by `tools/sksl-minify`, not `skslc` (`minify_sksl_tests.py`): S24.
        return Verdict::Ignored("sksl-minify is not ported yet (S24)".to_owned());
    }
    let input_path = skia.join("resources/sksl").join(&job.input);
    let golden_path = skia.join(&job.id);
    let Ok(text) = std::fs::read_to_string(&input_path) else {
        return Verdict::Ignored(format!("missing input {}", input_path.display()));
    };
    let Ok(expected) = std::fs::read(&golden_path) else {
        return Verdict::Ignored(format!("missing golden {}", golden_path.display()));
    };
    match skslc(&job.input, &text, &job.id, true) {
        Ok(actual) if actual == expected => Verdict::Ok,
        Ok(_) => Verdict::Failed("output differs from the golden".to_owned()),
        Err(SkslcError::NotPorted(what)) => Verdict::Ignored(what.to_owned()),
        Err(e) => Verdict::Failed(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{GniLists, parse_gni, plan};
    use std::path::Path;

    const ORPHAN_ERROR_GOLDENS: &[&str] = &[
        "tests/sksl/errors/ReservedNameSampler1DShadow.glsl",
        "tests/sksl/errors/ReservedNameSampler2DShadow.glsl",
        "tests/sksl/errors/ReservedNameSampler2DRectShadow.glsl",
        "tests/sksl/errors/ReservedNameSampler3DRect.glsl",
        "tests/sksl/errors/ReservedNameSamplerCube.glsl",
    ];

    #[test]
    fn parses_lists_and_concatenations() {
        let text = "# comment\n\
                    a = [\n  \"x/One.sksl\",\n  \"x/Two.sksl\",\n]\n\
                    b = [ \"y/Three.rts\" ]\n\
                    c = a + b\n";
        let lists = parse_gni(text).unwrap();
        assert_eq!(lists["a"], ["x/One.sksl", "x/Two.sksl"]);
        assert_eq!(lists["c"], ["x/One.sksl", "x/Two.sksl", "y/Three.rts"]);
    }

    #[test]
    fn rejects_undefined_references() {
        assert!(parse_gni("c = a + b\n").is_err());
    }

    /// The mapping must reproduce the golden set the manifest scans (`xtask`'s
    /// `scan_sksl_goldens`): 365 `.skrp`, 62 `.stage`, 420 `.wgsl`, 338 error `.glsl` and 77
    /// `.minified.sksl`. Skipped without a Skia checkout.
    #[test]
    fn plan_matches_the_pinned_goldens() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../third_party/skia");
        let Ok(text) = std::fs::read_to_string(root.join("gn/sksl_tests.gni")) else {
            eprintln!("todo: skipping, missing Skia checkout");
            return;
        };
        let lists: GniLists = parse_gni(&text).unwrap();
        let jobs = plan(&lists).unwrap();
        let count = |ext: &str| jobs.iter().filter(|j| j.id.ends_with(ext)).count();
        assert_eq!(count(".skrp"), 365);
        assert_eq!(count(".stage"), 62);
        assert_eq!(count(".wgsl"), 420);
        assert_eq!(count(".minified.sksl"), 77);
        // Five error goldens have no input in `resources/sksl` and no `gni` entry, so there is
        // nothing to compile. The manifest still scans them (as `todo`); they are not planned.
        assert_eq!(count(".glsl"), 333);
        for orphan in ORPHAN_ERROR_GOLDENS {
            assert!(
                !jobs.iter().any(|j| j.id == *orphan),
                "{orphan} has no input"
            );
        }
        assert_eq!(jobs.len(), 365 + 62 + 420 + 77 + 333);
    }
}
