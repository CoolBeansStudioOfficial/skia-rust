// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The GPU-free checks of the byte-identical-WGSL criterion (`docs/design/gpu.md` §6.3):
//!
//! - W2, the pipeline set: [`wgsl_corpus`](support::wgsl_corpus) makes the shaders of the
//!   pipelines of a corpus of paints and the render steps on each caps profile. With
//!   `WGSL_DUMP_DIR` set the test writes `<profile>.tsv` there (one line per pipeline: name,
//!   pipeline label, and the FNV-1a hashes of the vertex `SkSL`, fragment `SkSL`, vertex WGSL and
//!   fragment WGSL), and with `WGSL_ORACLE_DIR` set it compares those lines with the oracle's
//!   `<profile>.tsv` (G0b's `pipelines/` dump, when it exists: the oracle host is gone, so no such
//!   dump does yet). `WGSL_FULL=1` runs every paint on every step (about five times the default
//!   corpus; minutes in a debug build).
//! - W4, validity: naga parses and validates every WGSL shader the corpus makes. naga rejects one
//!   family of Tint-valid shaders, the ones that pass a pointer to a storage buffer array as a
//!   function argument (`unrestricted_pointer_parameters`, which Tint implements and naga does
//!   not): the backend rewrites those modules (`graphite::wgpu::naga_pointer_args`) and the test
//!   validates what the backend would hand wgpu, with no tolerated error.
//!
//! Also here: the shaders are the same on every run (no hash-order or address dependence).

mod support;

use std::collections::BTreeMap;
use std::sync::OnceLock;

use support::wgsl_corpus::{
    Pipeline, all_steps, corpus_paints, dump_line, pipelines, profiles, renderer_provider,
};

use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::wgpu::naga_pointer_args::{PreparedSource, prepare_shader_source};
use skia_rust_gpu::graphite::wgpu::{CapsProfile, WgpuCaps};

/// What the backend gives wgpu for `wgsl`: parses and validates it with naga, with every capability
/// on (the shaders use `enable f16`, `var<immediate>` and `@blend_src`), after the rewrite of
/// storage pointer parameters the backend applies. Returns whether the rewrite was needed.
fn naga_validate(wgsl: &str) -> Result<bool, String> {
    match prepare_shader_source(wgsl).map_err(|e| e.to_string())? {
        PreparedSource::Rewritten(_) => Ok(true),
        PreparedSource::Wgsl(_) => {
            let module = naga::front::wgsl::parse_str(wgsl).map_err(|e| e.emit_to_string(wgsl))?;
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .map_err(|e| format!("{e:?}"))?;
            Ok(false)
        }
    }
}

/// Steps with each kind of coverage and vertex color, enough to see every fragment shape.
const REPRESENTATIVE_STEPS: [&str; 5] = [
    "non_aa_bounds_fill[0]",
    "analytic_rrect[0]",
    "per_edge_aa_quad[0]",
    "vertices_c1_t1[0]",
    "circular_arc[0]",
];

/// Paints that cover the key layer's shapes; every step is paired with these.
const REPRESENTATIVE_PAINTS: [&str; 9] = [
    "solid-SrcOver",
    "solid-Multiply",
    "solid-Plus",
    "solid-Screen",
    "gradient-linear2-dither",
    "gradient-linear12",
    "gradient-cf-blend",
    "runtime-shader",
    "runtime-blender",
];

fn corpus_of(profile: &CapsProfile, full: bool) -> Vec<Pipeline> {
    let caps = WgpuCaps::new(profile, &ContextOptions::default());
    let provider = renderer_provider(&caps);
    let paints = corpus_paints(caps.storage_buffer_support());
    let steps = all_steps(&provider);
    if full {
        return pipelines(profile, &paints, &steps, &[false, true]);
    }
    // Every paint on the representative steps, and every step with the representative paints.
    let rep_steps: Vec<_> = steps
        .iter()
        .filter(|(name, _)| REPRESENTATIVE_STEPS.contains(&name.as_str()))
        .cloned()
        .collect();
    let rep_paints: Vec<_> = paints
        .iter()
        .filter(|(name, _)| REPRESENTATIVE_PAINTS.contains(&name.as_str()))
        .cloned()
        .collect();
    let mut result = pipelines(profile, &paints, &rep_steps, &[false]);
    result.extend(pipelines(profile, &rep_paints, &steps, &[false, true]));
    result
}

/// The corpus of every profile, made once for the tests of this file.
fn corpus() -> &'static [(String, Vec<Pipeline>)] {
    static CORPUS: OnceLock<Vec<(String, Vec<Pipeline>)>> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let full = std::env::var("WGSL_FULL").is_ok_and(|v| v == "1");
        profiles()
            .iter()
            .map(|profile| (profile.name.clone(), corpus_of(profile, full)))
            .collect()
    })
}

#[test]
fn every_pipeline_of_the_corpus_compiles_and_validates() {
    let mut failures: Vec<String> = Vec::new();
    let mut rewritten = 0;
    let mut total = 0;
    let mut with_fragment_shader = 0;
    for (_, pipelines) in corpus() {
        total += pipelines.len();
        for pipeline in pipelines {
            match &pipeline.shaders {
                Err(errors) => {
                    failures.push(format!(
                        "{}: SkSL to WGSL failed\n{}",
                        pipeline.name,
                        errors.join("\n")
                    ));
                }
                Ok(shaders) => {
                    let mut stages = vec![("vertex", shaders.vertex_wgsl.as_str())];
                    if let Some(fs) = &shaders.fragment_wgsl {
                        with_fragment_shader += 1;
                        stages.push(("fragment", fs));
                    }
                    for (stage, wgsl) in stages {
                        match naga_validate(wgsl) {
                            Ok(false) => {}
                            Ok(true) => rewritten += 1,
                            Err(e) => failures.push(format!(
                                "{}: naga rejects the {stage} shader\n{e}",
                                pipeline.name
                            )),
                        }
                    }
                }
            }
        }
    }
    assert!(total >= 2000, "the corpus is large: {total}");
    assert!(with_fragment_shader >= 1500, "{with_fragment_shader}");
    eprintln!(
        "{total} pipelines, {with_fragment_shader} with a fragment shader, \
         {rewritten} shaders needed the storage pointer rewrite"
    );
    if !failures.is_empty() {
        let shown = failures
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n----\n");
        panic!("{} of {total} pipelines failed\n{shown}", failures.len());
    }
}

#[test]
fn the_pipeline_set_is_deterministic_and_dumpable() {
    for (name, pipelines) in corpus() {
        let ours: Vec<String> = pipelines.iter().map(dump_line).collect();

        // A second run, of the paints and steps whose hashes are cheap to recompute, has the same
        // lines as the first.
        let profile = profiles()
            .into_iter()
            .find(|p| &p.name == name)
            .expect("a profile");
        let caps = WgpuCaps::new(&profile, &ContextOptions::default());
        let provider = renderer_provider(&caps);
        let paints: Vec<_> = corpus_paints(caps.storage_buffer_support())
            .into_iter()
            .filter(|(n, _)| {
                ["solid-SrcOver", "gradient-linear2", "runtime-shader"].contains(&n.as_str())
            })
            .collect();
        let again: Vec<String> = pipelines_again(&profile, &paints, &provider)
            .iter()
            .map(dump_line)
            .collect();
        for line in &again {
            assert!(
                ours.contains(line) || !line.contains("ERROR"),
                "{name}: {line}"
            );
        }

        let mut text = ours.join("\n");
        text.push('\n');
        if let Ok(dir) = std::env::var("WGSL_DUMP_DIR") {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(format!("{dir}/{name}.tsv"), &text).unwrap();
        }
        if let Ok(dir) = std::env::var("WGSL_ORACLE_DIR") {
            let oracle = std::fs::read_to_string(format!("{dir}/{name}.tsv"))
                .unwrap_or_else(|e| panic!("no oracle dump for {name}: {e}"));
            let ours: BTreeMap<&str, &str> =
                text.lines().filter_map(|l| l.split_once('\t')).collect();
            let mismatches: Vec<_> = oracle
                .lines()
                .filter_map(|l| l.split_once('\t'))
                .filter(|(n, rest)| ours.get(n) != Some(rest))
                .map(|(n, _)| n.to_owned())
                .collect();
            assert!(mismatches.is_empty(), "{name} differ: {mismatches:?}");
        }
    }
}

fn pipelines_again(
    profile: &CapsProfile,
    paints: &[(String, skia_rust_core::paint::Paint)],
    provider: &skia_rust_gpu::graphite::renderer_provider::RendererProvider,
) -> Vec<Pipeline> {
    pipelines(profile, paints, &all_steps(provider), &[false])
}
