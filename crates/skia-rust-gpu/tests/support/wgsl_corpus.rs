// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The headless pipeline set (`docs/design/gpu.md` §6.3, W2): the shaders of every pipeline a set
//! of paints makes with every `RenderStep` the `RendererProvider` has, for a `CapsProfile`.
//!
//! This is what a GM's pipeline set will be once a headless `Recorder` can draw (G10): each draw
//! is a `(RenderPassDesc, RenderStep, PaintParamsKey)` triple, and the shaders follow from the
//! triple and the caps alone. Until then the triples come from a corpus of paints and the steps of
//! the renderer provider.
#![allow(dead_code)] // each test file uses a different part

use std::cell::RefCell;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters;
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::image_info::ColorInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::runtime_effect::RuntimeEffect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_gpu::gpu::shader_error_handler::{ShaderErrorHandler, build_shader_error_message};
use skia_rust_gpu::gpu::swizzle::Swizzle;
use skia_rust_gpu::graphite::buffer_manager::StaticBufferManager;
use skia_rust_gpu::graphite::caps::Caps;
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::draw_types::DstUsage;
use skia_rust_gpu::graphite::key_context::KeyContext;
use skia_rust_gpu::graphite::paint_params::{PaintParams, ShadingParams};
use skia_rust_gpu::graphite::paint_params_key::PaintParamsKeyBuilder;
use skia_rust_gpu::graphite::pipeline_data::PipelineDataGatherer;
use skia_rust_gpu::graphite::render_pass_desc::RenderPassDesc;
use skia_rust_gpu::graphite::render_step::RenderStep;
use skia_rust_gpu::graphite::renderer::Renderer;
use skia_rust_gpu::graphite::renderer_provider::RendererProvider;
use skia_rust_gpu::graphite::resource_types::{DstReadStrategy, Layout};
use skia_rust_gpu::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use skia_rust_gpu::graphite::shader_code_dictionary::ShaderCodeDictionary;
use skia_rust_gpu::graphite::texture_format::TextureFormat;
use skia_rust_gpu::graphite::unique_paint_params_id::UniquePaintParamsID;
use skia_rust_gpu::graphite::wgpu::pipeline_shaders::{PipelineShaders, make_pipeline_shaders};
use skia_rust_gpu::graphite::wgpu::{CapsProfile, WgpuCaps};

use super::{MockCaps, shared_provider};

/// The caps profiles of the criterion: what Dawn reported on D3D12 and Vulkan, and the Vulkan one
/// restricted to what wgpu can express.
pub fn profiles() -> Vec<CapsProfile> {
    vec![
        CapsProfile::dawn_d3d12(),
        CapsProfile::dawn_vulkan(),
        CapsProfile::dawn_vulkan().wgpu_restricted(),
    ]
}

/// A shader error handler that records the failure, so a test can report which pipeline failed.
#[derive(Default)]
pub struct Recorded(pub Mutex<Vec<String>>);

impl ShaderErrorHandler for Recorded {
    fn compile_error(&self, shader: &str, errors: &str, _was_cached: bool) {
        self.0
            .lock()
            .unwrap()
            .push(build_shader_error_message(shader, errors));
    }
}

/// The renderer provider over mock buffers, as the tests of the render steps make it.
pub fn renderer_provider(caps: &WgpuCaps) -> RendererProvider {
    let (resource_provider, _) = shared_provider();
    let mut manager = StaticBufferManager::new(resource_provider, &MockCaps::default());
    RendererProvider::new(
        caps.resource_binding_requirements().uniform_buffer_layout,
        caps.shader_caps().infinity_support,
        &mut manager,
    )
}

/// Every `RenderStep` of the provider, with the name of its renderer's slot.
pub fn all_steps(provider: &RendererProvider) -> Vec<(String, Arc<dyn RenderStep>)> {
    use skia_rust_core::path_types::PathFillType;
    let mut renderers: Vec<(String, &Renderer)> = vec![
        ("analytic_rrect".into(), provider.analytic_rrect()),
        ("per_edge_aa_quad".into(), provider.per_edge_aa_quad()),
        ("non_aa_bounds_fill".into(), provider.non_aa_bounds_fill()),
        ("circular_arc".into(), provider.circular_arc()),
        (
            "convex_tessellated_wedges".into(),
            provider.convex_tessellated_wedges(),
        ),
        (
            "tessellated_strokes".into(),
            provider.tessellated_strokes(false),
        ),
        (
            "tessellated_strokes_inverse".into(),
            provider.tessellated_strokes(true),
        ),
    ];
    for (has_color, has_tex_coords) in [(false, false), (false, true), (true, false), (true, true)]
    {
        renderers.push((
            format!(
                "vertices_c{}_t{}",
                u8::from(has_color),
                u8::from(has_tex_coords)
            ),
            provider.vertices(has_color, has_tex_coords),
        ));
    }
    for fill_type in [
        PathFillType::EvenOdd,
        PathFillType::Winding,
        PathFillType::InverseEvenOdd,
        PathFillType::InverseWinding,
    ] {
        renderers.push((
            format!("stencil_curves_{fill_type:?}"),
            provider.stencil_tessellated_curves_and_tris(fill_type),
        ));
        renderers.push((
            format!("stencil_wedges_{fill_type:?}"),
            provider.stencil_tessellated_wedges(fill_type),
        ));
    }
    let mut steps = Vec::new();
    for (name, renderer) in renderers {
        for (i, step) in renderer.steps().iter().enumerate() {
            steps.push((format!("{name}[{i}]"), step.clone()));
        }
    }
    steps
}

fn srgb_info() -> ColorInfo {
    ColorInfo::new(
        ColorType::RGBA8888,
        AlphaType::Premul,
        Some(ColorSpace::new_srgb()),
    )
}

fn gradient(colors: &[Color4f], positions: Option<&[f32]>) -> Gradient<'static> {
    // The slices live as long as the test: leak them to get `'static`.
    let colors: &'static [Color4f] = Box::leak(colors.to_vec().into_boxed_slice());
    let positions: Option<&'static [f32]> =
        positions.map(|p| &*Box::leak(p.to_vec().into_boxed_slice()));
    Gradient::new(
        Colors::new(colors, positions, TileMode::Clamp, None::<ColorSpace>),
        Interpolation::default(),
    )
}

fn red_blue() -> Vec<Color4f> {
    vec![
        Color4f::new(1.0, 0.0, 0.0, 1.0),
        Color4f::new(0.0, 0.0, 1.0, 1.0),
    ]
}

fn many_colors(n: usize) -> Vec<Color4f> {
    #[allow(clippy::cast_precision_loss)]
    (0..n)
        .map(|i| Color4f::new(i as f32 / n as f32, 0.5, 1.0 - i as f32 / n as f32, 1.0))
        .collect()
}

/// The paints of the corpus: solid colors in every blend mode, the gradients (2, 5 and 12 stops),
/// dither, color filters, and runtime shaders, color filters and blenders.
///
/// Missing, with the feature each needs: image shaders and YUV (`Image_Graphite`, G10), picture
/// shaders (`Image_Graphite`), perlin noise, mesh and vertices colors with a blender
/// (`Device`, G10), clip shaders and analytic clips (`ClipStack`, G10).
#[allow(clippy::too_many_lines)] // a list of paints
pub fn corpus_paints(storage_buffers: bool) -> Vec<(String, Paint)> {
    let mut paints: Vec<(String, Paint)> = Vec::new();

    for mode in BlendMode::VALUES {
        let mut paint = Paint::new(Color4f::new(0.25, 0.5, 0.75, 0.5), None);
        paint.set_blend_mode(mode);
        paints.push((format!("solid-{mode:?}"), paint));
    }
    let mut opaque = Paint::new(Color4f::new(0.25, 0.5, 0.75, 1.0), None);
    opaque.set_blend_mode(BlendMode::Src);
    paints.push(("solid-opaque-Src".into(), opaque));

    let pts = [Point::new(0.0, 0.0), Point::new(100.0, 0.0)];
    let gradients: Vec<(&str, Option<skia_rust_core::shader::Shader>)> = vec![
        (
            "linear2",
            shaders::linear_gradient((pts[0], pts[1]), &gradient(&red_blue(), None), None),
        ),
        (
            "linear5",
            shaders::linear_gradient((pts[0], pts[1]), &gradient(&many_colors(5), None), None),
        ),
        (
            "linear12",
            shaders::linear_gradient((pts[0], pts[1]), &gradient(&many_colors(12), None), None),
        ),
        (
            "linear3-positions",
            shaders::linear_gradient(
                (pts[0], pts[1]),
                &gradient(&many_colors(3), Some(&[0.0, 0.25, 1.0])),
                None,
            ),
        ),
        (
            "radial2",
            shaders::radial_gradient(
                (Point::new(50.0, 50.0), 40.0),
                &gradient(&red_blue(), None),
                None,
            ),
        ),
        (
            "radial5",
            shaders::radial_gradient(
                (Point::new(50.0, 50.0), 40.0),
                &gradient(&many_colors(5), None),
                None,
            ),
        ),
        (
            "sweep2",
            shaders::sweep_gradient(
                Point::new(50.0, 50.0),
                (0.0, 360.0),
                &gradient(&red_blue(), None),
                None,
            ),
        ),
        (
            "conical2",
            shaders::two_point_conical_gradient(
                (Point::new(20.0, 50.0), 10.0),
                (Point::new(80.0, 50.0), 30.0),
                &gradient(&red_blue(), None),
                None,
            ),
        ),
    ];
    for (name, shader) in gradients {
        let Some(shader) = shader else {
            continue;
        };
        // More than eight stops are read from a storage buffer, or from a bitmap a recorder
        // makes (which this headless corpus does not have).
        if name == "linear12" && !storage_buffers {
            continue;
        }
        let mut paint = Paint::new(Color4f::new(1.0, 1.0, 1.0, 1.0), None);
        paint.set_shader(shader.clone());
        paints.push((format!("gradient-{name}"), paint.clone()));

        paint.set_dither(true);
        paints.push((format!("gradient-{name}-dither"), paint.clone()));
        paint.set_dither(false);

        paint.set_blend_mode(BlendMode::Multiply);
        paints.push((format!("gradient-{name}-Multiply"), paint));
    }

    let linear = shaders::linear_gradient((pts[0], pts[1]), &gradient(&red_blue(), None), None)
        .expect("a gradient");
    let blend_filter =
        color_filters::blend(Color4f::new(0.25, 0.5, 0.75, 0.5), None, BlendMode::SrcOver);
    let matrix_filter = color_filters::matrix(&ColorMatrix::default(), color_filters::Clamp::Yes);
    let filters: Vec<(&str, Option<ColorFilter>)> = vec![
        ("blend", blend_filter.clone()),
        ("matrix", matrix_filter),
        (
            "srgb-to-linear",
            Some(color_filters::srgb_to_linear_gamma()),
        ),
        (
            "compose",
            color_filters::compose(
                color_filters::matrix(&ColorMatrix::default(), color_filters::Clamp::No).as_ref(),
                blend_filter,
            ),
        ),
    ];
    for (name, filter) in filters {
        let mut paint = Paint::new(Color4f::new(0.2, 0.4, 0.6, 1.0), None);
        paint.set_color_filter(filter);
        paints.push((format!("solid-cf-{name}"), paint.clone()));
        paint.set_shader(linear.clone());
        paints.push((format!("gradient-cf-{name}"), paint));
    }

    // Runtime effects.
    let uniforms = Data::new_copy(
        &[0.5_f32, 0.25, 1.0, 0.75]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>(),
    );
    let shader_effect = RuntimeEffect::make_for_shader(
        "uniform half4 c; half4 main(float2 p) { return c * half(sin(p.x * 0.01)); }",
        None,
    )
    .expect("compiles");
    if let Some(shader) = shader_effect.make_shader(uniforms.clone(), &[], None) {
        let mut paint = Paint::new(Color4f::new(1.0, 1.0, 1.0, 1.0), None);
        paint.set_shader(shader);
        paints.push(("runtime-shader".into(), paint));
    }
    let filter_effect = RuntimeEffect::make_for_color_filter(
        "uniform half4 c; half4 main(half4 color) { return color * c; }",
        None,
    )
    .expect("compiles");
    if let Some(filter) = filter_effect.make_color_filter(uniforms.clone(), &[]) {
        let mut paint = Paint::new(Color4f::new(1.0, 1.0, 1.0, 1.0), None);
        paint.set_shader(linear);
        paint.set_color_filter(filter);
        paints.push(("runtime-color-filter".into(), paint));
    }
    let blender_effect = RuntimeEffect::make_for_blender(
        "uniform half4 c; half4 main(half4 src, half4 dst) { return src * c + dst * (1 - src.a); }",
        None,
    )
    .expect("compiles");
    if let Some(blender) = blender_effect.make_blender(uniforms, &[]) {
        let mut paint = Paint::new(Color4f::new(0.3, 0.6, 0.9, 1.0), None);
        paint.set_blender(blender);
        paints.push(("runtime-blender".into(), paint));
    }

    paints
}

/// The render pass of a target: an RGBA8 target, or an A8 target with its write swizzle.
pub fn render_pass_desc(caps: &WgpuCaps, alpha8: bool) -> RenderPassDesc {
    let mut desc = RenderPassDesc::default();
    if alpha8 {
        desc.color_attachment.format = TextureFormat::R8;
        desc.write_swizzle = skia_rust_gpu::graphite::texture_format::write_swizzle_for_color_type(
            ColorType::Alpha8,
            TextureFormat::R8,
        )
        .expect("a swizzle for A8 in R8");
    } else {
        desc.color_attachment.format = TextureFormat::RGBA8;
        desc.write_swizzle = Swizzle::rgba();
    }
    let _ = caps;
    desc
}

/// One pipeline of the set.
pub struct Pipeline {
    /// `<profile>/<target>/<paint>/<step>`.
    pub name: String,
    /// The shaders, or the compile errors.
    pub shaders: Result<PipelineShaders, Vec<String>>,
}

/// The pipelines `paints` make with `steps` on `profile`.
pub fn pipelines(
    profile: &CapsProfile,
    paints: &[(String, Paint)],
    steps: &[(String, Arc<dyn RenderStep>)],
    alpha8_targets: &[bool],
) -> Vec<Pipeline> {
    let caps = Arc::new(WgpuCaps::new(profile, &ContextOptions::default()));
    let mut result = Vec::new();
    for &alpha8 in alpha8_targets {
        let target_name = if alpha8 { "A8" } else { "RGBA8" };
        let mut rp_desc = render_pass_desc(&caps, alpha8);
        for (paint_name, paint) in paints {
            for (step_name, step) in steps {
                // Depth-only steps make no fragment shader.
                let dict = ShaderCodeDictionary::new(Layout::Std140, &[]);
                let builder = RefCell::new(PaintParamsKeyBuilder::new(&dict));
                let gatherer = RefCell::new(PipelineDataGatherer::new(Layout::Std140));
                let rte_dict = Arc::new(RuntimeEffectDictionary::new());
                let info = srgb_info();
                let key_caps: Arc<dyn Caps> = caps.clone();
                let context = KeyContext::new(
                    key_caps,
                    &builder,
                    &gatherer,
                    &dict,
                    rte_dict.clone(),
                    &info,
                );
                let params = PaintParams::new(paint, None, false, false);
                let shading = ShadingParams::new(
                    &*caps,
                    &params,
                    None,
                    None,
                    step.coverage(),
                    rp_desc.color_attachment.format,
                );
                let (paint_id, dst_usage): (UniquePaintParamsID, DstUsage) = shading
                    .to_key(&context)
                    .unwrap_or((UniquePaintParamsID::invalid(), DstUsage::NONE));
                rp_desc.dst_read_strategy = if dst_usage.contains(DstUsage::DST_READ_REQUIRED) {
                    caps.get_dst_read_strategy()
                } else {
                    DstReadStrategy::NoneRequired
                };

                let handler = Recorded::default();
                let shaders = make_pipeline_shaders(
                    &caps,
                    &dict,
                    Some(rte_dict),
                    &rp_desc,
                    step.as_ref(),
                    paint_id,
                    &handler,
                )
                .ok_or_else(|| std::mem::take(&mut *handler.0.lock().unwrap()));
                result.push(Pipeline {
                    name: format!("{}/{target_name}/{paint_name}/{step_name}", profile.name),
                    shaders,
                });
            }
        }
    }
    result
}

/// FNV-1a, 64 bits: the hash of the pipeline dumps.
pub fn fnv1a(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The line of the pipeline dump for `pipeline`: the label and the hashes of the four shaders, as
/// the `pipelines/` dumps of the oracle list them (`docs/design/gpu.md` §6.3).
pub fn dump_line(pipeline: &Pipeline) -> String {
    let mut line = String::new();
    match &pipeline.shaders {
        Ok(s) => {
            let _ = write!(
                line,
                "{}\t{}\t{:016x}\t{:016x}\t{:016x}\t{:016x}",
                pipeline.name,
                s.shader_info.pipeline_label(),
                fnv1a(s.shader_info.vertex_sksl()),
                fnv1a(s.shader_info.fragment_sksl()),
                fnv1a(&s.vertex_wgsl),
                fnv1a(s.fragment_wgsl.as_deref().unwrap_or("")),
            );
        }
        Err(_) => {
            let _ = write!(line, "{}\tERROR", pipeline.name);
        }
    }
    line
}
