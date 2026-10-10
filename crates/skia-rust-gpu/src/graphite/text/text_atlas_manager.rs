// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/text/TextAtlasManager.h, src/gpu/graphite/text/TextAtlasManager.cpp
// (chrome/m156).

//! [`TextAtlasManager`]: the glyph atlases of a recorder, one per mask format (A8, 565, ARGB), and
//! [`AtlasConfig`], their dimensions, chosen from the caps' maximum texture size and the client's
//! byte budget for one atlas texture.

use std::sync::Arc;

use skia_rust_core::color_data::{packed16_to_b32, packed16_to_g32, packed16_to_r32};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::distance_field_gen::DISTANCE_FIELD_INSET;
use skia_rust_core::glyph::Glyph as SkGlyph;
use skia_rust_core::mask::MaskFormat as GlyphMaskFormat;
use skia_rust_core::size::ISize;

use crate::gpu::gpu_types::{Mipmapped, Protected, Renderable};
use crate::gpu::mask_format::{MASK_FORMAT_COUNT, MaskFormat};
use crate::gpu::token::Token;
use crate::graphite::caps::Caps;
use crate::graphite::draw_atlas::{
    AllowMultitexturing, BulkUsePlotUpdater, DrawAtlas, ErrorCode, GenerationCounter,
    UseStorageTextures,
};
use crate::graphite::draw_context::DrawContext;
use crate::graphite::recorder::Recorder;
use crate::graphite::text::text_strike::GlyphEntry;
use crate::graphite::texture_proxy::TextureProxy;
use crate::text_gpu::format_from_glyph;
use crate::text_gpu::glyph_vector::RendererData;

/// `TextAtlasManager::AtlasConfig`.
#[doc(alias = "TextAtlasManager::AtlasConfig")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtlasConfig {
    argb_dimensions: ISize,
    max_texture_size: i32,
}

impl AtlasConfig {
    /// `kMaxAtlasDim`: texture coordinates are half-precision on some systems, which limits atlas
    /// dimensions to 2048x2048.
    // Port of: src/gpu/graphite/text/TextAtlasManager.h#L54 (chrome/m156)
    const K_MAX_ATLAS_DIM: i32 = 2048;

    /// `AtlasConfig(maxTextureSize, maxBytes)`: `max_bytes` is the largest a single atlas texture
    /// should be; multitexturing may use more space temporarily.
    // Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L47-L69 (chrome/m156)
    #[must_use]
    pub fn new(max_texture_size: i32, max_bytes: usize) -> Self {
        // {width, height} for maxBytes in [2^18 << i, 2^18 << (i + 1)).
        const K_ARGB_DIMENSIONS: [(i32, i32); 6] = [
            (256, 256),
            (512, 256),
            (512, 512),
            (1024, 512),
            (1024, 1024),
            (2048, 1024),
        ];

        // Index 0 corresponds to maxBytes of 2^18, so start by dividing it by that.
        let max_bytes = max_bytes >> 18;
        // Take the floor of the log to get the index.
        let index = if max_bytes > 0 {
            let prev_log2 = (usize::BITS - 1 - max_bytes.leading_zeros()) as usize;
            prev_log2.clamp(0, K_ARGB_DIMENSIONS.len() - 1)
        } else {
            0
        };

        let (w, h) = K_ARGB_DIMENSIONS[index];
        debug_assert!(w <= Self::K_MAX_ATLAS_DIM);
        debug_assert!(h <= Self::K_MAX_ATLAS_DIM);
        AtlasConfig {
            argb_dimensions: ISize::new(w.min(max_texture_size), h.min(max_texture_size)),
            max_texture_size: max_texture_size.min(Self::K_MAX_ATLAS_DIM),
        }
    }

    /// `atlasDimensions(type)`.
    // Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L71-L79 (chrome/m156)
    #[must_use]
    pub fn atlas_dimensions(&self, mask_format: MaskFormat) -> ISize {
        if mask_format == MaskFormat::A8 {
            // A8 is always 2x the ARGB dimensions, clamped to the max allowed texture size.
            ISize::new(
                (2 * self.argb_dimensions.width).min(self.max_texture_size),
                (2 * self.argb_dimensions.height).min(self.max_texture_size),
            )
        } else {
            self.argb_dimensions
        }
    }

    /// `plotDimensions(type)`.
    // Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L81-L98 (chrome/m156)
    #[must_use]
    pub fn plot_dimensions(&self, mask_format: MaskFormat) -> ISize {
        if mask_format == MaskFormat::A8 {
            // For A8 the plots grow at larger texture sizes, to accept the larger SDF glyphs.
            let atlas = self.atlas_dimensions(mask_format);
            // 512x256 plots for 2048x1024, 512x512 plots for 2048x2048, 256x256 otherwise.
            let plot_width = if atlas.width >= 2048 { 512 } else { 256 };
            let plot_height = if atlas.height >= 2048 { 512 } else { 256 };
            ISize::new(plot_width, plot_height)
        } else {
            // ARGB and LCD always use 256x256 plots.
            ISize::new(256, 256)
        }
    }
}

/// `expand_bits`: expands a 1 bit per pixel mask to `width` x `height` integers of
/// `bytes_per_pixel` bytes, all ones for a set bit and 0 otherwise.
// Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L113-L132 (chrome/m156)
fn expand_bits(
    dst: &mut [u8],
    bytes_per_pixel: usize,
    src: &[u8],
    width: usize,
    height: usize,
    dst_row_bytes: usize,
    src_row_bytes: usize,
) {
    for y in 0..height {
        let mut row_writes_left = width;
        let mut s = y * src_row_bytes;
        let mut d = y * dst_row_bytes;
        while row_writes_left > 0 {
            let mask = src[s];
            s += 1;
            let mut x = 7;
            while x >= 0 && row_writes_left != 0 {
                let value = if mask & (1 << x) != 0 { 0xFF } else { 0 };
                dst[d..d + bytes_per_pixel].fill(value);
                d += bytes_per_pixel;
                x -= 1;
                row_writes_left -= 1;
            }
        }
    }
}

/// `get_packed_glyph_image`: copies the glyph's image into `dst` (with `dst_row_bytes` per row)
/// in the format of the atlas it goes in.
///
/// # Panics
/// If the glyph has no image, or there is no conversion from the glyph's format.
// Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L134-L228 (chrome/m156)
fn get_packed_glyph_image(
    glyph: &SkGlyph,
    dst_row_bytes: usize,
    expected_mask_format: MaskFormat,
    dst: &mut [u8],
) {
    let width = usize::from(glyph.width());
    let height = usize::from(glyph.height());
    let src = glyph.image().expect("a glyph in the atlas has an image");

    let mask_format = format_from_glyph(glyph.mask_format());
    if mask_format == expected_mask_format {
        let src_row_bytes = glyph.row_bytes();
        // Notice this comparison is with the glyphs raw mask format, and not its MaskFormat.
        if glyph.mask_format() == GlyphMaskFormat::BW {
            // Handle 8-bit format by expanding the mask to the expected format.
            match expected_mask_format {
                MaskFormat::A8 => {
                    expand_bits(dst, 1, src, width, height, dst_row_bytes, src_row_bytes);
                }
                MaskFormat::A565 => {
                    expand_bits(dst, 2, src, width, height, dst_row_bytes, src_row_bytes);
                }
                MaskFormat::Argb => panic!("Invalid MaskFormat"),
            }
        } else if src_row_bytes != dst_row_bytes {
            let bpp = expected_mask_format.bytes_per_pixel();
            for y in 0..height {
                dst[y * dst_row_bytes..y * dst_row_bytes + width * bpp]
                    .copy_from_slice(&src[y * src_row_bytes..y * src_row_bytes + width * bpp]);
            }
        } else {
            dst[..dst_row_bytes * height].copy_from_slice(&src[..dst_row_bytes * height]);
        }
    } else if mask_format == MaskFormat::A565 && expected_mask_format == MaskFormat::Argb {
        // Convert if the glyph uses a 565 mask format since it is using LCD text rendering but
        // the expected format is 8888 (will happen on Intel MacOS with Metal since that
        // combination does not support 565).
        let a565_bpp = MaskFormat::A565.bytes_per_pixel();
        let argb_bpp = MaskFormat::Argb.bytes_per_pixel();
        let bgra_is_native = ColorType::N32 == ColorType::BGRA8888;
        let src_row_bytes = glyph.row_bytes();
        for y in 0..height {
            for x in 0..width {
                let s = y * src_row_bytes + x * a565_bpp;
                let color565 = u32::from(u16::from_ne_bytes([src[s], src[s + 1]]));
                let r = packed16_to_r32(color565);
                let g = packed16_to_g32(color565);
                let b = packed16_to_b32(color565);
                // On Windows (and possibly others), font data is stored as BGR. So we need to
                // swizzle the data to reflect that.
                let color8888: u32 = if bgra_is_native {
                    b | (g << 8) | (r << 16) | (0xFF << 24)
                } else {
                    r | (g << 8) | (b << 16) | (0xFF << 24)
                };
                let d = y * dst_row_bytes + x * argb_bpp;
                dst[d..d + argb_bpp].copy_from_slice(&color8888.to_ne_bytes());
            }
        }
    } else {
        unreachable!("no conversion from {mask_format:?} to {expected_mask_format:?}");
    }
}

/// The glyph atlases of a recorder (`skgpu::graphite::TextAtlasManager`).
///
/// For text there are three atlases (A8, 565, ARGB) that are kept in relation with one another.
/// In general, because A8 is the most frequently used mask format its dimensions are 2x the 565
/// and ARGB dimensions, with the constraint that an atlas size will always contain at least one
/// plot. Since the ARGB atlas takes the most space, its dimensions are used to size the other
/// two atlases.
// Port of: src/gpu/graphite/text/TextAtlasManager.h#L30-L156 (chrome/m156)
#[doc(alias = "skgpu::graphite::TextAtlasManager")]
#[derive(Debug)]
pub struct TextAtlasManager {
    /// The generation counter all the atlases draw their ids from (`TextAtlasManager` is the
    /// `GenerationCounter` in C++).
    generation: GenerationCounter,
    /// `fAllowMultitexturing`.
    allow_multitexturing: AllowMultitexturing,
    /// `fAtlases`, indexed by the mask format.
    atlases: [Option<Box<DrawAtlas>>; MASK_FORMAT_COUNT],
    /// `fSupportBilerpAtlas`.
    support_bilerp_atlas: bool,
    /// Whether the caps have a texture for 565 masks: if not, they are ARGB.
    supports_565: bool,
    /// `fAtlasConfig`.
    atlas_config: AtlasConfig,
}

impl TextAtlasManager {
    /// `TextAtlasManager(recorder)`.
    // Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L230-L245 (chrome/m156)
    #[must_use]
    pub fn new(caps: &dyn Caps) -> Self {
        let allow_multitexturing = if !caps.allow_multiple_atlas_textures()
            // multitexturing supported only if range can represent the index + texcoords fully
            || !(caps.shader_caps().float_is_32_bits || caps.shader_caps().integer_support)
        {
            AllowMultitexturing::No
        } else {
            AllowMultitexturing::Yes
        };
        Self {
            generation: GenerationCounter::default(),
            allow_multitexturing,
            atlases: [None, None, None],
            support_bilerp_atlas: caps.support_bilerp_from_glyph_atlas(),
            supports_565: caps
                .get_default_sampled_texture_info(
                    ColorType::RGB565,
                    Mipmapped::No,
                    Protected::No,
                    Renderable::No,
                )
                .is_valid(),
            atlas_config: AtlasConfig::new(
                caps.max_texture_size(),
                caps.glyph_cache_texture_maximum_bytes(),
            ),
        }
    }

    // Port of: src/gpu/graphite/text/TextAtlasManager.h#L137-L142 (chrome/m156)
    // There is a 1:1 mapping between skgpu::MaskFormats and atlas indices.
    fn mask_format_to_atlas_index(format: MaskFormat) -> usize {
        format as usize
    }

    /// `resolveMaskFormat(format)`: changes an expected 565 mask format to 8888 if 565 is not
    /// supported (will happen when using Metal on Intel MacOS). The actual conversion of the data
    /// is handled in `get_packed_glyph_image`.
    // Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L280-L294 (chrome/m156)
    fn resolve_mask_format(&self, format: MaskFormat) -> MaskFormat {
        if format == MaskFormat::A565 && !self.supports_565 {
            MaskFormat::Argb
        } else {
            format
        }
    }

    /// `resolveRendererData(data)`: bumps direct mask glyphs (zero padding) up to 1px when the
    /// atlas can be bilerp'ed, and resolves the mask format. The SDF and LCD properties are left
    /// alone.
    // Port of: src/gpu/graphite/text/TextAtlasManager.h#L63-L69 (chrome/m156)
    #[must_use]
    pub fn resolve_renderer_data(&self, mut data: RendererData) -> RendererData {
        data.src_padding = if data.src_padding == 0 && self.support_bilerp_atlas {
            1
        } else {
            data.src_padding
        };
        data.mask_format = self.resolve_mask_format(data.mask_format);
        data
    }

    /// `initAtlas(resolvedMaskFormat)`.
    // Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L371-L391 (chrome/m156)
    fn init_atlas(&mut self, resolved_mask_format: MaskFormat) -> bool {
        debug_assert_eq!(
            resolved_mask_format,
            self.resolve_mask_format(resolved_mask_format)
        );

        let index = Self::mask_format_to_atlas_index(resolved_mask_format);
        if self.atlases[index].is_none() {
            let atlas_dimensions = self.atlas_config.atlas_dimensions(resolved_mask_format);
            let plot_dimensions = self.atlas_config.plot_dimensions(resolved_mask_format);
            self.atlases[index] = Some(DrawAtlas::make(
                resolved_mask_format,
                atlas_dimensions.width,
                atlas_dimensions.height,
                plot_dimensions.width,
                plot_dimensions.height,
                &self.generation,
                self.allow_multitexturing,
                UseStorageTextures::No,
                None,
                "TextAtlas",
            ));
        }
        true
    }

    fn atlas(&self, resolved_mask_format: MaskFormat) -> &DrawAtlas {
        debug_assert_eq!(
            resolved_mask_format,
            self.resolve_mask_format(resolved_mask_format)
        );
        self.atlases[Self::mask_format_to_atlas_index(resolved_mask_format)]
            .as_deref()
            .expect("the atlas was initialized by get_proxies()")
    }

    fn atlas_mut(&mut self, resolved_mask_format: MaskFormat) -> &mut DrawAtlas {
        debug_assert_eq!(
            resolved_mask_format,
            self.resolve_mask_format(resolved_mask_format)
        );
        self.atlases[Self::mask_format_to_atlas_index(resolved_mask_format)]
            .as_deref_mut()
            .expect("the atlas was initialized by get_proxies()")
    }

    /// `getProxies(resolvedMaskFormat, &numActiveProxies)`: the textures of the active pages of
    /// the atlas. If `None` is returned, the client must not try to use other functions on the
    /// manager which use the atlas. This function *must* be called first, before other functions
    /// which use the atlas.
    // Port of: src/gpu/graphite/text/TextAtlasManager.h#L53-L61 (chrome/m156)
    #[must_use]
    pub fn get_proxies(
        &mut self,
        resolved_mask_format: MaskFormat,
    ) -> Option<Vec<Arc<TextureProxy>>> {
        if self.init_atlas(resolved_mask_format) {
            let atlas = self.atlas(resolved_mask_format);
            let num_active_pages = atlas.num_active_pages() as usize;
            return Some(
                atlas
                    .get_proxies()
                    .iter()
                    .take(num_active_pages)
                    .filter_map(Clone::clone)
                    .collect(),
            );
        }
        None
    }

    /// `freeGpuResources()`.
    // Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L247-L254 (chrome/m156)
    pub fn free_gpu_resources(&mut self, recorder: &Recorder) {
        let token = recorder.priv_().token_tracker().borrow().next_flush_token();
        for atlas in self.atlases.iter_mut().flatten() {
            atlas.free_gpu_resources(token);
        }
    }

    /// `hasGlyph(glyph)`.
    // Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L256-L258 (chrome/m156)
    #[must_use]
    pub fn has_glyph(&self, glyph: &GlyphEntry) -> bool {
        self.atlas(glyph.key().mask_format())
            .has_id(&glyph.atlas_locator().plot_locator())
    }

    /// `addGlyphToAtlas(skGlyph, glyph)`: returns `Succeeded` if the glyph was successfully added
    /// to the texture atlas, `TryAgain` if a RenderPassTask needs to be snapped before adding the
    /// glyph, and `Error` if it can't be added at all.
    ///
    /// # Panics
    /// If the glyph image has no room for the padding (`SkASSERT_RELEASE`).
    // Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L296-L369 (chrome/m156)
    pub fn add_glyph_to_atlas(
        &mut self,
        recorder: &Recorder,
        sk_glyph: &SkGlyph,
        glyph: &GlyphEntry,
    ) -> ErrorCode {
        if sk_glyph.image().is_none() {
            return ErrorCode::Error;
        }

        let src_padding = glyph.key().padding();

        debug_assert!((0..=DISTANCE_FIELD_INSET).contains(&src_padding));
        debug_assert_eq!(
            sk_glyph.mask_format() == GlyphMaskFormat::Sdf,
            glyph.key().is_sdf()
        );

        let expected_mask_format = glyph.key().mask_format();
        debug_assert_eq!(
            expected_mask_format,
            self.resolve_mask_format(glyph.key().mask_format())
        );

        let bytes_per_pixel = expected_mask_format.bytes_per_pixel();

        let padding: usize = match src_padding {
            // The direct mask/image case; lifting to 1px padding should have happened earlier
            // when the glyph key was created.
            0 => {
                debug_assert!(!self.support_bilerp_atlas);
                0
            }
            // The transformed mask/image case.
            1 => 1,
            // The SDFT case. If the srcPadding == SK_DistanceFieldInset (SDFT case) then the
            // padding is built into the image on the glyph; no extra padding needed.
            // TODO: can the SDFT glyph image in the cache be reduced by the padding?
            DISTANCE_FIELD_INSET => {
                debug_assert!(glyph.key().is_sdf());
                0
            }
            // The padding is not one of the know forms.
            _ => return ErrorCode::Error,
        };

        let width = usize::from(sk_glyph.width()) + 2 * padding;
        let height = usize::from(sk_glyph.height()) + 2 * padding;

        // Verify that the glyph data (received from potentially untrusted source) actually has
        // room for the padding. Under normal flow, this should always be the case, but if a glyph
        // was corrupted or manipulated it has no bearing on the code that *should* have produced
        // the glyph. It's strict comparison since equality would imply the original glyph was
        // empty, which should have been dropped.
        let src_padding_usize = usize::try_from(src_padding).expect("padding is non-negative");
        assert!(width > 2 * src_padding_usize && height > 2 * src_padding_usize);

        let row_bytes = width * bytes_per_pixel;
        let size = height * row_bytes;

        // Temporary storage for normalizing glyph image.
        let mut storage = vec![0u8; size];
        let data_offset = if padding > 0 {
            // Advance in one row and one column.
            row_bytes + bytes_per_pixel
        } else {
            0
        };

        get_packed_glyph_image(
            sk_glyph,
            row_bytes,
            expected_mask_format,
            &mut storage[data_offset..],
        );

        let atlas = self.atlas_mut(expected_mask_format);
        let mut locator = glyph.atlas_locator();
        let error_code = atlas.add_to_atlas(
            recorder,
            i32::try_from(width).expect("glyphs fit the atlas"),
            i32::try_from(height).expect("glyphs fit the atlas"),
            &storage,
            &mut locator,
        );

        if error_code == ErrorCode::Succeeded {
            locator.inset_src(src_padding);
        }
        glyph.set_atlas_locator(locator);

        error_code
    }

    /// `recordUploads(dc)`.
    // Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L371-L377 (chrome/m156)
    pub fn record_uploads(&mut self, dc: &mut DrawContext, recorder: &Recorder) -> bool {
        for atlas in self.atlases.iter_mut().flatten() {
            if !atlas.record_uploads(dc, recorder) {
                return false;
            }
        }
        true
    }

    /// `addGlyphToBulkAndSetUseToken(updater, glyph, token)`: to ensure the DrawAtlas does not
    /// evict the glyph mask from its texture backing store, the client must pass in the current
    /// draw token along with the glyph. A `BulkUsePlotUpdater` is used to manage bulk last use
    /// token updating in the atlas. For convenience, this function will also set the use token
    /// for the current glyph if required.
    ///
    /// NOTE: the bulk uploader is only valid if the subrun has a valid atlasGeneration.
    // Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L379-L386 (chrome/m156)
    pub fn add_glyph_to_bulk_and_set_use_token(
        &mut self,
        updater: &mut BulkUsePlotUpdater,
        glyph: &GlyphEntry,
        token: Token,
    ) {
        let locator = glyph.atlas_locator();
        if updater.add(&locator) {
            self.atlas_mut(glyph.key().mask_format())
                .set_last_use_token(&locator, token);
        }
    }

    /// `setUseTokenBulk(updater, token, resolvedMaskFormat)`.
    // Port of: src/gpu/graphite/text/TextAtlasManager.h#L77-L81 (chrome/m156)
    pub fn set_use_token_bulk(
        &mut self,
        updater: &BulkUsePlotUpdater,
        token: Token,
        resolved_mask_format: MaskFormat,
    ) {
        self.atlas_mut(resolved_mask_format)
            .set_last_use_token_bulk(updater, token);
    }

    /// `evictAtlases()`.
    // Port of: src/gpu/graphite/text/TextAtlasManager.h#L85-L91 (chrome/m156)
    pub fn evict_atlases(&mut self) {
        for atlas in self.atlases.iter_mut().flatten() {
            atlas.evict_all_plots();
        }
    }

    /// `compact()`.
    // Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L393-L400 (chrome/m156)
    pub fn compact(&mut self, recorder: &Recorder) {
        let token = recorder.priv_().token_tracker().borrow().next_flush_token();
        for atlas in self.atlases.iter_mut().flatten() {
            atlas.compact(token);
        }
    }

    /// `atlasGeneration(resolvedMaskFormat)`: a monotonically increasing number which changes
    /// every time something is removed from the texture backing store of the atlas, for clients
    /// that wish to verify the integrity of it.
    // Port of: src/gpu/graphite/text/TextAtlasManager.h#L100-L102 (chrome/m156)
    #[must_use]
    pub fn atlas_generation(&self, resolved_mask_format: MaskFormat) -> u64 {
        self.atlas(resolved_mask_format).atlas_generation()
    }

    /// `setAtlasDimensionsToMinimum_ForTesting()`: deletes any old atlases (which should be safe
    /// as long as we are not in the middle of a flush) and sets all the atlas sizes to 1x1 plot
    /// each.
    // Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L360-L369 (chrome/m156)
    pub fn set_atlas_dimensions_to_minimum_for_testing(&mut self) {
        self.atlases = [None, None, None];
        self.atlas_config = AtlasConfig::new(2048, 0);
    }
}
