// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkKnownRuntimeEffects.{h,cpp}

//! `SkKnownRuntimeEffects`: the runtime effects Skia builds itself, each with a stable key.
//!
//! Each effect is built once, on first use, from the `SkSL` below (`GetKnownRuntimeEffect`).
//! The keys are serialized, so their values are fixed. The matrix convolution kernel sizes
//! (`MatrixConvolutionImageFilter`) are the only values from another module that the sources
//! use.

use std::sync::OnceLock;

use crate::runtime_effect::{Options, RuntimeEffect};
use crate::runtime_effect_priv;

/// `kSkiaBuiltInReservedCnt`.
// Port of: src/core/SkKnownRuntimeEffects.h#L23 (chrome/m156)
pub const SKIA_BUILT_IN_RESERVED_CNT: u32 = 500;
/// `kSkiaKnownRuntimeEffectsReservedCnt`.
// Port of: src/core/SkKnownRuntimeEffects.h#L24 (chrome/m156)
const SKIA_KNOWN_RUNTIME_EFFECTS_RESERVED_CNT: u32 = 500;
/// `kSkiaKnownRuntimeEffectsStart`.
// Port of: src/core/SkKnownRuntimeEffects.h#L27 (chrome/m156)
pub const SKIA_KNOWN_RUNTIME_EFFECTS_START: u32 = SKIA_BUILT_IN_RESERVED_CNT;
/// `kSkiaKnownRuntimeEffectsEnd`.
// Port of: src/core/SkKnownRuntimeEffects.h#L28-L29 (chrome/m156)
const SKIA_KNOWN_RUNTIME_EFFECTS_END: u32 =
    SKIA_KNOWN_RUNTIME_EFFECTS_START + SKIA_KNOWN_RUNTIME_EFFECTS_RESERVED_CNT;

/// `kUserDefinedKnownRuntimeEffectsStart`.
// Port of: src/core/SkKnownRuntimeEffects.h#L30 (chrome/m156)
pub const USER_DEFINED_KNOWN_RUNTIME_EFFECTS_START: u32 = SKIA_KNOWN_RUNTIME_EFFECTS_END;
/// `kUserDefinedKnownRuntimeEffectsReservedCnt`.
// Port of: src/core/SkKnownRuntimeEffects.h#L22 (chrome/m156)
pub const USER_DEFINED_KNOWN_RUNTIME_EFFECTS_RESERVED_CNT: u32 = 100;
/// `kUserDefinedKnownRuntimeEffectsEnd`: the reserved count is 100.
// Port of: src/core/SkKnownRuntimeEffects.h#L31-L32 (chrome/m156)
const USER_DEFINED_KNOWN_RUNTIME_EFFECTS_END: u32 =
    USER_DEFINED_KNOWN_RUNTIME_EFFECTS_START + USER_DEFINED_KNOWN_RUNTIME_EFFECTS_RESERVED_CNT;

/// `kUnknownRuntimeEffectIDStart`.
// Port of: src/core/SkKnownRuntimeEffects.h#L33 (chrome/m156)
pub const UNKNOWN_RUNTIME_EFFECT_ID_START: u32 = USER_DEFINED_KNOWN_RUNTIME_EFFECTS_END;

/// `SkKnownRuntimeEffects::StableKey`: the stable key of a Skia known runtime effect.
///
/// The `1DBlurBase` and `2DBlurBase` helper values of Skia are the constants
/// [`StableKey::ONE_D_BLUR_BASE`] and [`StableKey::TWO_D_BLUR_BASE`] (they equal the first key of
/// each block, so they are not variants).
// Port of: src/core/SkKnownRuntimeEffects.h#L36-L82 (chrome/m156)
#[doc(alias = "SkKnownRuntimeEffects::StableKey")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum StableKey {
    /// `kInvalid`: the not-a-key value (`kStart`).
    Invalid = SKIA_KNOWN_RUNTIME_EFFECTS_START,
    /// `k1DBlur4`.
    OneDBlur4 = 501,
    /// `k1DBlur8`.
    OneDBlur8 = 502,
    /// `k1DBlur12`.
    OneDBlur12 = 503,
    /// `k1DBlur16`.
    OneDBlur16 = 504,
    /// `k1DBlur20`.
    OneDBlur20 = 505,
    /// `k1DBlur28`.
    OneDBlur28 = 506,
    /// `k2DBlur4`.
    TwoDBlur4 = 507,
    /// `k2DBlur8`.
    TwoDBlur8 = 508,
    /// `k2DBlur12`.
    TwoDBlur12 = 509,
    /// `k2DBlur16`.
    TwoDBlur16 = 510,
    /// `k2DBlur20`.
    TwoDBlur20 = 511,
    /// `k2DBlur28`.
    TwoDBlur28 = 512,
    /// `kBlend`.
    Blend = 513,
    /// `kDecal`.
    Decal = 514,
    /// `kDisplacement`.
    Displacement = 515,
    /// `kLighting`.
    Lighting = 516,
    /// `kLinearMorphology`.
    LinearMorphology = 517,
    /// `kMagnifier`.
    Magnifier = 518,
    /// `kMatrixConvUniforms`.
    MatrixConvUniforms = 519,
    /// `kMatrixConvTexSm`.
    MatrixConvTexSm = 520,
    /// `kMatrixConvTexLg`.
    MatrixConvTexLg = 521,
    /// `kNormal`.
    Normal = 522,
    /// `kSparseMorphology`.
    SparseMorphology = 523,
    /// `kArithmetic`.
    Arithmetic = 524,
    /// `kHighContrast`.
    HighContrast = 525,
    /// `kLerp`.
    Lerp = 526,
    /// `kLuma`.
    Luma = 527,
    /// `kOverdraw`: the last key (`kLast`).
    Overdraw = 528,
}

impl StableKey {
    /// `k1DBlurBase`: the first of the six consecutive 1D blur keys.
    #[doc(alias = "k1DBlurBase")]
    pub const ONE_D_BLUR_BASE: StableKey = StableKey::OneDBlur4;
    /// `k2DBlurBase`: the first of the six consecutive 2D blur keys.
    #[doc(alias = "k2DBlurBase")]
    pub const TWO_D_BLUR_BASE: StableKey = StableKey::TwoDBlur4;
    /// `kLast`.
    #[doc(alias = "kLast")]
    pub const LAST: StableKey = StableKey::Overdraw;
}

/// Is `candidate` a viable `StableKey` value (`IsSkiaKnownRuntimeEffect`). This includes the
/// `Invalid` key.
// Port of: src/core/SkKnownRuntimeEffects.cpp#L489-L492 (chrome/m156)
#[doc(alias = "IsSkiaKnownRuntimeEffect")]
#[must_use]
pub fn is_skia_known_runtime_effect(candidate: i32) -> bool {
    (StableKey::Invalid as i32..=StableKey::LAST as i32).contains(&candidate)
}

/// `IsUserDefinedRuntimeEffect`.
// Port of: src/core/SkKnownRuntimeEffects.cpp#L494-L496 (chrome/m156)
#[doc(alias = "IsUserDefinedRuntimeEffect")]
#[must_use]
pub fn is_user_defined_runtime_effect(candidate: i32) -> bool {
    // (A negative candidate is below every block, as in C++.)
    u32::try_from(candidate).is_ok_and(|candidate| candidate >= UNKNOWN_RUNTIME_EFFECT_ID_START)
}

/// `IsViableUserDefinedKnownRuntimeEffect`.
// Port of: src/core/SkKnownRuntimeEffects.cpp#L498-L502 (chrome/m156)
#[doc(alias = "IsViableUserDefinedKnownRuntimeEffect")]
#[must_use]
pub fn is_viable_user_defined_known_runtime_effect(candidate: i32) -> bool {
    u32::try_from(candidate).is_ok_and(|candidate| {
        (USER_DEFINED_KNOWN_RUNTIME_EFFECTS_START..USER_DEFINED_KNOWN_RUNTIME_EFFECTS_END)
            .contains(&candidate)
    })
}

/// The known runtime effect with `candidate` as its key, validated first (`MaybeGetKnownRuntimeEffect`).
// Port of: src/core/SkKnownRuntimeEffects.cpp#L504-L511 (chrome/m156)
#[doc(alias = "MaybeGetKnownRuntimeEffect")]
#[must_use]
pub fn maybe_get_known_runtime_effect(candidate: u32) -> Option<RuntimeEffect> {
    // (`IsSkiaKnownRuntimeEffect(candidate)` converts the `uint32_t` to an `int`, as in C++.)
    #[allow(clippy::cast_possible_wrap)] // mirrors the `uint32_t` to `int` conversion of C++
    let as_int = candidate as i32;
    if is_skia_known_runtime_effect(as_int) {
        let stable_key = stable_key_from_u32(candidate)?;
        return get_known_runtime_effect(stable_key).cloned();
    }
    None
}

/// The `StableKey` with the value `candidate`, if there is one (`static_cast<StableKey>` of a
/// viable value).
#[must_use]
pub fn stable_key_from_u32(candidate: u32) -> Option<StableKey> {
    let key = match candidate {
        500 => StableKey::Invalid,
        501 => StableKey::OneDBlur4,
        502 => StableKey::OneDBlur8,
        503 => StableKey::OneDBlur12,
        504 => StableKey::OneDBlur16,
        505 => StableKey::OneDBlur20,
        506 => StableKey::OneDBlur28,
        507 => StableKey::TwoDBlur4,
        508 => StableKey::TwoDBlur8,
        509 => StableKey::TwoDBlur12,
        510 => StableKey::TwoDBlur16,
        511 => StableKey::TwoDBlur20,
        512 => StableKey::TwoDBlur28,
        513 => StableKey::Blend,
        514 => StableKey::Decal,
        515 => StableKey::Displacement,
        516 => StableKey::Lighting,
        517 => StableKey::LinearMorphology,
        518 => StableKey::Magnifier,
        519 => StableKey::MatrixConvUniforms,
        520 => StableKey::MatrixConvTexSm,
        521 => StableKey::MatrixConvTexLg,
        522 => StableKey::Normal,
        523 => StableKey::SparseMorphology,
        524 => StableKey::Arithmetic,
        525 => StableKey::HighContrast,
        526 => StableKey::Lerp,
        527 => StableKey::Luma,
        528 => StableKey::Overdraw,
        _ => return None,
    };
    Some(key)
}

/// `kMaxBlurSamples`: this must be kept in sync with the version in `BlurUtils.h`.
// Port of: src/core/SkKnownRuntimeEffects.cpp#L31-L32 (chrome/m156)
const K_MAX_BLUR_SAMPLES: i32 = 28;

/// `MatrixConvolutionImageFilter::kLargeKernelSize`.
// Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.h#L22 (chrome/m156)
const MATRIX_CONVOLUTION_LARGE_KERNEL_SIZE: i32 = 256;
/// `MatrixConvolutionImageFilter::kSmallKernelSize`.
// Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.h#L27 (chrome/m156)
const MATRIX_CONVOLUTION_SMALL_KERNEL_SIZE: i32 = 64;
/// `MatrixConvolutionImageFilter::kMaxUniformKernelSize`.
// Port of: src/effects/imagefilters/SkMatrixConvolutionImageFilter.h#L31 (chrome/m156)
const MATRIX_CONVOLUTION_MAX_UNIFORM_KERNEL_SIZE: i32 = 28;

/// The options every known effect is made with (`get_options`).
// Port of: src/core/SkKnownRuntimeEffects.cpp#L16-L22 (chrome/m156)
fn get_options(stable_key: StableKey) -> Options<'static> {
    let mut options = Options::default();
    runtime_effect_priv::set_stable_key_on_options(&mut options, stable_key as u32);
    runtime_effect_priv::allow_private_access(&mut options);
    options
}

/// `SkMakeRuntimeEffect`: makes the effect, which must compile (`SkASSERTF`).
// Port of: src/core/SkRuntimeEffect.cpp (SkMakeRuntimeEffect helper, chrome/m156)
fn make_known(kind: MakeKind, sksl: &str, options: &Options<'_>) -> RuntimeEffect {
    let made = match kind {
        MakeKind::Shader => RuntimeEffect::make_for_shader(sksl, Some(options)),
        MakeKind::ColorFilter => RuntimeEffect::make_for_color_filter(sksl, Some(options)),
        MakeKind::Blender => RuntimeEffect::make_for_blender(sksl, Some(options)),
    };
    made.unwrap_or_else(|err| panic!("known runtime effect: {err}"))
}

/// The `SkRuntimeEffect::MakeFor*` factory a known effect is made with.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum MakeKind {
    Shader,
    ColorFilter,
    Blender,
}

// Port of: src/core/SkKnownRuntimeEffects.cpp#L34-L67 (chrome/m156)
fn make_blur_1d_shader(kernel_width: i32, stable_key: StableKey) -> RuntimeEffect {
    let options = get_options(stable_key);
    debug_assert!(kernel_width <= K_MAX_BLUR_SAMPLES);
    // The SkSL structure performs two kernel taps; if the kernel has an odd width the last
    // sample will be skipped with the current loop limit calculation.
    debug_assert_eq!(kernel_width % 2, 0);
    // The coefficients are always stored for the max radius to keep the uniform block consistent
    // across all effects. But we generate an exact loop over the kernel size. Note that this
    // program can be used for kernels smaller than the constructed max as long as the kernel
    // weights for excess entries are set to 0.
    let sksl = format!(
        concat!(
            "const int kMaxUniformKernelSize = {} / 2;",
            "const int kMaxLoopLimit = {} / 2;",
            "uniform half4 offsetsAndKernel[kMaxUniformKernelSize];",
            "uniform half2 dir;",
            "uniform shader child;",
            "half4 main(float2 coord) {{",
            "half4 sum = half4(0);",
            "for (int i = 0; i < kMaxLoopLimit; ++i) {{",
            "half4 s = offsetsAndKernel[i];",
            "sum += s.y * child.eval(coord + s.x*dir);",
            "sum += s.w * child.eval(coord + s.z*dir);",
            "}}",
            "return sum;",
            "}}"
        ),
        K_MAX_BLUR_SAMPLES, kernel_width
    );
    make_known(MakeKind::Shader, &sksl, &options)
}

// Port of: src/core/SkKnownRuntimeEffects.cpp#L69-L100 (chrome/m156)
fn make_blur_2d_shader(max_kernel_size: i32, stable_key: StableKey) -> RuntimeEffect {
    let options = get_options(stable_key);
    debug_assert_eq!(max_kernel_size % 4, 0);
    // The coefficients are always stored for the max radius to keep the uniform block consistent
    // across all effects. But we generate an exact loop over the kernel size. Note that this
    // program can be used for kernels smaller than the constructed max as long as the kernel
    // weights for excess entries are set to 0.
    //
    // Pack scalar coefficients into half4 for better packing on std140, and upload offsets to
    // avoid having to transform the 1D index into a 2D coord.
    let sksl = format!(
        concat!(
            "const int kMaxUniformKernelSize = {} / 4;",
            "const int kMaxUniformOffsetsSize = 2*kMaxUniformKernelSize;",
            "const int kMaxLoopLimit = {} / 4;",
            "uniform half4 kernel[kMaxUniformKernelSize];",
            "uniform half4 offsets[kMaxUniformOffsetsSize];",
            "uniform shader child;",
            "half4 main(float2 coord) {{",
            "half4 sum = half4(0);",
            "for (int i = 0; i < kMaxLoopLimit; ++i) {{",
            "half4 k = kernel[i];",
            "half4 o = offsets[2*i];",
            "sum += k.x * child.eval(coord + o.xy);",
            "sum += k.y * child.eval(coord + o.zw);",
            "o = offsets[2*i + 1];",
            "sum += k.z * child.eval(coord + o.xy);",
            "sum += k.w * child.eval(coord + o.zw);",
            "}}",
            "return sum;",
            "}}"
        ),
        K_MAX_BLUR_SAMPLES, max_kernel_size
    );
    make_known(MakeKind::Shader, &sksl, &options)
}

// Port of: src/core/SkKnownRuntimeEffects.cpp#L102-L118 (chrome/m156)
fn make_blend_shader() -> RuntimeEffect {
    let options = get_options(StableKey::Blend);
    let sksl = concat!(
        "uniform shader s, d;",
        "uniform blender b;",
        "half4 main(float2 xy) {",
        "return b.eval(s.eval(xy), d.eval(xy));",
        "}"
    );
    make_known(MakeKind::Shader, sksl, &options)
}

// Port of: src/core/SkKnownRuntimeEffects.cpp#L120-L133 (chrome/m156)
fn make_lerp_shader() -> RuntimeEffect {
    let options = get_options(StableKey::Lerp);
    let sksl = concat!(
        "uniform colorFilter cf0;",
        "uniform colorFilter cf1;",
        "uniform half weight;",
        "half4 main(half4 color) {",
        "return mix(cf0.eval(color), cf1.eval(color), weight);",
        "}"
    );
    make_known(MakeKind::ColorFilter, sksl, &options)
}

/// `MatrixConvolutionImpl`.
// Port of: src/core/SkKnownRuntimeEffects.cpp#L135-L139 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum MatrixConvolutionImpl {
    UniformBased,
    TextureBasedSm,
    TextureBasedLg,
}

/// `kHeaderAndBeginLoopSkSL`.
// Port of: src/core/SkKnownRuntimeEffects.cpp#L154-L165 (chrome/m156)
const MATRIX_CONV_HEADER_AND_BEGIN_LOOP_SKSL: &str = concat!(
    "uniform int2 size;",
    "uniform int2 offset;",
    "uniform half2 gainAndBias;",
    "uniform int convolveAlpha;", // FIXME not a full  int? Put in a half3 w/ gainAndBias?
    "uniform shader child;",
    "half4 main(float2 coord) {",
    "half4 sum = half4(0);",
    "half origAlpha = 0;",
    "int2 kernelPos = int2(0);",
    "for (int i = 0; i < kMaxKernelSize; ++i) {",
    "if (kernelPos.y >= size.y) { break; }",
);

/// `kAccumulateAndIncrementSkSL`.
// Port of: src/core/SkKnownRuntimeEffects.cpp#L167-L185 (chrome/m156)
const MATRIX_CONV_ACCUMULATE_AND_INCREMENT_SKSL: &str = concat!(
    "half4 c = child.eval(coord + half2(kernelPos) - half2(offset));",
    "if (convolveAlpha == 0) {",
    // When not convolving alpha, remember the original alpha for actual sample
    // coord, and perform accumulation on unpremul colors.
    "if (kernelPos == offset) {",
    "origAlpha = c.a;",
    "}",
    "c = unpremul(c);",
    "}",
    "sum += c*k;",
    "kernelPos.x += 1;",
    "if (kernelPos.x >= size.x) {",
    "kernelPos.x = 0;",
    "kernelPos.y += 1;",
    "}",
);

/// `kCloseLoopAndFooterSkSL`.
// Port of: src/core/SkKnownRuntimeEffects.cpp#L187-L203 (chrome/m156)
const MATRIX_CONV_CLOSE_LOOP_AND_FOOTER_SKSL: &str = concat!(
    "}",
    "half4 color = sum*gainAndBias.x + gainAndBias.y;",
    "if (convolveAlpha == 0) {",
    // Reset the alpha to the original and convert to premul RGB
    "color = half4(color.rgb*origAlpha, origAlpha);",
    "} else {",
    // Ensure convolved alpha is within [0, 1]
    "color.a = saturate(color.a);",
    "}",
    // Make RGB valid premul w/ respect to the alpha (either original or convolved)
    "color.rgb = clamp(color.rgb, 0, color.a);",
    "return color;",
    "}",
);

/// `makeTextureEffect`.
// Port of: src/core/SkKnownRuntimeEffects.cpp#L205-L218 (chrome/m156)
fn make_texture_matrix_conv_effect(
    max_texture_kernel_size: i32,
    options: &Options<'_>,
) -> RuntimeEffect {
    let sksl = format!(
        "const int kMaxKernelSize = {max_texture_kernel_size};\
         uniform shader kernel;\
         uniform half2 innerGainAndBias;\
         {MATRIX_CONV_HEADER_AND_BEGIN_LOOP_SKSL}\
         half k = kernel.eval(half2(half(i) + 0.5, 0.5)).a;\
         k = k * innerGainAndBias.x + innerGainAndBias.y;\
         {MATRIX_CONV_ACCUMULATE_AND_INCREMENT_SKSL}\
         {MATRIX_CONV_CLOSE_LOOP_AND_FOOTER_SKSL}",
    );
    make_known(MakeKind::Shader, &sksl, options)
}

// There are three shader variants:
//    a smaller kernel version that stores the matrix in uniforms and iterates in 1D
//    a larger kernel version that stores the matrix in a 1D texture. The texture version has small
//    and large variants w/ the actual kernel size uploaded as a uniform.
// Port of: src/core/SkKnownRuntimeEffects.cpp#L220-L266 (chrome/m156)
fn make_matrix_conv_shader(
    implementation: MatrixConvolutionImpl,
    stable_key: StableKey,
) -> RuntimeEffect {
    let options = get_options(stable_key);
    match implementation {
        MatrixConvolutionImpl::UniformBased => {
            let sksl = format!(
                "const int kMaxKernelSize = {MATRIX_CONVOLUTION_MAX_UNIFORM_KERNEL_SIZE} / 4;\
                 uniform half4 kernel[kMaxKernelSize];\
                 {MATRIX_CONV_HEADER_AND_BEGIN_LOOP_SKSL}\
                 half4 k4 = kernel[i];\
                 for (int j = 0; j < 4; ++j) {{\
                     if (kernelPos.y >= size.y) {{ break; }}\
                     half k = k4[j];\
                     {MATRIX_CONV_ACCUMULATE_AND_INCREMENT_SKSL}\
                 }}\
                 {MATRIX_CONV_CLOSE_LOOP_AND_FOOTER_SKSL}",
            );
            make_known(MakeKind::Shader, &sksl, &options)
        }
        MatrixConvolutionImpl::TextureBasedSm => {
            make_texture_matrix_conv_effect(MATRIX_CONVOLUTION_SMALL_KERNEL_SIZE, &options)
        }
        MatrixConvolutionImpl::TextureBasedLg => {
            make_texture_matrix_conv_effect(MATRIX_CONVOLUTION_LARGE_KERNEL_SIZE, &options)
        }
    }
}

// Port of: src/core/SkKnownRuntimeEffects.cpp#L268-L281 (chrome/m156)
fn make_decal_shader() -> RuntimeEffect {
    let options = get_options(StableKey::Decal);
    let sksl = concat!(
        "uniform shader image;",
        "uniform float4 decalBounds;",
        "half4 main(float2 coord) {",
        "return sk_decal(image, coord, decalBounds);",
        "}"
    );
    make_known(MakeKind::Shader, sksl, &options)
}

// Port of: src/core/SkKnownRuntimeEffects.cpp#L283-L300 (chrome/m156)
fn make_displacement_shader() -> RuntimeEffect {
    let options = get_options(StableKey::Displacement);
    // NOTE: This uses dot product selection to work on all GLES2 hardware (enforced by
    // public runtime effect restrictions). Otherwise, this would use a "uniform ivec2"
    // and component indexing to convert the displacement color into a vector.
    let sksl = concat!(
        "uniform shader displMap;",
        "uniform shader colorMap;",
        "uniform half2 scale;",
        "uniform half4 xSelect;", // Only one of RGBA will be 1, the rest are 0
        "uniform half4 ySelect;",
        "half4 main(float2 coord) {",
        "return sk_displacement(displMap, colorMap, coord, scale, xSelect, ySelect);",
        "}"
    );
    make_known(MakeKind::Shader, sksl, &options)
}

// Port of: src/core/SkKnownRuntimeEffects.cpp#L302-L327 (chrome/m156)
fn make_lighting_shader() -> RuntimeEffect {
    let options = get_options(StableKey::Lighting);
    let sksl = concat!(
        "uniform shader normalMap;",
        // Packs surface depth, shininess, material type (0 == diffuse) and light type
        // (< 0 = distant, 0 = point, > 0 = spot)
        "uniform half4 materialAndLightType;",
        "uniform half4 lightPosAndSpotFalloff;", // (x,y,z) are lightPos, w is spot falloff
        // exponent
        "uniform half4 lightDirAndSpotCutoff;", // (x,y,z) are lightDir,
        // w is spot cos(cutoffAngle)
        "uniform half3 lightColor;", // Material's k has already been multiplied in
        "half4 main(float2 coord) {",
        "return sk_lighting(normalMap, coord,",
        /*depth=*/
        "materialAndLightType.x,",
        /*shininess=*/
        "materialAndLightType.y,",
        /*materialType=*/
        "materialAndLightType.z,",
        /*lightType=*/
        "materialAndLightType.w,",
        /*lightPos=*/
        "lightPosAndSpotFalloff.xyz,",
        /*spotFalloff=*/
        "lightPosAndSpotFalloff.w,",
        /*lightDir=*/
        "lightDirAndSpotCutoff.xyz,",
        /*cosCutoffAngle=*/
        "lightDirAndSpotCutoff.w,",
        "lightColor);",
        "}"
    );
    make_known(MakeKind::Shader, sksl, &options)
}

// Port of: src/core/SkKnownRuntimeEffects.cpp#L329-L341 (chrome/m156)
fn make_linear_morphology_shader() -> RuntimeEffect {
    let options = get_options(StableKey::LinearMorphology);
    let sksl = concat!(
        "uniform shader child;",
        "uniform half2 offset;",
        "uniform half flip;", // -1 converts the max() calls to min()
        "uniform int radius;",
        "half4 main(float2 coord) {",
        "return sk_linear_morphology(child, coord, offset, flip, radius);",
        "}"
    );
    make_known(MakeKind::Shader, sksl, &options)
}

// Port of: src/core/SkKnownRuntimeEffects.cpp#L343-L355 (chrome/m156)
fn make_magnifier_shader() -> RuntimeEffect {
    let options = get_options(StableKey::Magnifier);
    let sksl = concat!(
        "uniform shader src;",
        "uniform float4 lensBounds;",
        "uniform float4 zoomXform;",
        "uniform float2 invInset;",
        "half4 main(float2 coord) {",
        "return sk_magnifier(src, coord, lensBounds, zoomXform, invInset);",
        "}"
    );
    make_known(MakeKind::Shader, sksl, &options)
}

// Port of: src/core/SkKnownRuntimeEffects.cpp#L357-L368 (chrome/m156)
fn make_normal_shader() -> RuntimeEffect {
    let options = get_options(StableKey::Normal);
    let sksl = concat!(
        "uniform shader alphaMap;",
        "uniform float4 edgeBounds;",
        "uniform half negSurfaceDepth;",
        "half4 main(float2 coord) {",
        "return sk_normal(alphaMap, coord, edgeBounds, negSurfaceDepth);",
        "}"
    );
    make_known(MakeKind::Shader, sksl, &options)
}

// Port of: src/core/SkKnownRuntimeEffects.cpp#L370-L382 (chrome/m156)
fn make_sparse_morphology_shader() -> RuntimeEffect {
    let options = get_options(StableKey::SparseMorphology);
    let sksl = concat!(
        "uniform shader child;",
        "uniform half2 offset;",
        "uniform half flip;",
        "half4 main(float2 coord) {",
        "return sk_sparse_morphology(child, coord, offset, flip);",
        "}"
    );
    make_known(MakeKind::Shader, sksl, &options)
}

// Port of: src/core/SkKnownRuntimeEffects.cpp#L384-L396 (chrome/m156)
fn make_arithmetic_blender() -> RuntimeEffect {
    let options = get_options(StableKey::Arithmetic);
    let sksl = concat!(
        "uniform half4 k;",
        "uniform half pmClamp;",
        "half4 main(half4 src, half4 dst) {",
        "return sk_arithmetic_blend(src, dst, k, pmClamp);",
        "}"
    );
    make_known(MakeKind::Blender, sksl, &options)
}

// Port of: src/core/SkKnownRuntimeEffects.cpp#L398-L409 (chrome/m156)
fn make_high_contrast_color_filter() -> RuntimeEffect {
    let options = get_options(StableKey::HighContrast);
    let sksl = concat!(
        "uniform half grayscale, invertStyle, contrast;",
        "half4 main(half4 color) {",
        "return half4(sk_high_contrast(color.rgb, grayscale, invertStyle, contrast), color.a);",
        "}"
    );
    make_known(MakeKind::ColorFilter, sksl, &options)
}

// Port of: src/core/SkKnownRuntimeEffects.cpp#L411-L421 (chrome/m156)
fn make_luma_color_filter() -> RuntimeEffect {
    let options = get_options(StableKey::Luma);
    let sksl = concat!(
        "half4 main(half4 color) {",
        "return sk_luma(color.rgb);",
        "}"
    );
    make_known(MakeKind::ColorFilter, sksl, &options)
}

// Port of: src/core/SkKnownRuntimeEffects.cpp#L423-L434 (chrome/m156)
fn make_overdraw_color_filter() -> RuntimeEffect {
    let options = get_options(StableKey::Overdraw);
    let sksl = concat!(
        "uniform half4 color0, color1, color2, color3, color4, color5;",
        "half4 main(half4 color) {",
        "return sk_overdraw(color.a, color0, color1, color2, color3, color4, color5);",
        "}"
    );
    make_known(MakeKind::ColorFilter, sksl, &options)
}

/// The cache: one slot per key from `kStart` to `kLast`, each built on first use (the C++ function
/// statics of `GetKnownRuntimeEffect`).
static CACHE: [OnceLock<RuntimeEffect>; 29] = [const { OnceLock::new() }; 29];

/// The effect for `stable_key`, built on first use (`GetKnownRuntimeEffect`). `None` for
/// [`StableKey::Invalid`].
///
/// # Panics
/// If the effect's `SkSL` does not compile (as in Skia, where it asserts).
// Port of: src/core/SkKnownRuntimeEffects.cpp#L513-L604 (chrome/m156)
#[doc(alias = "GetKnownRuntimeEffect")]
#[must_use]
pub fn get_known_runtime_effect(stable_key: StableKey) -> Option<&'static RuntimeEffect> {
    let slot = (stable_key as u32 - SKIA_KNOWN_RUNTIME_EFFECTS_START) as usize;
    let build: fn(StableKey) -> RuntimeEffect = match stable_key {
        StableKey::Invalid => return None,
        // Shaders
        StableKey::OneDBlur4 => |k| make_blur_1d_shader(4, k),
        StableKey::OneDBlur8 => |k| make_blur_1d_shader(8, k),
        StableKey::OneDBlur12 => |k| make_blur_1d_shader(12, k),
        StableKey::OneDBlur16 => |k| make_blur_1d_shader(16, k),
        StableKey::OneDBlur20 => |k| make_blur_1d_shader(20, k),
        StableKey::OneDBlur28 => |k| make_blur_1d_shader(28, k),
        StableKey::TwoDBlur4 => |k| make_blur_2d_shader(4, k),
        StableKey::TwoDBlur8 => |k| make_blur_2d_shader(8, k),
        StableKey::TwoDBlur12 => |k| make_blur_2d_shader(12, k),
        StableKey::TwoDBlur16 => |k| make_blur_2d_shader(16, k),
        StableKey::TwoDBlur20 => |k| make_blur_2d_shader(20, k),
        StableKey::TwoDBlur28 => |k| make_blur_2d_shader(28, k),
        StableKey::Blend => |_| make_blend_shader(),
        StableKey::Lerp => |_| make_lerp_shader(),
        StableKey::MatrixConvUniforms => {
            |k| make_matrix_conv_shader(MatrixConvolutionImpl::UniformBased, k)
        }
        StableKey::MatrixConvTexSm => {
            |k| make_matrix_conv_shader(MatrixConvolutionImpl::TextureBasedSm, k)
        }
        StableKey::MatrixConvTexLg => {
            |k| make_matrix_conv_shader(MatrixConvolutionImpl::TextureBasedLg, k)
        }
        StableKey::Decal => |_| make_decal_shader(),
        StableKey::Displacement => |_| make_displacement_shader(),
        StableKey::Lighting => |_| make_lighting_shader(),
        StableKey::LinearMorphology => |_| make_linear_morphology_shader(),
        StableKey::Magnifier => |_| make_magnifier_shader(),
        StableKey::Normal => |_| make_normal_shader(),
        StableKey::SparseMorphology => |_| make_sparse_morphology_shader(),
        // Blenders
        StableKey::Arithmetic => |_| make_arithmetic_blender(),
        // Color Filters
        StableKey::HighContrast => |_| make_high_contrast_color_filter(),
        StableKey::Luma => |_| make_luma_color_filter(),
        StableKey::Overdraw => |_| make_overdraw_color_filter(),
    };
    Some(CACHE[slot].get_or_init(|| build(stable_key)))
}
