// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/TiledTextureUtils.h, src/gpu/TiledTextureUtils.cpp
//
// Only the CPU helpers are ported: `ShouldTileImage`, `OptimizeSampleArea`, `CanDisableMipmap`
// and `ClampedOutsetWithOffset`. `DrawAsTiledImageRect` draws through `SkCanvas` and the Ganesh
// and Graphite contexts, so it waits for the drawing layers (G10).

// The integer casts below mirror the C++ `int`/`size_t` arithmetic of the original: tile counts
// and byte sizes are `usize` with the same wrap-around as C++ unsigned arithmetic.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{Contains, IRect, Rect, RoundOut};
use skia_rust_core::safe_math::SafeMath;
use skia_rust_core::size::ISize;

// Port of: src/gpu/TiledTextureUtils.cpp#L19 (chrome/m156)
const K_BMP_SMALL_TILE_SIZE: i32 = 1 << 10;

// SkPMColor is 32 bits (include/core/SkColor.h).
const SK_PMCOLOR_BYTES: usize = 4;

// SK_ScalarRoot2Over2 (include/core/SkScalar.h): the float nearest sqrt(2)/2, which is
// `FRAC_1_SQRT_2` rounded to f32.
const SK_SCALAR_ROOT2_OVER2: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// The result of [`optimize_sample_area`] (`TiledTextureUtils::ImageDrawMode`).
// Port of: src/gpu/TiledTextureUtils.h#L37-L51 (chrome/m156)
#[doc(alias = "skgpu::TiledTextureUtils::ImageDrawMode")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageDrawMode {
    /// Src and dst have been restricted to the image content. May need to clamp, no need to
    /// decal.
    Optimized,
    /// Src and dst are their original sizes, requires use of a decal instead of plain clamping.
    /// This is used when a dst clip is provided and extends outside of the optimized dst rect.
    Decal,
    /// Src or dst are empty, or do not intersect the image content so don't draw anything.
    Skip,
}

// Port of: src/gpu/TiledTextureUtils.cpp#L33-L41 (chrome/m156)
fn get_tile_count(src_rect: &IRect, tile_size: i32) -> usize {
    let tiles_x = (src_rect.right / tile_size) - (src_rect.left / tile_size) + 1;
    let tiles_y = (src_rect.bottom / tile_size) - (src_rect.top / tile_size) + 1;
    // We calculate expected tile count before we read the bitmap's pixels, so hypothetically we
    // can have lazy images with excessive dimensions that would cause (tilesX*tilesY) to
    // overflow int. `SkSafeMath::Mul` saturates instead.
    let mut sm = SafeMath::new();
    sm.mul(tiles_x as usize, tiles_y as usize)
}

// Port of: src/gpu/TiledTextureUtils.cpp#L43-L59 (chrome/m156)
fn determine_tile_size(src: &IRect, max_tile_size: i32) -> i32 {
    if max_tile_size <= K_BMP_SMALL_TILE_SIZE {
        return max_tile_size;
    }

    let mut max_tile_total_tile_size = get_tile_count(src, max_tile_size);
    let mut small_total_tile_size = get_tile_count(src, K_BMP_SMALL_TILE_SIZE);

    max_tile_total_tile_size =
        max_tile_total_tile_size.wrapping_mul(max_tile_size.wrapping_mul(max_tile_size) as usize);
    small_total_tile_size = small_total_tile_size
        .wrapping_mul((K_BMP_SMALL_TILE_SIZE * K_BMP_SMALL_TILE_SIZE) as usize);

    if max_tile_total_tile_size > small_total_tile_size.wrapping_mul(2) {
        K_BMP_SMALL_TILE_SIZE
    } else {
        max_tile_size
    }
}

// Given a bitmap, an optional src rect, and a context with a clip and matrix determine what
// pixels from the bitmap are necessary.
// Port of: src/gpu/TiledTextureUtils.cpp#L63-L88 (chrome/m156)
fn determine_clipped_src_rect(
    clipped_src_irect: IRect,
    view_matrix: &Matrix,
    src_to_dst_rect: &Matrix,
    image_dimensions: ISize,
    src_rect_ptr: Option<&Rect>,
) -> IRect {
    let Some(inv) = Matrix::concat(view_matrix, src_to_dst_rect).invert() else {
        return IRect::new_empty();
    };

    let mut clipped_rect = Rect::from_irect(clipped_src_irect);
    clipped_rect = inv.map_rect(clipped_rect).0;
    if src_rect_ptr.is_some_and(|src_rect| !clipped_rect.intersect(*src_rect)) {
        return IRect::new_empty();
    }

    let clipped_src_irect: IRect = clipped_rect.round_out();
    let bmp_bounds = IRect::from_size(image_dimensions);
    match IRect::intersect(&clipped_src_irect, &bmp_bounds) {
        Some(r) => r,
        None => IRect::new_empty(),
    }
}

// tileSize and clippedSubset are valid if true is returned
// Port of: src/gpu/TiledTextureUtils.cpp#L209-L257 (chrome/m156)
#[doc(alias = "skgpu::TiledTextureUtils::ShouldTileImage")]
#[must_use]
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
pub fn should_tile_image(
    conservative_clip_bounds: IRect,
    image_size: ISize,
    ctm: &Matrix,
    src_to_dst: &Matrix,
    src: Option<&Rect>,
    max_tile_size: i32,
    cache_size: usize,
    tile_size: &mut i32,
    clipped_subset: &mut IRect,
) -> bool {
    // if it's larger than the max tile size, then we have no choice but tiling.
    if image_size.width > max_tile_size || image_size.height > max_tile_size {
        *clipped_subset =
            determine_clipped_src_rect(conservative_clip_bounds, ctm, src_to_dst, image_size, src);
        *tile_size = determine_tile_size(clipped_subset, max_tile_size);
        return true;
    }

    // If the image would only produce 4 tiles of the smaller size, don't bother tiling it.
    let area = image_size.width.wrapping_mul(image_size.height) as usize;
    if area < 4 * (K_BMP_SMALL_TILE_SIZE as usize) * (K_BMP_SMALL_TILE_SIZE as usize) {
        return false;
    }

    // At this point we know we could do the draw by uploading the entire bitmap as a texture.
    // However, if the texture would be large compared to the cache size and we don't require most
    // of it for this draw then tile to reduce the amount of upload and cache spill.
    if cache_size == 0 {
        // We don't have access to the cacheSize so we will just upload the entire image
        // to be on the safe side and not tile.
        return false;
    }

    // An assumption here is that sw bitmap size is a good proxy for its size as a texture
    let bmp_size = area.wrapping_mul(SK_PMCOLOR_BYTES);
    if bmp_size < cache_size / 2 {
        return false;
    }

    // Figure out how much of the src we will need based on the src rect and clipping. Reject if
    // tiling memory savings would be < 50%.
    *clipped_subset =
        determine_clipped_src_rect(conservative_clip_bounds, ctm, src_to_dst, image_size, src);
    *tile_size = K_BMP_SMALL_TILE_SIZE; // already know whole bitmap fits in one max sized tile.

    let used_tile_bytes = get_tile_count(clipped_subset, K_BMP_SMALL_TILE_SIZE)
        .wrapping_mul(K_BMP_SMALL_TILE_SIZE as usize)
        .wrapping_mul(K_BMP_SMALL_TILE_SIZE as usize)
        .wrapping_mul(SK_PMCOLOR_BYTES); // assume 32bit pixels
    used_tile_bytes.wrapping_mul(2) < bmp_size
}

/// Optimize the src rect sampling area within an image (sized `image_size`) such that
/// `out_src_rect` will be completely contained in the image's bounds. The corresponding rect to
/// draw will be output to `out_dst_rect`. The mapping between src and dst will be cached in
/// `out_src_to_dst`. Outputs are not always updated when [`ImageDrawMode::Skip`] is returned.
///
/// `dst_clip` should be `None` when there is no additional clipping.
// Port of: src/gpu/TiledTextureUtils.cpp#L267-L311 (chrome/m156)
#[doc(alias = "skgpu::TiledTextureUtils::OptimizeSampleArea")]
#[must_use]
pub fn optimize_sample_area(
    image_size: ISize,
    orig_src_rect: &Rect,
    orig_dst_rect: &Rect,
    dst_clip: Option<&[Point; 4]>,
    out_src_rect: &mut Rect,
    out_dst_rect: &mut Rect,
    out_src_to_dst: &mut Matrix,
) -> ImageDrawMode {
    if orig_src_rect.is_empty() || orig_dst_rect.is_empty() {
        return ImageDrawMode::Skip;
    }

    *out_src_to_dst = Matrix::rect_to_rect_or_identity(orig_src_rect, orig_dst_rect, None);

    let mut src = *orig_src_rect;
    let mut dst = *orig_dst_rect;

    let src_bounds = Rect::from_isize(image_size);
    if !src_bounds.contains(src) {
        if !src.intersect(src_bounds) {
            return ImageDrawMode::Skip;
        }

        dst = out_src_to_dst.map_rect(src).0;

        // Both src and dst have gotten smaller. If dstClip is provided, confirm it is still
        // contained in dst, otherwise cannot optimize the sample area and must use a decal
        // instead
        if let Some(clip) = dst_clip {
            for corner in clip {
                if !dst.contains(*corner) {
                    // Must resort to using a decal mode restricted to the clipped 'src', and
                    // use the original dst rect (filling in src bounds as needed)
                    *out_src_rect = src;
                    *out_dst_rect = *orig_dst_rect;
                    return ImageDrawMode::Decal;
                }
            }
        }
    }

    // The original src and dst were fully contained in the image, or there was no dst clip to
    // worry about, or the clip was still contained in the restricted dst rect.
    *out_src_rect = src;
    *out_dst_rect = dst;
    ImageDrawMode::Optimized
}

// Port of: src/gpu/TiledTextureUtils.cpp#L313-L329 (chrome/m156)
#[doc(alias = "skgpu::TiledTextureUtils::CanDisableMipmap")]
#[must_use]
pub fn can_disable_mipmap(
    view_m: &Matrix,
    local_m: &Matrix,
    sharpen_mipmapped_textures: bool,
) -> bool {
    let matrix = Matrix::concat(view_m, local_m);
    // With sharp mips, we bias mipmap lookups by -0.5. That means our final LOD is >= 0 until
    // the computed LOD is >= 0.5. At what scale factor does a texture get an LOD of 0.5?
    //
    // Want:  0       = log2(1/s) - 0.5
    //        0.5     = log2(1/s)
    //        2^0.5   = 1/s
    //        1/2^0.5 = s
    //        2^0.5/2 = s
    let mip_scale = if sharpen_mipmapped_textures {
        SK_SCALAR_ROOT2_OVER2
    } else {
        1.0
    };
    matrix.min_scale() >= mip_scale
}

// This method outsets 'iRect' by 'outset' all around and then clamps its extents to 'clamp'.
// 'offset' is adjusted to remain positioned over the top-left corner of 'iRect' for all possible
// outsets/clamps.
// Port of: src/gpu/TiledTextureUtils.cpp#L335-L361 (chrome/m156)
#[doc(alias = "skgpu::TiledTextureUtils::ClampedOutsetWithOffset")]
pub fn clamped_outset_with_offset(
    i_rect: &mut IRect,
    outset: i32,
    offset: &mut Point,
    clamp: IRect,
) {
    i_rect.outset((outset, outset));

    let left_clamp_delta = clamp.left - i_rect.left;
    if left_clamp_delta > 0 {
        offset.x -= (outset - left_clamp_delta) as f32;
        i_rect.left = clamp.left;
    } else {
        offset.x -= outset as f32;
    }

    let top_clamp_delta = clamp.top - i_rect.top;
    if top_clamp_delta > 0 {
        offset.y -= (outset - top_clamp_delta) as f32;
        i_rect.top = clamp.top;
    } else {
        offset.y -= outset as f32;
    }

    if i_rect.right > clamp.right {
        i_rect.right = clamp.right;
    }
    if i_rect.bottom > clamp.bottom {
        i_rect.bottom = clamp.bottom;
    }
}
