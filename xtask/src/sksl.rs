//! `cargo xtask sksl …`: maintenance of the `SkSL` port's embedded data.
//!
//! `sync-modules` copies Skia's built-in `SkSL` module sources into the `skia-rust-sksl` crate, in
//! both variants Skia uses (`docs/design/sksl.md` §3): the original `src/sksl/sksl_*.sksl` files,
//! and the minified texts in `src/sksl/generated/*.minified.sksl`, whose C string-literal wrapping
//! is undone. `--check` compares the crate's copies with the pinned tree instead of writing them.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

/// The nine built-in modules, in `SKSL_MODULE_LIST` order (`src/sksl/SkSLModule.h#L28-L37`).
const MODULES: &[&str] = &[
    "sksl_shared",
    "sksl_compute",
    "sksl_frag",
    "sksl_gpu",
    "sksl_public",
    "sksl_rt_shader",
    "sksl_vert",
    "sksl_graphite_frag",
    "sksl_graphite_vert",
];

/// Where the crate keeps its copies, relative to the workspace root.
const CRATE_MODULES: &str = "crates/skia-rust-sksl/src/modules";

/// Decodes a generated `*.minified.sksl` file: skips the `static constexpr char NAME[] =` line and
/// concatenates the contents of every C string literal (`"…"`) after it. Rejects backslash
/// escapes, which Skia's generator never emits for these files.
pub fn decode_minified(text: &str) -> Result<String> {
    let mut out = String::new();
    for line in text.lines() {
        if line.starts_with("static constexpr char ") {
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find('"') {
            let after = &rest[start + 1..];
            let end = after
                .find('"')
                .with_context(|| format!("unterminated string literal in line {line:?}"))?;
            let literal = &after[..end];
            ensure!(
                !literal.contains('\\'),
                "unexpected escape in minified module line {line:?}"
            );
            out.push_str(literal);
            rest = &after[end + 1..];
        }
    }
    Ok(out)
}

/// Copies (or with `check`, compares) the module texts between the pinned Skia tree and the crate.
pub fn sync_modules(root: &Path, check: bool) -> Result<()> {
    let skia = root.join("third_party/skia/src/sksl");
    ensure!(
        skia.is_dir(),
        "{} is missing; run `cargo xtask skia fetch`",
        skia.display()
    );
    let dest = root.join(CRATE_MODULES);
    let mut mismatches = Vec::new();
    for name in MODULES {
        let original = std::fs::read(skia.join(format!("{name}.sksl")))
            .with_context(|| format!("reading {name}.sksl"))?;
        let minified_text =
            std::fs::read_to_string(skia.join("generated").join(format!("{name}.minified.sksl")))
                .with_context(|| format!("reading {name}.minified.sksl"))?;
        let minified = decode_minified(&minified_text)?;
        let wanted = [
            (dest.join("original").join(format!("{name}.sksl")), original),
            (
                dest.join("minified").join(format!("{name}.sksl")),
                minified.into_bytes(),
            ),
        ];
        for (path, bytes) in wanted {
            if check {
                let have = std::fs::read(&path).ok();
                if have.as_deref() != Some(bytes.as_slice()) {
                    mismatches.push(rel(root, &path));
                }
            } else {
                write_file(&path, &bytes)?;
            }
        }
    }
    if check {
        for path in &mismatches {
            println!("STALE {path}");
        }
        if !mismatches.is_empty() {
            bail!(
                "{} module file(s) differ from the pinned Skia tree; run `cargo xtask sksl sync-modules`",
                mismatches.len()
            );
        }
        println!("embedded SkSL modules match the pinned Skia tree");
    } else {
        println!(
            "wrote {} module files under {CRATE_MODULES}",
            MODULES.len() * 2
        );
    }
    Ok(())
}

fn write_file(path: &PathBuf, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::decode_minified;

    #[test]
    fn decodes_concatenated_literals() {
        let text = "static constexpr char SKSL_MINIFIED_sksl_public[] =\n\
                    \"$pure half3 toLinearSrgb(half3);\"\n\
                    \"half4 $eval;\";\n";
        assert_eq!(
            decode_minified(text).unwrap(),
            "$pure half3 toLinearSrgb(half3);half4 $eval;"
        );
    }

    #[test]
    fn rejects_escapes() {
        assert!(decode_minified("static constexpr char X[] =\n\"a\\\"b\";\n").is_err());
    }
}
