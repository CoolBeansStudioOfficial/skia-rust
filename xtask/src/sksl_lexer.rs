//! `cargo xtask sksl gen-lexer`: transcribes the lexer's generated DFA tables and its token-kind
//! enum from Skia's `src/sksl/SkSLLexer.cpp` and `SkSLLexer.h` into
//! `crates/skia-rust-sksl/src/lexer/{tables,kinds}.rs`.
//!
//! Skia generates the tables with `sksllex` (`src/sksl/lex/sksl.lex`). Copying the arrays rather
//! than regenerating the DFA keeps the port tied to the exact tables that produced the goldens.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result, ensure};

const CPP: &str = "third_party/skia/src/sksl/SkSLLexer.cpp";
const HEADER: &str = "third_party/skia/src/sksl/SkSLLexer.h";
const OUT_DIR: &str = "crates/skia-rust-sksl/src/lexer";

/// The text between the braces that follow `name` (for example `kMappings[118] = {`).
fn braced<'a>(text: &'a str, name: &str) -> Result<&'a str> {
    let at = text
        .find(name)
        .with_context(|| format!("no {name} in SkSLLexer.cpp"))?;
    let open = at
        + text[at..]
            .find('{')
            .context("no '{' after the table name")?;
    let mut depth = 0_usize;
    for (i, c) in text[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(&text[open + 1..open + i]);
                }
            }
            _ => {}
        }
    }
    anyhow::bail!("unterminated table {name}")
}

/// The top-level `{ … }` groups inside `inner`.
fn groups(inner: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0_usize;
    let mut start = 0;
    for (i, c) in inner.char_indices() {
        match c {
            '{' => {
                if depth == 0 {
                    start = i + 1;
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    out.push(&inner[start..i]);
                }
            }
            _ => {}
        }
    }
    out
}

/// Every integer literal in `text`, in order (with a leading `-` when present).
fn numbers(text: &str) -> Result<Vec<i64>> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let negative = bytes[i] == b'-';
        let start = if negative { i + 1 } else { i };
        if start < bytes.len() && bytes[start].is_ascii_digit() {
            let end = bytes[start..]
                .iter()
                .position(|b| !b.is_ascii_digit())
                .map_or(bytes.len(), |p| start + p);
            let value: i64 = text[start..end].parse()?;
            out.push(if negative { -value } else { value });
            i = end;
        } else {
            i += 1;
        }
    }
    Ok(out)
}

fn list<T: TryFrom<i64>>(text: &str) -> Result<Vec<T>>
where
    T::Error: std::fmt::Debug,
{
    numbers(text)?
        .into_iter()
        .map(|v| T::try_from(v).map_err(|e| anyhow::anyhow!("{v} out of range: {e:?}")))
        .collect()
}

/// Evaluates a `kCompact` values expression: `|`-separated terms, each a number, or a `<<` of two
/// numbers, optionally in parentheses. (`|` binds looser than `<<`, as in C.)
fn eval_values(expr: &str) -> Result<u32> {
    let mut value: u32 = 0;
    for term in expr.split('|') {
        let term = term
            .trim()
            .trim_start_matches('(')
            .trim_end_matches(')')
            .trim();
        let shifted = match term.split_once("<<") {
            Some((lhs, rhs)) => {
                let lhs: u32 = lhs.trim().parse()?;
                let rhs: u32 = rhs.trim().parse()?;
                lhs << rhs
            }
            None => term.parse()?,
        };
        value |= shifted;
    }
    Ok(value)
}

/// `TK_END_OF_FILE` → `EndOfFile`, `TK_ES3` → `Es3`.
fn variant_name(tk: &str) -> String {
    tk.trim_start_matches("TK_")
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase()
                }
                None => String::new(),
            }
        })
        .collect()
}

/// The `Token::Kind` enumerators, in declaration order.
fn token_kinds(header: &str) -> Result<Vec<String>> {
    let at = header
        .find("enum class Kind {")
        .context("no `enum class Kind` in SkSLLexer.h")?;
    let body = &header[at..];
    let end = body.find("};").context("unterminated Kind enum")?;
    Ok(body[..end]
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| w.starts_with("TK_"))
        .map(str::to_owned)
        .collect())
}

/// Writes the lexer tables and token kinds for the pinned Skia tree.
// One straight-line pass over the arrays; splitting it up would not make it clearer.
#[allow(clippy::too_many_lines)]
pub fn gen_lexer(root: &Path) -> Result<()> {
    let cpp = std::fs::read_to_string(root.join(CPP)).with_context(|| format!("reading {CPP}"))?;
    let header =
        std::fs::read_to_string(root.join(HEADER)).with_context(|| format!("reading {HEADER}"))?;

    let kinds = token_kinds(&header)?;
    ensure!(
        kinds.len() == 94,
        "expected 94 token kinds, found {}",
        kinds.len()
    );
    ensure!(
        kinds.last().map(String::as_str) == Some("TK_NONE"),
        "TK_NONE must come last"
    );

    // Only the initializer: the type name `uint8_t` contains digits of its own.
    let invalid_char: u8 = numbers(
        cpp.lines()
            .find(|l| l.contains("kInvalidChar ="))
            .and_then(|l| l.split_once('='))
            .map(|(_, value)| value)
            .context("no kInvalidChar")?,
    )?
    .first()
    .copied()
    .context("kInvalidChar has no value")?
    .try_into()?;

    let mappings: Vec<u8> = list(braced(&cpp, "kMappings[118]")?)?;
    ensure!(
        mappings.len() == 118,
        "kMappings has {} entries",
        mappings.len()
    );

    let full_groups = groups(braced(&cpp, "kFull[]")?);
    let mut full: Vec<Vec<u16>> = Vec::new();
    for g in full_groups {
        let row: Vec<u16> = list(g)?;
        ensure!(row.len() == 75, "kFull row has {} entries", row.len());
        full.push(row);
    }

    let mut compact: Vec<(u32, Vec<u8>)> = Vec::new();
    for g in groups(braced(&cpp, "kCompact[]")?) {
        // Each entry is `{ <values expr>, { 19 bytes } }`. The values expression can be a shift
        // and an or, such as `21 | (10 << 10)`.
        let data_at = g.find('{').context("kCompact entry without data")?;
        let values = eval_values(g[..data_at].trim().trim_end_matches(','))?;
        let data_numbers = numbers(&g[data_at..])?;
        ensure!(
            data_numbers.len() == 19,
            "kCompact entry has {} data bytes",
            data_numbers.len()
        );
        let data: Vec<u8> = data_numbers
            .iter()
            .map(|&v| u8::try_from(v))
            .collect::<Result<_, _>>()?;
        compact.push((values, data));
    }

    let indices: Vec<i16> = list(braced(&cpp, "kIndices[]")?)?;
    let accepts: Vec<u8> = list(braced(&cpp, "kAccepts[597]")?)?;
    ensure!(
        accepts.len() == 597,
        "kAccepts has {} entries",
        accepts.len()
    );
    ensure!(
        indices.len() == accepts.len(),
        "kIndices and kAccepts disagree in size"
    );

    let mut tables = String::new();
    tables.push_str(
        "// @generated by `cargo xtask sksl gen-lexer` from third_party/skia/src/sksl/SkSLLexer.cpp.\n\
         // Do not edit: the DFA tables are Skia's, copied as they are.\n\n\
         #![allow(clippy::unreadable_literal)]\n\n",
    );
    writeln!(
        tables,
        "/// `kInvalidChar`.\npub const INVALID_CHAR: u8 = {invalid_char};\n"
    )?;
    write_array(&mut tables, "MAPPINGS", "u8", &mappings)?;
    writeln!(
        tables,
        "/// `kFull`: one row of 75 next states per full state."
    )?;
    writeln!(tables, "pub const FULL: [[u16; 75]; {}] = [", full.len())?;
    for row in &full {
        writeln!(tables, "    {},", bracketed(row))?;
    }
    writeln!(tables, "];\n")?;
    writeln!(
        tables,
        "/// `kCompact`: packed 2-bit transitions, as `(values, data)`."
    )?;
    writeln!(
        tables,
        "pub const COMPACT: [(u32, [u8; 19]); {}] = [",
        compact.len()
    )?;
    for (values, data) in &compact {
        writeln!(tables, "    ({values}, {}),", bracketed_u8(data))?;
    }
    writeln!(tables, "];\n")?;
    write_array(&mut tables, "INDICES", "i16", &indices)?;
    write_array(&mut tables, "ACCEPTS", "u8", &accepts)?;

    let mut kinds_src = String::from(
        "// @generated by `cargo xtask sksl gen-lexer` from third_party/skia/src/sksl/SkSLLexer.h.\n\
         // Do not edit.\n\n\
         /// `Token::Kind`: the token kinds, in Skia's order (the values the DFA accepts).\n\
         #[repr(u8)]\n\
         #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\n\
         pub enum TokenKind {\n",
    );
    for (i, tk) in kinds.iter().enumerate() {
        writeln!(
            kinds_src,
            "    /// `{tk}`.\n    {} = {i},",
            variant_name(tk)
        )?;
    }
    kinds_src.push_str("}\n\n");
    writeln!(
        kinds_src,
        "impl TokenKind {{\n    /// Every kind, in declaration order.\n    pub const ALL: [Self; {}] = [",
        kinds.len()
    )?;
    for tk in &kinds {
        writeln!(kinds_src, "        Self::{},", variant_name(tk))?;
    }
    kinds_src.push_str("    ];\n}\n");

    let dir = root.join(OUT_DIR);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("tables.rs"), tables)?;
    std::fs::write(dir.join("kinds.rs"), kinds_src)?;
    println!(
        "wrote {OUT_DIR}/tables.rs and kinds.rs ({} kinds, {} states)",
        kinds.len(),
        accepts.len()
    );
    Ok(())
}

fn write_array<T: std::fmt::Display>(
    out: &mut String,
    name: &str,
    ty: &str,
    values: &[T],
) -> Result<()> {
    writeln!(out, "pub const {name}: [{ty}; {}] = [", values.len())?;
    for chunk in values.chunks(16) {
        let line: Vec<String> = chunk.iter().map(ToString::to_string).collect();
        writeln!(out, "    {},", line.join(", "))?;
    }
    writeln!(out, "];\n")?;
    Ok(())
}

fn bracketed(row: &[u16]) -> String {
    let items: Vec<String> = row.iter().map(ToString::to_string).collect();
    format!("[{}]", items.join(", "))
}

fn bracketed_u8(row: &[u8]) -> String {
    let items: Vec<String> = row.iter().map(ToString::to_string).collect();
    format!("[{}]", items.join(", "))
}
