// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/SkImageShader.{h,cpp}

//! `SkImageShader`: a shader that samples an image.
//!
//! skia-rust: Skia is built without `SK_ENABLE_LEGACY_SHADERCONTEXT` (and so are the oracle
//! builds), so there is no legacy fixed-point bitmap proc path (`onMakeContext`,
//! `SkBitmapProcLegacyShader`): every image shader goes through [`ShaderBase::append_stages`].
//! The fused `bilerp_clamp_8888`/`bicubic_clamp_8888` stages are its fast paths. Flattening and
//! the GPU-only subset are not ported ([`ImageShader::make_subset`] only takes a subset that is
//! the whole image, as raster `appendStages` asserts).

use std::sync::Arc;

use skia_rust_simd::rp::contexts::{
    DecalTileCtx, GatherCtx, GatherPixels, MipmapCtx, PixelBytes, SamplerCtx, TileCtx,
};

use crate::alpha_type::AlphaType;
use crate::arena_alloc::ArenaAlloc;
use crate::blend_mode::BlendMode;
use crate::color_space::ColorSpace;
use crate::color_space_xform_steps::ColorSpaceXformSteps;
use crate::color_type::ColorType;
use crate::effect_priv::StageRec;
use crate::image::Image;
use crate::image_info_priv::color_type_is_alpha_only;
use crate::m44::M44;
use crate::matrix::{Matrix, TypeMask};
use crate::mipmap_accessor::MipmapAccessor;
use crate::paint::{Paint, Style};
use crate::raster_pipeline::{RasterPipeline, SRGB_TRANSFER_FUNCTION, Stage};
use crate::rect::{Contains, Rect};
use crate::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use crate::sampling_priv::aniso_fallback;
use crate::shader::Shader;
use crate::shaders;
use crate::shaders::shader_base::{MatrixRec, ShaderBase, ShaderType};
use crate::size::ISize;
use crate::tile_mode::TileMode;

/// A shader that samples an image with tiling and sampling options (`SkImageShader`).
// Port of: src/shaders/SkImageShader.h#L33-L102 (chrome/m156)
#[doc(alias = "SkImageShader")]
#[derive(Clone, Debug)]
pub struct ImageShader {
    image: Image,
    sampling: SamplingOptions,
    tile_mode_x: TileMode,
    tile_mode_y: TileMode,

    // TODO(skbug.com/40043877): This is only supported for GPU images currently.
    // If subset == (0,0,w,h) of the image, then no subset is applied. Subset will not be empty.
    subset: Rect,

    raw: bool,
    clamp_as_if_unpremul: bool,
}

/// We are faster in clamp, so always use that tiling when we can.
// Port of: src/shaders/SkImageShader.cpp#L67-L78 (chrome/m156)
fn optimize(tm: TileMode, dimension: i32) -> TileMode {
    debug_assert!(dimension > 0);
    // mirror and repeat on a 1px axis are the same as clamping, but decal will still transition
    // to transparent black.
    if tm != TileMode::Decal && dimension == 1 {
        TileMode::Clamp
    } else {
        tm
    }
}

impl ImageShader {
    /// The matrix that expands the four rows of cubic weights: `B` and `C` of a
    /// [`CubicResampler`](crate::sampling_options::CubicResampler) to the sampling matrix
    /// (`CubicResamplerMatrix`).
    // Port of: src/shaders/SkImageShader.cpp#L43-L65 (chrome/m156)
    #[doc(alias = "CubicResamplerMatrix")]
    #[must_use]
    pub fn cubic_resampler_matrix(b: f32, c: f32) -> M44 {
        M44::new(
            (1.0f32 / 6.0) * b,
            -(3.0f32 / 6.0) * b - c,
            (3.0f32 / 6.0) * b + 2.0 * c,
            -(1.0f32 / 6.0) * b - c,
            1.0 - (2.0f32 / 6.0) * b,
            0.0,
            -3.0 + (12.0f32 / 6.0) * b + c,
            2.0 - (9.0f32 / 6.0) * b - c,
            (1.0f32 / 6.0) * b,
            (3.0f32 / 6.0) * b + c,
            3.0 - (15.0f32 / 6.0) * b - 2.0 * c,
            -2.0 + (9.0f32 / 6.0) * b + c,
            0.0,
            0.0,
            -c,
            (1.0f32 / 6.0) * b + c,
        )
    }

    /// `SkImageShader(image, subset, tmx, tmy, sampling, raw, clampAsIfUnpremul)`.
    // Port of: src/shaders/SkImageShader.cpp#L84-L100 (chrome/m156)
    #[must_use]
    pub fn new(
        img: Image,
        subset: Rect,
        tmx: TileMode,
        tmy: TileMode,
        sampling: SamplingOptions,
        raw: bool,
        clamp_as_if_unpremul: bool,
    ) -> ImageShader {
        // These options should never appear together:
        debug_assert!(!raw || !clamp_as_if_unpremul);

        // Bicubic filtering of raw image shaders would add a surprising clamp - so we don't
        // support it
        debug_assert!(!raw || !sampling.use_cubic);

        ImageShader {
            tile_mode_x: optimize(tmx, img.width()),
            tile_mode_y: optimize(tmy, img.height()),
            image: img,
            sampling,
            subset,
            raw,
            clamp_as_if_unpremul,
        }
    }

    /// The tile mode in x (`tileModeX`).
    #[doc(alias = "tileModeX")]
    #[must_use]
    pub fn tile_mode_x(&self) -> TileMode {
        self.tile_mode_x
    }

    /// The tile mode in y (`tileModeY`).
    #[doc(alias = "tileModeY")]
    #[must_use]
    pub fn tile_mode_y(&self) -> TileMode {
        self.tile_mode_y
    }

    /// The image (`image`).
    #[must_use]
    pub fn image(&self) -> &Image {
        &self.image
    }

    /// The sampling options (`sampling`).
    #[must_use]
    pub fn sampling(&self) -> SamplingOptions {
        self.sampling
    }

    /// The subset of the image sampled (`subset`).
    #[must_use]
    pub fn subset(&self) -> Rect {
        self.subset
    }

    /// Whether the shader is raw (`isRaw`).
    #[doc(alias = "isRaw")]
    #[must_use]
    pub fn is_raw(&self) -> bool {
        self.raw
    }

    /// A shader of `image` with the given tiling and sampling, mapped by `local_matrix`
    /// (`SkImageShader::Make`). `None` if the sampling options are invalid; an empty shader if
    /// `image` is `None`.
    // Port of: src/shaders/SkImageShader.cpp#L246-L254 (chrome/m156)
    #[must_use]
    pub fn make(
        image: Option<Image>,
        tmx: TileMode,
        tmy: TileMode,
        options: &SamplingOptions,
        local_matrix: Option<&Matrix>,
        clamp_as_if_unpremul: bool,
    ) -> Option<Shader> {
        let subset = image
            .as_ref()
            .map_or_else(Rect::new_empty, |i| Rect::from_isize(i.dimensions()));
        Self::make_subset(
            image,
            &subset,
            tmx,
            tmy,
            options,
            local_matrix,
            clamp_as_if_unpremul,
        )
    }

    /// Like [`make`](Self::make), for images that contain non-color data (`MakeRaw`): `None` for
    /// cubic sampling.
    // Port of: src/shaders/SkImageShader.cpp#L256-L274 (chrome/m156)
    #[doc(alias = "MakeRaw")]
    #[must_use]
    pub fn make_raw(
        image: Option<Image>,
        tmx: TileMode,
        tmy: TileMode,
        options: &SamplingOptions,
        local_matrix: Option<&Matrix>,
    ) -> Option<Shader> {
        if options.use_cubic {
            return None;
        }
        let Some(image) = image else {
            return Some(shaders::empty());
        };
        let subset = Rect::from_isize(image.dimensions());

        let s = Shader::from_base(ImageShader::new(
            image, subset, tmx, tmy, *options, /* raw= */ true,
            /* clamp_as_if_unpremul= */ false,
        ));
        Some(s.with_local_matrix(local_matrix.unwrap_or(Matrix::i())))
    }

    /// A shader of `subset` of `image` (`MakeSubset`).
    // Port of: src/shaders/SkImageShader.cpp#L276-L308 (chrome/m156)
    #[doc(alias = "MakeSubset")]
    #[must_use]
    pub fn make_subset(
        image: Option<Image>,
        subset: &Rect,
        tmx: TileMode,
        tmy: TileMode,
        options: &SamplingOptions,
        local_matrix: Option<&Matrix>,
        clamp_as_if_unpremul: bool,
    ) -> Option<Shader> {
        let is_unit = |x: f32| (0.0..=1.0).contains(&x);
        if options.use_cubic && (!is_unit(options.cubic.b) || !is_unit(options.cubic.c)) {
            return None;
        }
        let image = match image {
            Some(image) if !subset.is_empty() => image,
            _ => return Some(shaders::empty()),
        };

        // Validate subset and check if we can drop it
        if !Rect::from_irect(image.bounds()).contains(subset) {
            return None;
        }

        let s = Shader::from_base(ImageShader::new(
            image,
            *subset,
            tmx,
            tmy,
            *options,
            /* raw= */ false,
            clamp_as_if_unpremul,
        ));
        Some(s.with_local_matrix(local_matrix.unwrap_or(Matrix::i())))
    }

    /// Given arguments for a call to `Canvas::draw_image_rect`, returns a possibly adjusted `dst`
    /// rect and a shader to apply to the paint such that calling `Canvas::draw_rect(new_dst,
    /// paint)` produces visually equivalent results to the original `draw_image_rect()` call
    /// (`MakeForDrawRect`). The shader is `None` if there is nothing to draw.
    // Port of: src/shaders/SkImageShader.cpp#L312-L359 (chrome/m156)
    #[doc(alias = "MakeForDrawRect")]
    #[must_use]
    pub fn make_for_draw_rect(
        image: &Image,
        paint: &Paint,
        sampling: &SamplingOptions,
        mut src: Rect,
        mut dst: Rect,
        strict_src_subset: bool,
    ) -> (Rect, Option<Shader>) {
        // The paint should have already been cleaned for a regular drawImageRect, e.g. no path
        // effect and is a fill.
        debug_assert!(paint.style() == Style::Fill && paint.path_effect().is_none());

        let img_bounds = Rect::from_irect(image.bounds());

        debug_assert!(src.is_finite() && dst.is_finite() && dst.is_sorted());
        let local_matrix = Matrix::rect_to_rect_or_identity(src, dst, None);
        if !img_bounds.contains(&src) {
            if !src.intersect(img_bounds) {
                return (Rect::new_empty(), None); // Nothing to draw for this entry
            }
            // Update dst to match smaller src
            dst = local_matrix.map_rect(src).0;
        }

        let image_is_alpha_only = color_type_is_alpha_only(image.color_type());

        let img_shader = if strict_src_subset {
            ImageShader::make_subset(
                Some(image.clone()),
                &src,
                TileMode::Clamp,
                TileMode::Clamp,
                sampling,
                Some(&local_matrix),
                false,
            )
        } else {
            image.to_shader((TileMode::Clamp, TileMode::Clamp), *sampling, &local_matrix)
        };
        let Some(mut img_shader) = img_shader else {
            return (Rect::new_empty(), None);
        };
        if image_is_alpha_only && let Some(paint_shader) = paint.shader() {
            // Compose the image shader with the paint's shader. Alpha images+shaders should
            // output the texture's alpha multiplied by the shader's color. DstIn (d*sa) will
            // achieve this with the source image and dst shader (MakeBlend takes dst first, src
            // second).
            img_shader = shaders::blend(BlendMode::DstIn, paint_shader, img_shader);
        }
        (dst, Some(img_shader))
    }
}

// Port of: src/shaders/SkImageShader.cpp#L492-L505 (chrome/m156)
fn tweak_sampling(sampling: &SamplingOptions, matrix: &Matrix) -> SamplingOptions {
    let mut filter = sampling.filter;

    // When the matrix is just an integer translate, bilerp == nearest neighbor.
    #[allow(clippy::cast_possible_truncation)] // mirrors the (int) casts of the translation
    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::float_cmp)]
    if filter == FilterMode::Linear
        && matrix.get_type().bits() <= TypeMask::TRANSLATE.bits()
        && matrix.translate_x() == (matrix.translate_x() as i32) as f32
        && matrix.translate_y() == (matrix.translate_y() as i32) as f32
    {
        filter = FilterMode::Nearest;
    }

    SamplingOptions::new(filter, sampling.mipmap)
}

/// One level of the image being sampled and the contexts of its stages (`MipLevelHelper`).
struct MipLevelHelper<'a> {
    color_type: ColorType,
    gather: &'a GatherCtx<'static>,
    limit_x: &'a TileCtx,
    limit_y: &'a TileCtx,
    decal_ctx: Option<&'a DecalTileCtx>,
}

impl<'a> MipLevelHelper<'a> {
    // Port of: src/shaders/SkImageShader.cpp#L413-L474 (chrome/m156)
    fn alloc_and_init(
        alloc: &'a ArenaAlloc,
        pixels: Arc<dyn PixelBytes>,
        pm_info: (ISize, ColorType, usize),
        sampling: &SamplingOptions,
        tile_mode_x: TileMode,
        tile_mode_y: TileMode,
    ) -> MipLevelHelper<'a> {
        let (dims, color_type, stride) = pm_info;
        #[allow(clippy::cast_precision_loss)] // mirrors the float conversions of the C++
        let (w, h) = (dims.width as f32, dims.height as f32);
        let mut gather = GatherCtx {
            pixels: GatherPixels::Shared(pixels),
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            // a row of pixels fits an int
            stride: stride as i32,
            width: w,
            height: h,
            weights: [0.0; 16],
            round_down_at_integer: false,
        };

        if sampling.use_cubic {
            ImageShader::cubic_resampler_matrix(sampling.cubic.b, sampling.cubic.c)
                .get_col_major(&mut gather.weights);
        }

        let mut limit_x = TileCtx {
            scale: w,
            inv_scale: 1.0 / w,
            mirror_bias_dir: -1,
        };
        let mut limit_y = TileCtx {
            scale: h,
            inv_scale: 1.0 / h,
            mirror_bias_dir: -1,
        };

        // We would like an image that is mapped 1:1 with device pixels but at a half pixel
        // offset to select every pixel from the src image once. Our rasterizer biases upward.
        // That is a rect from 0.5...1.5 fills pixel 1 and not pixel 0. So we make exact integer
        // pixel sample values select the pixel to the left/above the integer value.
        //
        // Note that a mirror mapping between canvas and image space will not have this property -
        // on one side of the image a row/column will be skipped and one repeated on the other
        // side.
        //
        // The GM nearest_half_pixel_image tests both of the above scenarios.
        //
        // The implementation of SkTileMode::kMirror also modifies integer pixel snapping to
        // create consistency when the sample coords are running backwards and must account for
        // gather modification we perform here. The GM mirror_tile tests this.
        if !sampling.use_cubic && sampling.filter == FilterMode::Nearest {
            gather.round_down_at_integer = true;
            limit_x.mirror_bias_dir = 1;
            limit_y.mirror_bias_dir = 1;
        }

        let decal_ctx = if tile_mode_x == TileMode::Decal || tile_mode_y == TileMode::Decal {
            let mut decal = DecalTileCtx {
                limit_x: limit_x.scale,
                limit_y: limit_y.scale,
                ..DecalTileCtx::default()
            };

            // When integer sample coords snap left/up then we want the right/bottom edge of the
            // image bounds to be inside the image rather than the left/top edge, that is (0, w]
            // rather than [0, w).
            if gather.round_down_at_integer {
                decal.inclusive_edge_x = decal.limit_x;
                decal.inclusive_edge_y = decal.limit_y;
            }
            Some(alloc.make(decal))
        } else {
            None
        };

        MipLevelHelper {
            color_type,
            gather: alloc.make(gather),
            limit_x: alloc.make(limit_x),
            limit_y: alloc.make(limit_y),
            decal_ctx,
        }
    }
}

/// The tiling stages and the gather of one level (`append_tiling_and_gather`).
// Port of: src/shaders/SkImageShader.cpp#L577-L698 (chrome/m156)
fn append_tiling_and_gather<'a>(
    p: &mut RasterPipeline<'a>,
    level: &MipLevelHelper<'a>,
    tile_mode_x: TileMode,
    tile_mode_y: TileMode,
    decal_both_axes: bool,
) {
    let decal = |level: &MipLevelHelper<'a>| level.decal_ctx.expect("a decal context");
    if decal_both_axes {
        p.append(Stage::DecalXAndY(decal(level)));
    } else {
        match tile_mode_x {
            TileMode::Clamp => { /* The gather_xxx stage will clamp for us. */ }
            TileMode::Mirror => p.append(Stage::MirrorX(level.limit_x)),
            TileMode::Repeat => p.append(Stage::RepeatX(level.limit_x)),
            TileMode::Decal => p.append(Stage::DecalX(decal(level))),
        }
        match tile_mode_y {
            TileMode::Clamp => { /* The gather_xxx stage will clamp for us. */ }
            TileMode::Mirror => p.append(Stage::MirrorY(level.limit_y)),
            TileMode::Repeat => p.append(Stage::RepeatY(level.limit_y)),
            TileMode::Decal => p.append(Stage::DecalY(decal(level))),
        }
    }

    let ctx = level.gather;
    match level.color_type {
        ColorType::Alpha8 => p.append(Stage::GatherA8(ctx)),
        ColorType::A16UNorm => p.append(Stage::GatherA16(ctx)),
        ColorType::A16Float => p.append(Stage::GatherAf16(ctx)),
        ColorType::R16Float => p.append(Stage::GatherRf16(ctx)),
        ColorType::RGB565 => p.append(Stage::Gather565(ctx)),
        ColorType::ARGB4444 => p.append(Stage::Gather4444(ctx)),
        ColorType::R8G8UNorm => p.append(Stage::GatherRg88(ctx)),
        ColorType::R16UNorm => p.append(Stage::GatherR16(ctx)),
        ColorType::R16G16UNorm => p.append(Stage::GatherRg1616(ctx)),
        ColorType::R16G16Float => p.append(Stage::GatherRgf16(ctx)),
        ColorType::RGBA8888 => p.append(Stage::Gather8888(ctx)),

        ColorType::RGBA1010102 => p.append(Stage::Gather1010102(ctx)),

        ColorType::R16G16B16A16UNorm => p.append(Stage::Gather16161616(ctx)),

        ColorType::RGBAF16Norm | ColorType::RGBAF16 => p.append(Stage::GatherF16(ctx)),
        ColorType::RGBAF32 => p.append(Stage::GatherF32(ctx)),
        ColorType::BGRA10101010XR => {
            p.append(Stage::Gather10101010Xr(ctx));
            p.append(Stage::SwapRb);
        }
        ColorType::RGBA10x6 => p.append(Stage::Gather10x6(ctx)),

        ColorType::Gray8 => {
            p.append(Stage::GatherA8(ctx));
            p.append(Stage::AlphaToGray);
        }

        ColorType::R8UNorm => {
            p.append(Stage::GatherA8(ctx));
            p.append(Stage::AlphaToRed);
        }

        ColorType::RGB888x => {
            p.append(Stage::Gather8888(ctx));
            p.append(Stage::ForceOpaque);
        }
        ColorType::RGBF16F16F16x => {
            p.append(Stage::GatherF16(ctx));
            p.append(Stage::ForceOpaque);
        }
        ColorType::BGRA1010102 => {
            p.append(Stage::Gather1010102(ctx));
            p.append(Stage::SwapRb);
        }

        ColorType::RGB101010x => {
            p.append(Stage::Gather1010102(ctx));
            p.append(Stage::ForceOpaque);
        }

        ColorType::BGR101010xXR => {
            p.append(Stage::Gather1010102Xr(ctx));
            p.append(Stage::ForceOpaque);
            p.append(Stage::SwapRb);
        }

        ColorType::BGR101010x => {
            p.append(Stage::Gather1010102(ctx));
            p.append(Stage::ForceOpaque);
            p.append(Stage::SwapRb);
        }

        ColorType::BGRA8888 => {
            p.append(Stage::Gather8888(ctx));
            p.append(Stage::SwapRb);
        }

        ColorType::SRGBA8888 => {
            p.append(Stage::Gather8888(ctx));
            p.append_transfer_function(&SRGB_TRANSFER_FUNCTION);
        }

        ColorType::Unknown => debug_assert!(false),
    }
    if let Some(decal_ctx) = level.decal_ctx {
        p.append(Stage::CheckDecalMask(decal_ctx));
    }
}

impl ImageShader {
    // Port of: src/shaders/SkImageShader.cpp#L507-L862 (chrome/m156)
    #[allow(clippy::too_many_lines)] // one function in the C++
    fn append_stages_impl<'a>(&self, rec: &mut StageRec<'_, 'a>, m_rec: &MatrixRec) -> bool {
        debug_assert_eq!(self.subset, Rect::from_isize(self.image.dimensions())); // TODO(skbug.com/40043877)

        // We only support certain sampling options in stages so far
        let mut sampling = self.sampling;
        if sampling.is_aniso() {
            sampling = aniso_fallback(self.image.has_mipmaps());
        }

        let alloc: &'a ArenaAlloc = rec.alloc;

        let mut base_inv = Matrix::new_identity();
        // If the total matrix isn't valid then we will always access the base MIP level.
        if m_rec.total_matrix_is_valid() {
            let Some(inv) = m_rec.total_inverse() else {
                return false;
            };
            base_inv = inv;
            base_inv.normalize_perspective();
        }

        debug_assert!(!sampling.use_cubic || sampling.mipmap == MipmapMode::None);
        let Some(access) = MipmapAccessor::make(alloc, &self.image, &base_inv, sampling.mipmap)
        else {
            return false;
        };

        let (upper_pm, upper_inv) = access.level();
        let upper_info = (
            upper_pm.dimensions(),
            upper_pm.color_type(),
            upper_pm.row_bytes_as_pixels(),
        );
        let upper_cs = upper_pm.color_space();
        let upper_at = upper_pm.alpha_type();
        let upper_color_type = upper_pm.color_type();

        if !sampling.use_cubic {
            // TODO: can tweak_sampling sometimes for cubic too when B=0
            if m_rec.total_matrix_is_valid() {
                sampling = tweak_sampling(&sampling, &Matrix::concat(&upper_inv, &base_inv));
            }
        }

        if m_rec.apply(rec, &upper_inv).is_none() {
            return false;
        }

        let upper = MipLevelHelper::alloc_and_init(
            alloc,
            access.upper_bytes(),
            upper_info,
            &sampling,
            self.tile_mode_x,
            self.tile_mode_y,
        );

        let mut lower = None;
        let mut mipmap_ctx: Option<&'a MipmapCtx> = None;
        let lower_weight = access.lower_weight();
        if lower_weight > 0.0 {
            let (lower_pm, _lower_inv) = access.lower_level();
            #[allow(clippy::cast_precision_loss)] // mirrors static_cast<float>
            let ctx = MipmapCtx {
                lower_weight,
                scale_x: lower_pm.width() as f32 / upper_pm.width() as f32,
                scale_y: lower_pm.height() as f32 / upper_pm.height() as f32,
                ..MipmapCtx::default()
            };
            mipmap_ctx = Some(alloc.make(ctx));

            lower = Some(MipLevelHelper::alloc_and_init(
                alloc,
                access.lower_bytes(),
                (
                    lower_pm.dimensions(),
                    lower_pm.color_type(),
                    lower_pm.row_bytes_as_pixels(),
                ),
                &sampling,
                self.tile_mode_x,
                self.tile_mode_y,
            ));

            rec.pipeline
                .append(Stage::MipmapLinearInit(mipmap_ctx.expect("just made")));
        }

        let decal_both_axes =
            self.tile_mode_x == TileMode::Decal && self.tile_mode_y == TileMode::Decal;

        let fast_8888 = (upper_color_type == ColorType::RGBA8888
            || upper_color_type == ColorType::BGRA8888)
            && !sampling.use_cubic
            && sampling.filter == FilterMode::Linear
            && sampling.mipmap != MipmapMode::Linear
            && self.tile_mode_x == TileMode::Clamp
            && self.tile_mode_y == TileMode::Clamp;
        let fast_bicubic_8888 = (upper_color_type == ColorType::RGBA8888
            || upper_color_type == ColorType::BGRA8888)
            && sampling.use_cubic
            && self.tile_mode_x == TileMode::Clamp
            && self.tile_mode_y == TileMode::Clamp;

        // Check for fast-path stages.
        // TODO: Could we use the fast-path stages for each level when doing linear mipmap
        // filtering?
        if fast_8888 {
            // Check bounding box of points we will sample to see if we can use lowp
            // and not over/under flow.
            let mut should_use_highp_bilerp = false;
            if !rec.dst_bounds.is_empty() {
                let mut quad = rec.dst_bounds.to_quad(None);
                base_inv.map_points_inplace(&mut quad);
                let mut device_image_space = Rect::new_empty();
                device_image_space.set_bounds(&quad);
                for val in [
                    device_image_space.left,
                    device_image_space.top,
                    device_image_space.right,
                    device_image_space.bottom,
                ] {
                    if val > f32::from(i16::MAX) || val < f32::from(i16::MIN) || !val.is_finite() {
                        should_use_highp_bilerp = true;
                        break;
                    }
                }
            }

            if should_use_highp_bilerp {
                rec.pipeline
                    .append(Stage::BilerpClamp8888ForceHighp(upper.gather));
            } else {
                rec.pipeline.append(Stage::BilerpClamp8888(upper.gather));
            }

            if upper_color_type == ColorType::BGRA8888 {
                rec.pipeline.append(Stage::SwapRb);
            }
            return self.append_misc(rec, &sampling, upper_cs, upper_at, upper_color_type);
        }
        if fast_bicubic_8888 {
            rec.pipeline.append(Stage::BicubicClamp8888(upper.gather));
            if upper_color_type == ColorType::BGRA8888 {
                rec.pipeline.append(Stage::SwapRb);
            }
            return self.append_misc(rec, &sampling, upper_cs, upper_at, upper_color_type);
        }

        // This context can be shared by both levels when doing linear mipmap filtering
        let mut sampler = SamplerCtx::default();
        if sampling.use_cubic {
            sampler.weights = upper.gather.weights;
        }
        let sampler: &'a SamplerCtx = alloc.make(sampler);

        let (tmx, tmy) = (self.tile_mode_x, self.tile_mode_y);
        let sample_level = |p: &mut RasterPipeline<'a>, level: &MipLevelHelper<'a>| {
            let sample = |p: &mut RasterPipeline<'a>, setup_x: Stage<'a>, setup_y: Stage<'a>| {
                p.append(setup_x);
                p.append(setup_y);
                append_tiling_and_gather(p, level, tmx, tmy, decal_both_axes);
                p.append(Stage::Accumulate(sampler));
            };

            if sampling.use_cubic {
                p.append(Stage::BicubicSetup(sampler));

                let xs = [
                    Stage::BicubicN3x(sampler),
                    Stage::BicubicN1x(sampler),
                    Stage::BicubicP1x(sampler),
                    Stage::BicubicP3x(sampler),
                ];
                let ys = [
                    Stage::BicubicN3y(sampler),
                    Stage::BicubicN1y(sampler),
                    Stage::BicubicP1y(sampler),
                    Stage::BicubicP3y(sampler),
                ];
                for y in ys {
                    for x in xs {
                        sample(p, x, y);
                    }
                }

                p.append(Stage::MoveDstSrc);
            } else if sampling.filter == FilterMode::Linear {
                p.append(Stage::BilinearSetup(sampler));

                sample(p, Stage::BilinearNx(sampler), Stage::BilinearNy(sampler));
                sample(p, Stage::BilinearPx(sampler), Stage::BilinearNy(sampler));
                sample(p, Stage::BilinearNx(sampler), Stage::BilinearPy(sampler));
                sample(p, Stage::BilinearPx(sampler), Stage::BilinearPy(sampler));

                p.append(Stage::MoveDstSrc);
            } else {
                append_tiling_and_gather(p, level, tmx, tmy, decal_both_axes);
            }
        };

        sample_level(rec.pipeline, &upper);

        if let Some(mipmap_ctx) = mipmap_ctx {
            rec.pipeline.append(Stage::MipmapLinearUpdate(mipmap_ctx));
            sample_level(rec.pipeline, lower.as_ref().expect("a lower level"));
            rec.pipeline.append(Stage::MipmapLinearFinish(mipmap_ctx));
        }

        self.append_misc(rec, &sampling, upper_cs, upper_at, upper_color_type)
    }

    // Port of: src/shaders/SkImageShader.cpp#L706-L735 (chrome/m156)
    fn append_misc<'a>(
        &self,
        rec: &mut StageRec<'_, 'a>,
        sampling: &SamplingOptions,
        upper_cs: Option<ColorSpace>,
        upper_at: AlphaType,
        upper_color_type: ColorType,
    ) -> bool {
        let alloc: &'a ArenaAlloc = rec.alloc;
        let mut cs = upper_cs;
        let mut at = upper_at;

        // Color for alpha-only images comes from the paint (already converted to dst color
        // space). If we were sampled by a runtime effect, the paint color was replaced with
        // transparent black, so this tinting is effectively suppressed. See also:
        // RuntimeEffectRPCallbacks
        if color_type_is_alpha_only(upper_color_type) && !self.raw {
            rec.pipeline.append_set_rgb_color4f(alloc, &rec.paint_color);

            cs = rec.dst_cs.cloned();
            at = AlphaType::Unpremul;
        }

        // Bicubic filtering naturally produces out of range values on both sides of [0,1].
        if sampling.use_cubic {
            rec.pipeline
                .append(if at == AlphaType::Unpremul || self.clamp_as_if_unpremul {
                    Stage::Clamp01
                } else {
                    Stage::ClampGamut
                });
        }

        // Transform color space and alpha type to match shader convention (dst CS, premul
        // alpha).
        if !self.raw {
            ColorSpaceXformSteps::new(cs.as_ref(), at, rec.dst_cs, AlphaType::Premul)
                .apply_to_pipeline(rec.pipeline, alloc);
        }

        true
    }
}

impl ShaderBase for ImageShader {
    // Port of: src/shaders/SkImageShader.cpp#L180-L183 (chrome/m156)
    fn is_opaque(&self) -> bool {
        self.image.is_opaque()
            && self.tile_mode_x != TileMode::Decal
            && self.tile_mode_y != TileMode::Decal
    }

    // Port of: src/shaders/SkImageShader.h#L56 (chrome/m156)
    fn shader_type(&self) -> ShaderType {
        ShaderType::Image
    }

    // Port of: src/shaders/SkImageShader.cpp#L235-L244 (chrome/m156)
    fn on_is_a_image(&self) -> Option<(Image, Matrix, (TileMode, TileMode))> {
        Some((
            self.image.clone(),
            Matrix::new_identity(),
            (self.tile_mode_x, self.tile_mode_y),
        ))
    }

    fn append_stages(&self, rec: &mut StageRec<'_, '_>, m_rec: &MatrixRec) -> bool {
        self.append_stages_impl(rec, m_rec)
    }
}
