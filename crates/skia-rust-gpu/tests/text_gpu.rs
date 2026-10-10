// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! G12b: GPU text. The choice of sub runs (direct masks, distance field text, paths) by the size
//! and matrix of the text, the placement of glyphs in the text atlas, the strike and text blob
//! caches, slugs, and the passes that text draws record on wgpu's noop adapter. The pixels are
//! checked in `text_pixels.rs`.

#![cfg(not(target_arch = "wasm32"))]
// Glyph counts and font sizes in these tests are tiny.
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]

use std::collections::HashSet;
use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::device::{Device as CoreDevice, draw_glyph_run_list};
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::glyph_run::{GlyphRunBuilder, GlyphRunList};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::packed_glyph_id::PackedGlyphId;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::scaler_context::ScalerContextBuildFlags;
use skia_rust_core::strike_spec::BulkGlyphMetricsAndImages;
use skia_rust_core::surface_props::{PixelGeometry, SurfaceProps, SurfacePropsFlags};
use skia_rust_core::text_blob::TextBlobBuilder;
use skia_rust_gpu::gpu::backing_fit::BackingFit;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, Mipmapped};
use skia_rust_gpu::gpu::mask_format::MaskFormat;
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::device::Device;
use skia_rust_gpu::graphite::draw_atlas::ErrorCode;
use skia_rust_gpu::graphite::draw_pass::DrawPassCommand;
use skia_rust_gpu::graphite::graphics_pipeline_desc::GraphicsPipelineDesc;
use skia_rust_gpu::graphite::recorder::{Recorder, RecorderSharedContext};
use skia_rust_gpu::graphite::render_step::RenderStepID;
use skia_rust_gpu::graphite::renderer::Renderer;
use skia_rust_gpu::graphite::renderer_provider::RendererProvider;
use skia_rust_gpu::graphite::resource_types::LoadOp;
use skia_rust_gpu::graphite::task::Task;
use skia_rust_gpu::graphite::text::text_strike::GlyphEntry;
use skia_rust_gpu::graphite::wgpu::{
    WgpuContext, make_context, noop_backend_context_with_features,
};
use skia_rust_gpu::text_gpu::packed_gpu_glyph_id::PackedGpuGlyphId;
use skia_rust_gpu::text_gpu::strike_cache::StrikeCache;
use skia_rust_gpu::text_gpu::sub_run_container::{
    AtlasSubRunKind, StrikeDeviceInfo, SubRun, SubRunContainer, SubRunCreationBehavior,
};
use skia_rust_gpu::text_gpu::sub_run_control::SubRunControl;
use skia_rust_tools::font_tool_utils::{add_to_text_blob, default_typeface};

const SIZE: i32 = 512;

/// The control of the default Graphite caps: distance field text from 18 px (but not for small
/// text, which uses direct masks until 162 px), paths from 324 px.
fn control() -> SubRunControl {
    SubRunControl::new(true, false, true, 18.0, 324.0, true, false)
}

fn strike_device_info(control: SubRunControl) -> StrikeDeviceInfo {
    StrikeDeviceInfo {
        surface_props: SurfaceProps::default(),
        scaler_context_flags: ScalerContextBuildFlags::FAKE_GAMMA_AND_BOOST_CONTRAST,
        sub_run_control: control,
    }
}

fn font(size: f32) -> Font {
    let mut font = Font::from_size(default_typeface(), size);
    font.set_edging(Edging::AntiAlias);
    font
}

fn paint() -> Paint {
    let mut paint = Paint::new(Color4f::new(1.0, 0.0, 0.0, 1.0), None);
    paint.set_anti_alias(true);
    paint
}

const TEXT: &str = "Hamburgefons";

/// The sub runs of `TEXT` drawn with `font` and `paint` through `matrix`.
fn sub_runs(
    font: &Font,
    paint: &Paint,
    matrix: &Matrix,
    control: SubRunControl,
) -> SubRunContainer {
    let mut builder = GlyphRunBuilder::new();
    let origin = Point::new(0.0, 0.0);
    let list =
        builder.text_to_glyph_run_list(font, paint, TEXT.as_bytes(), origin, TextEncoding::UTF8);
    SubRunContainer::make(
        &list,
        matrix,
        paint,
        &strike_device_info(control),
        SubRunCreationBehavior::AddSubRuns,
    )
}

#[derive(Debug, PartialEq, Eq)]
enum Kind {
    Direct,
    Transformed,
    Sdft,
    Path,
    Drawable,
}

fn kinds(container: &SubRunContainer) -> Vec<Kind> {
    container
        .sub_runs()
        .iter()
        .map(|sub_run| match sub_run {
            SubRun::Atlas(atlas) => match atlas.kind() {
                AtlasSubRunKind::DirectMask => Kind::Direct,
                AtlasSubRunKind::TransformedMask { .. } => Kind::Transformed,
                AtlasSubRunKind::Sdft { .. } => Kind::Sdft,
            },
            SubRun::Path(_) => Kind::Path,
            SubRun::Drawable(_) => Kind::Drawable,
        })
        .collect()
}

// Small text is made of direct masks.
// Port of: src/text/gpu/SubRunContainer.cpp#L1357-L1659 (chrome/m156), the direct mask case
#[test]
fn small_text_is_made_of_direct_masks() {
    let container = sub_runs(&font(12.0), &paint(), Matrix::i(), control());
    assert_eq!(kinds(&container), [Kind::Direct]);
    let SubRun::Atlas(atlas) = &container.sub_runs()[0] else {
        unreachable!();
    };
    assert_eq!(atlas.glyph_count(), TEXT.len());
    assert_eq!(atlas.mask_format(), MaskFormat::A8);
}

// Text rotated or scaled to a medium size still uses direct masks: a mask is made for the
// transformed glyph.
#[test]
fn rotated_and_scaled_text_is_made_of_direct_masks() {
    let mut matrix = Matrix::rotate_deg(30.0);
    matrix.post_scale((2.0, 2.0), None);
    let container = sub_runs(&font(12.0), &paint(), &matrix, control());
    assert_eq!(kinds(&container), [Kind::Direct]);
}

// Text from the largest direct size up to the path threshold is distance field text.
// Port of: src/text/gpu/SubRunControl.cpp#L75-L86 (chrome/m156), isSDFT
#[test]
fn large_text_is_distance_field_text() {
    let container = sub_runs(&font(200.0), &paint(), Matrix::i(), control());
    assert_eq!(kinds(&container), [Kind::Sdft]);
    // The same size from a smaller font under a scale.
    let container = sub_runs(
        &font(20.0),
        &paint(),
        &Matrix::scale((10.0, 10.0)),
        control(),
    );
    assert_eq!(kinds(&container), [Kind::Sdft]);
}

// Text beyond the largest distance field size is made of paths.
// Port of: src/text/gpu/SubRunContainer.cpp#L1357-L1659 (chrome/m156), the path case
#[test]
fn huge_text_is_made_of_paths() {
    let container = sub_runs(&font(400.0), &paint(), Matrix::i(), control());
    assert_eq!(kinds(&container), [Kind::Path]);
}

// Hairline strokes are drawn as paths, and a wide stroke can still be distance field text.
#[test]
fn stroked_text_is_made_of_paths_or_distance_fields() {
    let mut stroke = paint();
    stroke.set_style(Style::Stroke);
    stroke.set_stroke_width(0.0);
    assert_eq!(
        kinds(&sub_runs(&font(200.0), &stroke, Matrix::i(), control())),
        [Kind::Path]
    );
    stroke.set_stroke_width(3.0);
    assert_eq!(
        kinds(&sub_runs(&font(200.0), &stroke, Matrix::i(), control())),
        [Kind::Sdft]
    );
}

// Without distance field support, text whose glyphs fit the atlas is made of direct masks, and
// larger text of paths.
#[test]
fn without_distance_fields_large_text_is_made_of_paths() {
    let no_sdft = SubRunControl::new(false, false, false, 18.0, 324.0, true, false);
    assert_eq!(
        kinds(&sub_runs(&font(200.0), &paint(), Matrix::i(), no_sdft)),
        [Kind::Direct]
    );
    assert_eq!(
        kinds(&sub_runs(&font(400.0), &paint(), Matrix::i(), no_sdft)),
        [Kind::Path]
    );
    assert_eq!(
        kinds(&sub_runs(&font(12.0), &paint(), Matrix::i(), no_sdft)),
        [Kind::Direct]
    );
}

// Text in perspective cannot use direct masks, so it is distance field text (or paths).
#[test]
fn perspective_text_is_not_made_of_direct_masks() {
    let mut matrix = Matrix::i().clone();
    matrix.set_persp_x(0.001);
    let container = sub_runs(&font(12.0), &paint(), &matrix, control());
    assert_eq!(kinds(&container), [Kind::Sdft]);
    let no_sdft = SubRunControl::new(false, false, false, 18.0, 324.0, true, false);
    let container = sub_runs(&font(12.0), &paint(), &matrix, no_sdft);
    assert_eq!(kinds(&container), [Kind::Path]);
}

// With the glyphs of small text in the atlas, the choice of when a made sub run can be reused:
// direct masks only under integer translations, distance fields within their scale range.
// Port of: src/text/gpu/SubRunContainer.cpp#L505-L508, #L609-L612 (chrome/m156), canReuse
#[test]
fn sub_runs_can_be_reused_within_their_limits() {
    let paint = paint();
    let direct = sub_runs(&font(12.0), &paint, Matrix::i(), control());
    assert!(direct.can_reuse(&paint, &Matrix::translate((5.0, -3.0))));
    assert!(!direct.can_reuse(&paint, &Matrix::translate((0.5, 0.0))));
    assert!(!direct.can_reuse(&paint, &Matrix::scale((2.0, 2.0))));

    let sdft = sub_runs(&font(200.0), &paint, Matrix::i(), control());
    // Made for 200 px text from the 162 px distance field: reusable from 72/200 to 324/200.
    assert!(sdft.can_reuse(&paint, Matrix::i()));
    assert!(sdft.can_reuse(&paint, &Matrix::scale((1.5, 1.5))));
    assert!(!sdft.can_reuse(&paint, &Matrix::scale((0.1, 0.1))));

    let paths = sub_runs(&font(400.0), &paint, Matrix::i(), control());
    assert!(paths.can_reuse(&paint, &Matrix::scale((7.0, 7.0))));
}

// Non-finite positions are dropped, and a run with no glyphs makes no sub runs.
#[test]
fn glyphs_with_no_masks_make_no_sub_runs() {
    let container = {
        let mut builder = GlyphRunBuilder::new();
        let list = builder.text_to_glyph_run_list(
            &font(12.0),
            &paint(),
            b"   ",
            Point::new(0.0, 0.0),
            TextEncoding::UTF8,
        );
        SubRunContainer::make(
            &list,
            Matrix::i(),
            &paint(),
            &strike_device_info(control()),
            SubRunCreationBehavior::AddSubRuns,
        )
    };
    assert!(container.is_empty());
}

// With `StrikeCalculationsOnly` the container is empty but the strikes are made.
#[test]
fn strike_calculations_only_makes_no_sub_runs() {
    let mut builder = GlyphRunBuilder::new();
    let list = builder.text_to_glyph_run_list(
        &font(12.0),
        &paint(),
        TEXT.as_bytes(),
        Point::new(0.0, 0.0),
        TextEncoding::UTF8,
    );
    let container = SubRunContainer::make(
        &list,
        Matrix::i(),
        &paint(),
        &strike_device_info(control()),
        SubRunCreationBehavior::StrikeCalculationsOnly,
    );
    assert!(container.is_empty());
}

// ---- the recorder ----------------------------------------------------------------------------

/// A noop device that can create the pipelines of the draws.
fn pipeline_device() -> skia_rust_gpu::graphite::wgpu::WgpuBackendContext {
    noop_backend_context_with_features(
        wgpu::Features::IMMEDIATES | wgpu::Features::DUAL_SOURCE_BLENDING,
        wgpu::Limits {
            max_immediate_size: 64,
            ..wgpu::Limits::default()
        },
    )
    .expect("the noop backend creates a device")
}

fn context() -> WgpuContext {
    make_context(&pipeline_device(), &ContextOptions::default()).expect("a context")
}

fn make_device_with(recorder: &Recorder, props: &SurfaceProps) -> Device {
    Device::make_with_info(
        Some(recorder),
        &ImageInfo::new((SIZE, SIZE), ColorType::RGBA8888, AlphaType::Premul, None),
        Budgeted::Yes,
        Mipmapped::No,
        BackingFit::Exact,
        props,
        LoadOp::Clear,
        "TextTest",
        true,
        false,
    )
    .expect("a device on the noop context")
}

fn make_device(recorder: &Recorder) -> Device {
    make_device_with(recorder, &SurfaceProps::default())
}

struct Pass {
    commands: Vec<DrawPassCommand>,
    steps: Vec<RenderStepID>,
}

fn snap(device: &mut Device) -> Vec<Pass> {
    let task = device
        .testing_only_snap_draw_task()
        .expect("the device recorded something");
    let guard = task.lock();
    let Task::Draw(draw_task) = &*guard else {
        panic!("the device snaps a DrawTask");
    };
    let mut passes = Vec::new();
    draw_task.child_tasks().visit(|child, _| {
        let child = child.lock();
        if let Task::RenderPass(render_pass) = &*child {
            for pass in render_pass.draw_passes() {
                passes.push(Pass {
                    commands: pass.commands().to_vec(),
                    steps: pass
                        .pipeline_descs()
                        .iter()
                        .map(GraphicsPipelineDesc::render_step_id)
                        .collect(),
                });
            }
        }
    });
    passes
}

fn bound_steps(passes: &[Pass]) -> HashSet<RenderStepID> {
    passes
        .iter()
        .flat_map(|pass| pass.steps.iter().copied())
        .collect()
}

fn render_step_ids(renderer: &Renderer) -> HashSet<RenderStepID> {
    renderer
        .steps()
        .iter()
        .map(|step| step.render_step_id())
        .collect()
}

/// The instances the passes draw.
fn instances_drawn(passes: &[Pass]) -> u32 {
    passes
        .iter()
        .flat_map(|pass| pass.commands.iter())
        .map(|command| match command {
            DrawPassCommand::DrawInstanced { instance_count, .. }
            | DrawPassCommand::DrawIndexedInstanced { instance_count, .. } => *instance_count,
            _ => 0,
        })
        .sum()
}

fn provider(context: &WgpuContext) -> &RendererProvider {
    RecorderSharedContext::renderer_provider(&**context.shared_context())
}

fn list_of<'a>(
    builder: &'a mut GlyphRunBuilder,
    font: &Font,
    paint: &Paint,
    text: &str,
    origin: Point,
) -> GlyphRunList<'a> {
    builder.text_to_glyph_run_list(font, paint, text.as_bytes(), origin, TextEncoding::UTF8)
}

fn draw_text(device: &mut Device, font: &Font, paint: &Paint, text: &str, origin: Point) {
    let mut builder = GlyphRunBuilder::new();
    let list = list_of(&mut builder, font, paint, text, origin);
    draw_glyph_run_list(device, &list, paint);
}

// Small text is drawn with the A8 bitmap text renderer: one instance per glyph.
// Port of: src/gpu/graphite/Device.cpp#L2155-L2164 (chrome/m156), chooseRenderer for sub runs
#[test]
fn small_text_is_drawn_with_the_bitmap_text_mask_renderer() {
    let context = context();
    let recorder = context.make_recorder(None);
    let mut device = make_device(&recorder);
    draw_text(
        &mut device,
        &font(12.0),
        &paint(),
        TEXT,
        Point::new(10.0, 40.0),
    );
    let passes = snap(&mut device);
    assert_eq!(
        bound_steps(&passes),
        render_step_ids(provider(&context).bitmap_text(false, MaskFormat::A8))
    );
    assert_eq!(instances_drawn(&passes), TEXT.len() as u32);
}

// Large text is drawn with the distance field text renderer.
#[test]
fn large_text_is_drawn_with_the_sdf_text_renderer() {
    let context = context();
    let recorder = context.make_recorder(None);
    let mut device = make_device(&recorder);
    draw_text(
        &mut device,
        &font(200.0),
        &paint(),
        "Hg",
        Point::new(10.0, 200.0),
    );
    let passes = snap(&mut device);
    assert_eq!(
        bound_steps(&passes),
        render_step_ids(provider(&context).sdf_text(false))
    );
    assert_eq!(instances_drawn(&passes), 2);
}

// LCD text on a surface with a pixel geometry is drawn with the LCD bitmap text renderer; without
// a pixel geometry it falls back to A8.
#[test]
fn lcd_text_needs_a_pixel_geometry() {
    let context = context();
    let mut lcd_font = font(12.0);
    lcd_font.set_edging(Edging::SubpixelAntiAlias);

    let recorder = context.make_recorder(None);
    let props = SurfaceProps::new(SurfacePropsFlags::DEFAULT, PixelGeometry::RGBH);
    let mut device = make_device_with(&recorder, &props);
    draw_text(
        &mut device,
        &lcd_font,
        &paint(),
        TEXT,
        Point::new(10.0, 40.0),
    );
    let passes = snap(&mut device);
    assert_eq!(
        bound_steps(&passes),
        render_step_ids(provider(&context).bitmap_text(true, MaskFormat::A565))
    );
    drop(device);

    let recorder = context.make_recorder(None);
    let mut device = make_device(&recorder);
    draw_text(
        &mut device,
        &lcd_font,
        &paint(),
        TEXT,
        Point::new(10.0, 40.0),
    );
    let passes = snap(&mut device);
    assert_eq!(
        bound_steps(&passes),
        render_step_ids(provider(&context).bitmap_text(false, MaskFormat::A8))
    );
}

// Text beyond the largest distance field size is drawn as paths: none of the text renderers.
#[test]
fn huge_text_is_drawn_as_paths() {
    let context = context();
    let recorder = context.make_recorder(None);
    let mut device = make_device(&recorder);
    draw_text(
        &mut device,
        &font(400.0),
        &paint(),
        "H",
        Point::new(10.0, 400.0),
    );
    let passes = snap(&mut device);
    let steps = bound_steps(&passes);
    assert!(!steps.is_empty());
    for text_step in [
        RenderStepID::BitmapText_Mask,
        RenderStepID::BitmapText_LCD,
        RenderStepID::BitmapText_Color,
        RenderStepID::SDFText,
        RenderStepID::SDFTextLCD,
    ] {
        assert!(!steps.contains(&text_step), "{text_step:?}");
    }
}

// The glyphs of the draw are in the atlas, each in its own place, and drawing them again does not
// add them again.
// Port of: src/gpu/graphite/text/GlyphData.cpp#L55-L132 (chrome/m156), regenerateAtlas
#[test]
fn the_glyphs_are_placed_in_the_atlas_without_overlap() {
    let context = context();
    let recorder = context.make_recorder(None);
    let mut device = make_device(&recorder);
    let font = font(24.0);
    let paint = paint();

    let mut blob_builder = TextBlobBuilder::new();
    add_to_text_blob(&mut blob_builder, "Hamburgefons", &font, 10.0, 40.0);
    let blob = blob_builder.make().expect("a blob");
    let mut builder = GlyphRunBuilder::new();
    let list = builder.blob_to_glyph_run_list(&blob, Point::new(0.0, 0.0));
    draw_glyph_run_list(&mut device, &list, &paint);

    let rp = recorder.priv_();
    let cache = rp.text_blob_cache();
    assert_eq!(cache.blob_count(), 1);
    let info = strike_device_info(control());
    let text_blob = cache.find_or_create_blob(Matrix::i(), &list, &paint, &info);
    let SubRun::Atlas(sub_run) = &text_blob.sub_runs().sub_runs()[0] else {
        panic!("a direct mask sub run");
    };
    let backend = sub_run.glyph_vector().backend();
    let glyph_data = backend.as_ref().expect("the glyphs have backend data");
    let renderer_data = glyph_data.renderer_data();
    assert_eq!(renderer_data.mask_format, MaskFormat::A8);
    assert!(!renderer_data.is_sdf);

    // Every glyph has a rectangle in the atlas the size of its mask, and the rectangles of
    // distinct glyphs do not overlap.
    let mut rects = Vec::new();
    let mut seen_ids = HashSet::new();
    for glyph in glyph_data.glyphs() {
        let locator = glyph.entry().atlas_locator();
        let [l, t, r, b] = locator.uvs().map(|uv| i32::from(uv & 0x1fff));
        assert!(r > l && b > t, "{:?}", locator.uvs());
        if seen_ids.insert(glyph.packed_id()) {
            rects.push((l, t, r, b));
        }
    }
    for (i, a) in rects.iter().enumerate() {
        for b in &rects[i + 1..] {
            let disjoint = a.2 <= b.0 || b.2 <= a.0 || a.3 <= b.1 || b.3 <= a.1;
            assert!(disjoint, "{a:?} overlaps {b:?}");
        }
    }
    // The same glyph (the repeated 'e'... in "Hamburgefons" the 'e' is once, 'o' once) is one
    // entry: the entries are shared per packed id.
    assert!(seen_ids.len() <= TEXT.len());
}

// Glyphs added to the atlas: the padding that a sub run asks for is around the glyph, and the
// locator is inset back to the glyph itself.
// Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L296-L369 (chrome/m156), addGlyphToAtlas
#[test]
fn add_glyph_to_atlas_pads_transformed_glyphs_and_insets_the_locator() {
    let context = context();
    let recorder = context.make_recorder(None);
    let font = font(20.0);
    let (spec, _) = skia_rust_core::strike_spec::StrikeSpec::make_canonicalized(&font, None);
    let images = BulkGlyphMetricsAndImages::new(&spec);
    let glyph_id = font.unichar_to_glyph('H' as i32);
    let sk_glyph = images.glyph(PackedGlyphId::from_glyph_id(glyph_id));
    assert!(sk_glyph.image().is_some());

    let rp = recorder.priv_();
    let mut atlas_provider = rp.atlas_provider().borrow_mut();
    let manager = atlas_provider.text_atlas_manager_mut();
    // The textures are made when the proxies are asked for.
    assert!(
        manager
            .get_proxies(MaskFormat::A8)
            .is_some_and(|p| p.is_empty())
    );

    let mut placed = Vec::new();
    for padding in [0, 1] {
        let key = PackedGpuGlyphId::new(
            PackedGlyphId::from_glyph_id(glyph_id),
            MaskFormat::A8,
            padding,
            false,
        );
        let entry = GlyphEntry::new(key);
        assert!(!entry.atlas_locator().plot_locator().is_valid());
        let code = manager.add_glyph_to_atlas(&recorder, &sk_glyph, &entry);
        assert_eq!(code, ErrorCode::Succeeded);
        assert!(manager.has_glyph(&entry));

        // The locator is the glyph itself, with `padding` pixels of the atlas around it.
        let locator = entry.atlas_locator();
        assert_eq!(i32::from(locator.width()), i32::from(sk_glyph.width()));
        assert_eq!(i32::from(locator.height()), i32::from(sk_glyph.height()));
        placed.push(locator);
    }
    // Padded glyphs take two more pixels each way, so the second is further from the first than
    // its own size.
    let (a, b) = (placed[0].top_left(), placed[1].top_left());
    assert!(
        a != b && (a.x - b.x).abs().max((a.y - b.y).abs()) >= i32::from(sk_glyph.width()),
        "{a:?} {b:?}"
    );
}

// The same text blob drawn again is the same processed blob, and a translation by whole pixels
// still can use it; one by a fraction of a pixel cannot.
// Port of: src/text/gpu/TextBlobRedrawCoordinator.cpp#L58-L91 (chrome/m156), findOrCreateBlob
#[test]
fn a_text_blob_is_reused_by_whole_pixel_translations() {
    let context = context();
    let recorder = context.make_recorder(None);
    let mut device = make_device(&recorder);
    let font = font(12.0);
    let paint = paint();
    let mut blob_builder = TextBlobBuilder::new();
    add_to_text_blob(&mut blob_builder, TEXT, &font, 0.0, 0.0);
    let blob = blob_builder.make().expect("a blob");

    let draw = |device: &mut Device, x: f32, y: f32| {
        let mut builder = GlyphRunBuilder::new();
        let list = builder.blob_to_glyph_run_list(&blob, Point::new(x, y));
        draw_glyph_run_list(device, &list, &paint);
    };
    draw(&mut device, 10.0, 30.0);
    let rp = recorder.priv_();
    assert_eq!(rp.text_blob_cache().blob_count(), 1);
    let used = rp.text_blob_cache().used_bytes();
    assert!(used > 0);

    // Another whole-pixel position reuses the blob.
    draw(&mut device, 25.0, 80.0);
    assert_eq!(rp.text_blob_cache().blob_count(), 1);
    assert_eq!(rp.text_blob_cache().used_bytes(), used);

    // A fraction of a pixel needs different subpixel glyph masks, so a new blob is made.
    draw(&mut device, 10.5, 30.0);
    assert_eq!(rp.text_blob_cache().blob_count(), 2);

    // Purging the blob removes both.
    rp.text_blob_cache().purge_blob(blob.unique_id());
    assert_eq!(rp.text_blob_cache().blob_count(), 0);
    assert_eq!(rp.text_blob_cache().used_bytes(), 0);
}

// Text that is not from a blob is not cached.
#[test]
fn text_that_is_not_from_a_blob_is_not_cached() {
    let context = context();
    let recorder = context.make_recorder(None);
    let mut device = make_device(&recorder);
    draw_text(
        &mut device,
        &font(12.0),
        &paint(),
        TEXT,
        Point::new(10.0, 40.0),
    );
    assert_eq!(recorder.priv_().text_blob_cache().blob_count(), 0);
}

// A slug is made of a blob and drawn later with the same renderer as the blob, even with a
// different matrix.
// Port of: src/gpu/graphite/Device.cpp#L2573-L2585 (chrome/m156), convertGlyphRunListToSlug, drawSlug
#[test]
fn a_slug_draws_the_text_it_was_made_of() {
    let context = context();
    let recorder = context.make_recorder(None);
    let mut device = make_device(&recorder);
    let font = font(12.0);
    let paint = paint();
    let mut builder = GlyphRunBuilder::new();
    let list = list_of(&mut builder, &font, &paint, TEXT, Point::new(10.0, 40.0));

    let slug = device
        .convert_glyph_run_list_to_slug(&list, &paint)
        .expect("a slug of text");
    assert!(!slug.source_bounds().is_empty());
    assert_eq!(
        slug.source_bounds_with_origin(),
        slug.source_bounds().with_offset(Point::new(10.0, 40.0))
    );
    // Nothing was drawn by making the slug.
    assert_eq!(device.testing_only_pending_render_steps(), 0);

    device.draw_slug(&slug, &paint);
    let passes = snap(&mut device);
    assert_eq!(
        bound_steps(&passes),
        render_step_ids(provider(&context).bitmap_text(false, MaskFormat::A8))
    );
    assert_eq!(instances_drawn(&passes), TEXT.len() as u32);
}

// Text with nothing to draw makes no slug.
#[test]
fn a_slug_of_blank_text_is_none() {
    let context = context();
    let recorder = context.make_recorder(None);
    let mut device = make_device(&recorder);
    let mut builder = GlyphRunBuilder::new();
    let list = list_of(
        &mut builder,
        &font(12.0),
        &paint(),
        "   ",
        Point::new(0.0, 0.0),
    );
    assert!(
        device
            .convert_glyph_run_list_to_slug(&list, &paint())
            .is_none()
    );
}

// When the atlas has no room for the glyphs of a draw, the draws recorded so far are flushed to
// free the plots and the rest of the glyphs are drawn after.
// Port of: src/gpu/graphite/Device.cpp#L1593-L1605 (chrome/m156), the atlas full flush
#[test]
fn a_full_atlas_flushes_and_the_draw_continues() {
    // One page of four plots.
    let options = ContextOptions {
        allow_multiple_atlas_textures: false,
        ..ContextOptions::default()
    };
    let context = make_context(&pipeline_device(), &options).expect("a context");
    let recorder = context.make_recorder(None);
    recorder
        .priv_()
        .atlas_provider()
        .borrow_mut()
        .text_atlas_manager_mut()
        .set_atlas_dimensions_to_minimum_for_testing();
    let mut device = make_device(&recorder);
    // Distance field glyphs of 200 px fill a plot each: the 26 of them do not fit the atlas.
    let text = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    draw_text(
        &mut device,
        &font(200.0),
        &paint(),
        text,
        Point::new(0.0, 200.0),
    );
    let passes = snap(&mut device);
    let drawn = instances_drawn(&passes);
    assert!(drawn > 0, "the last glyphs are still pending");
    assert!(
        (drawn as usize) < text.len(),
        "{drawn} glyphs were left to the last flush: the earlier ones flushed already"
    );
}

// The strike cache purges the oldest strikes when it is over its limits.
// Port of: src/text/gpu/StrikeCache.cpp#L40-L97 (chrome/m156), internalPurge
#[test]
fn the_strike_cache_purges_the_oldest_strikes() {
    use skia_rust_gpu::graphite::text::text_strike::TextStrike;
    let mut cache = StrikeCache::new();
    cache.set_limits(usize::MAX, 4);
    let mut specs = Vec::new();
    for size in 10..30 {
        let (spec, _) =
            skia_rust_core::strike_spec::StrikeSpec::make_canonicalized(&font(size as f32), None);
        specs.push(spec);
    }
    let strikes: Vec<_> = specs
        .iter()
        .take(4)
        .map(|spec| TextStrike::get_or_create(&mut cache, spec))
        .collect();
    assert_eq!(cache.cache_count(), 4);
    // The same spec finds the same strike.
    assert!(Arc::ptr_eq(
        &strikes[0],
        &TextStrike::get_or_create(&mut cache, &specs[0])
    ));
    // One more than the limit purges a quarter (at least one) of the oldest.
    let newest = TextStrike::get_or_create(&mut cache, &specs[4]);
    assert!(cache.cache_count() <= 4);
    assert!(cache.find(newest.descriptor()).is_some());
    assert!(cache.find(strikes[0].descriptor()).is_none());
    assert!(cache.find(strikes[3].descriptor()).is_some());
    cache.free_all();
    assert_eq!(cache.cache_count(), 0);
    assert_eq!(cache.total_memory_used(), 0);
}

// The glyph entries of a strike are made once per key, and count in the strike's memory.
#[test]
fn a_text_strike_shares_its_glyph_entries() {
    use skia_rust_gpu::graphite::text::text_strike::TextStrike;
    let mut cache = StrikeCache::new();
    let (spec, _) = skia_rust_core::strike_spec::StrikeSpec::make_canonicalized(&font(12.0), None);
    let strike = TextStrike::get_or_create(&mut cache, &spec);
    let before = strike.memory_used();
    let key = PackedGpuGlyphId::new(PackedGlyphId::from_glyph_id(5), MaskFormat::A8, 0, false);
    let a = strike.get_glyph(key);
    let b = strike.get_glyph(key);
    assert!(Arc::ptr_eq(&a, &b));
    assert!(strike.memory_used() > before);
    // A different padding is a different entry.
    let other = PackedGpuGlyphId::new(PackedGlyphId::from_glyph_id(5), MaskFormat::A8, 1, false);
    assert!(!Arc::ptr_eq(&a, &strike.get_glyph(other)));
    assert_eq!(cache.total_memory_used(), strike.memory_used());
}

// Freeing the recorder's GPU resources empties its strike cache.
#[test]
fn free_gpu_resources_empties_the_strike_cache() {
    let context = context();
    let mut recorder = context.make_recorder(None);
    {
        let mut device = make_device(&recorder);
        draw_text(
            &mut device,
            &font(12.0),
            &paint(),
            TEXT,
            Point::new(10.0, 40.0),
        );
        let _ = snap(&mut device);
    }
    assert!(recorder.priv_().strike_cache().borrow().cache_count() > 0);
    recorder.free_gpu_resources();
    assert_eq!(recorder.priv_().strike_cache().borrow().cache_count(), 0);
}
