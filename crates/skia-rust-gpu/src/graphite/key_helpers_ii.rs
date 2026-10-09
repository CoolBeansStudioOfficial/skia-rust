// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/KeyHelpers.{h,cpp} (the color filter, blender, runtime
// effect, color space transform and primitive color parts, KeyHelpers.cpp#L928-L1480 and
// KeyHelpers.cpp#L1500-L2784 minus the shader dispatch)

//! The `KeyHelpers` blocks that build the `PaintParamsKey` of blenders and color filters: the
//! color filter blocks (matrix, color space transform, compose, working format), the blender
//! blocks (fixed, Porter-Duff, HSLC), the runtime effect blocks and their children, primitive
//! color, and `AddToKey` for the core color filters and blenders.
//!
//! Each block writes its uniforms through the gatherer in the order and types of Skia's
//! `BEGIN_WRITE_UNIFORMS` scope, then appends its snippet to the key builder.
//!
//! Not ported here, with the reason:
//!
//! - `AddToKey(SkShader*)` is `KeyHelpers` I (G5b, `key_helpers::add_to_key_shader`). Runtime effect
//!   children and mesh children that are shaders go through it via `add_child_shader_to_key`.
//! - `TableColorFilterBlock` and `SkTableColorFilter`: Skia creates the table's texture with
//!   `RecorderPriv::CreateCachedProxy` and binds it through the texture half of
//!   `PipelineDataGatherer`, neither of which is ported (G8/G9a and the texture half of G3). The
//!   table arm of [`add_to_key_color_filter`] panics until then.
//! - `AddAnalyticClip` and the `NonMSAAClip` data: `NonMSAAClip` is G10b's `ClipStack`.
//! - `AddDitherBlock`: needs `CreateCachedProxy` for the dither LUT (`KeyHelpers` I, with the
//!   dither shader).
//! - `SolidColorShaderBlock` is `KeyHelpers` I (G5b), which delegates to `solid_color_shader_add_block`
//!   here: one definition serves both.
//! - `ScopedUniformWriter` is defined once, here, and `KeyHelpers` I uses it too.
//!
//! Deviations from the C++:
//!
//! - Skia's `SkSpan`-based coefficient parameters are `&[f32]`.
//! - `ColorSpaceTransformData`'s `SkColorSpace*` pointers are `Option<&ColorSpace>`.
//! - `SkBlenderBase`/`SkColorFilterBase` dispatch on the type tag and then downcasts the
//!   `dyn` object (through `Any`) to the concrete type, as `static_cast` does in C++.
//! - The `BlendModeColorFilter` accessors `color()` and `mode()` were added to `skia-rust-core`
//!   for this port.

use std::any::Any;
use std::cell::RefMut;
use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blend_mode_blender::BlendModeBlender;
use skia_rust_core::blender::{Blender, BlenderType};
use skia_rust_core::color::{Color4f, PMColor4f};
use skia_rust_core::color_filter::{ColorFilter, ColorFilterType};
use skia_rust_core::color_filters::BlendModeColorFilter;
use skia_rust_core::color_filters::Clamp;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_space_priv::{srgb_linear_singleton, srgb_singleton};
use skia_rust_core::color_space_xform_color_filter::ColorSpaceXformColorFilter;
use skia_rust_core::color_space_xform_steps::ColorSpaceXformSteps;
use skia_rust_core::compose_color_filter::ComposeColorFilter;
use skia_rust_core::data::Data;
use skia_rust_core::image_info::ColorInfo;
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::matrix_color_filter::{Domain, MatrixColorFilter};
use skia_rust_core::mesh::MeshSpecification;
use skia_rust_core::runtime_blender::RuntimeBlender;
use skia_rust_core::runtime_color_filter::RuntimeColorFilter;
use skia_rust_core::runtime_effect::{ChildPtr, ChildType, RuntimeEffect};
use skia_rust_core::runtime_effect_priv;
use skia_rust_core::swizzle::Swizzle;
use skia_rust_core::working_format_color_filter::WorkingFormatColorFilter;
use skia_rust_skcms::{TfType, TransferFunction};

use crate::gpu::blend::{get_porter_duff_blend_constants, get_reduced_blend_mode_info};
use crate::graphite::built_in_code_snippet_id::{BuiltInCodeSnippetID, FIXED_BLEND_ID_OFFSET};
use crate::graphite::key_context::{KeyContext, KeyGenFlags};
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::uniform::Uniform;
use crate::graphite::uniform_manager::UniformManager;

/// `ScopedUniformWriter`: begins the uniform struct of a snippet (when it has one) and registers
/// the snippet's uniforms as the expected ones in debug builds. Both end when it is dropped.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L93-L121 (chrome/m156)
pub(crate) struct ScopedUniformWriter<'a> {
    gatherer: RefMut<'a, PipelineDataGatherer>,
    is_struct: bool,
}

impl<'a> ScopedUniformWriter<'a> {
    /// Opens the uniform scope of the built-in snippet `snippet_id`.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L93-L121 (chrome/m156)
    pub(crate) fn new(key_context: &KeyContext<'a>, snippet_id: BuiltInCodeSnippetID) -> Self {
        let snippet = key_context.dict().get_entry_built_in(snippet_id);
        let is_struct = snippet.uniform_struct_name.is_some();
        let mut gatherer = key_context.pipeline_data_gatherer().borrow_mut();
        #[cfg(debug_assertions)]
        gatherer
            .uniform_manager()
            .set_expected_uniforms(&snippet.uniforms, is_struct);
        if is_struct {
            gatherer
                .uniform_manager()
                .begin_struct(snippet.required_alignment);
        }
        Self {
            gatherer,
            is_struct,
        }
    }

    /// The uniform manager the snippet's uniforms are written through.
    pub(crate) fn uniforms(&mut self) -> &mut UniformManager {
        self.gatherer.uniform_manager()
    }
}

impl Drop for ScopedUniformWriter<'_> {
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L93-L121 (chrome/m156), and the expectations
    // validator's destructor (src/gpu/graphite/PipelineData.h#L469-L471)
    fn drop(&mut self) {
        if self.is_struct {
            self.gatherer.uniform_manager().end_struct();
        }
        #[cfg(debug_assertions)]
        self.gatherer
            .uniform_manager()
            .done_with_expected_uniforms();
    }
}

/// `SkBlenderBase::BlenderType`-independent helper: the snippet id of a built-in or user-defined
/// snippet from the `int` the dictionary returns (never negative here).
fn snippet_id_u32(code_snippet_id: i32) -> u32 {
    debug_assert!(code_snippet_id >= 0);
    // The caller has ruled out negative ids (`findOrCreate` returns -1 on failure).
    #[allow(clippy::cast_sign_loss)]
    let id = code_snippet_id as u32;
    id
}

/// The built-in snippet at the `int` id `id` (a fixed blend id is always one).
fn built_in_from_i32(id: i32) -> BuiltInCodeSnippetID {
    BuiltInCodeSnippetID::from_u32(snippet_id_u32(id)).expect("a built-in snippet id")
}

// Port of: src/gpu/graphite/KeyHelpers.cpp#L126-L136 (chrome/m156), `add_solid_uniform_data` and
// `SolidColorShaderBlock::AddBlock` (KeyHelpers I's; repeated here, see the module docs)
pub(crate) fn solid_color_shader_add_block(key_context: &KeyContext<'_>, premul_color: &PMColor4f) {
    {
        let mut scope =
            ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::SolidColorShader);
        scope.uniforms().write_color(premul_color);
    }
    key_context
        .paint_params_key_builder()
        .borrow_mut()
        .add_block(BuiltInCodeSnippetID::SolidColorShader);
}

/// `blend(...)`: a Porter-Duff or fixed blend whose src and dst are chained as a
/// `BlendCompose` block.
// Port of: src/gpu/graphite/KeyHelpers.h#L446-L460 (chrome/m156)
#[doc(alias = "Blend")]
pub fn blend(
    key_context: &KeyContext<'_>,
    add_blend_to_key: impl FnOnce(),
    add_src_to_key: impl FnOnce(),
    add_dst_to_key: impl FnOnce(),
) {
    BlendComposeBlock::begin_block(key_context);

    add_src_to_key();

    add_dst_to_key();

    add_blend_to_key();

    key_context
        .paint_params_key_builder()
        .borrow_mut()
        .end_block(); // BlendComposeBlock
}

/// `compose(...)`: the inner and outer effects chained in a `Compose` block.
// Port of: src/gpu/graphite/KeyHelpers.h#L462-L476 (chrome/m156)
#[doc(alias = "Compose")]
pub fn compose(
    key_context: &KeyContext<'_>,
    add_inner_to_key: impl FnOnce(),
    add_outer_to_key: impl FnOnce(),
) {
    ComposeBlock::begin_block(key_context);

    add_inner_to_key();

    add_outer_to_key();

    key_context
        .paint_params_key_builder()
        .borrow_mut()
        .end_block(); // ComposeBlock
}

//--------------------------------------------------------------------------------------------------

/// `BlendComposeBlock`: opens the `BlendCompose` block that `blend` wraps its children in.
// Port of: src/gpu/graphite/KeyHelpers.h#L252-L254 and KeyHelpers.cpp#L931-L936 (chrome/m156)
#[derive(Debug)]
pub struct BlendComposeBlock;

impl BlendComposeBlock {
    /// `BeginBlock`.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L931-L936 (chrome/m156)
    pub fn begin_block(key_context: &KeyContext<'_>) {
        {
            let _scope = ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::BlendCompose);
        }

        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .begin_block(BuiltInCodeSnippetID::BlendCompose);
    }
}

/// `PorterDuffBlenderBlock`: a blend whose four coefficients are the uniforms.
// Port of: src/gpu/graphite/KeyHelpers.h#L256-L258 and KeyHelpers.cpp#L939-L946 (chrome/m156)
#[derive(Debug)]
pub struct PorterDuffBlenderBlock;

impl PorterDuffBlenderBlock {
    /// `AddBlock`.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L939-L946 (chrome/m156)
    pub fn add_block(key_context: &KeyContext<'_>, coeffs: &[f32]) {
        {
            let mut scope =
                ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::PorterDuffBlender);
            debug_assert_eq!(coeffs.len(), 4);
            scope
                .uniforms()
                .write_half_vec([coeffs[0], coeffs[1], coeffs[2], coeffs[3]]);
        }

        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .add_block(BuiltInCodeSnippetID::PorterDuffBlender);
    }
}

/// `HSLCBlenderBlock`: one of the four non-separable (HSL) blend modes, with its two coefficients
/// as uniforms.
// Port of: src/gpu/graphite/KeyHelpers.h#L260-L262 and KeyHelpers.cpp#L949-L956 (chrome/m156)
#[derive(Debug)]
pub struct HSLCBlenderBlock;

impl HSLCBlenderBlock {
    /// `AddBlock`.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L949-L956 (chrome/m156)
    pub fn add_block(key_context: &KeyContext<'_>, coeffs: &[f32]) {
        {
            let mut scope =
                ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::HSLCBlender);
            debug_assert_eq!(coeffs.len(), 2);
            scope.uniforms().write_half_vec([coeffs[0], coeffs[1]]);
        }

        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .add_block(BuiltInCodeSnippetID::HSLCBlender);
    }
}

/// `ComposeBlock`: opens the `Compose` block that `compose` wraps its children in.
// Port of: src/gpu/graphite/KeyHelpers.h#L264-L266 and KeyHelpers.cpp#L959-L962 (chrome/m156)
#[derive(Debug)]
pub struct ComposeBlock;

impl ComposeBlock {
    /// `BeginBlock`.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L959-L962 (chrome/m156)
    pub fn begin_block(key_context: &KeyContext<'_>) {
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .begin_block(BuiltInCodeSnippetID::Compose);
    }
}

//--------------------------------------------------------------------------------------------------

/// The data of a matrix color filter: the 4x4 part, the translation and the two flags.
// Port of: src/gpu/graphite/KeyHelpers.h#L269-L285 (chrome/m156), `MatrixColorFilterData`
#[derive(Clone, Debug)]
pub struct MatrixColorFilterData {
    /// `fMatrix`.
    pub matrix: M44,
    /// `fTranslate`.
    pub translate: [f32; 4],
    /// `fInHSLA`.
    pub in_hsla: bool,
    /// `fClamp`.
    pub clamp: bool,
}

impl MatrixColorFilterData {
    /// `MatrixColorFilterData(const float matrix[20], bool inHSLA, bool clamp)`: the 4x4 part is
    /// row-major in `matrix[0..20]` with the translation in the fifth column.
    // Port of: src/gpu/graphite/KeyHelpers.h#L269-L285 (chrome/m156)
    #[must_use]
    pub fn new(matrix: &[f32; 20], in_hsla: bool, clamp: bool) -> Self {
        Self {
            matrix: M44::row_major(&[
                matrix[0], matrix[1], matrix[2], matrix[3], //
                matrix[5], matrix[6], matrix[7], matrix[8], //
                matrix[10], matrix[11], matrix[12], matrix[13], //
                matrix[15], matrix[16], matrix[17], matrix[18],
            ]),
            translate: [matrix[4], matrix[9], matrix[14], matrix[19]],
            in_hsla,
            clamp,
        }
    }
}

/// `MatrixColorFilterBlock`: a 4x4 matrix and translation, in RGBA or HSLA.
// Port of: src/gpu/graphite/KeyHelpers.h#L269-L285 (chrome/m156)
#[derive(Debug)]
pub struct MatrixColorFilterBlock;

impl MatrixColorFilterBlock {
    /// `AddBlock`.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L996-L1007 (chrome/m156)
    pub fn add_block(key_context: &KeyContext<'_>, matrix_cf_data: &MatrixColorFilterData) {
        if matrix_cf_data.in_hsla {
            {
                let mut scope = ScopedUniformWriter::new(
                    key_context,
                    BuiltInCodeSnippetID::HSLMatrixColorFilter,
                );
                let uniforms = scope.uniforms();
                uniforms.write_half_m44(&matrix_cf_data.matrix);
                uniforms.write_half_vec(matrix_cf_data.translate);
            }

            key_context
                .paint_params_key_builder()
                .borrow_mut()
                .add_block(BuiltInCodeSnippetID::HSLMatrixColorFilter);
        } else {
            {
                let mut scope =
                    ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::MatrixColorFilter);
                let uniforms = scope.uniforms();
                uniforms.write_half_m44(&matrix_cf_data.matrix);
                uniforms.write_half_vec(matrix_cf_data.translate);
                if matrix_cf_data.clamp {
                    uniforms.write_half_vec([0.0, 1.0]);
                } else {
                    // Alpha is always clamped to 1. RGB clamp to the max finite half value.
                    const K_UNCLAMPED: f32 = 65504.0; // SK_HalfMax converted back to float
                    uniforms.write_half_vec([-K_UNCLAMPED, K_UNCLAMPED]);
                }
            }

            key_context
                .paint_params_key_builder()
                .borrow_mut()
                .add_block(BuiltInCodeSnippetID::MatrixColorFilter);
        }
    }
}

//--------------------------------------------------------------------------------------------------

/// The data of a color space transform: the steps, the read swizzle and whether the input is
/// alpha-only.
// Port of: src/gpu/graphite/KeyHelpers.h#L300-L313 (chrome/m156), `ColorSpaceTransformData`
#[derive(Clone, Debug)]
pub struct ColorSpaceTransformData {
    /// `fSteps`.
    pub steps: ColorSpaceXformSteps,
    /// `fReadSwizzle`.
    pub read_swizzle: Swizzle,
    /// `fIsAlphaOnly`.
    pub is_alpha_only: bool,
}

impl ColorSpaceTransformData {
    /// `ColorSpaceTransformData(src, srcAT, dst, dstAT)`: the steps from `src` to `dst`.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L1171-L1174 (chrome/m156)
    #[must_use]
    pub fn from_color_spaces(
        src: Option<&ColorSpace>,
        src_at: AlphaType,
        dst: Option<&ColorSpace>,
        dst_at: AlphaType,
    ) -> Self {
        Self::from_steps(ColorSpaceXformSteps::new(src, src_at, dst, dst_at))
    }

    /// `ColorSpaceTransformData(const SkColorSpaceXformSteps&)`.
    // Port of: src/gpu/graphite/KeyHelpers.h#L300-L313 (chrome/m156)
    #[must_use]
    pub fn from_steps(steps: ColorSpaceXformSteps) -> Self {
        Self {
            steps,
            read_swizzle: Swizzle::new("rgba"),
            is_alpha_only: false,
        }
    }

    /// `ColorSpaceTransformData(Swizzle)`: a read swizzle with no color space change.
    // Port of: src/gpu/graphite/KeyHelpers.h#L300-L313 (chrome/m156)
    #[must_use]
    pub fn from_swizzle(swizzle: Swizzle) -> Self {
        // (The C++ asserts that the default steps have no effect, `fFlags.mask() == 0`.)
        Self {
            steps: ColorSpaceXformSteps::default(),
            read_swizzle: swizzle,
            is_alpha_only: false,
        }
    }
}

/// The identity gamut transform, as the 3x3 matrix Skia's `kIdentityGamut` lists it.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1270 (chrome/m156), `kIdentityGamut`
const IDENTITY_GAMUT: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

/// `kLinearTF`: the transfer function of `SkNamedTransferFn::kLinear`.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1037 (chrome/m156)
const LINEAR_TF: TransferFunction = TransferFunction {
    g: 1.0,
    a: 1.0,
    b: 0.0,
    c: 0.0,
    d: 0.0,
    e: 0.0,
    f: 0.0,
};

/// `memcmp(&xferFn, &kLinearTF, sizeof(skcms_TransferFunction)) == 0`: the bits of all seven
/// parameters match, so `-0.0` is not `0.0`.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1080-L1083 (chrome/m156)
fn is_linear_tf(tf: &TransferFunction) -> bool {
    let fields = |t: &TransferFunction| [t.g, t.a, t.b, t.c, t.d, t.e, t.f].map(f32::to_bits);
    fields(tf) == fields(&LINEAR_TF)
}

/// The swizzle's characters as bytes, `"rgba"` style.
fn swizzle_bytes(swizzle: Swizzle) -> [u8; 4] {
    let s = swizzle.as_string();
    let b = s.as_bytes();
    [b[0], b[1], b[2], b[3]]
}

/// `swizzle_ootf`: the OOTF's parameters, reordered by the read swizzle. A `w` of 0 disables
/// applying the OOTF.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1039-L1061 (chrome/m156)
fn swizzle_ootf(ootf_swizzle: Swizzle, ootf: Option<&[f32; 4]>) -> [f32; 4] {
    let mut encoded_ootf = [0.0_f32; 4]; // A W of 0 disables applying the ootf
    if let Some(ootf) = ootf {
        // If the OOTF is applied before the gamut transform, the inverse of the read swizzle needs
        // to be included so the channels are scaled correctly.
        encoded_ootf[3] = ootf[3];
        let sw = swizzle_bytes(ootf_swizzle);
        for (i, &channel) in sw.iter().take(3).enumerate() {
            match channel {
                b'r' => encoded_ootf[i] += ootf[0],
                b'g' => encoded_ootf[i] += ootf[1],
                b'b' => encoded_ootf[i] += ootf[2],
                b'0' => {} // leave as 0
                // Unexpected swizzles for RGB channels as these are used for alpha handling
                _ => {
                    debug_assert!(channel != b'a' && channel != b'1');
                }
            }
        }
    }
    encoded_ootf
}

/// Adds the uniforms and snippet of one transfer function stage, and returns its snippet id.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1064-L1116 (chrome/m156), `add_xfer_fn`
fn add_xfer_fn(
    key_context: &KeyContext<'_>,
    xfer_fn: &TransferFunction,
    ootf_swizzle: Swizzle,
    ootf: Option<&[f32; 4]>,
) -> BuiltInCodeSnippetID {
    match xfer_fn.tf_type() {
        TfType::SRGBish => {
            debug_assert!(ootf.is_none());
            // Actual sRGB gamma curve to apply
            let mut scope =
                ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::CSXformsRGB);
            // sk_csxform_srgb skips all work when g == 0, so prefer that for an identity step
            // (vs. calling $apply_srgb_xfer_fn with values resulting in the identity).
            let g = if is_linear_tf(xfer_fn) {
                0.0
            } else {
                xfer_fn.g
            };
            scope
                .uniforms()
                .write_vec([g, xfer_fn.a, xfer_fn.b, xfer_fn.c]);
            scope
                .uniforms()
                .write_vec([xfer_fn.d, xfer_fn.e, xfer_fn.f]);
            BuiltInCodeSnippetID::CSXformsRGB
        }

        TfType::PQ | TfType::PQish => {
            debug_assert!(ootf.is_none());
            let mut scope = ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::CSXformPQ);
            scope
                .uniforms()
                .write_vec([xfer_fn.a, xfer_fn.b, xfer_fn.c]);
            scope
                .uniforms()
                .write_vec([xfer_fn.d, xfer_fn.e, xfer_fn.f]);
            BuiltInCodeSnippetID::CSXformPQ
        }

        TfType::HLG | TfType::HLGish => {
            let mut scope = ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::CSXformHLG);
            scope.uniforms().write_vec(swizzle_ootf(ootf_swizzle, ootf));
            scope
                .uniforms()
                .write_vec([xfer_fn.a, xfer_fn.b, xfer_fn.c]);
            scope
                .uniforms()
                .write_vec([xfer_fn.d, xfer_fn.e, xfer_fn.f]);
            BuiltInCodeSnippetID::CSXformHLG
        }

        TfType::HLGinvish => {
            let mut scope =
                ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::CSXformHLGInv);
            scope.uniforms().write_vec(swizzle_ootf(ootf_swizzle, ootf));
            // skcms has already updated the inverse HLG coefficients to be 1/X except for `f`;
            // it assumes it will be evaluated as 1 / (f + 1), but that can be precomputed here.
            let f = 1.0 / (xfer_fn.f + 1.0);
            scope
                .uniforms()
                .write_vec([xfer_fn.a, xfer_fn.b, xfer_fn.c]);
            scope.uniforms().write_vec([xfer_fn.d, xfer_fn.e, f]);
            BuiltInCodeSnippetID::CSXformHLGInv
        }

        TfType::Invalid => unreachable!("skcms transfer functions are classified"),
    }
}

/// The 3x3 gamut transform reordered so that its columns follow the read swizzle.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1118-L1161 (chrome/m156), `swizzle_gamut_transform`
fn swizzle_gamut_transform(gamut: &[f32; 9], read_swizzle: Swizzle) -> Matrix {
    let mut gamut_transform = [0.0_f32; 9];

    // Accumulate each column of the gamut transform based on the swizzle
    let sw = swizzle_bytes(read_swizzle);
    for (i, &channel) in sw.iter().take(3).enumerate() {
        // The ith column of the gamut transform contributes to the jth column when we're
        // transforming a sample value that is in the readSwizzle channel order instead of RGB
        // order. This is a sum so that 0 swizzles and replicated channels accumulate correctly.
        let ci = 3 * i;
        match channel {
            // j = 0
            b'r' => {
                gamut_transform[0] += gamut[ci];
                gamut_transform[1] += gamut[ci + 1];
                gamut_transform[2] += gamut[ci + 2];
            }
            // j = 1
            b'g' => {
                gamut_transform[3] += gamut[ci];
                gamut_transform[4] += gamut[ci + 1];
                gamut_transform[5] += gamut[ci + 2];
            }
            // j = 2
            b'b' => {
                gamut_transform[6] += gamut[ci];
                gamut_transform[7] += gamut[ci + 1];
                gamut_transform[8] += gamut[ci + 2];
            }
            // No contribution to the final gamut matrix
            b'0' => {}
            // Unexpected swizzle components for RGB as these are for alpha handling.
            _ => {
                debug_assert!(channel != b'a' && channel != b'1');
            }
        }
    }

    // SkMatrix::MakeAll takes the rows; the accumulated columns are transposed into them.
    Matrix::new_all(
        gamut_transform[0],
        gamut_transform[3],
        gamut_transform[6],
        gamut_transform[1],
        gamut_transform[4],
        gamut_transform[7],
        gamut_transform[2],
        gamut_transform[5],
        gamut_transform[8],
    )
}

/// `ColorSpaceTransformBlock`: a sequence of stages (pre-alpha, linear transfer function, gamut,
/// encode transfer function, post-alpha) chained with `Compose` blocks.
// Port of: src/gpu/graphite/KeyHelpers.h#L300-L313 and KeyHelpers.cpp#L1171-L1315 (chrome/m156)
#[derive(Debug)]
pub struct ColorSpaceTransformBlock;

impl ColorSpaceTransformBlock {
    /// `AddBlock`.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L1171-L1315 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the structure of the C++ function
    pub fn add_block(key_context: &KeyContext<'_>, data: &ColorSpaceTransformData) {
        if data.is_alpha_only {
            // We always specialize alpha-only because the rest of the pipeline is specializing the
            // colorization of alpha data.
            debug_assert!(
                data.read_swizzle == Swizzle::new("rgba") || // alpha texture format
                    data.read_swizzle == Swizzle::new("000r") || // red texture format
                    data.read_swizzle == Swizzle::new("000a") // rgba texture format
            );
            {
                let mut scope =
                    ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::CSXformAlphaOnly);
                let r_to_alpha = swizzle_bytes(data.read_swizzle)[3] == b'r';
                scope
                    .uniforms()
                    .write_half(if r_to_alpha { 1.0 } else { 0.0 });
            }
            key_context
                .paint_params_key_builder()
                .borrow_mut()
                .add_block(BuiltInCodeSnippetID::CSXformAlphaOnly);
            return;
        }

        // For color, Graphite applies read swizzles as part of the gamut transform, so it does not
        // support referencing A or 1 in a swizzle component for RGB; it only supports no-swizzle or
        // forcing opaque for alpha.
        let sw = swizzle_bytes(data.read_swizzle);
        debug_assert!(sw[0] != b'a' && sw[0] != b'1');
        debug_assert!(sw[1] != b'a' && sw[1] != b'1');
        debug_assert!(sw[2] != b'a' && sw[2] != b'1');
        debug_assert!(sw[3] == b'a' || sw[3] == b'1');

        let has_rgb_swizzle = sw[0] != b'r' || sw[1] != b'g' || sw[2] != b'b';

        // There are up to 5 stages: pre-alpha, linear xfer fn, gamut, encode xfer fn, post-alpha.
        // NOTE: SkColorSpaceXformSteps also includes src/dst OOTF, but Graphite treats these as
        // optional steps in the HLG transfer function.
        let steps = &data.steps;
        let mut stage_ids: Vec<BuiltInCodeSnippetID> = Vec::with_capacity(5);

        // Each stage can be optimized to exactly what is needed, or remain general to limit the
        // pipeline combinations in simple pipelines (e.g. one image sample can afford the
        // generalization overhead).
        let specialize = key_context
            .flags()
            .contains(KeyGenFlags::SPECIALIZE_COLOR_SPACE_XFORM);

        // When not hyper-specializing, there are 10 allowed semi-specializations for the combined
        // linear + gamut + encode stages: full identity, and TF1 + gamut + TF2 where a TF1 and TF2
        // are restricted to sRGB, PQ, or HLG[Inv]. When not the full identity, an identity linear
        // or encode step is mapped to sRGB.
        let identity_conversion = !steps.flags.linearize
            && !steps.flags.gamut_transform
            && !steps.flags.encode
            && !has_rgb_swizzle;
        let src_tf: Option<TransferFunction> = if steps.flags.linearize {
            Some(steps.src_tf)
        } else if !specialize && !identity_conversion {
            Some(LINEAR_TF)
        } else {
            None
        };
        // The equivalent logic applies for the dst inverse function.
        let dst_tf_inv: Option<TransferFunction> = if steps.flags.encode {
            Some(steps.dst_tf_inv)
        } else if !specialize && !identity_conversion {
            Some(LINEAR_TF)
        } else {
            None
        };

        // Pre-alpha
        if specialize {
            // None of the specialized pre-alpha stages take uniforms
            if sw[3] == b'1' {
                stage_ids.push(BuiltInCodeSnippetID::CSXformForceOpaque);
            } else if steps.flags.unpremul {
                stage_ids.push(BuiltInCodeSnippetID::CSXformUnpremul);
            } // else elide the no-op entirely
        } else {
            stage_ids.push(BuiltInCodeSnippetID::CSXformPreAlpha);
            let mut scope =
                ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::CSXformPreAlpha);
            let inline_premul = identity_conversion && steps.flags.premul;
            let mode = if sw[3] == b'1' {
                2.0 // force-opaque
            } else if steps.flags.unpremul {
                -1.0 // unpremul
            } else if inline_premul {
                1.0 // premul w/o a PostAlpha stage
            } else {
                0.0 // no-op
            };
            scope.uniforms().write_half(mode);
        }

        // Linear transfer function; this is applied before the gamut, so any ootf must be swizzled
        if let Some(src_tf) = src_tf {
            let ootf = if steps.flags.src_ootf {
                Some(&steps.src_ootf)
            } else {
                None
            };
            stage_ids.push(add_xfer_fn(key_context, &src_tf, data.read_swizzle, ootf));
        } // else specialization allows us to elide the transfer function entirely

        // Gamut transform and RGB swizzle. This is specialized when overall specialization is
        // enabled, or if there are no transfer functions at play.
        let specialize_gamut = specialize || identity_conversion;
        if !specialize_gamut || has_rgb_swizzle || steps.flags.gamut_transform {
            stage_ids.push(BuiltInCodeSnippetID::CSXformGamut);
            let mut scope =
                ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::CSXformGamut);

            let gamut_src = if steps.flags.gamut_transform {
                &steps.src_to_dst_matrix
            } else {
                &IDENTITY_GAMUT
            };
            let gamut = swizzle_gamut_transform(gamut_src, data.read_swizzle);
            scope.uniforms().write_half_matrix(&gamut);
        } // else elide the stage entirely

        // Encode transfer function; this is applied after the gamut so the ootf does not need
        // adjusting
        if let Some(dst_tf_inv) = dst_tf_inv {
            let ootf = if steps.flags.dst_ootf {
                Some(&steps.dst_ootf)
            } else {
                None
            };
            stage_ids.push(add_xfer_fn(
                key_context,
                &dst_tf_inv,
                Swizzle::new("rgba"),
                ootf,
            ));
        } // else specialization allows us to elide the transfer function entirely

        // Post-alpha
        if specialize {
            if steps.flags.premul {
                stage_ids.push(BuiltInCodeSnippetID::CSXformPremul);
            } // else elide the no-op
        } else if !identity_conversion {
            stage_ids.push(BuiltInCodeSnippetID::CSXformPostAlpha);
            let mut scope =
                ScopedUniformWriter::new(key_context, BuiltInCodeSnippetID::CSXformPostAlpha);
            let premul = if steps.flags.premul { 0.0 } else { 1.0 };
            scope.uniforms().write_half(premul);
        } // else any premul is handled by the PreAlpha stage

        let mut builder = key_context.paint_params_key_builder().borrow_mut();
        if stage_ids.is_empty() {
            // We've specialized down to the identity function, but we need to add a block
            builder.add_block(BuiltInCodeSnippetID::PriorOutput);
        } else {
            // The linear sequence can be represented as a composition chain:
            // [stageID[0], stageID[1], ...] => Compose [ stageID[0] Compose [ stageID[1] ... ]]
            // When size = 1, this code reduces to just appending stageIDs[0] w/o Compose blocks
            let last = stage_ids.len() - 1;
            for &stage in &stage_ids[..last] {
                builder.begin_block(BuiltInCodeSnippetID::Compose);
                builder.add_block(stage);
            }
            // The last stage does not need to be composed with anything else
            builder.add_block(stage_ids[last]);
            // Close the Compose blocks
            for _ in 0..last {
                builder.end_block();
            }
        }
    }
}

//--------------------------------------------------------------------------------------------------

/// `AddPrimitiveColor`: the primitive color the render step produces, transformed from its color
/// space (sRGB unless one is given) to the destination's.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1437-L1464 (chrome/m156)
#[doc(alias = "AddPrimitiveColor")]
pub fn add_primitive_color(
    key_context: &KeyContext<'_>,
    skip_color_xform: bool,
    primitive_color_space: Option<&ColorSpace>,
    primitive_alpha_type: AlphaType,
) {
    // When skip_color_xform is true, we assume the primitive color is already in the dst color
    // space.
    if skip_color_xform {
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .add_block(BuiltInCodeSnippetID::PrimitiveColor);
        return;
    }

    // If skip_color_xform is false (most cases), the primitive color is assumed to be in sRGB
    // unless an explicit primitive color space is provided.
    let src_cs = primitive_color_space.unwrap_or(srgb_singleton());
    let to_dst = ColorSpaceTransformData::from_color_spaces(
        Some(src_cs),
        primitive_alpha_type,
        key_context.dst_color_info().color_space_ref(),
        key_context.dst_color_info().alpha_type(),
    );

    compose(
        key_context,
        || {
            key_context
                .paint_params_key_builder()
                .borrow_mut()
                .add_block(BuiltInCodeSnippetID::PrimitiveColor);
        },
        || ColorSpaceTransformBlock::add_block(key_context, &to_dst),
    );
}

//--------------------------------------------------------------------------------------------------

/// `AddFixedBlendMode`: a blend mode with its own snippet.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L2703-L2708 (chrome/m156)
#[doc(alias = "AddFixedBlendMode")]
pub fn add_fixed_blend_mode(key_context: &KeyContext<'_>, bm: BlendMode) {
    let id = built_in_from_i32(FIXED_BLEND_ID_OFFSET + bm as i32);
    key_context
        .paint_params_key_builder()
        .borrow_mut()
        .add_block(id);
}

/// `AddBlendMode`: a Porter-Duff coefficient blend, an HSL blend, or a fixed blend mode.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L2710-L2724 (chrome/m156)
#[doc(alias = "AddBlendMode")]
pub fn add_blend_mode(key_context: &KeyContext<'_>, bm: BlendMode) {
    // For non-fixed blends, coefficient blend modes are combined into the same shader snippet.
    // The same goes for the HSLC advanced blends. The remaining advanced blends are fairly unique
    // in their implementations. To avoid having to compile all of their SkSL, they are treated as
    // fixed blend modes.
    let coeffs = get_porter_duff_blend_constants(bm);
    if !coeffs.is_empty() {
        PorterDuffBlenderBlock::add_block(key_context, coeffs);
    } else if bm >= BlendMode::Hue {
        let blend_info = get_reduced_blend_mode_info(bm);
        HSLCBlenderBlock::add_block(key_context, blend_info.uniform_data);
    } else {
        add_fixed_blend_mode(key_context, bm);
    }
}

/// `AddBlendModeColorFilter`: blends the input (the dst) with a constant color (the src).
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1468-L1480 (chrome/m156)
#[doc(alias = "AddBlendModeColorFilter")]
pub fn add_blend_mode_color_filter(
    key_context: &KeyContext<'_>,
    bm: BlendMode,
    src_color: &PMColor4f,
) {
    blend(
        key_context,
        || add_blend_mode(key_context, bm),
        || solid_color_shader_add_block(key_context, src_color),
        || {
            key_context
                .paint_params_key_builder()
                .borrow_mut()
                .add_block(BuiltInCodeSnippetID::PriorOutput);
        },
    );
}

//--------------------------------------------------------------------------------------------------

/// The data of a runtime effect: the effect and its uniforms. `None` uniforms mean the key is
/// generated for pre-compilation, which has no uniform values.
// Port of: src/gpu/graphite/KeyHelpers.h#L370-L387 (chrome/m156), `RuntimeEffectBlock::ShaderData`
#[derive(Clone, Debug)]
pub struct RuntimeEffectShaderData {
    /// `fEffect`.
    pub effect: RuntimeEffect,
    /// `fUniforms`.
    pub uniforms: Option<Data>,
}

impl RuntimeEffectShaderData {
    /// `operator==`: the same effect and the same uniform contents.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L1496-L1498 (chrome/m156)
    #[must_use]
    pub fn same_as(&self, rhs: &Self) -> bool {
        self.effect.ptr_eq(&rhs.effect)
            && Data::equals_opt(self.uniforms.as_ref(), rhs.uniforms.as_ref())
    }
}

/// Writes the runtime effect's uniforms from `uniform_data`, one per `effect` uniform, with the
/// graphite uniform of the same index.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1500-L1524 (chrome/m156), `gather_runtime_effect_uniforms`
fn gather_runtime_effect_uniforms(
    effect: &RuntimeEffect,
    graphite_uniforms: &[Uniform],
    uniform_data: Option<&Data>,
    gatherer: &mut PipelineDataGatherer,
) {
    let Some(uniform_data) = uniform_data else {
        return; // precompiling
    };

    #[cfg(debug_assertions)]
    gatherer
        .uniform_manager()
        .set_expected_uniforms(graphite_uniforms, false);

    let rts_uniforms = effect.uniforms();
    if !rts_uniforms.is_empty() {
        // Collect all the other uniforms from the provided SkData.
        let uniform_base = uniform_data.as_bytes();
        for (index, rts_uniform) in rts_uniforms.iter().enumerate() {
            let uniform = &graphite_uniforms[index];
            // Get a pointer to the offset in our data for this uniform, and pass the data to the
            // gatherer.
            let uniform_ptr = &uniform_base[rts_uniform.offset()..];
            gatherer
                .uniform_manager()
                .write_uniform(uniform, uniform_ptr);
        }
    }

    #[cfg(debug_assertions)]
    gatherer.uniform_manager().done_with_expected_uniforms();
}

/// `RuntimeEffectBlock`: the runtime effect snippet, its uniforms, and the intrinsics that
/// require color space transforms.
// Port of: src/gpu/graphite/KeyHelpers.h#L369-L395 (chrome/m156)
#[derive(Debug)]
pub struct RuntimeEffectBlock;

impl RuntimeEffectBlock {
    /// `BeginBlock`: on a false return no block has been started.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L1528-L1553 (chrome/m156)
    pub fn begin_block(
        key_context: &KeyContext<'_>,
        shader_data: &RuntimeEffectShaderData,
    ) -> bool {
        let dict = key_context.dict();
        let Some(code_snippet_id) = dict.find_or_create_runtime_effect_snippet(&shader_data.effect)
        else {
            return false;
        };

        if skia_rust_core::known_runtime_effects::is_user_defined_runtime_effect(code_snippet_id) {
            key_context
                .rt_effect_dict()
                .set(code_snippet_id, shader_data.effect.clone());
        }

        let Some(entry) = dict.get_entry(code_snippet_id) else {
            return false;
        };

        {
            let mut gatherer = key_context.pipeline_data_gatherer().borrow_mut();
            gather_runtime_effect_uniforms(
                &shader_data.effect,
                &entry.uniforms,
                shader_data.uniforms.as_ref(),
                &mut gatherer,
            );
        }

        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .begin_block(snippet_id_u32(code_snippet_id));
        true
    }

    /// Adds a no-op placeholder for an incorrect runtime effect.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L1557-L1569 (chrome/m156)
    pub fn add_no_op_effect(key_context: &KeyContext<'_>, effect: &RuntimeEffect) {
        if effect.allow_shader() {
            // A missing shader returns transparent black
            solid_color_shader_add_block(key_context, &PMColor4f::new(0.0, 0.0, 0.0, 0.0));
        } else if effect.allow_color_filter() {
            // A "passthrough" color filter returns the input color as-is.
            key_context
                .paint_params_key_builder()
                .borrow_mut()
                .add_block(BuiltInCodeSnippetID::PriorOutput);
        } else {
            debug_assert!(effect.allow_blender());
            // A "passthrough" blender performs `blend_src_over(src, dest)`.
            add_fixed_blend_mode(key_context, BlendMode::SrcOver);
        }
    }

    /// Adds a post-amble for runtime effects that use the `toLinearSrgb`/`fromLinearSrgb`
    /// intrinsics: two color space transforms bound to the last two children.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L1571-L1605 (chrome/m156)
    pub fn handle_intrinsics(key_context: &KeyContext<'_>, effect: &RuntimeEffect) {
        // Runtime effects that reference color transform intrinsics have two extra children that
        // are bound to the colorspace xform snippet with values to go to and from the linear srgb
        // to the current working/dst color space.
        if runtime_effect_priv::uses_color_transform(effect) {
            let dst_cs = key_context
                .dst_color_info()
                .color_space_ref()
                .unwrap_or(srgb_linear_singleton()); // turn colorspace conversion into a noop

            // NOTE: This must be kept in sync with the logic used to generate the toLinearSrgb() and
            // fromLinearSrgb() expressions for each runtime effect. toLinearSrgb() is assumed to be
            // the second to last child, and fromLinearSrgb() is assumed to be the last.
            //
            // The conversions use kOpaque for their alpha type because the public signature takes
            // a half3 value; any alpha is assumed to be handled by the calling runtime effect.
            let dst_to_linear = ColorSpaceTransformData::from_color_spaces(
                Some(dst_cs),
                AlphaType::Opaque,
                Some(srgb_linear_singleton()),
                AlphaType::Opaque,
            );
            let linear_to_dst = ColorSpaceTransformData::from_color_spaces(
                Some(srgb_linear_singleton()),
                AlphaType::Opaque,
                Some(dst_cs),
                AlphaType::Opaque,
            );

            // Like working color space shaders, we allow these color space conversions to be
            // specialized as much as possible.
            let cs_context =
                key_context.with_extra_flags(KeyGenFlags::SPECIALIZE_COLOR_SPACE_XFORM);
            ColorSpaceTransformBlock::add_block(&cs_context, &dst_to_linear);
            ColorSpaceTransformBlock::add_block(&cs_context, &linear_to_dst);
        }
    }
}

/// `add_children_to_key`: the children of a runtime effect (shader, color filter or blender),
/// each keyed in its own context, with a no-op standing in for a missing child.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1617-L1659 (chrome/m156)
fn add_children_to_key(
    key_context: &KeyContext<'_>,
    children: &[ChildPtr],
    effect: &RuntimeEffect,
) {
    let child_info = effect.children();
    debug_assert_eq!(children.len(), child_info.len());

    for (index, child) in children.iter().enumerate() {
        let child_context = key_context.for_runtime_effect(effect, index);

        match child.ty() {
            Some(ChildType::Shader) => add_child_shader_to_key(&child_context, child.shader()),
            Some(ChildType::ColorFilter) => {
                add_to_key_color_filter(&child_context, child.color_filter());
            }
            Some(ChildType::Blender) => add_to_key_blender(&child_context, child.blender()),
            None => {
                // We don't have a child effect. Substitute in a no-op effect.
                match child_info[index].ty() {
                    ChildType::Shader => {
                        // A missing shader returns transparent black
                        solid_color_shader_add_block(
                            &child_context,
                            &PMColor4f::new(0.0, 0.0, 0.0, 0.0),
                        );
                    }
                    ChildType::ColorFilter => {
                        // A "passthrough" color filter returns the input color as-is.
                        key_context
                            .paint_params_key_builder()
                            .borrow_mut()
                            .add_block(BuiltInCodeSnippetID::PriorOutput);
                    }
                    ChildType::Blender => {
                        // A "passthrough" blender performs `blend_src_over(src, dest)`.
                        add_fixed_blend_mode(&child_context, BlendMode::SrcOver);
                    }
                }
            }
        }
    }

    RuntimeEffectBlock::handle_intrinsics(key_context, effect);
}

/// `AddToKey(SkShader*)` for a runtime effect's shader child, through `KeyHelpers` I (G5b).
// Port of: src/gpu/graphite/KeyHelpers.cpp#L2665-L2684 (chrome/m156), used by add_children_to_key
fn add_child_shader_to_key(
    key_context: &KeyContext<'_>,
    shader: Option<&skia_rust_core::shader::Shader>,
) {
    crate::graphite::key_helpers::add_to_key_shader(key_context, shader);
}

/// `MeshShaderBlock`: a mesh specification's snippet and its children.
// Port of: src/gpu/graphite/KeyHelpers.h#L397-L401 and KeyHelpers.cpp#L2742-L2782 (chrome/m156)
#[derive(Debug)]
pub struct MeshShaderBlock;

impl MeshShaderBlock {
    /// `AddBlock`.
    // Port of: src/gpu/graphite/KeyHelpers.cpp#L2742-L2782 (chrome/m156)
    pub fn add_block(
        key_context: &KeyContext<'_>,
        spec: &Arc<MeshSpecification>,
        children: &[ChildPtr],
    ) {
        let mesh_snippet_id = key_context.dict().find_or_create_mesh_snippet(spec);
        key_context
            .rt_effect_dict()
            .set_mesh_spec(mesh_snippet_id, spec.clone());

        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .begin_block(snippet_id_u32(mesh_snippet_id));

        let child_info = spec.children();
        debug_assert_eq!(children.len(), child_info.len());

        for (index, child) in children.iter().enumerate() {
            let child_context = key_context.for_mesh_spec_child();

            match child.ty() {
                Some(ChildType::Shader) => add_child_shader_to_key(&child_context, child.shader()),
                Some(ChildType::ColorFilter) => {
                    add_to_key_color_filter(&child_context, child.color_filter());
                }
                Some(ChildType::Blender) => add_to_key_blender(&child_context, child.blender()),
                None => match child_info[index].ty() {
                    ChildType::Shader => {
                        solid_color_shader_add_block(
                            &child_context,
                            &PMColor4f::new(0.0, 0.0, 0.0, 0.0),
                        );
                    }
                    ChildType::ColorFilter => {
                        key_context
                            .paint_params_key_builder()
                            .borrow_mut()
                            .add_block(BuiltInCodeSnippetID::PriorOutput);
                    }
                    ChildType::Blender => {
                        add_fixed_blend_mode(&child_context, BlendMode::SrcOver);
                    }
                },
            }
        }

        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .end_block();
    }
}

//--------------------------------------------------------------------------------------------------

/// `AddToKey(const KeyContext&, const SkBlender*)`: the blend mode blender, the runtime blender,
/// or, for a missing blender, a `SrcOver` with an assertion.
///
/// # Panics
/// If a blender's type tag does not match its concrete type (a bug in a blender implementation).
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1683-L1702 (chrome/m156)
#[doc(alias = "AddToKey")]
pub fn add_to_key_blender(key_context: &KeyContext<'_>, blender: Option<&Blender>) {
    let Some(blender) = blender else {
        // Calling code assumes a block will be appended. Add a fixed block to preserve shader
        // and PaintParamsKey structure in release builds but assert since this should either not
        // happen or should be changing high-level logic within PaintParams::toKey().
        debug_assert!(false, "AddToKey called with a null blender");
        add_fixed_blend_mode(key_context, BlendMode::SrcOver);
        return;
    };
    let base: &dyn Any = blender.as_base();
    match blender.as_base().blender_type() {
        BlenderType::BlendMode => {
            let blender = base
                .downcast_ref::<BlendModeBlender>()
                .expect("a BlenderType::BlendMode is a BlendModeBlender");
            // Port of: src/gpu/graphite/KeyHelpers.cpp#L1611-L1614 (chrome/m156)
            add_blend_mode(key_context, blender.mode());
        }
        BlenderType::Runtime => {
            let blender = base
                .downcast_ref::<RuntimeBlender>()
                .expect("a BlenderType::Runtime is a RuntimeBlender");
            add_runtime_blender_to_key(key_context, blender);
        }
    }
}

/// `add_to_key(SkRuntimeBlender*)`.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1661-L1679 (chrome/m156)
fn add_runtime_blender_to_key(key_context: &KeyContext<'_>, blender: &RuntimeBlender) {
    let effect = blender.effect().clone();
    let uniforms = runtime_effect_priv::transform_uniforms(
        effect.uniforms(),
        blender.uniforms(),
        key_context.dst_color_info().color_space_ref(),
    );

    let shader_data = RuntimeEffectShaderData {
        effect: effect.clone(),
        uniforms: Some(uniforms),
    };
    if !RuntimeEffectBlock::begin_block(key_context, &shader_data) {
        RuntimeEffectBlock::add_no_op_effect(key_context, &effect);
        return;
    }

    add_children_to_key(key_context, blender.children(), &effect);

    key_context
        .paint_params_key_builder()
        .borrow_mut()
        .end_block();
}

//--------------------------------------------------------------------------------------------------

/// `map_color`: an unpremultiplied sRGB color transformed to the destination's color space and
/// alpha type.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1706-L1713 (chrome/m156)
fn map_color(c: Color4f, dst_color_info: &ColorInfo) -> PMColor4f {
    let mut color = c.as_array();
    ColorSpaceXformSteps::new(
        Some(srgb_singleton()),
        AlphaType::Unpremul,
        dst_color_info.color_space_ref(),
        dst_color_info.alpha_type(),
    )
    .apply(&mut color);
    PMColor4f::new(color[0], color[1], color[2], color[3])
}

/// `add_to_key(SkBlendModeColorFilter*)`.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1714-L1722 (chrome/m156)
fn add_blend_mode_color_filter_to_key(key_context: &KeyContext<'_>, filter: &BlendModeColorFilter) {
    let color = map_color(filter.color(), key_context.dst_color_info());
    add_blend_mode_color_filter(key_context, filter.mode(), &color);
}

/// `add_to_key(SkColorSpaceXformColorFilter*)`.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1724-L1731 (chrome/m156)
fn add_color_space_xform_color_filter_to_key(
    key_context: &KeyContext<'_>,
    filter: &ColorSpaceXformColorFilter,
) {
    const ALPHA_TYPE: AlphaType = AlphaType::Premul;
    let cs_data = ColorSpaceTransformData::from_color_spaces(
        Some(filter.src()),
        ALPHA_TYPE,
        Some(filter.dst()),
        ALPHA_TYPE,
    );
    ColorSpaceTransformBlock::add_block(key_context, &cs_data);
}

/// `add_to_key(SkComposeColorFilter*)`.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1733-L1743 (chrome/m156)
fn add_compose_color_filter_to_key(key_context: &KeyContext<'_>, filter: &ComposeColorFilter) {
    compose(
        key_context,
        || add_to_key_color_filter(key_context, Some(filter.inner())),
        || add_to_key_color_filter(key_context, Some(filter.outer())),
    );
}

/// `add_to_key(SkGaussianColorFilter*)`.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1745-L1747 (chrome/m156)
fn add_gaussian_color_filter_to_key(key_context: &KeyContext<'_>) {
    key_context
        .paint_params_key_builder()
        .borrow_mut()
        .add_block(BuiltInCodeSnippetID::GaussianColorFilter);
}

/// `add_to_key(SkMatrixColorFilter*)`.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1749-L1757 (chrome/m156)
fn add_matrix_color_filter_to_key(key_context: &KeyContext<'_>, filter: &MatrixColorFilter) {
    let in_hsla = filter.domain() == Domain::Hsla;
    let clamp = filter.clamp() == Clamp::Yes;
    let matrix_cf_data = MatrixColorFilterData::new(filter.matrix(), in_hsla, clamp);

    MatrixColorFilterBlock::add_block(key_context, &matrix_cf_data);
}

/// `add_to_key(SkRuntimeColorFilter*)`.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1759-L1775 (chrome/m156)
fn add_runtime_color_filter_to_key(key_context: &KeyContext<'_>, filter: &RuntimeColorFilter) {
    let effect = filter.effect().clone();
    let uniforms = runtime_effect_priv::transform_uniforms(
        effect.uniforms(),
        filter.uniforms(),
        key_context.dst_color_info().color_space_ref(),
    );

    let shader_data = RuntimeEffectShaderData {
        effect: effect.clone(),
        uniforms: Some(uniforms),
    };
    if !RuntimeEffectBlock::begin_block(key_context, &shader_data) {
        RuntimeEffectBlock::add_no_op_effect(key_context, &effect);
        return;
    }

    add_children_to_key(key_context, filter.children(), &effect);

    key_context
        .paint_params_key_builder()
        .borrow_mut()
        .end_block();
}

/// `add_to_key(SkWorkingFormatColorFilter*)`: the child runs in the working format, between two
/// color space transforms to and from the destination.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1796-L1836 (chrome/m156)
fn add_working_format_color_filter_to_key(
    key_context: &KeyContext<'_>,
    filter: &WorkingFormatColorFilter,
) {
    let dst_info = key_context.dst_color_info();
    let dst_at = dst_info.alpha_type();
    let dst_cs = dst_info.color_space().unwrap_or_else(ColorSpace::new_srgb);

    let cs_optimize = key_context.with_extra_flags(KeyGenFlags::SPECIALIZE_COLOR_SPACE_XFORM);

    let (working_cs, working_at) = filter.working_format(&dst_cs);
    let working_context = cs_optimize.with_color_info(&ColorInfo::new(
        dst_info.color_type(),
        working_at,
        working_cs.clone(),
    ));

    // Use two nested compose blocks to chain (dst->working), child, and (working->dst) together
    // while appearing as one block to the parent node.
    compose(
        &cs_optimize,
        || {
            // Inner compose
            compose(
                &cs_optimize,
                || {
                    // Innermost (inner of inner compose)
                    let data1 = ColorSpaceTransformData::from_color_spaces(
                        Some(&dst_cs),
                        dst_at,
                        working_cs.as_ref(),
                        working_at,
                    );
                    ColorSpaceTransformBlock::add_block(&cs_optimize, &data1);
                },
                || {
                    // Middle (outer of inner compose)
                    add_to_key_color_filter(&working_context, Some(filter.child()));
                },
            );
        },
        || {
            // Outermost (outer of outer compose)
            let data2 = ColorSpaceTransformData::from_color_spaces(
                working_cs.as_ref(),
                working_at,
                Some(&dst_cs),
                dst_at,
            );
            ColorSpaceTransformBlock::add_block(&cs_optimize, &data2);
        },
    );
}

/// `AddToKey(const KeyContext&, const SkColorFilter*)`: the color filter's blocks, or a
/// pass-through with an assertion for a missing filter.
///
/// # Panics
/// On a color filter type whose key is not ported yet: [`ColorFilterType::Table`] (see the module
/// docs). Also if a filter's type tag does not match its concrete type.
// Port of: src/gpu/graphite/KeyHelpers.cpp#L1838-L1861 (chrome/m156)
#[doc(alias = "AddToKey")]
pub fn add_to_key_color_filter(key_context: &KeyContext<'_>, filter: Option<&ColorFilter>) {
    let Some(filter) = filter else {
        // Calling code assumes a block will be appended. Add a fixed block to preserve shader
        // and PaintParamsKey structure in release builds but assert since this should either not
        // happen or should be changing high-level logic within PaintParams::toKey().
        debug_assert!(false, "AddToKey called with a null color filter");
        key_context
            .paint_params_key_builder()
            .borrow_mut()
            .add_block(BuiltInCodeSnippetID::PriorOutput);
        return;
    };
    let base: &dyn Any = filter.as_base();
    match filter.as_base().color_filter_type() {
        ColorFilterType::Noop => {
            // Return the input color as-is.
            key_context
                .paint_params_key_builder()
                .borrow_mut()
                .add_block(BuiltInCodeSnippetID::PriorOutput);
        }
        ColorFilterType::BlendMode => add_blend_mode_color_filter_to_key(
            key_context,
            base.downcast_ref::<BlendModeColorFilter>()
                .expect("a BlendMode filter is a BlendModeColorFilter"),
        ),
        ColorFilterType::ColorSpaceXform => add_color_space_xform_color_filter_to_key(
            key_context,
            base.downcast_ref::<ColorSpaceXformColorFilter>()
                .expect("a ColorSpaceXform filter is a ColorSpaceXformColorFilter"),
        ),
        ColorFilterType::Compose => add_compose_color_filter_to_key(
            key_context,
            base.downcast_ref::<ComposeColorFilter>()
                .expect("a Compose filter is a ComposeColorFilter"),
        ),
        ColorFilterType::Gaussian => add_gaussian_color_filter_to_key(key_context),
        ColorFilterType::Matrix => add_matrix_color_filter_to_key(
            key_context,
            base.downcast_ref::<MatrixColorFilter>()
                .expect("a Matrix filter is a MatrixColorFilter"),
        ),
        ColorFilterType::Runtime => add_runtime_color_filter_to_key(
            key_context,
            base.downcast_ref::<RuntimeColorFilter>()
                .expect("a Runtime filter is a RuntimeColorFilter"),
        ),
        ColorFilterType::Table => unimplemented!(
            "TableColorFilter needs RecorderPriv::CreateCachedProxy and the texture half of \
             PipelineDataGatherer (G8/G9a, G3); it is not on this branch"
        ),
        ColorFilterType::WorkingFormat => add_working_format_color_filter_to_key(
            key_context,
            base.downcast_ref::<WorkingFormatColorFilter>()
                .expect("a WorkingFormat filter is a WorkingFormatColorFilter"),
        ),
    }
}
