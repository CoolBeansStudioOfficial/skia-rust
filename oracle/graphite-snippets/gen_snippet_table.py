#!/usr/bin/env python3
"""Generates the built-in snippet table of Graphite's ShaderCodeDictionary from the pinned Skia source.

The table's SkSL names and static function names are part of the generated SkSL, so they must be
byte-identical to Skia's. This script reads the `fBuiltInCodeSnippets[...] = {...}` initializers
out of `src/gpu/graphite/ShaderCodeDictionary.cpp` and writes them as Rust, instead of having them
transcribed by hand:

    python3 oracle/graphite-snippets/gen_snippet_table.py            # rewrite the generated file
    python3 oracle/graphite-snippets/gen_snippet_table.py --check    # fail if it is out of date

The preprocessor conditional on `SK_GRAPHITE_USE_LEGACY_RRECT_CLIP_SHADER` is resolved with the
macro undefined (the default, and what the goldens were made with). The fixed-function blend
snippets are created by a loop in the C++ constructor, not by an initializer, so the Rust
constructor creates them in the same way.

The test `snippet_table_matches_skia_source` in `shader_code_dictionary.rs` independently parses the
same source and compares what the dictionary holds, so a bug in this script cannot hide.
"""

import os
import re
import subprocess
import sys

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
SRC = os.path.join(ROOT, "third_party/skia/src/gpu/graphite/ShaderCodeDictionary.cpp")
OUT = os.path.join(
    ROOT, "crates/skia-rust-gpu/src/graphite/shader_code_dictionary/snippet_table.rs"
)
SRC_REL = "src/gpu/graphite/ShaderCodeDictionary.cpp"


def resolve_preprocessor(text):
    """Keeps the `#else` side of `#if defined(SK_GRAPHITE_USE_LEGACY_RRECT_CLIP_SHADER)`."""
    out = []
    state = None  # None, "if" (skipping), "else" (keeping)
    for line in text.split("\n"):
        stripped = line.strip()
        if stripped.startswith("#if defined(SK_GRAPHITE_USE_LEGACY_RRECT_CLIP_SHADER)"):
            state = "if"
            continue
        if state == "if" and stripped == "#else":
            state = "else"
            continue
        if state is not None and stripped == "#endif":
            state = None
            continue
        if state == "if":
            continue
        out.append(line)
    return "\n".join(out)


def strip_comments(text):
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return re.sub(r"//[^\n]*", "", text)


def split_top_level(text):
    """Splits on commas that are outside braces, parentheses and string literals."""
    parts, depth, cur, in_str = [], 0, [], False
    i = 0
    while i < len(text):
        c = text[i]
        if in_str:
            cur.append(c)
            if c == "\\":
                i += 1
                cur.append(text[i])
            elif c == '"':
                in_str = False
        elif c == '"':
            in_str = True
            cur.append(c)
        elif c in "{(":
            depth += 1
            cur.append(c)
        elif c in "})":
            depth -= 1
            cur.append(c)
        elif c == "," and depth == 0:
            parts.append("".join(cur).strip())
            cur = []
        else:
            cur.append(c)
        i += 1
    last = "".join(cur).strip()
    if last:
        parts.append(last)
    return parts


def unwrap(text):
    text = text.strip()
    assert text.startswith("{") and text.endswith("}"), text
    return text[1:-1].strip()


def snake(name):
    """`GenerateSolidColorPreamble` -> `generate_solid_color_preamble`."""
    s = re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", name)
    return s.lower()


def screaming(name):
    """`kLocalCoords` -> `LOCAL_COORDS`."""
    assert name.startswith("k"), name
    return snake(name[1:]).upper()


def camel(name):
    """`kSolidColorShader` -> `SolidColorShader` (the Rust variant name)."""
    assert name.startswith("k"), name
    return name[1:].replace("_", "")


def rust_string(lit):
    assert lit.startswith('"') and lit.endswith('"'), lit
    return lit


def parse_blocks(text):
    """Yields (id, [top-level fields]) for each built-in snippet initializer."""
    pat = re.compile(
        r"fBuiltInCodeSnippets\[\(int\) BuiltInCodeSnippetID::(k\w+)\]\s*=\s*\{", re.S
    )
    for m in pat.finditer(text):
        start = m.end() - 1
        depth, i, in_str = 0, start, False
        while True:
            c = text[i]
            if in_str:
                if c == "\\":
                    i += 1
                elif c == '"':
                    in_str = False
            elif c == '"':
                in_str = True
            elif c == "{":
                depth += 1
            elif c == "}":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        yield m.group(1), split_top_level(text[start + 1 : i])


def uniform_expr(item):
    item = item.strip()
    if item == "Uniform::PaintColor()":
        return "Uniform::paint_color()"
    inner = split_top_level(unwrap(item))
    name = rust_string(inner[0])
    ty = re.fullmatch(r"SkSLType::k(\w+)", inner[1]).group(1)
    if len(inner) == 2:
        return f"Uniform::new({name}, SkSLType::{ty})"
    return f"Uniform::new_array({name}, SkSLType::{ty}, {inner[2]})"


def uniforms_expr(field):
    field = field.strip()
    if field == "{}":
        return []
    body = unwrap(unwrap(field))  # `{{ ... }}`
    if not body:
        return []
    return [uniform_expr(i) for i in split_top_level(body)]


def textures_expr(field):
    return [f"TextureAndSampler::new({s})" for s in re.findall(r'"[^"]*"', field)]


def flags_expr(field):
    names = [f.strip() for f in field.split("|")]
    out = []
    for n in names:
        m = re.fullmatch(r"SnippetRequirementFlags::(k\w+)", n)
        assert m, n
        out.append(f"SnippetRequirementFlags::{screaming(m.group(1))}")
    return " | ".join(out)


def option_fn(field):
    field = field.strip()
    if field == "nullptr":
        return "None"
    return f"Some(super::{snake(field)})"


def block_to_rust(sid, fields):
    # Positional fields: name, staticFn, flags, uniforms, [textures, preamble, numChildren,
    # liftable expression, liftable type, liftable interpolation].
    name = rust_string(fields[0])
    static_fn = fields[1].strip()
    static_fn = "None" if static_fn == "nullptr" else f"Some({rust_string(static_fn)})"
    flags = flags_expr(fields[2])
    uniforms = uniforms_expr(fields[3])
    textures = textures_expr(fields[4]) if len(fields) > 4 else []
    preamble = option_fn(fields[5]) if len(fields) > 5 else "None"
    num_children = "0"
    if len(fields) > 6:
        n = fields[6].strip()
        num_children = "super::NUM_COORDINATE_MANIPULATE_CHILDREN" if n == "kNumCoordinateManipulateChildren" else n
    liftable = option_fn(fields[7]) if len(fields) > 7 else "None"
    lift_ty = "None"
    if len(fields) > 8:
        m = re.fullmatch(r"ShaderSnippet::LiftableExpressionType::k(\w+)", fields[8].strip())
        lift_ty = m.group(1)
    interp = "Perspective"
    if len(fields) > 9:
        m = re.fullmatch(r"Interpolation::k(\w+)", fields[9].strip())
        interp = m.group(1)

    lines = [f"    table[BuiltInCodeSnippetID::{camel(sid)} as usize] = Some(ShaderSnippet {{"]
    lines.append(f"        name: String::from({name}),")
    lines.append(f"        static_function_name: {static_fn},")
    lines.append(f"        snippet_requirement_flags: {flags},")
    if uniforms:
        lines.append("        uniforms: vec![")
        lines.extend(f"            {u}," for u in uniforms)
        lines.append("        ],")
    if textures:
        lines.append("        textures_and_samplers: vec![")
        lines.extend(f"            {t}," for t in textures)
        lines.append("        ],")
    if num_children != "0":
        lines.append(f"        num_children: {num_children},")
    if preamble != "None":
        lines.append(f"        preamble_generator: {preamble},")
    if interp != "Perspective":
        lines.append(f"        liftable_expression_interpolation: Interpolation::{interp},")
    if lift_ty != "None":
        lines.append(f"        liftable_expression_type: LiftableExpressionType::{lift_ty},")
    if liftable != "None":
        lines.append(f"        liftable_expression_generator: {liftable},")
    lines.append("        ..ShaderSnippet::default()")
    lines.append("    });")
    return "\n".join(lines)


def generate():
    text = open(SRC, encoding="utf-8").read()
    text = strip_comments(resolve_preprocessor(text))
    blocks = list(parse_blocks(text))
    print(len(blocks), file=sys.stderr)
    out = [
        "// Copyright 2022 Google LLC",
        "// Copyright 2026 The skia-rust Authors",
        "// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.",
        f"// Ported from Skia: {SRC_REL}",
        "",
        "//! The built-in snippet table of the `ShaderCodeDictionary` constructor.",
        "//!",
        "//! GENERATED by `oracle/graphite-snippets/gen_snippet_table.py` from the pinned Skia source.",
        "//! Do not edit by hand: the `SkSL` names and static function names below end up in the generated",
        "//! `SkSL`, so they must be byte-identical to Skia's.",
        "",
        "use super::{",
        "    LiftableExpressionType, ShaderSnippet, SnippetRequirementFlags, TextureAndSampler,",
        "};",
        "use crate::graphite::attribute::Interpolation;",
        "use crate::graphite::built_in_code_snippet_id::BuiltInCodeSnippetID;",
        "use crate::graphite::uniform::Uniform;",
        "use crate::sksl_type_shared::SkSLType;",
        "",
        "/// Fills the slots of the snippets that `ShaderCodeDictionary`'s constructor initializes one",
        "/// by one (all but the fixed-function blends, which it creates in a loop).",
        f"// Port of: {SRC_REL}#L{{FIRST}}-L{{LAST}} (chrome/m156)",
        "#[allow(clippy::too_many_lines)] // mirrors the C++ constructor's table",
        "pub(super) fn fill_built_in_snippets(table: &mut [Option<ShaderSnippet>]) {",
    ]
    for sid, fields in blocks:
        out.append(block_to_rust(sid, fields))
    out.append("}")
    out.append("")
    rendered = "\n".join(out)
    raw = open(SRC, encoding="utf-8").read().split("\n")
    first = next(i for i, l in enumerate(raw, 1) if "BuiltInCodeSnippetID::kError] = {" in l)
    last = next(
        i for i, l in enumerate(raw, 1) if "BuiltInCodeSnippetID::kHSLCBlender] = {" in l
    ) + 5
    rendered = rendered.replace("{FIRST}", str(first)).replace("{LAST}", str(last))
    # Format like `cargo fmt` would, so that the checked-in file passes `cargo fmt --check`.
    formatted = subprocess.run(
        ["rustfmt", "--edition", "2024", "--emit", "stdout"],
        input=rendered,
        capture_output=True,
        text=True,
        cwd=ROOT,
        check=True,
    ).stdout
    # `--emit stdout` prefixes the file name on some versions; keep the source text only.
    return formatted if formatted.startswith("//") else formatted.split("\n\n", 1)[1]


def main():
    rendered = generate()
    if "--check" in sys.argv:
        current = open(OUT, encoding="utf-8").read() if os.path.exists(OUT) else ""
        if current != rendered:
            print(f"{OUT} is out of date; run gen_snippet_table.py", file=sys.stderr)
            return 1
        return 0
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        f.write(rendered)
    return 0


if __name__ == "__main__":
    sys.exit(main())
