// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Adapted from Skia: src/gpu/graphite/ShaderCodeDictionary.cpp (the tests that need ShaderInfo,
//                    tests/graphite/KeyTest.cpp and friends, wait for G6)

//! The `ShaderCodeDictionary`: its contents against the pinned Skia source, key interning, runtime
//! effect snippets and the snippet preamble generators.
//!
//! The first group of tests parses `ShaderCodeDictionary.cpp`, `BuiltInCodeSnippetID.h`,
//! `SkKnownRuntimeEffects.h`, `Blend.cpp` and `SkBlendMode.cpp` of the pinned Skia source, with a
//! parser that is independent of `oracle/graphite-snippets/gen_snippet_table.py` (it reads the
//! `/*name=*/` style argument comments instead of the positions), and compares what the
//! dictionary holds: the `SkSL` names and static function names of the snippet table are part of
//! the generated `SkSL`, so they must be byte-identical to Skia's.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::known_runtime_effects::{
    StableKey, UNKNOWN_RUNTIME_EFFECT_ID_START, USER_DEFINED_KNOWN_RUNTIME_EFFECTS_START,
    get_known_runtime_effect, stable_key_from_u32,
};
use skia_rust_core::mesh::{Attribute, AttributeType, MeshSpecification, Varying};
use skia_rust_core::runtime_effect::RuntimeEffect;
use skia_rust_core::runtime_effect_priv;
use skia_rust_gpu::graphite::built_in_code_snippet_id::{
    BUILT_IN_CODE_SNIPPET_ID_COUNT, BuiltInCodeSnippetID, FIXED_BLEND_ID_OFFSET,
};
use skia_rust_gpu::graphite::resource_types::Layout;
use skia_rust_gpu::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use skia_rust_gpu::graphite::shader_code_dictionary::{
    ShaderCodeDictionary, ShaderNode, ShaderSnippetArgs, SnippetRequirementFlags,
    known_runtime_effect_snippet_name,
};
use skia_rust_gpu::graphite::shader_info::ShaderInfo;
use skia_rust_gpu::sksl_type_shared::SkSLType;

fn skia_source(path: &str) -> String {
    let full = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party/skia")
        .join(path);
    fs::read_to_string(&full).unwrap_or_else(|e| panic!("cannot read {}: {e}", full.display()))
}

fn new_dictionary() -> ShaderCodeDictionary {
    ShaderCodeDictionary::new(Layout::Std140, &[])
}

//--------------------------------------------------------------------------------------------------
// The contents of the dictionary against Skia's source

/// What one `fBuiltInCodeSnippets[...] = {...}` initializer says.
#[derive(Debug)]
struct SourceSnippet {
    id: String,
    name: String,
    static_fn: Option<String>,
    /// (name, `SkSLType` name, array count); the paint color has the name `paintColor`.
    uniforms: Vec<(String, String, i32)>,
    textures: Vec<String>,
    num_children: Option<String>,
}

/// Keeps the `#else` side of `#if defined(SK_GRAPHITE_USE_LEGACY_RRECT_CLIP_SHADER)`.
fn without_legacy_branch(text: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    let mut in_if = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("#if defined(SK_GRAPHITE_USE_LEGACY_RRECT_CLIP_SHADER)") {
            in_if = true;
            skipping = true;
            continue;
        }
        if in_if && t == "#else" {
            skipping = false;
            continue;
        }
        if in_if && t == "#endif" {
            in_if = false;
            skipping = false;
            continue;
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// The text from the first `{` at or after `from` to its matching `}` (both included).
fn balanced(text: &str, from: usize) -> &str {
    let bytes = text.as_bytes();
    let start = from + text[from..].find('{').expect("an opening brace");
    let mut depth = 0;
    let mut in_str = false;
    for (i, &c) in bytes.iter().enumerate().skip(start) {
        match c {
            b'"' => in_str = !in_str,
            b'{' if !in_str => depth += 1,
            b'}' if !in_str => {
                depth -= 1;
                if depth == 0 {
                    return &text[start..=i];
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced braces");
}

fn string_literals(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let end = after.find('"').expect("a closing quote");
        out.push(after[..end].to_owned());
        rest = &after[end + 1..];
    }
    out
}

/// The text after the argument comment `tag` (like `/*name=*/`), if the block has it.
fn after_tag<'a>(block: &'a str, tag: &str) -> Option<&'a str> {
    block.find(tag).map(|i| block[i + tag.len()..].trim_start())
}

fn parse_source_snippets() -> Vec<SourceSnippet> {
    let text = without_legacy_branch(&skia_source("src/gpu/graphite/ShaderCodeDictionary.cpp"));
    let head = "fBuiltInCodeSnippets[(int) BuiltInCodeSnippetID::k";
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = text[at..].find(head) {
        let id_start = at + i + head.len();
        let id_end = id_start + text[id_start..].find(']').unwrap();
        let id = text[id_start..id_end].to_owned();
        let eq = id_end + text[id_end..].find('=').unwrap();
        let block = balanced(&text, eq);
        at = eq + block.len();

        let name_text = after_tag(block, "/*name=*/").expect("a name tag");
        if !name_text.starts_with('"') {
            // The fixed blends of the loop (`SkBlendMode_Name(...)`) are checked separately.
            continue;
        }
        let name = string_literals(name_text)[0].clone();

        let static_text = after_tag(block, "/*staticFn=*/").expect("a staticFn tag");
        let static_fn = if static_text.starts_with("nullptr") {
            None
        } else {
            Some(string_literals(static_text)[0].clone())
        };

        let mut uniforms = Vec::new();
        let uniforms_text = after_tag(block, "/*uniforms=*/").expect("a uniforms tag");
        let uniforms_block = balanced(uniforms_text, 0);
        if uniforms_block.contains("Uniform::PaintColor()") {
            uniforms.push((String::from("paintColor"), String::from("Half4"), 0));
        }
        let mut rest = uniforms_block;
        while let Some(j) = rest.find("{ \"") {
            let end = rest[j..].find('}').unwrap() + j;
            let parts: Vec<&str> = rest[j + 1..end].split(',').map(str::trim).collect();
            let uname = parts[0].trim_matches('"').to_owned();
            let ty = parts[1].strip_prefix("SkSLType::k").unwrap().to_owned();
            let count = parts.get(2).map_or(0, |c| c.parse().unwrap());
            uniforms.push((uname, ty, count));
            rest = &rest[end..];
        }

        let textures = after_tag(block, "/*texturesAndSamplers=*/")
            .map(|t| string_literals(balanced(t, 0)))
            .unwrap_or_default();

        let num_children = after_tag(block, "/*numChildren=*/").map(|t| {
            t.chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect::<String>()
        });

        out.push(SourceSnippet {
            id,
            name,
            static_fn,
            uniforms,
            textures,
            num_children,
        });
    }
    out
}

/// `kCSXform_sRGB` -> `CSXformsRGB`, the Rust variant name.
fn variant_name(c_name: &str) -> String {
    c_name.strip_prefix('k').unwrap().replace('_', "")
}

fn header_ids() -> Vec<String> {
    let text = skia_source("src/gpu/graphite/BuiltInCodeSnippetID.h");
    let start = text.find("enum class BuiltInCodeSnippetID").unwrap();
    let end = text.find("kFirstFixedBlend =").unwrap();
    let mut ids = Vec::new();
    for line in text[start..end].lines().skip(1) {
        let t = line.trim_start();
        if t.starts_with('k') {
            let ident: String = t
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            ids.push(ident);
        }
    }
    ids
}

#[test]
fn built_in_ids_match_skia_source() {
    let ids = header_ids();
    assert_eq!(ids.len(), BUILT_IN_CODE_SNIPPET_ID_COUNT as usize);
    for (i, c_name) in ids.iter().enumerate() {
        let id = BuiltInCodeSnippetID::from_u32(u32::try_from(i).unwrap()).unwrap();
        assert_eq!(format!("{id:?}"), variant_name(c_name), "id {i}");
    }
    assert_eq!(BuiltInCodeSnippetID::LAST as usize, ids.len() - 1);
    assert_eq!(ids[FIXED_BLEND_ID_OFFSET as usize], "kFixedBlend_Clear");
}

#[test]
fn snippet_table_matches_skia_source() {
    let dict = new_dictionary();
    let parsed = parse_source_snippets();
    // 57 initializers; the 29 fixed blends are made by a loop.
    assert_eq!(parsed.len(), FIXED_BLEND_ID_OFFSET as usize);

    let mut seen = HashSet::new();
    for p in &parsed {
        let id = header_ids()
            .iter()
            .position(|c_name| c_name.strip_prefix('k').unwrap() == p.id)
            .unwrap_or_else(|| panic!("{} is not a built-in id", p.id));
        assert!(seen.insert(id), "{} is initialized twice", p.id);
        let entry = dict.get_entry(i32::try_from(id).unwrap()).unwrap();

        assert_eq!(entry.name, p.name, "name of {}", p.id);
        assert_eq!(
            entry.static_function_name.map(str::to_owned),
            p.static_fn,
            "static function of {}",
            p.id
        );
        let uniforms: Vec<(String, String, i32)> = entry
            .uniforms
            .iter()
            .map(|u| (u.name().to_owned(), format!("{:?}", u.ty()), u.count()))
            .collect();
        assert_eq!(uniforms, p.uniforms, "uniforms of {}", p.id);
        let textures: Vec<String> = entry
            .textures_and_samplers
            .iter()
            .map(|t| t.name().to_owned())
            .collect();
        assert_eq!(textures, p.textures, "textures of {}", p.id);
        match p.num_children.as_deref() {
            None => assert_eq!(entry.num_children, 0, "children of {}", p.id),
            Some("kNumCoordinateManipulateChildren") => {
                assert_eq!(entry.num_children, 1, "children of {}", p.id);
            }
            Some(n) => assert_eq!(entry.num_children, n.parse::<i32>().unwrap(), "{}", p.id),
        }
    }
    assert_eq!(seen.len(), 57);
}

/// `case SkBlendMode::kClear: return "blend_clear";` lines of `text`, in order.
fn blend_mode_returns(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let t = line.trim();
            let rest = t.strip_prefix("case SkBlendMode::k")?;
            let (mode, after_colon) = rest.split_once(':')?;
            let returned = after_colon.trim_start().strip_prefix("return \"")?;
            Some((mode.to_owned(), returned.trim_end_matches("\";").to_owned()))
        })
        .collect()
}

#[test]
fn fixed_blend_snippets_match_skia_source() {
    let dict = new_dictionary();
    let func_returns = blend_mode_returns(&skia_source("src/gpu/Blend.cpp"));
    let name_returns = blend_mode_returns(&skia_source("src/core/SkBlendMode.cpp"));
    let ids = header_ids();

    for i in 0..=(BlendMode::LAST_MODE as i32) {
        let entry = dict.get_entry(FIXED_BLEND_ID_OFFSET + i).unwrap();
        let c_id = &ids[usize::try_from(FIXED_BLEND_ID_OFFSET + i).unwrap()];
        let mode = c_id.strip_prefix("kFixedBlend_").unwrap();

        let func = func_returns
            .iter()
            .find(|(m, _)| m == mode)
            .unwrap_or_else(|| panic!("no BlendFuncName for {mode}"));
        let name = name_returns
            .iter()
            .find(|(m, _)| m == mode)
            .unwrap_or_else(|| panic!("no SkBlendMode_Name for {mode}"));
        assert_eq!(entry.name, name.1, "name of {c_id}");
        assert_eq!(
            entry.static_function_name,
            Some(func.1.as_str()),
            "static function of {c_id}"
        );
        assert_eq!(
            entry.snippet_requirement_flags,
            SnippetRequirementFlags::PRIOR_STAGE_OUTPUT
                | SnippetRequirementFlags::BLENDER_DST_COLOR
        );
        assert_eq!(entry.uniforms.len(), 0);
        assert_eq!(entry.num_children, 0);
        // Fixed blend ids are ordered like `SkBlendMode`.
        assert_eq!(
            BlendMode::from_i32(i).unwrap().name(),
            entry.name,
            "{c_id} is out of order"
        );
    }
}

#[test]
fn known_runtime_effect_names_match_skia_source() {
    let text = skia_source("src/core/SkKnownRuntimeEffects.h");
    let start = text.find("#define SK_ALL_STABLEKEYS").unwrap();
    let mut names = Vec::new();
    for line in text[start..].lines() {
        let t = line.trim_start();
        let name = if let Some(r) = t.strip_prefix("M2(") {
            r.split(',').next().unwrap().to_owned()
        } else if let Some(r) = t.strip_prefix("M(") {
            r.split(')').next().unwrap().to_owned()
        } else {
            continue;
        };
        names.push(format!("${name}"));
    }
    // The first line of the macro has the M2(Invalid, Start) entry, which the filter above saw.
    assert_eq!(names.len(), 29);
    for (i, name) in names.iter().enumerate() {
        let key = stable_key_from_u32(500 + u32::try_from(i).unwrap()).unwrap();
        assert_eq!(known_runtime_effect_snippet_name(key), name, "{key:?}");
    }
    assert_eq!(StableKey::LAST as usize, 500 + names.len() - 1);
}

#[test]
fn built_in_snippets_are_complete_and_unique() {
    let dict = new_dictionary();
    let mut names = HashSet::new();
    for id in 0..BUILT_IN_CODE_SNIPPET_ID_COUNT {
        let entry = dict.get_entry(id).expect("a snippet");
        assert_ne!(entry.name, "");
        assert!(
            entry.static_function_name.is_some() || entry.preamble_generator.is_some(),
            "{} can't generate code",
            entry.name
        );
        assert!(
            names.insert(entry.name.clone()),
            "{} is a duplicate",
            entry.name
        );
        assert!(dict.is_valid_id(id));
    }
    assert!(dict.get_entry(BUILT_IN_CODE_SNIPPET_ID_COUNT).is_none());
    assert!(dict.get_entry(-1).is_none());
    assert!(!dict.is_valid_id(-1));
}

#[test]
fn snippet_requirements_of_a_few_snippets() {
    let dict = new_dictionary();
    let solid = dict.get_entry_built_in(BuiltInCodeSnippetID::SolidColorShader);
    assert_eq!(solid.name, "SolidColor");
    assert!(solid.static_function_name.is_none());
    assert!(solid.preamble_generator.is_some());
    assert!(solid.liftable_expression_generator.is_some());

    let image = dict.get_entry_built_in(BuiltInCodeSnippetID::ImageShader);
    assert!(image.needs_local_coords());
    assert!(image.stores_sampler_desc_data());
    assert!(!image.needs_prior_stage_output());

    let blender = dict.get_entry_built_in(BuiltInCodeSnippetID::PorterDuffBlender);
    assert!(blender.needs_prior_stage_output());
    assert!(blender.needs_blender_dst_color());

    let compose = dict.get_entry_built_in(BuiltInCodeSnippetID::Compose);
    assert_eq!(compose.num_children, 2);
    assert_eq!(
        dict.get_entry_built_in(BuiltInCodeSnippetID::BlendCompose)
            .num_children,
        3
    );
    let paint = dict.get_entry_built_in(BuiltInCodeSnippetID::RGBPaintColor);
    assert!(paint.uniforms[0].is_paint_color());
    assert_eq!(paint.uniforms[0].ty(), SkSLType::Half4);
}

//--------------------------------------------------------------------------------------------------
// Runtime effects

const SHADER_SKSL: &str = "uniform float4 color;\n\
                           half4 main(float2 p) { return half4(color); }";

fn make_shader(sksl: &str, name: &str) -> RuntimeEffect {
    // (The options have private fields, so a struct expression can't fill the others in.)
    #[allow(clippy::field_reassign_with_default)]
    let options = {
        let mut options = skia_rust_core::runtime_effect::Options::default();
        options.name = name;
        options
    };
    RuntimeEffect::make_for_shader(sksl, Some(&options)).expect("the SkSL compiles")
}

#[test]
fn runtime_effect_snippets_are_assigned_in_first_use_order() {
    let dict = new_dictionary();
    let a = make_shader(SHADER_SKSL, "A");
    let b = make_shader(
        "uniform float4 other;\nhalf4 main(float2 p) { return half4(other); }",
        "",
    );
    let start = UNKNOWN_RUNTIME_EFFECT_ID_START.cast_signed();
    assert_eq!(dict.num_user_defined_runtime_effects(), 0);
    assert!(!dict.is_valid_id(start));

    assert_eq!(dict.find_or_create_runtime_effect_snippet(&a), Some(start));
    assert_eq!(
        dict.find_or_create_runtime_effect_snippet(&b),
        Some(start + 1)
    );
    // The same program gets the same id, also from another object.
    let a2 = make_shader(SHADER_SKSL, "A");
    assert_eq!(dict.find_or_create_runtime_effect_snippet(&a2), Some(start));
    assert_eq!(
        dict.find_or_create_runtime_effect_snippet(&b),
        Some(start + 1)
    );
    assert_eq!(dict.num_user_defined_runtime_effects(), 2);
    assert!(dict.is_valid_id(start + 1));
    assert!(!dict.is_valid_id(start + 2));

    let entry_a = dict.get_entry(start).unwrap();
    assert_eq!(entry_a.name, "A");
    let entry_b = dict.get_entry(start + 1).unwrap();
    assert_eq!(entry_b.name, "RuntimeEffect");
    assert!(dict.get_entry(start + 2).is_none());

    // The snippet of a shader needs local coords and passes them through (it has no children).
    assert_eq!(
        entry_a.snippet_requirement_flags,
        SnippetRequirementFlags::LOCAL_COORDS | SnippetRequirementFlags::PASSTHROUGH_LOCAL_COORDS
    );
    assert_eq!(entry_a.num_children, 0);
    assert!(entry_a.static_function_name.is_none());
    assert_eq!(entry_a.uniforms.len(), 1);
    assert_eq!(entry_a.uniforms[0].name(), "color");
    assert_eq!(entry_a.uniforms[0].ty(), SkSLType::Float4);
}

#[test]
fn runtime_effect_uniform_conversion() {
    let effect = make_shader(
        "uniform half4 c; uniform float2x2 m; uniform int i; uniform float arr[3];\n\
         half4 main(float2 p) { return half4(c) * half(m[0][0] + float(i) + arr[1]); }",
        "Many",
    );
    let uniforms = ShaderCodeDictionary::convert_runtime_effect_uniforms(effect.uniforms());
    let described: Vec<(String, SkSLType, i32)> = uniforms
        .iter()
        .map(|u| (u.name().to_owned(), u.ty(), u.count()))
        .collect();
    assert_eq!(
        described,
        vec![
            (String::from("c"), SkSLType::Half4, 0),
            (String::from("m"), SkSLType::Float2x2, 0),
            (String::from("i"), SkSLType::Int, 0),
            (String::from("arr"), SkSLType::Float, 3),
        ]
    );
}

#[test]
fn runtime_color_filter_and_blender_snippets() {
    let dict = new_dictionary();
    let filter =
        RuntimeEffect::make_for_color_filter("half4 main(half4 c) { return c.bgra; }", None)
            .expect("compiles");
    let blender = RuntimeEffect::make_for_blender(
        "half4 main(half4 src, half4 dst) { return src + dst; }",
        None,
    )
    .expect("compiles");
    let f = dict.find_or_create_runtime_effect_snippet(&filter).unwrap();
    let b = dict
        .find_or_create_runtime_effect_snippet(&blender)
        .unwrap();
    assert_eq!(
        dict.get_entry(f).unwrap().snippet_requirement_flags,
        SnippetRequirementFlags::PRIOR_STAGE_OUTPUT
    );
    assert_eq!(
        dict.get_entry(b).unwrap().snippet_requirement_flags,
        SnippetRequirementFlags::PRIOR_STAGE_OUTPUT | SnippetRequirementFlags::BLENDER_DST_COLOR
    );
}

#[test]
fn runtime_effect_children_and_color_transforms() {
    let dict = new_dictionary();
    let with_children = make_shader(
        "uniform shader a; uniform colorFilter b;\n\
         half4 main(float2 p) { return b.eval(a.eval(p)); }",
        "Kids",
    );
    let id = dict
        .find_or_create_runtime_effect_snippet(&with_children)
        .unwrap();
    let entry = dict.get_entry(id).unwrap();
    assert_eq!(entry.num_children, 2);
    // `a` is sampled with the passed-through coordinates and `b` takes a color.
    assert!(
        entry
            .snippet_requirement_flags
            .contains(SnippetRequirementFlags::LOCAL_COORDS)
    );

    // toLinearSrgb() and fromLinearSrgb() add two children.
    let with_transforms = make_shader(
        "half4 main(float2 p) { return half4(fromLinearSrgb(toLinearSrgb(half3(1)).rgb), 1); }",
        "Transforms",
    );
    assert!(runtime_effect_priv::uses_color_transform(&with_transforms));
    let id = dict
        .find_or_create_runtime_effect_snippet(&with_transforms)
        .unwrap();
    assert_eq!(dict.get_entry(id).unwrap().num_children, 2);
}

#[test]
fn known_runtime_effect_snippets() {
    let dict = new_dictionary();
    let blend = get_known_runtime_effect(StableKey::Blend).unwrap();
    let id = dict.find_or_create_runtime_effect_snippet(blend).unwrap();
    assert_eq!(id, StableKey::Blend as i32);
    assert_eq!(dict.get_entry(id).unwrap().name, "$Blend");
    // A second call keeps the snippet and the id.
    assert_eq!(dict.find_or_create_runtime_effect_snippet(blend), Some(id));
    assert_eq!(dict.num_user_defined_runtime_effects(), 0);

    let blur = get_known_runtime_effect(StableKey::TwoDBlur12).unwrap();
    let blur_id = dict.find_or_create_runtime_effect_snippet(blur).unwrap();
    assert_eq!(blur_id, StableKey::TwoDBlur12 as i32);
    assert_eq!(dict.get_entry(blur_id).unwrap().name, "$2DBlur12");
    // A known effect that was not registered yet has no entry, and the dictionary is valid for
    // every known key.
    assert!(dict.is_valid_id(StableKey::Overdraw as i32));
}

#[test]
fn user_defined_known_runtime_effects() {
    let first = make_shader(SHADER_SKSL, "UserKnown");
    let second = make_shader(
        "uniform float4 other;\nhalf4 main(float2 p) { return half4(other); }",
        "",
    );
    // A duplicate of the first is dropped.
    let first_again = make_shader(SHADER_SKSL, "UserKnown");
    let dict = ShaderCodeDictionary::new(
        Layout::Std140,
        &[first.clone(), second.clone(), first_again],
    );
    assert_eq!(dict.num_user_defined_known_runtime_effects(), 2);

    let start = USER_DEFINED_KNOWN_RUNTIME_EFFECTS_START.cast_signed();
    assert_eq!(
        runtime_effect_priv::stable_key(&first),
        start.cast_unsigned()
    );
    assert_eq!(
        runtime_effect_priv::stable_key(&second),
        start.cast_unsigned() + 1
    );
    assert!(dict.is_user_defined_known_runtime_effect(start));
    assert!(dict.is_user_defined_known_runtime_effect(start + 1));
    assert!(!dict.is_user_defined_known_runtime_effect(start + 2));
    assert!(dict.is_valid_id(start + 1));
    assert_eq!(dict.get_entry(start).unwrap().name, "UserKnown");
    assert_eq!(
        dict.get_entry(start + 1).unwrap().name,
        "UserDefinedKnownRuntimeEffect"
    );
    assert!(dict.get_entry(start + 2).is_none());
    assert!(dict.get_user_defined_known_runtime_effect(start).is_some());
    assert!(
        dict.get_user_defined_known_runtime_effect(start + 2)
            .is_none()
    );

    // Looking the effects up gives their stable ids, without making unknown snippets.
    assert_eq!(
        dict.find_or_create_runtime_effect_snippet(&first),
        Some(start)
    );
    assert_eq!(
        dict.find_or_create_runtime_effect_snippet(&second),
        Some(start + 1)
    );
    assert_eq!(dict.num_user_defined_runtime_effects(), 0);
}

fn make_mesh_spec() -> Arc<MeshSpecification> {
    let attributes = [Attribute {
        ty: AttributeType::Float2,
        offset: 0,
        name: String::from("pos"),
    }];
    let varyings: [Varying; 0] = [];
    let result = MeshSpecification::make(
        &attributes,
        8,
        &varyings,
        "Varyings main(const Attributes a) { Varyings v; v.position = a.pos; return v; }",
        "float2 main(const Varyings v) { return v.position; }",
    );
    result.specification.expect(&result.error)
}

#[test]
fn mesh_snippets() {
    let dict = new_dictionary();
    let spec = make_mesh_spec();
    let id = dict.find_or_create_mesh_snippet(&spec);
    assert_eq!(id, UNKNOWN_RUNTIME_EFFECT_ID_START.cast_signed());
    // The same specification has the same id.
    assert_eq!(dict.find_or_create_mesh_snippet(&spec), id);
    assert_eq!(dict.find_or_create_mesh_snippet(&make_mesh_spec()), id);
    let entry = dict.get_entry(id).unwrap();
    assert_eq!(entry.name, "MeshShader");
    assert_eq!(entry.num_children, 0);
    assert_eq!(
        entry.snippet_requirement_flags,
        SnippetRequirementFlags::NONE
    );

    // Mesh snippets and runtime effects share the id range.
    let effect = make_shader(SHADER_SKSL, "A");
    assert_eq!(
        dict.find_or_create_runtime_effect_snippet(&effect),
        Some(id + 1)
    );
}

#[test]
fn mesh_preambles() {
    let dict = new_dictionary();
    let rte_dict = Arc::new(RuntimeEffectDictionary::new());
    let spec = make_mesh_spec();
    let id = dict.find_or_create_mesh_snippet(&spec);
    rte_dict.set_mesh_spec(id, spec);
    assert!(rte_dict.find_mesh_spec(id).is_some());
    assert!(rte_dict.find_mesh_spec(id + 1).is_none());

    let node = ShaderNode::new(dict.get_entry(id).unwrap(), Vec::new(), id, 0, Vec::new());
    let shader_info = ShaderInfo::new(&dict, rte_dict, None);
    // The default preamble of a snippet without children is empty.
    assert_eq!(node.generate_default_preamble(&shader_info), "");

    let vs = ShaderCodeDictionary::generate_mesh_vs_preamble(&shader_info, &node);
    assert!(vs.contains("drawMeshVSMain"), "{vs}");
    let fs = ShaderCodeDictionary::generate_mesh_fs_preamble(&shader_info, &node);
    assert!(fs.contains("drawMeshFSMain"), "{fs}");
}

//--------------------------------------------------------------------------------------------------
// The preamble generators

fn node(dict: &ShaderCodeDictionary, id: BuiltInCodeSnippetID, key_index: i32) -> ShaderNode {
    node_with_children(dict, id as i32, key_index, Vec::new())
}

fn node_with_children(
    dict: &ShaderCodeDictionary,
    id: i32,
    key_index: i32,
    children: Vec<ShaderNode>,
) -> ShaderNode {
    ShaderNode::new(
        dict.get_entry(id).unwrap(),
        children,
        id,
        key_index,
        Vec::new(),
    )
}

fn shader_info(dict: &ShaderCodeDictionary, ssbo: Option<&'static str>) -> ShaderInfo {
    ShaderInfo::new(dict, Arc::new(RuntimeEffectDictionary::new()), ssbo)
}

fn preamble(node: &ShaderNode, info: &ShaderInfo) -> String {
    (node.entry().preamble_generator.unwrap())(info, node)
}

#[test]
fn solid_color_preamble() {
    let dict = new_dictionary();
    let solid = node(&dict, BuiltInCodeSnippetID::SolidColorShader, 3);
    let info = shader_info(&dict, None);
    assert_eq!(
        preamble(&solid, &info),
        "half4 SolidColor_3() {return half4(color_3);}"
    );
    // The uniform is read from the storage buffer when there is an index for it.
    let ssbo_info = shader_info(&dict, Some("ssboIndex"));
    assert_eq!(
        preamble(&solid, &ssbo_info),
        "half4 SolidColor_3() {return half4(combinedUniformData[ssboIndex].color_3);}"
    );

    // A lifted expression is read from its varying, an omitted one is zero.
    let mut lifted = solid.clone();
    lifted.set_lift_expression_flag();
    assert_eq!(
        preamble(&lifted, &info),
        "half4 SolidColor_3() {return SolidColor_3_Var;}"
    );
    assert_eq!(lifted.get_expression_varying_name(), "SolidColor_3_Var");
    let mut omitted = solid.clone();
    omitted.set_omit_expression_flag();
    assert_eq!(
        preamble(&omitted, &info),
        "half4 SolidColor_3() {return half4(0);}"
    );
}

#[test]
fn local_matrix_preamble() {
    let dict = new_dictionary();
    let info = shader_info(&dict, None);
    // LocalMatrix [ HardwareImage ]: the child needs coordinates.
    let image = node(&dict, BuiltInCodeSnippetID::HWImageShader, 1);
    let local_matrix = node_with_children(
        &dict,
        BuiltInCodeSnippetID::LocalMatrixShader as i32,
        0,
        vec![image.clone()],
    );
    assert!(
        local_matrix
            .required_flags()
            .contains(SnippetRequirementFlags::LOCAL_COORDS)
    );
    assert_eq!(
        preamble(&local_matrix, &info),
        "half4 LocalMatrix_0(float2 pos) {  return \
         sk_hw_image_shader(float2x2(upper2x2_0.xy, upper2x2_0.zw)*pos + translation_0, image_1); }"
    );

    // Lifted: the coordinates come from the varying.
    let mut lifted = local_matrix.clone();
    lifted.set_lift_expression_flag();
    lifted.unset_local_coords_flag();
    assert_eq!(
        preamble(&lifted, &info),
        "half4 LocalMatrix_0() {  return sk_hw_image_shader(LocalMatrix_0_Var, image_1); }"
    );

    // A child that needs no coordinates leaves them alone.
    let solid = node(&dict, BuiltInCodeSnippetID::SolidColorShader, 1);
    let local_matrix = node_with_children(
        &dict,
        BuiltInCodeSnippetID::LocalMatrixShader as i32,
        0,
        vec![solid],
    );
    assert_eq!(
        preamble(&local_matrix, &info),
        "half4 LocalMatrix_0() {  return SolidColor_1(); }"
    );

    // The perspective version works in float3.
    let persp = node_with_children(
        &dict,
        BuiltInCodeSnippetID::LocalMatrixShaderPersp as i32,
        0,
        vec![image],
    );
    assert_eq!(
        preamble(&persp, &info),
        "half4 LocalMatrixShaderPersp_0(float2 pos) { float3 perspCoord = localMatrix_0 * pos.xy1; \
         return sk_hw_image_shader(perspCoord.xy / perspCoord.z, image_1); }"
    );
}

#[test]
fn coord_clamp_and_normalize_preambles() {
    let dict = new_dictionary();
    let info = shader_info(&dict, None);
    let image = node(&dict, BuiltInCodeSnippetID::HWImageShader, 1);
    let clamp = node_with_children(
        &dict,
        BuiltInCodeSnippetID::CoordClampShader as i32,
        0,
        vec![image.clone()],
    );
    assert_eq!(
        preamble(&clamp, &info),
        "half4 CoordClamp_0(float2 pos) {  return \
         sk_hw_image_shader(clamp(pos, subset_0.LT, subset_0.RB), image_1); }"
    );
    let normalize = node_with_children(
        &dict,
        BuiltInCodeSnippetID::CoordNormalizeShader as i32,
        0,
        vec![image],
    );
    assert_eq!(
        preamble(&normalize, &info),
        "half4 CoordNormalize_0(float2 pos) {  return \
         sk_hw_image_shader((invDimensions_0 * pos), image_1); }"
    );
}

#[test]
fn compose_preamble_and_default_preamble() {
    let dict = new_dictionary();
    let info = shader_info(&dict, None);
    // Compose [ SolidColor, MatrixColorFilter ]: the filter (outer) takes the solid color.
    let solid = node(&dict, BuiltInCodeSnippetID::SolidColorShader, 1);
    let filter = node(&dict, BuiltInCodeSnippetID::MatrixColorFilter, 2);
    let compose = node_with_children(
        &dict,
        BuiltInCodeSnippetID::Compose as i32,
        0,
        vec![solid, filter],
    );
    // The compose node gets its input from the solid color so needs nothing but passes the
    // local coordinates through.
    assert_eq!(
        compose.required_flags(),
        SnippetRequirementFlags::PASSTHROUGH_LOCAL_COORDS
    );
    assert_eq!(
        preamble(&compose, &info),
        "half4 Compose_0() { return sk_matrix_colorfilter(SolidColor_1(), colorMatrix_2, \
         colorTranslate_2, minMaxRGB_2); }"
    );

    // A blend compose passes the src and dst.
    let src = node(&dict, BuiltInCodeSnippetID::SolidColorShader, 1);
    let dst = node(&dict, BuiltInCodeSnippetID::SolidColorShader, 2);
    let blender = node(&dict, BuiltInCodeSnippetID::FixedBlendSrcOver, 3);
    let blend = node_with_children(
        &dict,
        BuiltInCodeSnippetID::BlendCompose as i32,
        0,
        vec![src, dst, blender],
    );
    assert_eq!(
        preamble(&blend, &info),
        "half4 BlendCompose_0() { return blend_src_over(SolidColor_1(), SolidColor_2()); }"
    );

    // The default preamble of a snippet with children (here a made-up use of the machinery with
    // a runtime effect, below) is empty for a leaf.
    let leaf = node(&dict, BuiltInCodeSnippetID::MatrixColorFilter, 7);
    assert_eq!(leaf.generate_default_preamble(&info), "");
    let mut body = String::new();
    let out = leaf.invoke_and_assign(&info, &ShaderSnippetArgs::default_args(), &mut body);
    assert_eq!(out, "outColor_7");
    assert_eq!(
        body,
        "half4 outColor_7 = sk_matrix_colorfilter(inColor, colorMatrix_7, colorTranslate_7, \
         minMaxRGB_7);"
    );
}

#[test]
fn invoke_with_the_storage_buffer_and_textures() {
    let dict = new_dictionary();
    let info = shader_info(&dict, Some("ssboIndex"));
    let table = node(&dict, BuiltInCodeSnippetID::TableColorFilter, 4);
    let mut body = String::new();
    let _ = table.invoke_and_assign(&info, &ShaderSnippetArgs::default_args(), &mut body);
    // The samplers are never in the storage buffer.
    assert_eq!(
        body,
        "half4 outColor_4 = sk_table_colorfilter(inColor, table_4);"
    );

    let gradient = node(&dict, BuiltInCodeSnippetID::LinearGradientShaderBuffer, 5);
    assert!(
        gradient
            .required_flags()
            .contains(SnippetRequirementFlags::STORAGE_BUFFER)
    );
    let mut body = String::new();
    let _ = gradient.invoke_and_assign(&info, &ShaderSnippetArgs::default_args(), &mut body);
    assert_eq!(
        body,
        "half4 outColor_5 = sk_linear_grad_buf_shader(pos, \
         combinedUniformData[ssboIndex].numStops_5, \
         combinedUniformData[ssboIndex].bufferOffset_5, \
         combinedUniformData[ssboIndex].tilemode_5, \
         combinedUniformData[ssboIndex].colorSpace_5, \
         combinedUniformData[ssboIndex].doUnPremul_5, fsStorageBuffer);"
    );
}

#[test]
fn paint_color_is_not_mangled() {
    let dict = new_dictionary();
    let info = shader_info(&dict, None);
    let paint = node(&dict, BuiltInCodeSnippetID::RGBPaintColor, 9);
    let mut body = String::new();
    let _ = paint.invoke_and_assign(&info, &ShaderSnippetArgs::default_args(), &mut body);
    assert_eq!(body, "half4 outColor_9 = sk_rgb_opaque(paintColor);");
}

#[test]
fn runtime_effect_preamble() {
    let dict = new_dictionary();
    let rte_dict = Arc::new(RuntimeEffectDictionary::new());
    let effect = make_shader(SHADER_SKSL, "Colored");
    let id = dict.find_or_create_runtime_effect_snippet(&effect).unwrap();
    rte_dict.set(id, effect.clone());
    assert!(rte_dict.find(id).is_some());
    assert!(rte_dict.find(id + 1).is_none());
    // Setting the same effect again is fine.
    rte_dict.set(id, effect);

    let info = ShaderInfo::new(&dict, rte_dict, None);
    let node = node_with_children(&dict, id, 6, Vec::new());
    // Without children the node calls its helper directly: the helper is what the generated
    // preamble defines.
    assert_eq!(
        preamble(&node, &info),
        "half4 Colored_6(float2 pos) { return half4(half4(color_6));\n }"
    );
    let mut body = String::new();
    let _ = node.invoke_and_assign(&info, &ShaderSnippetArgs::default_args(), &mut body);
    assert_eq!(body, "half4 outColor_6 = Colored_6(pos);");
}

#[test]
fn runtime_effect_with_children_preamble() {
    let dict = new_dictionary();
    let rte_dict = Arc::new(RuntimeEffectDictionary::new());
    let effect = make_shader(
        "uniform shader a; uniform colorFilter b;\n\
         half4 main(float2 p) { return b.eval(a.eval(p + float2(1))); }",
        "Kids",
    );
    let id = dict.find_or_create_runtime_effect_snippet(&effect).unwrap();
    rte_dict.set(id, effect);

    let info = ShaderInfo::new(&dict, rte_dict, None);
    let shader_child = node(&dict, BuiltInCodeSnippetID::HWImageShader, 1);
    let filter_child = node(&dict, BuiltInCodeSnippetID::MatrixColorFilter, 2);
    let node = node_with_children(&dict, id, 0, vec![shader_child, filter_child]);
    assert_eq!(
        preamble(&node, &info),
        "half4 Kids_0(float2 pos) { return half4(sk_matrix_colorfilter(sk_hw_image_shader(pos + \
         float2(1.0), image_1), colorMatrix_2, colorTranslate_2, minMaxRGB_2));\n }"
    );
}
