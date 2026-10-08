// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/SkRuntimeEffect.h, src/core/SkRuntimeEffect.cpp

//! `SkRuntimeEffect`: custom [`Shader`]s (and, with S19, color filters and blenders) written in
//! Skia's `SkSL` shading language.
//!
//! The public shape follows skia-safe's `effects::runtime_effect`. The private hooks of
//! `SkRuntimeEffectPriv` are in [`crate::runtime_effect_priv`].
//!
//! skia-rust: a [`RuntimeEffect`] is a cheaply clonable `Arc` handle. The effect keeps its base
//! program under a mutex, because the Raster Pipeline code generator needs `&mut Program`
//! (it allocates literals in the program's pool) where Skia's takes `const Program&`. Color
//! filters and blenders (`makeColorFilter`, `makeBlender`, `SkRuntimeColorFilter`,
//! `SkRuntimeBlender`) are S19; tracing (`MakeTraced`) is S23; flattening is not ported.

use core::fmt;
use std::sync::{Arc, Mutex, OnceLock};

use bitflags::bitflags;
use skia_rust_sksl::analysis::{self, SampleUsage};
use skia_rust_sksl::codegen::rp::{self, make_raster_pipeline_program};
use skia_rust_sksl::compiler::Compiler;
use skia_rust_sksl::defines::DEFAULT_INLINE_THRESHOLD;
use skia_rust_sksl::ir::{ElemId, Program, ProgramElementKind, StatementKind, TypeId, TypeRef};
use skia_rust_sksl::program_settings::{ProgramKind, ProgramSettings, Version};

use crate::blender::Blender;
use crate::capabilities::Capabilities;
use crate::checksum::hash32;
use crate::color_filter::ColorFilter;
use crate::data::Data;
use crate::matrix::Matrix;
use crate::runtime_effect_priv as priv_;
use crate::shader::Shader;
use crate::shaders::runtime_shader::RuntimeShader;

/// `SkRuntimeEffect::Uniform::Type`.
// Port of: include/effects/SkRuntimeEffect.h#L61-L73 (chrome/m156)
#[doc(alias = "SkRuntimeEffect::Uniform::Type")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum UniformType {
    /// `kFloat`.
    Float,
    /// `kFloat2`.
    Float2,
    /// `kFloat3`.
    Float3,
    /// `kFloat4`.
    Float4,
    /// `kFloat2x2`.
    Float2x2,
    /// `kFloat3x3`.
    Float3x3,
    /// `kFloat4x4`.
    Float4x4,
    /// `kInt`.
    Int,
    /// `kInt2`.
    Int2,
    /// `kInt3`.
    Int3,
    /// `kInt4`.
    Int4,
}

pub mod uniform {
    //! Types that describe the uniforms of a [`crate::runtime_effect::RuntimeEffect`], e.g.
    //! [`Type`] and [`Flags`].

    use bitflags::bitflags;

    pub use super::UniformType as Type;

    bitflags! {
        /// `SkRuntimeEffect::Uniform::Flags`.
        // Port of: include/effects/SkRuntimeEffect.h#L75-L96 (chrome/m156)
        #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct Flags: u32 {
            /// `kArray_Flag`: uniform is declared as an array. `count` contains the array
            /// length.
            const ARRAY = 0x1;
            /// `kColor_Flag`: uniform is declared with `layout(color)`. Colors should be
            /// supplied as unpremultiplied, extended-range (unclamped) sRGB. The uniform will be
            /// automatically transformed to unpremultiplied extended-range working-space
            /// colors.
            const COLOR = 0x2;
            /// `kVertex_Flag`: with a mesh specification, the uniform is present in the vertex
            /// shader. Not used with a runtime effect.
            const VERTEX = 0x4;
            /// `kFragment_Flag`: with a mesh specification, the uniform is present in the
            /// fragment shader. Not used with a runtime effect.
            const FRAGMENT = 0x8;
            /// `kHalfPrecision_Flag`: the `SkSL` uniform uses a medium-precision type (`half`
            /// instead of `float`).
            const HALF_PRECISION = 0x10;
        }
    }
}

/// Reflected description of a uniform variable in the effect's `SkSL`
/// (`SkRuntimeEffect::Uniform`).
// Port of: include/effects/SkRuntimeEffect.h#L58-L110 (chrome/m156)
#[doc(alias = "SkRuntimeEffect::Uniform")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uniform {
    pub(crate) name: String,
    pub(crate) offset: usize,
    pub(crate) ty: UniformType,
    pub(crate) count: i32,
    pub(crate) flags: uniform::Flags,
}

impl Uniform {
    /// The name of the uniform variable in the effect's `SkSL`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The offset in bytes of the uniform within the uniform data block.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// The `SkSL` type of the uniform.
    #[must_use]
    pub fn ty(&self) -> UniformType {
        self.ty
    }

    /// The number of elements in the uniform. 1 for non-array uniforms.
    #[must_use]
    pub fn count(&self) -> i32 {
        self.count
    }

    /// The flags of the uniform, see [`uniform::Flags`].
    #[must_use]
    pub fn flags(&self) -> uniform::Flags {
        self.flags
    }

    /// True if the uniform is declared as an array. [`Uniform::count`] contains the array length
    /// (`isArray`).
    #[doc(alias = "isArray")]
    #[must_use]
    pub fn is_array(&self) -> bool {
        self.flags.contains(uniform::Flags::ARRAY)
    }

    /// True if the uniform is declared with `layout(color)` (`isColor`).
    #[doc(alias = "isColor")]
    #[must_use]
    pub fn is_color(&self) -> bool {
        self.flags.contains(uniform::Flags::COLOR)
    }

    /// The size in bytes of the uniform (or the whole array, for array uniforms)
    /// (`sizeInBytes`).
    ///
    /// # Panics
    /// If the count is not positive.
    // Port of: src/core/SkRuntimeEffect.cpp#L738-L760 (chrome/m156)
    #[doc(alias = "sizeInBytes")]
    #[must_use]
    pub fn size_in_bytes(&self) -> usize {
        let element_size = match self.ty {
            UniformType::Float | UniformType::Int => 4,
            UniformType::Float2 | UniformType::Int2 => 4 * 2,
            UniformType::Float3 | UniformType::Int3 => 4 * 3,
            UniformType::Float4 | UniformType::Float2x2 | UniformType::Int4 => 4 * 4,
            UniformType::Float3x3 => 4 * 9,
            UniformType::Float4x4 => 4 * 16,
        };
        element_size * usize::try_from(self.count).expect("a positive uniform count")
    }
}

/// `SkRuntimeEffect::ChildType`.
// Port of: include/effects/SkRuntimeEffect.h#L113-L117 (chrome/m156)
#[doc(alias = "SkRuntimeEffect::ChildType")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ChildType {
    /// `kShader`.
    Shader,
    /// `kColorFilter`.
    ColorFilter,
    /// `kBlender`.
    Blender,
}

/// Reflected description of a uniform child (shader, color filter or blender) in the effect's
/// `SkSL` (`SkRuntimeEffect::Child`).
// Port of: include/effects/SkRuntimeEffect.h#L119-L123 (chrome/m156)
#[doc(alias = "SkRuntimeEffect::Child")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Child {
    pub(crate) name: String,
    pub(crate) ty: ChildType,
    pub(crate) index: usize,
}

impl Child {
    /// The name of the child in the effect's `SkSL`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The [`ChildType`] of the child.
    #[must_use]
    pub fn ty(&self) -> ChildType {
        self.ty
    }

    /// The index of the child in [`RuntimeEffect::children`].
    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }
}

/// Options for creating a [`RuntimeEffect`] (`SkRuntimeEffect::Options`).
///
/// `allowPrivateAccess`, `fStableKey` and `maxVersionAllowed` are private in Skia
/// (`SkRuntimeEffectPriv` sets them): see [`crate::runtime_effect_priv`].
// Port of: include/effects/SkRuntimeEffect.h#L125-L152 (chrome/m156)
#[doc(alias = "SkRuntimeEffect::Options")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct Options<'a> {
    /// For testing purposes, disables optimization and inlining. (Normally, runtime effects
    /// don't run the inliner directly, but they still get an inlining pass once they are
    /// painted.) (`forceUnoptimized`.)
    pub force_unoptimized: bool,
    /// When possible this name will be used to identify the created runtime effect (`fName`).
    pub name: &'a str,
    /// `allowPrivateAccess`: lets effects use Skia implementation details like `sk_FragCoord`
    /// and functions with private identifiers.
    pub(crate) allow_private_access: bool,
    /// `fStableKey`: when not 0, a stable key assigned to a known runtime effect.
    pub(crate) stable_key: u32,
    /// `maxVersionAllowed`: lifts the ES2 restrictions on runtime effects (tests and certain
    /// internally created effects only).
    pub(crate) max_version_allowed: Version,
}

/// An object that allows passing a [`Shader`], [`ColorFilter`] or [`Blender`] as a child to
/// [`RuntimeEffect::make_shader`] (`SkRuntimeEffect::ChildPtr`).
///
/// skia-rust: Skia's `ChildPtr` holds an `sk_sp<SkFlattenable>` that may be null, and a null
/// child is legal (a null shader samples transparent black, a null color filter returns its
/// input, a null blender is source-over). That is [`ChildPtr::Empty`], the default, and
/// [`ChildPtr::ty`] is an `Option` for it.
// Port of: include/effects/SkRuntimeEffect.h#L154-L180 (chrome/m156)
#[doc(alias = "SkRuntimeEffect::ChildPtr")]
#[derive(Clone, Debug, Default)]
pub enum ChildPtr {
    /// A null child.
    #[default]
    Empty,
    /// A shader.
    Shader(Shader),
    /// A color filter.
    ColorFilter(ColorFilter),
    /// A blender.
    Blender(Blender),
}

impl From<Shader> for ChildPtr {
    fn from(shader: Shader) -> Self {
        Self::Shader(shader)
    }
}

impl From<ColorFilter> for ChildPtr {
    fn from(color_filter: ColorFilter) -> Self {
        Self::ColorFilter(color_filter)
    }
}

impl From<Blender> for ChildPtr {
    fn from(blender: Blender) -> Self {
        Self::Blender(blender)
    }
}

impl ChildPtr {
    /// The [`ChildType`] of this child, or `None` for a null child (`type`).
    // Port of: src/core/SkRuntimeEffect.cpp#L947-L961 (chrome/m156)
    #[doc(alias = "type")]
    #[must_use]
    pub fn ty(&self) -> Option<ChildType> {
        match self {
            ChildPtr::Empty => None,
            ChildPtr::Shader(_) => Some(ChildType::Shader),
            ChildPtr::ColorFilter(_) => Some(ChildType::ColorFilter),
            ChildPtr::Blender(_) => Some(ChildType::Blender),
        }
    }

    /// The shader, if this child is one (`shader`).
    // Port of: src/core/SkRuntimeEffect.cpp#L963-L967 (chrome/m156)
    #[must_use]
    pub fn shader(&self) -> Option<&Shader> {
        match self {
            ChildPtr::Shader(s) => Some(s),
            _ => None,
        }
    }

    /// The color filter, if this child is one (`colorFilter`).
    // Port of: src/core/SkRuntimeEffect.cpp#L969-L973 (chrome/m156)
    #[doc(alias = "colorFilter")]
    #[must_use]
    pub fn color_filter(&self) -> Option<&ColorFilter> {
        match self {
            ChildPtr::ColorFilter(cf) => Some(cf),
            _ => None,
        }
    }

    /// The blender, if this child is one (`blender`).
    // Port of: src/core/SkRuntimeEffect.cpp#L975-L979 (chrome/m156)
    #[must_use]
    pub fn blender(&self) -> Option<&Blender> {
        match self {
            ChildPtr::Blender(b) => Some(b),
            _ => None,
        }
    }
}

bitflags! {
    /// `SkRuntimeEffect::Flags`.
    // Port of: include/effects/SkRuntimeEffect.h#L248-L258 (chrome/m156)
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub(crate) struct EffectFlags: u32 {
        const USES_SAMPLE_COORDS = 0x001;
        const ALLOW_COLOR_FILTER = 0x002;
        const ALLOW_SHADER = 0x004;
        const ALLOW_BLENDER = 0x008;
        const SAMPLES_OUTSIDE_MAIN = 0x010;
        const USES_COLOR_TRANSFORM = 0x020;
        const ALWAYS_OPAQUE = 0x040;
        const ALPHA_UNCHANGED = 0x080;
        const DISABLE_OPTIMIZATION = 0x100;
    }
}

/// The data of a [`RuntimeEffect`].
pub(crate) struct EffectData {
    /// `fHash`.
    pub(crate) hash: u32,
    /// `fStableKey`.
    pub(crate) stable_key: u32,
    /// `fName`.
    pub(crate) name: String,
    /// `fBaseProgram`.
    pub(crate) base_program: Mutex<Program>,
    /// `fBaseProgram->fConfig->fRequiredSkSLVersion`.
    pub(crate) required_sksl_version: Version,
    /// `fBaseProgram->fSource`.
    pub(crate) source: String,
    /// `fRPProgram`, compiled on first use (`fCompileRPProgramOnce`).
    pub(crate) rp_program: OnceLock<Option<rp::Program>>,
    /// `fUniforms`.
    pub(crate) uniforms: Vec<Uniform>,
    /// `fChildren`.
    pub(crate) children: Vec<Child>,
    /// `fSampleUsages`.
    pub(crate) sample_usages: Vec<SampleUsage>,
    /// `fFlags`.
    pub(crate) flags: EffectFlags,
}

/// Supports creating custom [`Shader`]s using Skia's `SkSL` shading language
/// (`SkRuntimeEffect`).
///
/// NOTE: This API is experimental and subject to change.
// Port of: include/effects/SkRuntimeEffect.h#L51-L330 (chrome/m156)
#[doc(alias = "SkRuntimeEffect")]
#[derive(Clone)]
pub struct RuntimeEffect(pub(crate) Arc<EffectData>);

impl fmt::Debug for RuntimeEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeEffect")
            .field("uniform_size", &self.uniform_size())
            .field("uniforms", &self.uniforms())
            .field("children", &self.children())
            .field("allow_shader", &self.allow_shader())
            .field("allow_color_filter", &self.allow_color_filter())
            .field("allow_blender", &self.allow_blender())
            .finish_non_exhaustive()
    }
}

/// Checks the children given to `makeShader` and friends against the reflected ones
/// (`verify_child_effects`).
// Port of: src/core/SkRuntimeEffect.cpp#L407-L422 (chrome/m156)
fn verify_child_effects(reflected: &[Child], effect_ptrs: &[ChildPtr]) -> bool {
    // Verify that the number of passed-in child-effect pointers matches the SkSL code.
    if reflected.len() != effect_ptrs.len() {
        return false;
    }

    // Verify that each child object's type matches its declared type in the SkSL.
    for (ptr, child) in effect_ptrs.iter().zip(reflected) {
        if let Some(effect_type) = ptr.ty()
            && effect_type != child.ty
        {
            return false;
        }
    }
    true
}

/// The `Result` of the factories, as skia-safe: the effect or the compiler's error text.
type MakeResult = Result<RuntimeEffect, String>;

impl RuntimeEffect {
    /// Creates a runtime effect for use as a color filter (`MakeForColorFilter`).
    ///
    /// Color filter `SkSL` requires an entry point that looks like `vec4 main(vec4 inColor)`.
    ///
    /// # Errors
    /// The compiler's error text.
    // Port of: src/core/SkRuntimeEffect.cpp#L683-L689 (chrome/m156)
    #[doc(alias = "MakeForColorFilter")]
    pub fn make_for_color_filter(
        sksl: impl AsRef<str>,
        options: Option<&Options<'_>>,
    ) -> MakeResult {
        let options = options.copied().unwrap_or_default();
        let kind = if options.allow_private_access {
            ProgramKind::PrivateRuntimeColorFilter
        } else {
            ProgramKind::RuntimeColorFilter
        };
        let result = Self::make_from_source(sksl.as_ref(), &options, kind);
        debug_assert!(
            result
                .as_ref()
                .map_or(true, RuntimeEffect::allow_color_filter)
        );
        result
    }

    /// Creates a runtime effect for use as a [`Shader`] (`MakeForShader`).
    ///
    /// Shader `SkSL` requires an entry point that looks like `vec4 main(vec2 inCoords)`. The
    /// color that is returned should be premultiplied.
    ///
    /// # Errors
    /// The compiler's error text.
    // Port of: src/core/SkRuntimeEffect.cpp#L691-L697 (chrome/m156)
    #[doc(alias = "MakeForShader")]
    pub fn make_for_shader(sksl: impl AsRef<str>, options: Option<&Options<'_>>) -> MakeResult {
        let options = options.copied().unwrap_or_default();
        let kind = if options.allow_private_access {
            ProgramKind::PrivateRuntimeShader
        } else {
            ProgramKind::RuntimeShader
        };
        let result = Self::make_from_source(sksl.as_ref(), &options, kind);
        debug_assert!(result.as_ref().map_or(true, RuntimeEffect::allow_shader));
        result
    }

    /// Creates a runtime effect for use as a [`Blender`] (`MakeForBlender`).
    ///
    /// Blend `SkSL` requires an entry point that looks like
    /// `vec4 main(vec4 srcColor, vec4 dstColor)`.
    ///
    /// # Errors
    /// The compiler's error text.
    // Port of: src/core/SkRuntimeEffect.cpp#L699-L705 (chrome/m156)
    #[doc(alias = "MakeForBlender")]
    pub fn make_for_blender(sksl: impl AsRef<str>, options: Option<&Options<'_>>) -> MakeResult {
        let options = options.copied().unwrap_or_default();
        let kind = if options.allow_private_access {
            ProgramKind::PrivateRuntimeBlender
        } else {
            ProgramKind::RuntimeBlender
        };
        let result = Self::make_from_source(sksl.as_ref(), &options, kind);
        debug_assert!(result.as_ref().map_or(true, RuntimeEffect::allow_blender));
        result
    }

    /// The settings the factories compile with (`MakeSettings`).
    // Port of: src/core/SkRuntimeEffect.cpp#L472-L488 (chrome/m156)
    #[doc(alias = "MakeSettings")]
    #[must_use]
    pub fn make_settings(options: &Options<'_>) -> ProgramSettings {
        const DISABLE_SKSL_INLINING: i32 = 0;
        ProgramSettings {
            inline_threshold: DISABLE_SKSL_INLINING,
            force_no_inline: options.force_unoptimized,
            optimize: !options.force_unoptimized,
            max_version_allowed: options.max_version_allowed,
            // SkSL created by the GPU backend is typically parsed, converted to a backend
            // format, and the IR is immediately discarded. In that situation, it makes sense to
            // use node pools to accelerate the IR allocations. Here, SkRuntimeEffect instances
            // are often long-lived: we're willing to pay for a slightly longer compile so that
            // we don't waste huge amounts of memory.
            use_memory_pool: false,
            ..ProgramSettings::default()
        }
    }

    // Port of: src/core/SkRuntimeEffect.cpp#L494-L507 (chrome/m156)
    fn make_from_source(sksl: &str, options: &Options<'_>, kind: ProgramKind) -> MakeResult {
        let mut compiler = Compiler::new();
        let settings = Self::make_settings(options);
        let Some(program) = compiler.convert_program(kind, sksl.as_bytes(), settings) else {
            return Err(String::from_utf8_lossy(&compiler.error_text_bytes(true)).into_owned());
        };

        Self::make_internal(program, options, kind)
    }

    // Port of: src/core/SkRuntimeEffect.cpp#L509-L642 (chrome/m156)
    #[allow(clippy::too_many_lines)] // one function in Skia, ported as written
    fn make_internal(program: Program, options: &Options<'_>, kind: ProgramKind) -> MakeResult {
        let mut flags = EffectFlags::empty();
        match kind {
            ProgramKind::PrivateRuntimeColorFilter | ProgramKind::RuntimeColorFilter => {
                // TODO(skbug.com/40042585): Figure out a way to run ES3+ color filters on the
                // CPU. This doesn't need to be fast - it could just be direct IR evaluation. But
                // without it, there's no way for us to fully implement the SkColorFilter API
                // (eg, `filterColor4f`)
                if !priv_::can_draw_program(Capabilities::raster_backend(), &program) {
                    return Err("SkSL color filters must target #version 100".to_owned());
                }
                flags |= EffectFlags::ALLOW_COLOR_FILTER;
            }
            ProgramKind::PrivateRuntimeShader | ProgramKind::RuntimeShader => {
                flags |= EffectFlags::ALLOW_SHADER;
            }
            ProgramKind::PrivateRuntimeBlender | ProgramKind::RuntimeBlender => {
                flags |= EffectFlags::ALLOW_BLENDER;
            }
            _ => unreachable!("not a runtime effect program kind"),
        }

        if options.force_unoptimized {
            flags |= EffectFlags::DISABLE_OPTIMIZATION;
        }

        // Find 'main', then locate the sample coords parameter. (It might not be present.)
        let Some(main) = program.get_function("main") else {
            return Err("missing 'main' function".to_owned());
        };
        let main_definition: ElemId = program
            .pool
            .function(main)
            .definition
            .expect("get_function finds only functions with a definition");
        let coords_param = program.pool.function(main).main_coords_parameter();

        let usage = analysis::get_usage(&program);
        let sample_coords_usage = coords_param
            .map(|c| usage.get_variable(c))
            .unwrap_or_default();

        if sample_coords_usage.read != 0 || sample_coords_usage.write != 0 {
            flags |= EffectFlags::USES_SAMPLE_COORDS;
        }

        // Color filters and blends are not allowed to depend on position (local or device) in
        // any way. The signature of main, and the declarations in sksl_rt_colorfilter/
        // sksl_rt_blend should guarantee this.
        if flags.intersects(EffectFlags::ALLOW_COLOR_FILTER | EffectFlags::ALLOW_BLENDER) {
            debug_assert!(!flags.contains(EffectFlags::USES_SAMPLE_COORDS));
            debug_assert!(!analysis::references_frag_coords(&program, &usage));
        }

        if analysis::calls_sample_outside_main(&program) {
            flags |= EffectFlags::SAMPLES_OUTSIDE_MAIN;
        }

        // Look for color filters that preserve the input alpha. This analysis is very
        // conservative, and only returns true when the input alpha is returned as-is from main()
        // with no intervening copies or arithmetic.
        if flags.contains(EffectFlags::ALLOW_COLOR_FILTER)
            && analysis::returns_input_alpha(&program.pool, main_definition, &usage)
        {
            flags |= EffectFlags::ALPHA_UNCHANGED;
        }

        // Determine if this effect uses of the color transform intrinsics. Effects need to know
        // this so they can allocate color transform objects, etc.
        if analysis::calls_color_transform_intrinsics(&program, &usage) {
            flags |= EffectFlags::USES_COLOR_TRANSFORM;
        }

        // Shaders are the only thing that cares about this, but it's inexpensive (and safe) to
        // call.
        if analysis::returns_opaque_color(&program.pool, main_definition) {
            flags |= EffectFlags::ALWAYS_OPAQUE;
        }

        // Go through program elements, pulling out information that we need
        let mut offset: usize = 0;
        let mut uniforms: Vec<Uniform> = Vec::new();
        let mut children: Vec<Child> = Vec::new();
        let mut sample_usages: Vec<SampleUsage> = Vec::new();
        let mut elided_sample_coords: i32 = 0;

        let elements: Vec<ElemId> = program.elements().collect();
        for elem in elements {
            // Variables (uniform, etc.)
            let ProgramElementKind::GlobalVar(global) = &program.pool.element(elem).kind else {
                continue;
            };
            let StatementKind::VarDeclaration(var_decl) =
                &program.pool.statement(global.declaration).kind
            else {
                unreachable!("a global variable declaration holds a VarDeclaration");
            };
            let var_id = var_decl.var;
            let var = program.pool.variable(var_id);

            // Child effects that can be sampled ('shader', 'colorFilter', 'blender')
            if program.pool.ty(var.ty).is_effect_child() {
                children.push(priv_::var_as_child(&program.pool, var_id, children.len()));
                let usage = analysis::get_sample_usage(
                    &program,
                    var_id,
                    sample_coords_usage.write != 0,
                    Some(&mut elided_sample_coords),
                );
                // If the child is never sampled, we pretend that it's actually in PassThrough
                // mode. Otherwise, the GP code for collecting transforms and emitting transform
                // code gets very confused, leading to asserts and bad (backend) shaders.
                // There's an implicit assumption that every FP is used by its parent.
                // (skbug.com/40043510)
                sample_usages.push(if usage.is_sampled() {
                    usage
                } else {
                    SampleUsage::pass_through()
                });
            }
            // 'uniform' variables
            else if var.modifier_flags.is_uniform() {
                uniforms.push(priv_::var_as_uniform(&program.pool, var_id, &mut offset));
            }
        }

        // If the sample coords are never written to, then we will have converted sample calls
        // that use them unmodified into "passthrough" sampling. If all references to the sample
        // coords were of that form, then we don't actually "use" sample coords. We unset the
        // flag to prevent creating an extra (unused) varying holding the coords.
        if elided_sample_coords == sample_coords_usage.read && sample_coords_usage.write == 0 {
            flags.remove(EffectFlags::USES_SAMPLE_COORDS);
        }

        // The `SkRuntimeEffect` constructor.
        let source = String::from_utf8_lossy(&program.source).into_owned();
        let required_sksl_version = program.config.required_sksl_version;
        // Everything from SkRuntimeEffect::Options which could influence the compiled result
        // needs to be accounted for in `fHash`.
        let mut hash = hash32(&program.source, 0);
        hash = hash32(&[u8::from(options.force_unoptimized)], hash);
        hash = hash32(&[u8::from(options.allow_private_access)], hash);
        hash = hash32(&options.stable_key.to_ne_bytes(), hash);
        // (`SkSL::Version` is an `int`-sized enum.)
        hash = hash32(&(options.max_version_allowed as i32).to_ne_bytes(), hash);

        debug_assert_eq!(children.len(), sample_usages.len());
        Ok(RuntimeEffect(Arc::new(EffectData {
            hash,
            stable_key: options.stable_key,
            name: options.name.to_owned(),
            base_program: Mutex::new(program),
            required_sksl_version,
            source,
            rp_program: OnceLock::new(),
            uniforms,
            children,
            sample_usages,
            flags,
        })))
    }

    /// Compiles (once) and returns the Raster Pipeline program of this effect, or `None` if the
    /// Raster Pipeline back end does not support it (`getRPProgram`).
    ///
    /// skia-rust: the debug-trace variants of `getRPProgram` come with S23.
    // Port of: src/core/SkRuntimeEffect.cpp#L218-L284 (chrome/m156)
    #[doc(alias = "getRPProgram")]
    pub(crate) fn rp_program(&self) -> Option<&rp::Program> {
        // Lazily compile the program the first time `getRPProgram` is called. By using a
        // `OnceLock`, we avoid thread hazards and behave in a conceptually const way, but we can
        // avoid the cost of invoking the RP code generator until it's actually needed.
        self.0
            .rp_program
            .get_or_init(|| {
                // We generally do not run the inliner when an SkRuntimeEffect program is
                // initially created, because the final compile to native shader code will do
                // this. However, in SkRP, there's no additional compilation occurring, so we
                // need to optimize/inline here if we want the performance boost of inlining.
                // If optimization is necessary, we re-compile the program from source with
                // inlining and optimization enabled to get a freshly optimized copy.
                let mut base = self
                    .0
                    .base_program
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);

                let should_optimize = !self.0.flags.contains(EffectFlags::DISABLE_OPTIMIZATION);
                let mut settings = base.config.settings;
                let needs_optimization =
                    !settings.optimize || settings.inline_threshold < DEFAULT_INLINE_THRESHOLD;
                let mut optimized_copy: Option<Program> = None;
                if should_optimize && needs_optimization {
                    let mut compiler = Compiler::new();
                    settings.optimize = true;
                    settings.inline_threshold = DEFAULT_INLINE_THRESHOLD;
                    // We might fail to inline the sksl if there's symbol conflicts. In that
                    // case, we'll use the unoptimized version which may or may not produce what
                    // the user wanted.
                    optimized_copy =
                        compiler.convert_program(base.config.kind, &base.source, settings);
                }
                let program: &mut Program = optimized_copy.as_mut().unwrap_or(&mut *base);
                let main = program.get_function("main").expect("the effect has a main");
                let main_definition = program
                    .pool
                    .function(main)
                    .definition
                    .expect("get_function finds only functions with a definition");
                make_raster_pipeline_program(program, main_definition, None, false)
            })
            .as_ref()
    }

    /// Creates a [`Shader`] from this effect (`makeShader`).
    ///
    /// - `uniforms`: a [`Data`] block of size [`RuntimeEffect::uniform_size`], containing values
    ///   for all uniform variables
    /// - `children`: the child shaders/color filters/blenders required by the effect, in the
    ///   order given by [`RuntimeEffect::children`]
    /// - `local_matrix`: an optional local matrix applied to the shader
    ///
    /// Returns `None` if the effect is not a shader effect or the arguments do not match it.
    // Port of: src/core/SkRuntimeEffect.cpp#L862-L882 (chrome/m156)
    #[doc(alias = "makeShader")]
    #[must_use]
    pub fn make_shader<'a>(
        &self,
        uniforms: impl Into<Data>,
        children: &[ChildPtr],
        local_matrix: impl Into<Option<&'a Matrix>>,
    ) -> Option<Shader> {
        if !self.allow_shader() {
            return None;
        }
        if !verify_child_effects(&self.0.children, children) {
            return None;
        }
        let uniforms: Data = uniforms.into();
        if uniforms.size() != self.uniform_size() {
            return None;
        }
        Some(priv_::make_wrapped(
            local_matrix.into(),
            RuntimeShader::new(self.clone(), uniforms, children),
        ))
    }

    /// The `SkSL` source of the runtime effect shader (`source`).
    // Port of: src/core/SkRuntimeEffect.cpp#L805-L807 (chrome/m156)
    #[must_use]
    pub fn source(&self) -> &str {
        &self.0.source
    }

    /// The combined size of all uniform variables. When calling [`RuntimeEffect::make_shader`],
    /// provide a [`Data`] of this size, containing values for all of those variables
    /// (`uniformSize`).
    // Port of: src/core/SkRuntimeEffect.cpp#L809-L812 (chrome/m156)
    #[doc(alias = "uniformSize")]
    #[must_use]
    pub fn uniform_size(&self) -> usize {
        self.0
            .uniforms
            .last()
            .map_or(0, |u| (u.offset + u.size_in_bytes()).next_multiple_of(4))
    }

    /// The descriptions of all uniform variables in the effect's `SkSL`.
    #[must_use]
    pub fn uniforms(&self) -> &[Uniform] {
        &self.0.uniforms
    }

    /// The descriptions of all child effects in the effect's `SkSL`.
    #[must_use]
    pub fn children(&self) -> &[Child] {
        &self.0.children
    }

    /// The description of the named uniform variable, or `None` if not found (`findUniform`).
    // Port of: src/core/SkRuntimeEffect.cpp#L814-L819 (chrome/m156)
    #[doc(alias = "findUniform")]
    #[must_use]
    pub fn find_uniform(&self, name: impl AsRef<str>) -> Option<&Uniform> {
        let name = name.as_ref();
        self.0.uniforms.iter().find(|u| u.name == name)
    }

    /// The description of the named child, or `None` if not found (`findChild`).
    // Port of: src/core/SkRuntimeEffect.cpp#L821-L826 (chrome/m156)
    #[doc(alias = "findChild")]
    #[must_use]
    pub fn find_child(&self, name: impl AsRef<str>) -> Option<&Child> {
        let name = name.as_ref();
        self.0.children.iter().find(|c| c.name == name)
    }

    /// Whether this effect can be used as a [`Shader`] (`allowShader`).
    #[doc(alias = "allowShader")]
    #[must_use]
    pub fn allow_shader(&self) -> bool {
        self.0.flags.contains(EffectFlags::ALLOW_SHADER)
    }

    /// Whether this effect can be used as a color filter (`allowColorFilter`).
    #[doc(alias = "allowColorFilter")]
    #[must_use]
    pub fn allow_color_filter(&self) -> bool {
        self.0.flags.contains(EffectFlags::ALLOW_COLOR_FILTER)
    }

    /// Whether this effect can be used as a [`Blender`] (`allowBlender`).
    #[doc(alias = "allowBlender")]
    #[must_use]
    pub fn allow_blender(&self) -> bool {
        self.0.flags.contains(EffectFlags::ALLOW_BLENDER)
    }

    /// Checks the children given to `makeShader` and friends against the reflected ones.
    pub(crate) fn verify_child_effects(&self, children: &[ChildPtr]) -> bool {
        verify_child_effects(&self.0.children, children)
    }

    /// True if the two handles are the same effect.
    #[must_use]
    pub fn ptr_eq(&self, other: &RuntimeEffect) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// An error from the uniform setters of a [`RuntimeEffectBuilder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderBuilderError {
    /// The uniform is missing, or the value is not the size the effect expects.
    UniformSizeNotSupported,
}

impl fmt::Display for ShaderBuilderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShaderBuilderError::UniformSizeNotSupported => write!(f, "Unsupported uniform size"),
        }
    }
}

impl std::error::Error for ShaderBuilderError {}

/// A utility to simplify creating shaders from a [`RuntimeEffect`]
/// (`SkRuntimeEffectBuilder`, with its deprecated aliases `SkRuntimeShaderBuilder`,
/// `SkRuntimeColorFilterBuilder` and `SkRuntimeBlendBuilder`).
///
/// Given an effect, the builder manages creating an input data block and provides named access to
/// the uniform variables in that block, as well as named access to a list of child slots.
// Port of: include/effects/SkRuntimeEffect.h#L335-L478 (chrome/m156)
#[doc(alias = "SkRuntimeEffectBuilder")]
#[doc(alias = "SkRuntimeShaderBuilder")]
#[derive(Clone, Debug)]
pub struct RuntimeEffectBuilder {
    effect: RuntimeEffect,
    uniforms: Data,
    children: Vec<ChildPtr>,
}

/// `SkRuntimeShaderBuilder`.
pub type RuntimeShaderBuilder = RuntimeEffectBuilder;

/// A named uniform of a [`RuntimeEffectBuilder`] (`SkRuntimeEffectBuilder::BuilderUniform`).
#[derive(Debug)]
pub struct BuilderUniform<'a> {
    owner: &'a mut RuntimeEffectBuilder,
    var: Option<Uniform>,
}

/// A named child of a [`RuntimeEffectBuilder`] (`SkRuntimeEffectBuilder::BuilderChild`).
#[derive(Debug)]
pub struct BuilderChild<'a> {
    owner: &'a mut RuntimeEffectBuilder,
    child: Option<Child>,
}

impl BuilderUniform<'_> {
    /// The description of the variable, `None` if the variable was not found (`fVar`).
    #[doc(alias = "fVar")]
    #[must_use]
    pub fn var(&self) -> Option<&Uniform> {
        self.var.as_ref()
    }

    /// Copies `val` to this variable. No type conversion is performed: `val` must be the same
    /// size as the effect expects. Returns false, and copies nothing, for a missing variable or
    /// a value of the wrong size (Skia aborts in debug builds) (`operator=`).
    #[doc(alias = "operator=")]
    pub fn assign_bytes(&mut self, val: &[u8]) -> bool {
        match &self.var {
            Some(var) if val.len() == var.size_in_bytes() => {
                let offset = var.offset;
                self.owner.writable_uniform_data()[offset..offset + val.len()].copy_from_slice(val);
                true
            }
            _ => false,
        }
    }

    /// Copies `val` to this variable (`operator=(const T&)`/`set` with `float`s).
    pub fn set_f32(&mut self, val: &[f32]) -> bool {
        let bytes: Vec<u8> = val.iter().flat_map(|v| v.to_ne_bytes()).collect();
        self.assign_bytes(&bytes)
    }

    /// Copies `val` to this variable (`set` with `int`s).
    pub fn set_i32(&mut self, val: &[i32]) -> bool {
        let bytes: Vec<u8> = val.iter().flat_map(|v| v.to_ne_bytes()).collect();
        self.assign_bytes(&bytes)
    }

    /// Copies the 3x3 `val` to this variable, column-major (`operator=(const SkMatrix&)`).
    // Port of: include/effects/SkRuntimeEffect.h#L387-L401 (chrome/m156)
    pub fn set_matrix(&mut self, val: &Matrix) -> bool {
        match &self.var {
            Some(var) if var.size_in_bytes() == 9 * 4 => {
                let data = [
                    val.get(0usize),
                    val.get(3usize),
                    val.get(6usize),
                    val.get(1usize),
                    val.get(4usize),
                    val.get(7usize),
                    val.get(2usize),
                    val.get(5usize),
                    val.get(8usize),
                ];
                self.set_f32(&data)
            }
            _ => false,
        }
    }
}

impl BuilderChild<'_> {
    /// The description of the child, `None` if the child was not found (`fChild`).
    #[doc(alias = "fChild")]
    #[must_use]
    pub fn child(&self) -> Option<&Child> {
        self.child.as_ref()
    }

    /// Sets the child (`operator=`). Returns false for a missing child (Skia aborts in debug
    /// builds).
    #[doc(alias = "operator=")]
    pub fn assign(&mut self, val: impl Into<ChildPtr>) -> bool {
        match &self.child {
            Some(child) => {
                self.owner.children[child.index] = val.into();
                true
            }
            None => false,
        }
    }

    /// Sets the child to null (`operator=(nullptr)`).
    pub fn assign_null(&mut self) -> bool {
        self.assign(ChildPtr::Empty)
    }
}

impl RuntimeEffectBuilder {
    /// A builder for `effect` with zeroed uniforms and null children.
    // Port of: include/effects/SkRuntimeEffect.h#L337-L341 (chrome/m156)
    #[must_use]
    pub fn new(effect: RuntimeEffect) -> Self {
        let uniforms = Data::new_zero_initialized(effect.uniform_size());
        let children = vec![ChildPtr::Empty; effect.children().len()];
        Self {
            effect,
            uniforms,
            children,
        }
    }

    /// A builder for `effect` starting from `uniforms`.
    // Port of: include/effects/SkRuntimeEffect.h#L342-L345 (chrome/m156)
    #[must_use]
    pub fn new_with_uniforms(effect: RuntimeEffect, uniforms: Data) -> Self {
        let children = vec![ChildPtr::Empty; effect.children().len()];
        Self {
            effect,
            uniforms,
            children,
        }
    }

    /// The effect (`effect`).
    #[must_use]
    pub fn effect(&self) -> &RuntimeEffect {
        &self.effect
    }

    /// Named access to a uniform (`uniform`).
    pub fn uniform(&mut self, name: impl AsRef<str>) -> BuilderUniform<'_> {
        let var = self.effect.find_uniform(name).cloned();
        BuilderUniform { owner: self, var }
    }

    /// Named access to a child (`child`).
    pub fn child(&mut self, name: impl AsRef<str>) -> BuilderChild<'_> {
        let child = self.effect.find_child(name).cloned();
        BuilderChild { owner: self, child }
    }

    /// Sets float uniform values by name, as `float`, `float2`, `float3`, `float4`, `float2x2`,
    /// `float3x3` or `float4x4`, with the matching number of values.
    ///
    /// # Errors
    /// [`ShaderBuilderError::UniformSizeNotSupported`] for a missing uniform or a value of the
    /// wrong size.
    pub fn set_uniform_float(
        &mut self,
        name: impl AsRef<str>,
        data: &[f32],
    ) -> Result<(), ShaderBuilderError> {
        if self.uniform(name).set_f32(data) {
            Ok(())
        } else {
            Err(ShaderBuilderError::UniformSizeNotSupported)
        }
    }

    /// Sets int uniform values by name, as `int`, `int2`, `int3` or `int4`.
    ///
    /// # Errors
    /// [`ShaderBuilderError::UniformSizeNotSupported`] for a missing uniform or a value of the
    /// wrong size.
    pub fn set_uniform_int(
        &mut self,
        name: impl AsRef<str>,
        data: &[i32],
    ) -> Result<(), ShaderBuilderError> {
        if self.uniform(name).set_i32(data) {
            Ok(())
        } else {
            Err(ShaderBuilderError::UniformSizeNotSupported)
        }
    }

    /// The collated uniforms (`uniforms`).
    #[must_use]
    pub fn uniforms(&self) -> &Data {
        &self.uniforms
    }

    /// The collated children, in the order [`RuntimeEffect::make_shader`] expects (`children`).
    #[must_use]
    pub fn children(&self) -> &[ChildPtr] {
        &self.children
    }

    /// Creates the [`Shader`] from the configured builder (`makeShader`).
    // Port of: src/core/SkRuntimeEffect.cpp#L992-L994 (chrome/m156)
    #[doc(alias = "makeShader")]
    #[must_use]
    pub fn make_shader<'a>(&self, local_matrix: impl Into<Option<&'a Matrix>>) -> Option<Shader> {
        self.effect
            .make_shader(self.uniforms.clone(), &self.children, local_matrix)
    }

    // Port of: include/effects/SkRuntimeEffect.h#L470-L475 (chrome/m156)
    fn writable_uniform_data(&mut self) -> &mut [u8] {
        if !self.uniforms.unique() {
            self.uniforms = Data::new_copy(self.uniforms.as_bytes());
        }
        self.uniforms
            .writable_data()
            .expect("unique data is writable")
    }
}

/// The uniform type of an `SkSL` type, `None` if it is not a uniform type (`init_uniform_type`).
// Port of: src/core/SkRuntimeEffect.cpp#L75-L100 (chrome/m156)
pub(crate) fn init_uniform_type(ty: TypeRef<'_>) -> Option<UniformType> {
    let pairs: [(TypeId, UniformType); 18] = [
        (TypeId::FLOAT, UniformType::Float),
        (TypeId::HALF, UniformType::Float),
        (TypeId::FLOAT2, UniformType::Float2),
        (TypeId::HALF2, UniformType::Float2),
        (TypeId::FLOAT3, UniformType::Float3),
        (TypeId::HALF3, UniformType::Float3),
        (TypeId::FLOAT4, UniformType::Float4),
        (TypeId::HALF4, UniformType::Float4),
        (TypeId::FLOAT2X2, UniformType::Float2x2),
        (TypeId::HALF2X2, UniformType::Float2x2),
        (TypeId::FLOAT3X3, UniformType::Float3x3),
        (TypeId::HALF3X3, UniformType::Float3x3),
        (TypeId::FLOAT4X4, UniformType::Float4x4),
        (TypeId::HALF4X4, UniformType::Float4x4),
        (TypeId::INT, UniformType::Int),
        (TypeId::INT2, UniformType::Int2),
        (TypeId::INT3, UniformType::Int3),
        (TypeId::INT4, UniformType::Int4),
    ];
    pairs
        .iter()
        .find(|&&(candidate, _)| ty.matches(candidate))
        .map(|&(_, uniform_type)| uniform_type)
}
