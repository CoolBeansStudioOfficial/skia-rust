// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/SkPictureShader.{h,cpp} (`SkPictureShader`, `ImageFromPictureKey`,
// `ImageFromPictureRec`), src/core/SkPicture.cpp (`SkPicture::makeShader`)

//! Picture shaders: `SkPicture::makeShader` draws a picture into a tile image, cached in the
//! resource cache, and tiles that image with an image shader.
//!
//! The tile is rasterized into a raster surface, so the shader lives in this crate and the
//! constructor is the [`PictureShaderExt::to_shader`] extension method (core cannot name the
//! raster device).
//!
//! skia-rust: flattening is not ported (`SkPicturePriv::Flatten` needs the nested `PICTURE`
//! section of the picture format, which the picture serializer does not write yet), so the
//! shader has no type name and cannot be serialized.

use std::any::Any;
use std::mem::size_of;
use std::sync::PoisonError;

use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::effect_priv::StageRec;
use skia_rust_core::floating_point::is_finite;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_info_priv::color_type_max_bits_per_channel;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_priv;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::resource_cache::{Key, Rec, ResourceCache};
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::{SCALAR_NEARLY_ZERO, scalar};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_core::shaders::shader_base::{MatrixRec, ShaderBase, ShaderType};
use skia_rust_core::size::{ISize, Size};
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::tile_mode::TileMode;

use crate::oracle_n32::ExplicitColorType;
use crate::surfaces;

/// `kMaxTileArea`: the tile is clamped to about 4M pixels.
// Port of: src/shaders/SkPictureShader.cpp#L234 (chrome/m156), `CachedImageInfo::Make`
const K_MAX_TILE_AREA: scalar = 2048.0 * 2048.0;

/// The namespace label of the picture shader cache keys (`gImageFromPictureKeyNamespaceLabel`).
// Port of: src/shaders/SkPictureShader.cpp#L43 (chrome/m156)
static IMAGE_FROM_PICTURE_KEY_NAMESPACE_LABEL: u8 = 0;

/// The `SkPictureShader::CachedImageInfo` of one tile: the tile image's info, its props, the
/// scale from the picture to the tile, and the matrix that draws the picture into the tile.
// Port of: src/shaders/SkPictureShader.h (`SkPictureShader::CachedImageInfo`, chrome/m156)
#[derive(Debug)]
struct CachedImageInfo {
    /// `tileScale`.
    tile_scale: Size,
    /// `matrixForDraw`.
    matrix_for_draw: Matrix,
    /// `imageInfo`.
    image_info: ImageInfo,
    /// `props`.
    props: SurfaceProps,
}

impl CachedImageInfo {
    // Port of: src/shaders/SkPictureShader.cpp#L158-L213 (chrome/m156), `CachedImageInfo::Make`
    //
    // `max_texture_size` is always 0 for the CPU path, so the GPU clamp is not ported.
    fn make(
        bounds: &Rect,
        total_m: &Matrix,
        dst_color_type: ColorType,
        dst_color_space: Option<&ColorSpace>,
        props_in: &SurfaceProps,
    ) -> Option<CachedImageInfo> {
        let props = props_in
            .clone_with_pixel_geometry(skia_rust_core::surface_props::PixelGeometry::Unknown);

        // Use a rotation-invariant scale.
        let mut size = total_m.decompose_scale(None).unwrap_or_else(|| {
            let center = Point::new(bounds.center_x(), bounds.center_y());
            let area = skia_rust_core::matrix_priv::differential_area_scale(total_m, center);
            if !is_finite(area) || area.abs() <= SCALAR_NEARLY_ZERO {
                Size::new(1.0, 1.0) // ill-conditioned matrix
            } else {
                let root = area.sqrt();
                Size::new(root, root)
            }
        });
        size.width *= bounds.width();
        size.height *= bounds.height();

        // Clamp the tile size to about 4M pixels.
        let tile_area = size.width * size.height;
        if tile_area > K_MAX_TILE_AREA {
            let clamp_scale = (K_MAX_TILE_AREA / tile_area).sqrt();
            size.set(size.width * clamp_scale, size.height * clamp_scale);
        }

        let tile_size: ISize = size.to_ceil();
        if tile_size.is_empty() {
            return None;
        }

        #[allow(clippy::cast_precision_loss)] // mirrors the int-to-float conversion in C++
        let tile_scale = Size::new(
            tile_size.width as scalar / bounds.width(),
            tile_size.height as scalar / bounds.height(),
        );

        let image_color_space = dst_color_space
            .cloned()
            .unwrap_or_else(ColorSpace::new_srgb);
        let image_color_type = if color_type_max_bits_per_channel(dst_color_type) <= 8 {
            ColorType::RGBA8888
        } else {
            ColorType::RGBAF16Norm
        };

        #[allow(clippy::cast_precision_loss)] // mirrors SkIntToScalar of the tile size (exact)
        let tile_rect = Rect::from_wh(tile_size.width as scalar, tile_size.height as scalar);
        Some(CachedImageInfo {
            tile_scale,
            matrix_for_draw: Matrix::rect_to_rect_or_identity(*bounds, tile_rect, None),
            image_info: ImageInfo::new(
                tile_size,
                image_color_type,
                skia_rust_core::alpha_type::AlphaType::Premul,
                image_color_space,
            ),
            props,
        })
    }

    // Port of: src/shaders/SkPictureShader.cpp#L215-L226 (chrome/m156), `CachedImageInfo::makeImage`
    fn make_image(&mut self, picture: &Picture) -> Option<Image> {
        // skia-rust: the tile's color type is explicit (`kRGBA_8888` is not N32 in a BGRA build;
        // see `oracle_n32`).
        let _explicit = ExplicitColorType::enter(self.image_info.color_type());
        let mut surf = surfaces::raster(&self.image_info, None::<usize>, Some(&self.props))?;
        {
            let canvas = surf.canvas();
            canvas.concat(&self.matrix_for_draw);
            canvas.draw_picture(picture, None, None);
        }
        // `surf->makeTemporaryImage()`: the raster surface's cached snapshot.
        surf.image_snapshot()
    }
}

/// The resource cache key of a tile image (`ImageFromPictureKey`).
// Port of: src/shaders/SkPictureShader.cpp#L27-L60 (chrome/m156)
fn image_from_picture_key(info: &CachedImageInfo, picture_id: u32, tile: &Rect) -> Key {
    let color_space = info
        .image_info
        .color_space()
        .unwrap_or_else(ColorSpace::new_srgb);
    let words = [
        color_space.to_xyzd50_hash().0,
        color_space.transfer_fn_hash(),
        info.image_info.color_type() as u32,
        tile.left.to_bits(),
        tile.top.to_bits(),
        tile.right.to_bits(),
        tile.bottom.to_bits(),
        info.tile_scale.width.to_bits(),
        info.tile_scale.height.to_bits(),
        info.props.flags().bits(),
        info.props.pixel_geometry() as u32,
    ];
    Key::new(
        std::ptr::addr_of!(IMAGE_FROM_PICTURE_KEY_NAMESPACE_LABEL) as usize,
        picture_priv::make_shared_id(picture_id),
        &words,
    )
}

/// A cached tile image (`ImageFromPictureRec`).
// Port of: src/shaders/SkPictureShader.cpp#L62-L91 (chrome/m156)
#[derive(Debug)]
struct ImageFromPictureRec {
    key: Key,
    image: Image,
}

impl Rec for ImageFromPictureRec {
    fn key(&self) -> &Key {
        &self.key
    }

    // Just the record overhead; the actual pixels are accounted by the lazy image.
    // Port of: src/shaders/SkPictureShader.cpp#L69-L73 (chrome/m156), `bytesUsed`
    fn bytes_used(&self) -> usize {
        let pixels = usize::try_from(self.image.width()).unwrap_or(0)
            * usize::try_from(self.image.height()).unwrap_or(0)
            * 4;
        size_of::<Key>() + pixels
    }

    fn category(&self) -> &'static str {
        "bitmap-shader"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A shader that draws a picture tiled (`SkPictureShader`): the picture is drawn once into a tile
/// image, which an image shader tiles.
// Port of: src/shaders/SkPictureShader.h#L17-L60 (chrome/m156)
#[doc(alias = "SkPictureShader")]
#[derive(Debug)]
pub struct PictureShader {
    picture: Picture,
    tile: Rect,
    tmx: TileMode,
    tmy: TileMode,
    filter: FilterMode,
}

impl PictureShader {
    /// `SkPictureShader::rasterShader`: the image shader that tiles the cached tile image of the
    /// picture at the total matrix `total_m`, with the local matrix that scales it back to the
    /// picture size.
    // Port of: src/shaders/SkPictureShader.cpp#L256-L290 (chrome/m156), `rasterShader`
    fn raster_shader(
        &self,
        total_m: &Matrix,
        dst_color_type: ColorType,
        dst_color_space: Option<&ColorSpace>,
        props_in: &SurfaceProps,
    ) -> Option<Shader> {
        let mut info = CachedImageInfo::make(
            &self.tile,
            total_m,
            dst_color_type,
            dst_color_space,
            props_in,
        )?;

        let key = image_from_picture_key(&info, self.picture.unique_id(), &self.tile);

        let mut image: Option<Image> = None;
        let found = ResourceCache::global()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .find(&key, |rec| {
                match rec.as_any().downcast_ref::<ImageFromPictureRec>() {
                    Some(rec) => {
                        image = Some(rec.image.clone());
                        true
                    }
                    None => false,
                }
            });
        let image = if found {
            image?
        } else {
            let made = info.make_image(&self.picture)?;
            ResourceCache::global()
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .add(Box::new(ImageFromPictureRec {
                    key,
                    image: made.clone(),
                }));
            picture_priv::added_to_cache(&self.picture);
            made
        };

        // Scale the image to the original picture size.
        let lm = Matrix::scale((1.0 / info.tile_scale.width, 1.0 / info.tile_scale.height));
        image.to_shader(
            (self.tmx, self.tmy),
            SamplingOptions::from(self.filter),
            &lm,
        )
    }

    // Port of: src/shaders/SkPictureShader.cpp#L292-L305 (chrome/m156), `appendStages`
    //
    // We don't check whether the total local matrix is valid here because we have to assume
    // *some* mapping to make an image.
    fn append_stages_impl<'a>(&self, rec: &mut StageRec<'_, 'a>, m_rec: &MatrixRec) -> bool {
        // Keep the bitmap shader alive by using the arena instead of stack memory.
        let alloc: &'a ArenaAlloc = rec.alloc;
        let Some(bitmap_shader) = self.raster_shader(
            &m_rec.total_matrix(),
            rec.dst_color_type,
            rec.dst_cs,
            &rec.surface_props,
        ) else {
            return false;
        };
        let bitmap_shader: &Shader = alloc.make(bitmap_shader);
        bitmap_shader.as_base().append_stages(rec, m_rec)
    }
}

impl ShaderBase for PictureShader {
    // Port of: src/shaders/SkPictureShader.h#L47 (chrome/m156), `type`
    fn shader_type(&self) -> ShaderType {
        ShaderType::Picture
    }

    // Port of: src/shaders/SkPictureShader.h#L50 (chrome/m156), `isOpaque`
    fn is_opaque(&self) -> bool {
        false
    }

    fn append_stages(&self, rec: &mut StageRec<'_, '_>, m_rec: &MatrixRec) -> bool {
        self.append_stages_impl(rec, m_rec)
    }
}

/// `SkPicture::makeShader`, as an extension method on [`Picture`] (core cannot name the raster
/// device that the tile image is drawn into).
pub trait PictureShaderExt {
    /// Returns a shader that draws the picture tiled in `tile_modes` with `mode` filtering, with
    /// `local_matrix` (identity if `None`) applied, and the picture's tile (its cull rect if
    /// `tile_rect` is `None`). `None` if `local_matrix` is not invertible (`makeShader`).
    fn to_shader<'a, 'b>(
        &self,
        tile_modes: impl Into<Option<(TileMode, TileMode)>>,
        mode: FilterMode,
        local_matrix: impl Into<Option<&'a Matrix>>,
        tile_rect: impl Into<Option<&'b Rect>>,
    ) -> Option<Shader>;
}

impl PictureShaderExt for Picture {
    // Port of: src/core/SkPicture.cpp#L24-L31 (chrome/m156), `SkPicture::makeShader`
    //          and src/shaders/SkPictureShader.cpp#L97-L107 (chrome/m156), `SkPictureShader::Make`
    #[doc(alias = "makeShader")]
    fn to_shader<'a, 'b>(
        &self,
        tile_modes: impl Into<Option<(TileMode, TileMode)>>,
        mode: FilterMode,
        local_matrix: impl Into<Option<&'a Matrix>>,
        tile_rect: impl Into<Option<&'b Rect>>,
    ) -> Option<Shader> {
        let (tmx, tmy) = tile_modes
            .into()
            .unwrap_or((TileMode::Clamp, TileMode::Clamp));
        let local_matrix = local_matrix.into();
        let tile = tile_rect.into();

        if let Some(lm) = local_matrix {
            lm.invert()?;
        }

        let cull = self.cull_rect();
        if cull.is_empty() || tile.is_some_and(Rect::is_empty) {
            return Some(shaders::empty());
        }
        let tile = tile.copied().unwrap_or(cull);

        let shader = Shader::from_base(PictureShader {
            picture: self.clone(),
            tile,
            tmx,
            tmy,
            filter: mode,
        });
        // `MakeWrapped`: `makeWithLocalMatrix` when there is a local matrix.
        Some(match local_matrix {
            Some(lm) => shader.with_local_matrix(lm),
            None => shader,
        })
    }
}
