// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRuntimeEffectPriv.h, src/core/SkRuntimeEffect.cpp (the
// `SkRuntimeEffectPriv` and `RuntimeEffectRPCallbacks` members)

//! `SkRuntimeEffectPriv`: the hooks Skia's own code (and tests) use on runtime effects, and the
//! Raster Pipeline callbacks that run an effect's children.
//!
//! skia-rust: `ReadChildEffects`/`WriteChildEffects` (flattening) are not ported.

use std::sync::Arc;

use skia_rust_simd::rp::{MemPtr, Stage};
use skia_rust_sksl::analysis::SampleUsage;
use skia_rust_sksl::codegen::rp::Callbacks;
use skia_rust_sksl::ir::{IrPool, LayoutFlags, VarId};
use skia_rust_sksl::ir::{Program, TypeKind};
use skia_rust_sksl::program_settings::Version;

use crate::alpha_type::AlphaType;
use crate::arena_alloc::ArenaAlloc;
use crate::capabilities::Capabilities;
use crate::color::colors;
use crate::color_space::ColorSpace;
use crate::color_space_priv::{srgb_linear_singleton, srgb_singleton};
use crate::color_space_xform_steps::ColorSpaceXformSteps;
use crate::color_type::ColorType;
use crate::data::Data;
use crate::effect_priv::StageRec;
use crate::matrix::Matrix;
use crate::raster_pipeline::RasterPipeline;
use crate::rect::Rect;
use crate::runtime_effect::{
    Child, ChildPtr, ChildType, Options, RuntimeEffect, Uniform, init_uniform_type, uniform,
};
use crate::shader::Shader;
use crate::shaders::runtime_shader::RuntimeShader;
use crate::shaders::shader_base::MatrixRec;
use crate::surface_props::SurfaceProps;

/// What a [`UniformsCallback`] is told (`SkRuntimeEffectPriv::UniformsCallbackContext`).
// Port of: src/core/SkRuntimeEffectPriv.h#L50-L52 (chrome/m156)
#[derive(Clone, Copy, Debug)]
pub struct UniformsCallbackContext<'a> {
    /// `fDstColorSpace`.
    pub dst_color_space: Option<&'a ColorSpace>,
}

/// Late-bound uniforms: invoked at draw time, it must produce a uniform data blob of the correct
/// size for the effect (`SkRuntimeEffectPriv::UniformsCallback`).
// Port of: src/core/SkRuntimeEffectPriv.h#L54-L60 (chrome/m156)
pub type UniformsCallback = Arc<dyn Fn(&UniformsCallbackContext<'_>) -> Data + Send + Sync>;

/// `SkRuntimeEffectPriv::ES3Options`: options that allow `#version 300`.
// Port of: src/core/SkRuntimeEffectPriv.h#L112-L116 (chrome/m156)
#[doc(alias = "ES3Options")]
#[must_use]
pub fn es3_options() -> Options<'static> {
    Options {
        max_version_allowed: Version::K300,
        ..Options::default()
    }
}

/// `SkRuntimeEffectPriv::AllowPrivateAccess`: lets the effect use Skia implementation details
/// like `sk_FragCoord` and functions with private identifiers.
// Port of: src/core/SkRuntimeEffectPriv.h#L118-L120 (chrome/m156)
#[doc(alias = "AllowPrivateAccess")]
pub fn allow_private_access(options: &mut Options<'_>) {
    options.allow_private_access = true;
}

/// `SkRuntimeEffectPriv::SetStableKeyOnOptions` (for Skia-internal known runtime effects).
// Port of: src/core/SkRuntimeEffectPriv.h#L98-L102 (chrome/m156)
#[doc(alias = "SetStableKeyOnOptions")]
pub fn set_stable_key_on_options(options: &mut Options<'_>, stable_key: u32) {
    debug_assert_eq!(options.stable_key, 0);
    options.stable_key = stable_key;
}

/// `SkRuntimeEffectPriv::Hash`.
// Port of: src/core/SkRuntimeEffectPriv.h#L76-L78 (chrome/m156)
#[doc(alias = "Hash")]
#[must_use]
pub fn hash(effect: &RuntimeEffect) -> u32 {
    effect.0.hash
}

/// `SkRuntimeEffectPriv::HasName`.
#[doc(alias = "HasName")]
#[must_use]
pub fn has_name(effect: &RuntimeEffect) -> bool {
    !effect.0.name.is_empty()
}

/// `SkRuntimeEffectPriv::GetName`.
#[doc(alias = "GetName")]
#[must_use]
pub fn get_name(effect: &RuntimeEffect) -> &str {
    &effect.0.name
}

/// `SkRuntimeEffectPriv::StableKey`.
#[doc(alias = "StableKey")]
#[must_use]
pub fn stable_key(effect: &RuntimeEffect) -> u32 {
    effect
        .0
        .stable_key
        .load(std::sync::atomic::Ordering::Relaxed)
}

/// `SkRuntimeEffectPriv::SetStableKey`: assigns the stable key of an effect that a Graphite
/// context registers as a user-defined known runtime effect. (Skia casts away the `const`; the
/// key is atomic here.)
// Port of: src/core/SkRuntimeEffectPriv.h#L97-L102 (chrome/m156)
#[doc(alias = "SetStableKey")]
pub fn set_stable_key(effect: &RuntimeEffect, stable_key: u32) {
    effect
        .0
        .stable_key
        .store(stable_key, std::sync::atomic::Ordering::Relaxed);
}

/// `SkRuntimeEffectPriv::Program`: runs `f` on the effect's compiled program.
///
/// Skia returns a reference to `fBaseProgram`; here the program sits behind a lock (the raster
/// pipeline compile may swap it), so the caller works inside a closure. `f` must not call back
/// into this function for the same effect.
// Port of: src/core/SkRuntimeEffectPriv.h#L114-L116 (chrome/m156)
#[doc(alias = "Program")]
pub fn with_program<R>(effect: &RuntimeEffect, f: impl FnOnce(&Program) -> R) -> R {
    let program = effect
        .0
        .base_program
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f(&program)
}

/// `SkRuntimeEffectPriv::UsesSampleCoords`.
#[doc(alias = "UsesSampleCoords")]
#[must_use]
pub fn uses_sample_coords(effect: &RuntimeEffect) -> bool {
    effect
        .0
        .flags
        .contains(crate::runtime_effect::EffectFlags::USES_SAMPLE_COORDS)
}

/// `SkRuntimeEffectPriv::SamplesOutsideMain`.
#[doc(alias = "SamplesOutsideMain")]
#[must_use]
pub fn samples_outside_main(effect: &RuntimeEffect) -> bool {
    effect
        .0
        .flags
        .contains(crate::runtime_effect::EffectFlags::SAMPLES_OUTSIDE_MAIN)
}

/// `SkRuntimeEffectPriv::UsesColorTransform`.
#[doc(alias = "UsesColorTransform")]
#[must_use]
pub fn uses_color_transform(effect: &RuntimeEffect) -> bool {
    effect
        .0
        .flags
        .contains(crate::runtime_effect::EffectFlags::USES_COLOR_TRANSFORM)
}

/// `SkRuntimeEffectPriv::AlwaysOpaque`.
#[doc(alias = "AlwaysOpaque")]
#[must_use]
pub fn always_opaque(effect: &RuntimeEffect) -> bool {
    effect
        .0
        .flags
        .contains(crate::runtime_effect::EffectFlags::ALWAYS_OPAQUE)
}

/// `SkRuntimeEffectPriv::IsAlphaUnchanged`.
#[doc(alias = "IsAlphaUnchanged")]
#[must_use]
pub fn is_alpha_unchanged(effect: &RuntimeEffect) -> bool {
    effect
        .0
        .flags
        .contains(crate::runtime_effect::EffectFlags::ALPHA_UNCHANGED)
}

/// `SkRuntimeEffectPriv::ChildSampleUsage`.
#[doc(alias = "ChildSampleUsage")]
#[must_use]
pub fn child_sample_usage(effect: &RuntimeEffect, child: usize) -> SampleUsage {
    effect.0.sample_usages[child]
}

/// `SkRuntimeEffectPriv::VarAsUniform`: the reflected description of the uniform `var`, which
/// sits at `*offset` in the uniform block (`*offset` is advanced past it).
///
/// # Panics
/// If `var` is not a uniform of a type a runtime effect can reflect.
// Port of: src/core/SkRuntimeEffect.cpp#L102-L132 (chrome/m156)
#[doc(alias = "VarAsUniform")]
pub fn var_as_uniform(pool: &IrPool, var_id: VarId, offset: &mut usize) -> Uniform {
    let var = pool.variable(var_id);
    debug_assert!(var.modifier_flags.is_uniform());
    let mut flags = uniform::Flags::empty();
    let mut count = 1;

    let mut ty = pool.ty(var.ty);
    if ty.is_array() {
        flags |= uniform::Flags::ARRAY;
        count = ty.columns();
        ty = ty.component_type();
    }

    if ty.has_precision() && !ty.high_precision() {
        flags |= uniform::Flags::HALF_PRECISION;
    }

    let uniform_type = init_uniform_type(ty).expect("a reflectable uniform type");
    if var.layout.flags.contains(LayoutFlags::COLOR) {
        flags |= uniform::Flags::COLOR;
    }

    let uni = Uniform {
        name: var.name.to_string(),
        offset: *offset,
        ty: uniform_type,
        count,
        flags,
    };
    *offset += uni.size_in_bytes();
    debug_assert_eq!(*offset % 4, 0);
    uni
}

/// `child_type`: the reflected type of an effect child's `SkSL` type.
// Port of: src/core/SkRuntimeEffect.cpp#L134-L141 (chrome/m156)
fn child_type(kind: TypeKind) -> ChildType {
    match kind {
        TypeKind::Blender => ChildType::Blender,
        TypeKind::ColorFilter => ChildType::ColorFilter,
        TypeKind::Shader => ChildType::Shader,
        _ => unreachable!("not an effect child type"),
    }
}

/// `SkRuntimeEffectPriv::ChildTypeToStr`.
// Port of: src/core/SkRuntimeEffect.cpp#L143-L150 (chrome/m156)
#[doc(alias = "ChildTypeToStr")]
#[must_use]
pub fn child_type_to_str(ty: ChildType) -> &'static str {
    match ty {
        ChildType::Blender => "blender",
        ChildType::ColorFilter => "color filter",
        ChildType::Shader => "shader",
    }
}

/// `SkRuntimeEffectPriv::VarAsChild`.
// Port of: src/core/SkRuntimeEffect.cpp#L152-L158 (chrome/m156)
#[doc(alias = "VarAsChild")]
#[must_use]
pub fn var_as_child(pool: &IrPool, var_id: VarId, index: usize) -> Child {
    let var = pool.variable(var_id);
    Child {
        name: var.name.to_string(),
        ty: child_type(pool.ty(var.ty).node().type_kind),
        index,
    }
}

/// `SkRuntimeEffectPriv::TransformUniforms(…, dstCS)`: if there are `layout(color)` uniforms,
/// transforms their values from sRGB to `dst_cs`, and returns the new data. Otherwise the
/// original data is returned.
// Port of: src/core/SkRuntimeEffect.cpp#L160-L171 (chrome/m156)
#[doc(alias = "TransformUniforms")]
#[must_use]
pub fn transform_uniforms(
    uniforms: &[Uniform],
    original_data: &Data,
    dst_cs: Option<&ColorSpace>,
) -> Data {
    let Some(dst_cs) = dst_cs else {
        // There's no destination color-space; we can early-out immediately.
        return original_data.clone();
    };
    let steps = ColorSpaceXformSteps::new(
        Some(srgb_singleton()),
        AlphaType::Unpremul,
        Some(dst_cs),
        AlphaType::Unpremul,
    );
    transform_uniforms_with_steps(uniforms, original_data, &steps)
}

/// `SkRuntimeEffectPriv::TransformUniforms(…, steps)`.
// Port of: src/core/SkRuntimeEffect.cpp#L173-L216 (chrome/m156)
#[doc(alias = "TransformUniforms")]
#[must_use]
pub fn transform_uniforms_with_steps(
    uniforms: &[Uniform],
    original_data: &Data,
    steps: &ColorSpaceXformSteps,
) -> Data {
    let mut data: Option<Vec<u8>> = None;
    let read = |bytes: &[u8], at: usize| -> f32 {
        f32::from_ne_bytes(bytes[at..at + 4].try_into().expect("four bytes"))
    };

    for u in uniforms {
        if u.flags.contains(uniform::Flags::COLOR) {
            debug_assert!(matches!(
                u.ty,
                crate::runtime_effect::UniformType::Float3
                    | crate::runtime_effect::UniformType::Float4
            ));
            if steps.flags.mask() != 0 {
                let bytes = data.get_or_insert_with(|| original_data.as_bytes().to_vec());
                let mut at = u.offset;
                let count = usize::try_from(u.count).expect("a positive uniform count");
                if u.ty == crate::runtime_effect::UniformType::Float4 {
                    // RGBA, easy case
                    for _ in 0..count {
                        let mut color = [
                            read(bytes, at),
                            read(bytes, at + 4),
                            read(bytes, at + 8),
                            read(bytes, at + 12),
                        ];
                        steps.apply(&mut color);
                        for (i, v) in color.iter().enumerate() {
                            bytes[at + 4 * i..at + 4 * i + 4].copy_from_slice(&v.to_ne_bytes());
                        }
                        at += 16;
                    }
                } else {
                    // RGB, need to pad out to include alpha. Technically, this isn't necessary,
                    // because steps shouldn't include unpremul or premul, and thus shouldn't
                    // read or write the fourth element. But let's be safe.
                    for _ in 0..count {
                        let mut rgba = [
                            read(bytes, at),
                            read(bytes, at + 4),
                            read(bytes, at + 8),
                            1.0,
                        ];
                        steps.apply(&mut rgba);
                        for (i, v) in rgba[..3].iter().enumerate() {
                            bytes[at + 4 * i..at + 4 * i + 4].copy_from_slice(&v.to_ne_bytes());
                        }
                        at += 12;
                    }
                }
            }
        }
    }
    match data {
        Some(bytes) => Data::new_from_vec(bytes),
        None => original_data.clone(),
    }
}

/// `SkRuntimeEffectPriv::UniformsAsSpan`: the uniform values as floats, transformed into the
/// destination color space.
///
/// skia-rust: Skia returns a span into the data or into a copy in the arena
/// (`alwaysCopyIntoAlloc`); the values are copied out of the (unaligned) bytes here, and
/// `Program::append_stages` copies them into its slab, so neither is needed.
// Port of: src/core/SkRuntimeEffect.cpp#L286-L308 (chrome/m156)
#[doc(alias = "UniformsAsSpan")]
#[must_use]
pub fn uniforms_as_span(
    uniforms: &[Uniform],
    original_data: &Data,
    dest_color_space: Option<&ColorSpace>,
) -> Vec<f32> {
    // Transform the uniforms into the destination colorspace.
    let transformed_data = transform_uniforms(uniforms, original_data, dest_color_space);
    transformed_data
        .as_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_ne_bytes(*c))
        .collect()
}

/// `SkRuntimeEffectPriv::CanDraw(caps, program)`.
// Port of: src/core/SkRuntimeEffect.cpp#L378-L382 (chrome/m156)
#[doc(alias = "CanDraw")]
#[must_use]
pub fn can_draw_program(caps: &Capabilities, program: &Program) -> bool {
    debug_assert!(program.config.enforces_sksl_version());
    program.config.required_sksl_version <= caps.sksl_version()
}

/// `SkRuntimeEffectPriv::CanDraw(caps, effect)`.
// Port of: src/core/SkRuntimeEffect.cpp#L384-L387 (chrome/m156)
#[doc(alias = "CanDraw")]
#[must_use]
pub fn can_draw(caps: &Capabilities, effect: &RuntimeEffect) -> bool {
    effect.0.required_sksl_version <= caps.sksl_version()
}

/// `SkLocalMatrixShader::MakeWrapped<T>`: wraps `shader` with a local matrix, if there is one.
// Port of: src/shaders/SkLocalMatrixShader.h#L31-L39 (chrome/m156)
pub(crate) fn make_wrapped(local_matrix: Option<&Matrix>, shader: RuntimeShader) -> Shader {
    let t = Shader::from_base(shader);
    match local_matrix {
        Some(local_matrix) => t.with_local_matrix(local_matrix),
        None => t,
    }
}

/// `SkRuntimeEffectPriv::MakeDeferredShader`: a private (experimental) API for creating runtime
/// shaders with late-bound uniforms. The callback is invoked at "draw" time.
// Port of: src/core/SkRuntimeEffect.cpp#L830-L849 (chrome/m156)
#[doc(alias = "MakeDeferredShader")]
#[must_use]
pub fn make_deferred_shader(
    effect: &RuntimeEffect,
    uniforms_callback: UniformsCallback,
    children: &[ChildPtr],
    local_matrix: Option<&Matrix>,
) -> Option<Shader> {
    if !effect.allow_shader() {
        return None;
    }
    if !effect.verify_child_effects(children) {
        return None;
    }
    Some(make_wrapped(
        local_matrix,
        RuntimeShader::new_deferred(effect.clone(), uniforms_callback, children),
    ))
}

/// `RuntimeEffectRPCallbacks`: appends the stages of an effect's children, and the color space
/// conversions its intrinsics ask for.
// Port of: src/core/SkRuntimeEffectPriv.h#L220-L255 (chrome/m156)
#[derive(Debug)]
pub struct RuntimeEffectRpCallbacks<'a, 'c> {
    // `fStage`: a `SkStageRec` that tells shaders to ignore the paint color.
    alloc: &'a ArenaAlloc,
    dst_color_type: ColorType,
    dst_cs: Option<ColorSpace>,
    surface_props: SurfaceProps,
    dst_bounds: Rect,
    matrix: MatrixRec,
    children: &'c [ChildPtr],
    sample_usages: &'c [SampleUsage],
}

impl<'a, 'c> RuntimeEffectRpCallbacks<'a, 'c> {
    /// The callbacks for appending an effect's stages with `s`, the matrix `m`, and the effect's
    /// children and their sample usages.
    ///
    /// `SkStageRec::fPaintColor` is used (strictly) to tint alpha-only image shaders with the
    /// paint color. We want to suppress that behavior when they're sampled from runtime
    /// effects, so we just override the paint color here. See also: `ImageShader::append_misc`.
    // Port of: src/core/SkRuntimeEffectPriv.h#L226-L241 (chrome/m156)
    #[must_use]
    pub fn new(
        s: &StageRec<'_, 'a>,
        m: &MatrixRec,
        c: &'c [ChildPtr],
        u: &'c [SampleUsage],
    ) -> Self {
        RuntimeEffectRpCallbacks {
            alloc: s.alloc,
            dst_color_type: s.dst_color_type,
            dst_cs: s.dst_cs.cloned(),
            surface_props: s.surface_props,
            dst_bounds: s.dst_bounds,
            matrix: m.clone(),
            children: c,
            sample_usages: u,
        }
    }

    /// `fStage` with the pipeline `p`.
    fn stage<'r>(&'r self, p: &'r mut RasterPipeline<'a>) -> StageRec<'r, 'a> {
        StageRec {
            pipeline: p,
            alloc: self.alloc,
            dst_color_type: self.dst_color_type,
            dst_cs: self.dst_cs.as_ref(),
            paint_color: colors::TRANSPARENT,
            surface_props: self.surface_props,
            dst_bounds: self.dst_bounds,
        }
    }

    // Port of: src/core/SkRuntimeEffect.cpp#L365-L376 (chrome/m156)
    fn apply_color_space_xform(
        &self,
        p: &mut RasterPipeline<'a>,
        temp_xform: &ColorSpaceXformSteps,
        color: MemPtr,
    ) {
        // Put the color into src.rgba (and temporarily stash the execution mask there instead).
        p.append(Stage::ExchangeSrc(color));
        // Add the color space transform to our raster pipeline.
        temp_xform.apply_to_pipeline(p, self.alloc);
        // Restore the execution mask, and move the color back into program data.
        p.append(Stage::ExchangeSrc(color));
    }
}

impl<'a> Callbacks<'a, RasterPipeline<'a>> for RuntimeEffectRpCallbacks<'a, '_> {
    // Port of: src/core/SkRuntimeEffect.cpp#L310-L324 (chrome/m156)
    fn append_shader(&mut self, p: &mut RasterPipeline<'a>, index: i32) -> bool {
        let index = usize::try_from(index).expect("a child index");
        if let Some(shader) = self.children[index].shader() {
            if self.sample_usages[index].is_pass_through() {
                // Given a passthrough sample, the total-matrix is still as valid as before.
                return shader
                    .as_base()
                    .append_stages(&mut self.stage(p), &self.matrix);
            }
            // For a non-passthrough sample, we need to explicitly mark the total-matrix as
            // invalid.
            let mut nonpassthrough_matrix = self.matrix.clone();
            nonpassthrough_matrix.mark_total_matrix_invalid();
            return shader
                .as_base()
                .append_stages(&mut self.stage(p), &nonpassthrough_matrix);
        }
        // Return transparent black when a null shader is evaluated.
        p.append_constant_color4f(self.alloc, &colors::TRANSPARENT);
        true
    }

    // Port of: src/core/SkRuntimeEffect.cpp#L325-L331 (chrome/m156)
    fn append_color_filter(&mut self, p: &mut RasterPipeline<'a>, index: i32) -> bool {
        let index = usize::try_from(index).expect("a child index");
        if let Some(color_filter) = self.children[index].color_filter() {
            return color_filter
                .as_base()
                .append_stages(&mut self.stage(p), /* shader_is_opaque */ false);
        }
        // Return the original color as-is when a null child color filter is evaluated.
        true
    }

    // Port of: src/core/SkRuntimeEffect.cpp#L332-L339 (chrome/m156)
    fn append_blender(&mut self, p: &mut RasterPipeline<'a>, index: i32) -> bool {
        let index = usize::try_from(index).expect("a child index");
        if let Some(blender) = self.children[index].blender() {
            return blender.as_base().append_stages(&mut self.stage(p));
        }
        // Return a source-over blend when a null blender is evaluated.
        p.append(Stage::Srcover);
        true
    }

    // TODO (Skia): If an effect calls these intrinsics more than once, we could cache and re-use
    // the steps object(s), rather than re-creating them in the arena repeatedly.
    // Port of: src/core/SkRuntimeEffect.cpp#L343-L352 (chrome/m156)
    fn to_linear_srgb(&mut self, p: &mut RasterPipeline<'a>, color: MemPtr) {
        if let Some(dst_cs) = &self.dst_cs {
            let xform = ColorSpaceXformSteps::new(
                Some(dst_cs),
                AlphaType::Unpremul,
                Some(srgb_linear_singleton()),
                AlphaType::Unpremul,
            );
            if xform.flags.mask() != 0 {
                // We have a non-identity colorspace transform; apply it.
                self.apply_color_space_xform(p, &xform, color);
            }
        }
    }

    // Port of: src/core/SkRuntimeEffect.cpp#L354-L363 (chrome/m156)
    fn from_linear_srgb(&mut self, p: &mut RasterPipeline<'a>, color: MemPtr) {
        if let Some(dst_cs) = &self.dst_cs {
            let xform = ColorSpaceXformSteps::new(
                Some(srgb_linear_singleton()),
                AlphaType::Unpremul,
                Some(dst_cs),
                AlphaType::Unpremul,
            );
            if xform.flags.mask() != 0 {
                // We have a non-identity colorspace transform; apply it.
                self.apply_color_space_xform(p, &xform, color);
            }
        }
    }
}
