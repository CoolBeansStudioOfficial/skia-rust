// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/KeyContext.{h,cpp}

//! [`KeyContext`]: what the key generation of a paint's effects reads and writes.
//!
//! The key context must always be able to provide a valid `ShaderCodeDictionary` and runtime
//! effect dictionary. Depending on the calling context it can also supply a backend-specific
//! resource providing object (e.g., a `Recorder`).
//!
//! Deviations from the C++:
//!
//! - The builder and the gatherer are shared between the copies of a context, as pointers are in
//!   C++; here they are `&RefCell`s.
//! - `KeyContext` holds the local matrix by value, so `KeyContextWithLocalMatrix` (which keeps
//!   the concatenated matrix in its own storage) is [`KeyContext::with_local_matrix`].
//! - `DrawContext` (G10a) is not ported yet. The context keeps the one thing it reads from it,
//!   the format of the target's texture (`targetFormat()`), and the recorder constructor takes
//!   that format where Skia takes the `DrawContext*`.
//! - `PaintParams::Color4fPrepForDst` (G5d) is [`color4f_prep_for_dst`] here until
//!   `PaintParams` is ported.

use std::cell::RefCell;
use std::sync::Arc;

use bitflags::bitflags;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::{Color4f, PMColor4f};
use skia_rust_core::color_data::PM_COLOR4F_BLACK;
use skia_rust_core::color_space_priv::srgb_singleton;
use skia_rust_core::color_space_xform_steps::ColorSpaceXformSteps;
use skia_rust_core::image_info::ColorInfo;
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::RuntimeEffect;
use skia_rust_core::runtime_effect_priv;

use crate::graphite::caps::Caps;
use crate::graphite::paint_params_key::PaintParamsKeyBuilder;
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::recorder::Recorder;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::shader_code_dictionary::ShaderCodeDictionary;
use crate::graphite::texture_format::TextureFormat;

bitflags! {
    /// Flags that change how a paint's effects are keyed (`KeyGenFlags`).
    // Port of: src/gpu/graphite/KeyContext.h#L33-L52 (chrome/m156)
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct KeyGenFlags: u8 {
        /// `kDefault`.
        const DEFAULT = 0x0;
        /// By default, linear sampling can be optimized to nearest when it's visually
        /// equivalent. This flag disables this behavior (`kDisableSamplingOptimization`).
        const DISABLE_SAMPLING_OPTIMIZATION = 0x1;
        /// By default, stages of color space transforms are generalized to minimize pipeline
        /// variations. However, in certain contexts (such as image filters, runtime effects)
        /// that sample an image many times *and* perform up front work to ensure there doesn't
        /// need to be any color conversion, or the working color space/format effects that
        /// spread out the stages around other effects, then skipping color space conversion in
        /// the shader produces meaningful performance improvements
        /// (`kSpecializeColorSpaceXform`).
        const SPECIALIZE_COLOR_SPACE_XFORM = 0x2;
        /// By default, alpha-only image shaders are colorized by the paint's color. In the
        /// context of a runtime effect this is disabled
        /// (`kDisableAlphaOnlyImageColorization`).
        const DISABLE_ALPHA_ONLY_IMAGE_COLORIZATION = 0x4;
        /// By default, key generation maintains the requested blend mode; if this flag is added
        /// it is a hint to keep the final blend as kSrc (so that either the draw or its
        /// corresponding inner fill benefit from disabling HW blending)
        /// (`kPreferFixedSrcBlend`).
        const PREFER_FIXED_SRC_BLEND = 0x8;
    }
}

/// Transforms `src_color` from sRGB to the destination's color space, leaving it unpremultiplied
/// (`PaintParams::Color4fPrepForDst`).
// Port of: src/gpu/graphite/PaintParams.cpp#L215-L223 (chrome/m156)
#[doc(alias = "Color4fPrepForDst")]
#[must_use]
pub fn color4f_prep_for_dst(src_color: Color4f, dst_color_info: &ColorInfo) -> Color4f {
    // xform from sRGB to the destination colorspace
    let steps = ColorSpaceXformSteps::new(
        Some(srgb_singleton()),
        AlphaType::Unpremul,
        dst_color_info.color_space_ref(),
        AlphaType::Unpremul,
    );

    let mut result = src_color;
    let mut vec = result.as_array();
    steps.apply(&mut vec);
    result = Color4f::new(vec[0], vec[1], vec[2], vec[3]);
    result
}

/// Runtime effects always disable paint-color colorization of alpha-only image shaders.
// Port of: src/gpu/graphite/KeyContext.cpp#L81-L82 (chrome/m156)
const RUNTIME_EFFECT_CHILD_DEFAULT_FLAGS: KeyGenFlags =
    KeyGenFlags::DISABLE_ALPHA_ONLY_IMAGE_COLORIZATION;

/// The state that key generation reads and writes while walking a paint's effects.
// Port of: src/gpu/graphite/KeyContext.h#L58-L152 (chrome/m156)
#[doc(alias = "skgpu::graphite::KeyContext")]
#[derive(Clone)]
pub struct KeyContext<'a> {
    // Fields which will not change over the course of building a paint key
    caps: Arc<dyn Caps>,
    recorder: Option<&'a Recorder>,
    target_format: Option<TextureFormat>,
    paint_params_key_builder: &'a RefCell<PaintParamsKeyBuilder>,
    pipeline_data_gatherer: &'a RefCell<PipelineDataGatherer>,
    dictionary: ShaderCodeDictionary,
    rt_effect_dict: Arc<RuntimeEffectDictionary>,
    local2dev: M44,
    clip_draw_bounds: Rect,

    // Fields that can be modified while walking a paint's effects for a key
    local_matrix: Option<Matrix>,
    dst_color_info: ColorInfo,
    // Although stored as premul the paint color is actually comprised of an opaque RGB portion
    // and a separate alpha portion. The two portions will never be used together but are stored
    // together to reduce the number of uniforms.
    paint_color: PMColor4f,
    key_gen_flags: KeyGenFlags,
}

impl std::fmt::Debug for KeyContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyContext")
            .field("target_format", &self.target_format)
            .field("local2dev", &self.local2dev)
            .field("clip_draw_bounds", &self.clip_draw_bounds)
            .field("local_matrix", &self.local_matrix)
            .field("paint_color", &self.paint_color)
            .field("key_gen_flags", &self.key_gen_flags)
            .finish_non_exhaustive()
    }
}

impl<'a> KeyContext<'a> {
    /// Constructor for the pre-compile code path (i.e., no `Recorder`).
    // Port of: src/gpu/graphite/KeyContext.cpp#L19-L31 (chrome/m156)
    #[must_use]
    pub fn new(
        caps: Arc<dyn Caps>,
        paint_params_key_builder: &'a RefCell<PaintParamsKeyBuilder>,
        pipeline_data_gatherer: &'a RefCell<PipelineDataGatherer>,
        dict: &ShaderCodeDictionary,
        rt_effect_dict: Arc<RuntimeEffectDictionary>,
        dst_color_info: &ColorInfo,
    ) -> Self {
        Self {
            caps,
            recorder: None,
            target_format: None,
            paint_params_key_builder,
            pipeline_data_gatherer,
            dictionary: dict.clone(),
            rt_effect_dict,
            local2dev: M44::default(),
            clip_draw_bounds: Rect::default(),
            local_matrix: None,
            dst_color_info: dst_color_info.clone(),
            paint_color: PM_COLOR4F_BLACK,
            key_gen_flags: KeyGenFlags::DEFAULT,
        }
    }

    /// Constructor for the `ExtractPaintData` code path (i.e., with a `Recorder`).
    ///
    /// `target_format` is the format of the texture the draw renders to, which Skia reads from the
    /// `DrawContext` it takes here (see the module docs).
    // Port of: src/gpu/graphite/KeyContext.cpp#L33-L56 (chrome/m156)
    #[must_use]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ constructor
    pub fn new_with_recorder(
        recorder: &'a Recorder,
        target_format: TextureFormat,
        paint_params_key_builder: &'a RefCell<PaintParamsKeyBuilder>,
        pipeline_data_gatherer: &'a RefCell<PipelineDataGatherer>,
        local2dev: &M44,
        clip_draw_bounds: &Rect,
        dst_color_info: &ColorInfo,
        initial_flags: KeyGenFlags,
        paint_color: &Color4f,
    ) -> Self {
        let priv_ = recorder.priv_();
        let mut context = Self {
            caps: priv_.caps().clone(),
            recorder: Some(recorder),
            target_format: Some(target_format),
            paint_params_key_builder,
            pipeline_data_gatherer,
            dictionary: priv_.shader_code_dictionary().clone(),
            rt_effect_dict: priv_.runtime_effect_dictionary(),
            local2dev: *local2dev,
            clip_draw_bounds: *clip_draw_bounds,
            local_matrix: None,
            dst_color_info: dst_color_info.clone(),
            paint_color: PM_COLOR4F_BLACK,
            key_gen_flags: initial_flags,
        };
        context.paint_color = color4f_prep_for_dst(*paint_color, &context.dst_color_info)
            .to_opaque()
            .premul();
        context.paint_color.a = paint_color.a;
        context
    }

    /// `KeyContext(const KeyContext&, xtraFlags)`: a copy with `xtra_flags` added to its flags.
    // Port of: src/gpu/graphite/KeyContext.cpp#L58-L72 (chrome/m156)
    #[must_use]
    pub fn with_extra_flags(&self, xtra_flags: KeyGenFlags) -> Self {
        let mut other = self.clone();
        other.key_gen_flags = self.key_gen_flags | xtra_flags;
        other
    }

    /// Create scoped `KeyContext`s that allow child effects to be processed differently
    /// (`withColorInfo`).
    // Port of: src/gpu/graphite/KeyContext.h#L85-L97 (chrome/m156)
    #[doc(alias = "withColorInfo")]
    #[must_use]
    #[allow(clippy::float_cmp)] // the transform leaves the alpha channel exactly alone
    pub fn with_color_info(&self, info: &ColorInfo) -> Self {
        let mut o = self.clone();
        o.dst_color_info = info.clone();

        // We want to keep fPaintColor's alpha value but replace the RGB with values in the new
        // color space. By overriding the alpha type of the old and new dst color infos to be
        // kOpaque, SkColorSpaceXformSteps will leave the alpha channel alone.
        let steps = ColorSpaceXformSteps::new(
            self.dst_color_info.color_space_ref(),
            AlphaType::Opaque,
            info.color_space_ref(),
            AlphaType::Opaque,
        );
        let mut vec = o.paint_color.as_array();
        steps.apply(&mut vec);
        o.paint_color = PMColor4f::new(vec[0], vec[1], vec[2], vec[3]);
        debug_assert_eq!(o.paint_color.a, self.paint_color.a);
        o
    }

    /// The `KeyContextWithLocalMatrix` of Skia: a copy whose local matrix is `child_lm`
    /// concatenated with this context's, if it has one.
    // Port of: src/gpu/graphite/KeyContext.h#L154-L172 (chrome/m156)
    #[doc(alias = "KeyContextWithLocalMatrix")]
    #[must_use]
    pub fn with_local_matrix(&self, child_lm: &Matrix) -> Self {
        let mut o = self.clone();
        o.local_matrix = Some(match &self.local_matrix {
            Some(local_matrix) => Matrix::concat(child_lm, local_matrix),
            None => child_lm.clone(),
        });
        o
    }

    /// The key generation flags vary in the scope of a `SkRuntimeEffect` per child based on how
    /// the `RuntimeEffect`'s `SkSL` invokes each child (`forRuntimeEffect`).
    // Port of: src/gpu/graphite/KeyContext.cpp#L84-L96 (chrome/m156)
    #[doc(alias = "forRuntimeEffect")]
    #[must_use]
    pub fn for_runtime_effect(&self, effect: &RuntimeEffect, child: usize) -> Self {
        let mut xtra_flags = RUNTIME_EFFECT_CHILD_DEFAULT_FLAGS;

        if runtime_effect_priv::child_sample_usage(effect, child).is_explicit() {
            // Assume explicit sampling as a proxy for either a likely data lookup (e.g. raw
            // shader) or an effect that might sample the child many times. This means it's
            // worth using eliding colorspace conversions, and we have to disable sampling
            // optimization.
            xtra_flags |= KeyGenFlags::SPECIALIZE_COLOR_SPACE_XFORM
                | KeyGenFlags::DISABLE_SAMPLING_OPTIMIZATION;
        }

        self.with_extra_flags(xtra_flags)
    }

    /// `forMeshSpecChild`.
    // Port of: src/gpu/graphite/KeyContext.cpp#L98-L101 (chrome/m156)
    #[doc(alias = "forMeshSpecChild")]
    #[must_use]
    pub fn for_mesh_spec_child(&self) -> Self {
        self.with_extra_flags(
            RUNTIME_EFFECT_CHILD_DEFAULT_FLAGS | KeyGenFlags::DISABLE_SAMPLING_OPTIMIZATION,
        )
    }

    /// `recorder()`: `None` on the pre-compile code path.
    #[must_use]
    pub fn recorder(&self) -> Option<&'a Recorder> {
        self.recorder
    }

    /// `caps()`.
    #[must_use]
    pub fn caps(&self) -> &dyn Caps {
        &*self.caps
    }

    /// `local2Dev()`.
    #[doc(alias = "local2Dev")]
    #[must_use]
    pub fn local2dev(&self) -> &M44 {
        &self.local2dev
    }

    /// `localMatrix()`.
    #[doc(alias = "localMatrix")]
    #[must_use]
    pub fn local_matrix(&self) -> Option<&Matrix> {
        self.local_matrix.as_ref()
    }

    /// `paintParamsKeyBuilder()`.
    #[doc(alias = "paintParamsKeyBuilder")]
    #[must_use]
    pub fn paint_params_key_builder(&self) -> &'a RefCell<PaintParamsKeyBuilder> {
        self.paint_params_key_builder
    }

    /// `pipelineDataGatherer()`.
    #[doc(alias = "pipelineDataGatherer")]
    #[must_use]
    pub fn pipeline_data_gatherer(&self) -> &'a RefCell<PipelineDataGatherer> {
        self.pipeline_data_gatherer
    }

    /// `dict()`.
    #[must_use]
    pub fn dict(&self) -> &ShaderCodeDictionary {
        &self.dictionary
    }

    /// `targetFormat()`: the format of the draw's target texture.
    ///
    /// # Panics
    /// If the context was made for the pre-compile code path, which has no target.
    #[doc(alias = "targetFormat")]
    #[must_use]
    pub fn target_format(&self) -> TextureFormat {
        self.target_format
            .expect("only a context with a recorder has a target")
    }

    /// `rtEffectDict()`.
    #[doc(alias = "rtEffectDict")]
    #[must_use]
    pub fn rt_effect_dict(&self) -> Arc<RuntimeEffectDictionary> {
        self.rt_effect_dict.clone()
    }

    /// `dstColorInfo()`.
    #[doc(alias = "dstColorInfo")]
    #[must_use]
    pub fn dst_color_info(&self) -> &ColorInfo {
        &self.dst_color_info
    }

    /// `paintColor()`.
    #[doc(alias = "paintColor")]
    #[must_use]
    pub fn paint_color(&self) -> &PMColor4f {
        &self.paint_color
    }

    /// `flags()`.
    #[must_use]
    pub fn flags(&self) -> KeyGenFlags {
        self.key_gen_flags
    }

    /// `clipDrawBounds()`.
    #[doc(alias = "clipDrawBounds")]
    #[must_use]
    pub fn clip_draw_bounds(&self) -> &Rect {
        &self.clip_draw_bounds
    }
}
