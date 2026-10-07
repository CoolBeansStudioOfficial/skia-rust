// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Builder tests of task D2: the stages blenders, color/empty shaders and `MatrixRec::apply`
//! append, against Skia's (`oracle/rp-builder/rp_builder.cpp`'s `run_d2`, run as
//! `rp_builder.exe d2` by `build.ps1`).
//!
//! - `skia_d2_dump.txt`: per case `== <case>`, `ok <0|1>` (the appender's result),
//!   `SkRasterPipeline::dump()`'s text, and `# <index> <op> <values>` lines with the contexts of
//!   `uniform_color`, `unbounded_uniform_color` and the matrix stages (floats printed with
//!   `%.9g`, compared bit for bit).
//! - `skia_d2_rp_dump.txt`: the oracle's compile records (lowp decision on the SSE2 baseline,
//!   op list, contexts), as in `tests.rs`.

use super::RasterPipeline;
use super::Stage;
use super::tests::{Record, Tok, ctx_tokens, parse_records};

use crate::arena_alloc::ArenaAlloc;
use crate::blend_mode::BlendMode;
use crate::blender::Blender;
use crate::color::{Color, Color4f};
use crate::color_space::{ColorSpace, named_gamut, named_transfer_fn};
use crate::color_type::ColorType;
use crate::effect_priv::StageRec;
use crate::matrix::Matrix;
use crate::raster_pipeline::{MemSlot, MemoryCtx};
use crate::rect::Rect;
use crate::shaders::{self, MatrixRec};
use skia_rust_simd::Tier;

const SKIA_D2_DUMP: &str = include_str!("skia_d2_dump.txt");
const SKIA_D2_RP_DUMP: &str = include_str!("skia_d2_rp_dump.txt");

/// A `StageRec` like `run_d2_case`'s: RGBA 8888 destination, opaque black paint color.
fn stage_rec<'r, 'a>(
    p: &'r mut RasterPipeline<'a>,
    alloc: &'a ArenaAlloc,
    dst_cs: Option<&'r ColorSpace>,
) -> StageRec<'r, 'a> {
    StageRec {
        pipeline: p,
        alloc,
        dst_color_type: ColorType::RGBA8888,
        dst_cs,
        paint_color: Color4f::new(0.0, 0.0, 0.0, 1.0),
        surface_props: crate::surface_props::SurfaceProps::default(),
        dst_bounds: Rect::new_empty(),
    }
}

/// Builds every case of `run_d2`, in order, and calls `f(name, ok, pipeline)`.
#[allow(clippy::too_many_lines)] // one block per case group, as in rp_builder.cpp
fn for_each_case(mut f: impl FnMut(&str, bool, &RasterPipeline<'_>)) {
    let alloc = ArenaAlloc::new();
    let dst_mem = MemoryCtx::new(MemSlot(1));

    // Every blend mode through its blender, between a dst load and a store.
    for mode in BlendMode::VALUES {
        let mut p = RasterPipeline::new();
        let ok = {
            let mut rec = stage_rec(&mut p, &alloc, None);
            rec.pipeline.append_load_dst(ColorType::RGBA8888, dst_mem);
            let ok = Blender::mode(mode).as_base().append_stages(&mut rec);
            rec.pipeline.append_store(ColorType::RGBA8888, dst_mem);
            ok
        };
        f(&format!("blender_{}", mode.name()), ok, &p);
    }

    // Color shaders: colors x source color spaces x destination color spaces.
    let colors = [
        ("opaque", Color4f::new(0.25, 0.5, 0.75, 1.0)),
        ("translucent", Color4f::new(0.1, 0.2, 0.3, 0.4)),
        ("white", Color4f::new(1.0, 1.0, 1.0, 1.0)),
        ("black", Color4f::new(0.0, 0.0, 0.0, 1.0)),
        ("transparent", Color4f::new(0.0, 0.0, 0.0, 0.0)),
        ("wide", Color4f::new(1.5, -0.25, 0.5, 0.8)),
        ("alpha_over", Color4f::new(0.5, 0.5, 0.5, 2.0)),
    ];
    let p3 = ColorSpace::new_rgb(&named_transfer_fn::SRGB, &named_gamut::DISPLAY_P3);
    let rec2020 = ColorSpace::new_rgb(&named_transfer_fn::REC2020, &named_gamut::REC2020);
    let srcs = [
        ("null", None),
        ("linear", Some(ColorSpace::new_srgb_linear())),
        ("p3", p3),
    ];
    let dsts = [
        ("null", None),
        ("srgb", Some(ColorSpace::new_srgb())),
        ("linear", Some(ColorSpace::new_srgb_linear())),
        ("rec2020", rec2020),
    ];
    for (color_name, color) in &colors {
        for (src_name, src) in &srcs {
            for (dst_name, dst) in &dsts {
                let shader = shaders::color_in_space(color, src.clone()).unwrap();
                let mut p = RasterPipeline::new();
                let ok = shader.as_base().append_root_stages(
                    &mut stage_rec(&mut p, &alloc, dst.as_ref()),
                    &Matrix::translate((3.0, 4.0)),
                );
                f(
                    &format!("color_shader_{color_name}_{src_name}_{dst_name}"),
                    ok,
                    &p,
                );
            }
        }
    }
    let skcolor = Color::from(0x80FF_8040u32);
    let mut p = RasterPipeline::new();
    let ok = shaders::color(skcolor)
        .as_base()
        .append_root_stages(&mut stage_rec(&mut p, &alloc, None), Matrix::i());
    f("color_shader_skcolor", ok, &p);
    let linear = ColorSpace::new_srgb_linear();
    let mut p = RasterPipeline::new();
    let ok = shaders::color(skcolor)
        .as_base()
        .append_root_stages(&mut stage_rec(&mut p, &alloc, Some(&linear)), Matrix::i());
    f("color_shader_skcolor_linear", ok, &p);
    let mut p = RasterPipeline::new();
    let ok = shaders::empty()
        .as_base()
        .append_root_stages(&mut stage_rec(&mut p, &alloc, None), Matrix::i());
    f("empty_shader", ok, &p);

    // MatrixRec::apply: ctm x pending local matrix x post-inverse.
    let i = Matrix::new_identity;
    let matrices = [
        ("mrec_identity", i(), i(), i(), false),
        (
            "mrec_ctm_translate",
            Matrix::translate((3.0, -4.5)),
            i(),
            i(),
            false,
        ),
        (
            "mrec_scale_lm_translate",
            Matrix::scale((2.0, 4.0)),
            Matrix::translate((1.0, 1.0)),
            i(),
            false,
        ),
        (
            "mrec_affine",
            Matrix::new_all(0.75, -0.5, 10.0, 0.5, 0.75, -20.0, 0.0, 0.0, 1.0),
            Matrix::scale((3.0, 3.0)),
            i(),
            false,
        ),
        (
            "mrec_perspective",
            Matrix::new_all(1.0, 0.25, 3.0, 0.5, 2.0, 4.0, 0.001, -0.002, 1.0),
            i(),
            i(),
            false,
        ),
        (
            "mrec_post_inv",
            Matrix::scale((2.0, 2.0)),
            i(),
            Matrix::translate((5.0, 6.0)),
            false,
        ),
        (
            "mrec_ctm_applied",
            Matrix::scale((2.0, 2.0)),
            Matrix::translate((7.0, 8.0)),
            i(),
            true,
        ),
        ("mrec_singular", Matrix::scale((0.0, 1.0)), i(), i(), false),
    ];
    for (name, ctm, lm, post_inv, ctm_applied) in &matrices {
        let mut m = MatrixRec::new(ctm);
        if *ctm_applied {
            m.mark_ctm_applied();
        }
        let mut p = RasterPipeline::new();
        let ok = m
            .concat(lm)
            .apply(&mut stage_rec(&mut p, &alloc, None), post_inv)
            .is_some();
        f(name, ok, &p);
    }
}

/// The context values `print_contexts` prints for `stage`, if any.
fn d2_ctx_tokens(stage: &Stage<'_>) -> Option<Vec<Tok>> {
    let nums = |v: &[f32]| v.iter().map(|&f| Tok::Num(f)).collect::<Vec<_>>();
    Some(match *stage {
        Stage::UniformColor(u) => {
            let mut t = nums(&[u.r, u.g, u.b, u.a]);
            t.push(Tok::Bar);
            t.extend(u.rgba.iter().map(|&c| Tok::Num(f32::from(c))));
            t
        }
        // (Skia leaves `rgba` uninitialized for this op.)
        Stage::UnboundedUniformColor(u) => nums(&[u.r, u.g, u.b, u.a]),
        Stage::MatrixTranslate(c) => nums(&c),
        Stage::MatrixScaleTranslate(c) => nums(c),
        Stage::Matrix2x3(c) => nums(c),
        Stage::MatrixPerspective(c) => nums(c),
        _ => return None,
    })
}

/// One case of `skia_d2_dump.txt`.
#[derive(Debug, PartialEq)]
struct Case {
    name: String,
    ok: bool,
    /// `dump()`'s text.
    dump: String,
    ctxs: Vec<(usize, String, Vec<Tok>)>,
}

fn parse_tokens(words: core::str::Split<'_, char>) -> Vec<Tok> {
    words
        .map(|w| {
            if w == "|" {
                Tok::Bar
            } else {
                Tok::Num(w.parse().unwrap())
            }
        })
        .collect()
}

fn parse_cases(text: &str) -> Vec<Case> {
    let mut cases: Vec<Case> = Vec::new();
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("== ") {
            cases.push(Case {
                name: name.to_owned(),
                ok: false,
                dump: String::new(),
                ctxs: Vec::new(),
            });
            continue;
        }
        let case = cases.last_mut().unwrap();
        if let Some(ok) = line.strip_prefix("ok ") {
            case.ok = ok == "1";
        } else if let Some(rest) = line.strip_prefix("# ") {
            let mut words = rest.split(' ');
            let index = words.next().unwrap().parse().unwrap();
            let op = words.next().unwrap().to_owned();
            case.ctxs.push((index, op, parse_tokens(words)));
        } else {
            case.dump.push_str(line);
            case.dump.push('\n');
        }
    }
    cases
}

#[test]
fn d2_stages_match_skia() {
    let skia = parse_cases(SKIA_D2_DUMP);
    let mut ours = Vec::new();
    for_each_case(|name, ok, p| {
        let mut dump = String::new();
        for line in p.to_string().lines() {
            dump.push_str(line);
            dump.push('\n');
        }
        ours.push(Case {
            name: name.to_owned(),
            ok,
            dump,
            ctxs: p
                .stages()
                .iter()
                .enumerate()
                .filter_map(|(i, s)| Some((i, s.op().name().to_owned(), d2_ctx_tokens(s)?)))
                .collect(),
        });
    });
    for (s, o) in skia.iter().zip(&ours) {
        assert_eq!(s, o);
    }
    assert_eq!(skia.len(), ours.len());
}

#[test]
fn d2_compiled_records_match_skia() {
    let mut ours = Vec::new();
    for_each_case(|name, _ok, p| {
        // `compile()` of an empty pipeline returns before building (no record).
        if p.empty() {
            return;
        }
        let stages = p.stages();
        ours.push(Record {
            name: name.to_owned(),
            lowp: p.is_lowp(Tier::Sse2),
            ops: stages.iter().map(|s| s.op().name().to_owned()).collect(),
            ctxs: stages
                .iter()
                .enumerate()
                .filter_map(|(i, s)| Some((i, s.op().name().to_owned(), ctx_tokens(s)?)))
                .collect(),
        });
    });
    let skia = parse_records(SKIA_D2_RP_DUMP);
    for (s, o) in skia.iter().zip(&ours) {
        assert_eq!(s, o);
    }
    assert_eq!(skia.len(), ours.len());
}
