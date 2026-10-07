// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: dm/DMSrcSink.{h,cpp} (GMSrc, RasterSink), dm/DM.cpp (create_sink),
// tools/flags/CommonFlagsConfig.cpp, and oracle/dm/OracleDump.cpp (byte layout)

//! DM's `GMSrc` and `RasterSink`: how a GM is rendered for one config, and the exact bytes the
//! oracle stores for it.

use std::fmt;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::size::ISize;

use crate::canvas::{PixelGeometry, Surface, SurfaceProps, SurfacePropsFlags};
use crate::registry::GmRegistration;
use crate::{DrawResult, GM, GmInstance};

/// A DM raster config: the `--config` tag and the color type `create_sink` gives its
/// `RasterSink`. None of these configs has a color-space "via" part, so the color space is null
/// (`SkCommandLineConfig::refColorSpace()`), as the goldens' `meta.json` (`"color_space":
/// "none"`) confirms.
// Port of: dm/DM.cpp#L1053-L1068 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Config {
    /// `8888`: `kN32_SkColorType` (BGRA on Windows, RGBA elsewhere), premul.
    N32,
    /// `565`: `kRGB_565_SkColorType`, opaque.
    Rgb565,
    /// `f16`: `kRGBA_F16_SkColorType`, premul.
    F16,
}

impl Config {
    /// The configs the oracle renders for CPU tiers (`cargo xtask oracle run --config 8888 f16
    /// 565`).
    pub const ALL: [Config; 3] = [Config::N32, Config::Rgb565, Config::F16];

    /// The DM tag, which is also the first component of the result id.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Config::N32 => "8888",
            Config::Rgb565 => "565",
            Config::F16 => "f16",
        }
    }

    /// The color type `create_sink` passes to `RasterSink`.
    #[must_use]
    pub const fn color_type(self) -> ColorType {
        match self {
            Config::N32 => ColorType::N32,
            Config::Rgb565 => ColorType::RGB565,
            Config::F16 => ColorType::RGBAF16,
        }
    }

    /// The color type the oracle's goldens store for this config (`meta.json` `color_type`).
    /// The oracle runs on a Windows host, where `kN32_SkColorType` is `BGRA_8888`.
    #[must_use]
    pub const fn golden_color_type(self) -> ColorType {
        match self {
            Config::N32 => ColorType::BGRA8888,
            Config::Rgb565 => ColorType::RGB565,
            Config::F16 => ColorType::RGBAF16,
        }
    }

    /// `RasterSink::colorInfo()`'s alpha type: premul, corrected by
    /// `SkColorTypeValidateAlphaType` (565 becomes opaque).
    // Port of: dm/DMSrcSink.h#L547-L553 (chrome/m156)
    #[must_use]
    pub fn alpha_type(self) -> AlphaType {
        self.color_type()
            .validate_alpha_type(AlphaType::Premul)
            .unwrap_or(AlphaType::Premul)
    }

    /// The `ImageInfo` `RasterSink::draw` allocates for a source of `size`.
    #[must_use]
    pub fn image_info(self, size: ISize) -> ImageInfo {
        ImageInfo::new(size, self.color_type(), self.alpha_type(), None)
    }

    /// The oracle result id: `<config>/gm/<name>`.
    #[must_use]
    pub fn result_id(self, gm_name: &str) -> String {
        format!("{}/gm/{gm_name}", self.tag())
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.tag())
    }
}

/// Whether `config` is compared with the RGBA oracle variants on a host whose
/// `kN32_SkColorType` is `host_n32`. Bytes are never swizzled: `8888` on a host whose N32 is
/// RGBA uses the `-rgba` tiers (built with `SK_R32_SHIFT=0`); everything else (BGRA hosts, and
/// the byte-order independent `565` and `f16`) uses the default tiers.
#[must_use]
pub fn uses_rgba_goldens(config: Config, host_n32: ColorType) -> bool {
    config == Config::N32 && host_n32 == ColorType::RGBA8888
}

/// `DM::Result::Status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    Ok,
    /// DM still writes the pixels of a fatal result (they are goldens too).
    Fatal,
    /// DM writes nothing for a skipped result.
    Skip,
}

/// `DM::Result`: a status and a message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DmResult {
    pub status: Status,
    pub msg: String,
}

impl DmResult {
    fn new(status: Status, msg: String) -> Self {
        Self { status, msg }
    }
}

/// `DM::GMSrc`: renders a registered GM, creating a fresh GM for every query as DM does.
#[derive(Clone, Copy)]
pub struct GmSrc {
    factory: fn() -> Box<dyn GM>,
}

impl fmt::Debug for GmSrc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GmSrc").field("name", &self.name()).finish()
    }
}

impl GmSrc {
    #[must_use]
    pub fn new(factory: fn() -> Box<dyn GM>) -> Self {
        Self { factory }
    }

    #[must_use]
    pub fn from_registration(reg: &GmRegistration) -> Self {
        Self::new(reg.factory)
    }

    /// `GMSrc::draw()` for a raster sink (no Graphite test context).
    // Port of: dm/DMSrcSink.cpp#L172-L194 (chrome/m156)
    #[must_use]
    pub fn draw(&self, canvas: &crate::canvas::Canvas) -> DmResult {
        let mut gm = GmInstance::new((self.factory)());
        let mut msg = String::new();

        match gm.gpu_setup(canvas, &mut msg) {
            DrawResult::Ok => {}
            DrawResult::Fail => return DmResult::new(Status::Fatal, msg),
            DrawResult::Skip => return DmResult::new(Status::Skip, msg),
        }

        match gm.draw(canvas, &mut msg) {
            DrawResult::Ok => DmResult::new(Status::Ok, msg),
            DrawResult::Fail => DmResult::new(Status::Fatal, msg),
            DrawResult::Skip => DmResult::new(Status::Skip, msg),
        }
    }

    /// `GMSrc::size()`.
    // Port of: dm/DMSrcSink.cpp#L196-L199 (chrome/m156)
    #[must_use]
    pub fn size(&self) -> ISize {
        (self.factory)().size()
    }

    /// `GMSrc::name()`.
    // Port of: dm/DMSrcSink.cpp#L201-L204 (chrome/m156)
    #[must_use]
    pub fn name(&self) -> String {
        (self.factory)().name()
    }

    /// `GMSrc::modifySurfaceProps()`.
    // Port of: dm/DMSrcSink.cpp#L206-L209 (chrome/m156)
    pub fn modify_surface_props(&self, props: &mut SurfaceProps) {
        (self.factory)().modify_surface_props(props);
    }
}

/// What a sink produced: the DM result and, unless skipped, the bitmap.
#[derive(Debug)]
pub struct Rendered {
    pub result: DmResult,
    pub bitmap: Option<Bitmap>,
}

/// `DM::RasterSink`.
#[derive(Clone, Copy, Debug)]
pub struct RasterSink {
    config: Config,
}

impl RasterSink {
    #[must_use]
    pub const fn new(config: Config) -> Self {
        Self { config }
    }

    #[must_use]
    pub const fn config(&self) -> Config {
        self.config
    }

    /// `RasterSink::draw()`: zeroed pixels of the config's color info, a surface with
    /// `SkSurfaceProps(0, kRGB_H_SkPixelGeometry)` adjusted by the GM, then `GMSrc::draw`.
    ///
    /// # Panics
    /// If the stub surface cannot wrap freshly allocated pixels (never).
    // Port of: dm/DMSrcSink.cpp#L2160-L2175 (chrome/m156)
    #[must_use]
    pub fn draw(&self, src: &GmSrc) -> Rendered {
        let size = src.size();
        if size.is_empty() {
            return Rendered {
                result: DmResult::new(
                    Status::Skip,
                    format!("Skipping empty source: {}", src.name()),
                ),
                bitmap: None,
            };
        }

        let mut dst = Bitmap::new();
        // allocPixelsFlags(..., kZeroPixels_AllocFlag): our allocation is always zeroed.
        dst.alloc_pixels_flags(&self.config.image_info(size));

        let mut props = SurfaceProps::new(SurfacePropsFlags::empty(), PixelGeometry::RGBH);
        src.modify_surface_props(&mut props);
        let mut surface =
            Surface::wrap_pixels(&mut dst, Some(&props)).expect("allocated pixels can be wrapped");
        let result = src.draw(surface.canvas());
        drop(surface); // gives the drawn pixels back to `dst`
        Rendered {
            result,
            bitmap: Some(dst),
        }
    }
}

/// The bytes the oracle stores for a pixel result: every row's first `minRowBytes()` bytes,
/// tightly packed (`OracleDump::Write`).
// Port of: oracle/dm/OracleDump.cpp#L117-L122
#[must_use]
#[allow(clippy::cast_sign_loss)] // height() of an allocated bitmap is non-negative
pub fn packed_bytes(bitmap: &Bitmap) -> Vec<u8> {
    let info = bitmap.info();
    let row = info.min_row_bytes();
    let row_bytes = bitmap.row_bytes();
    let pixmap = bitmap.pixmap();
    let bytes = pixmap.bytes().unwrap_or(&[]);
    let mut out = Vec::with_capacity(row * info.height() as usize);
    for y in 0..info.height() as usize {
        out.extend_from_slice(&bytes[y * row_bytes..y * row_bytes + row]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The color types and alpha types match what the oracle recorded in `meta.json` for every
    /// CPU result (`BGRA_8888`/`Premul`, `RGB_565`/`Opaque`, `RGBA_F16`/`Premul`, no color space).
    #[test]
    fn configs_match_the_oracle_metadata() {
        // The host's N32 is whichever order Skia picks for it; the goldens' is BGRA everywhere.
        let host_n32 = if skia_rust_core::color_priv::PMCOLOR_IS_BGRA {
            ColorType::BGRA8888
        } else {
            ColorType::RGBA8888
        };
        assert_eq!(Config::N32.color_type(), host_n32);
        assert_eq!(Config::N32.golden_color_type(), ColorType::BGRA8888);
        assert_eq!(Config::N32.alpha_type(), AlphaType::Premul);
        for c in [Config::Rgb565, Config::F16] {
            assert_eq!(c.color_type(), c.golden_color_type());
        }
        assert_eq!(Config::Rgb565.color_type(), ColorType::RGB565);
        assert_eq!(Config::Rgb565.alpha_type(), AlphaType::Opaque);
        assert_eq!(Config::F16.color_type(), ColorType::RGBAF16);
        assert_eq!(Config::F16.alpha_type(), AlphaType::Premul);
        assert!(
            Config::ALL
                .iter()
                .all(|c| c.image_info(ISize::new(1, 1)).color_space().is_none())
        );
        assert_eq!(Config::F16.result_id("aarectmodes"), "f16/gm/aarectmodes");
    }

    #[test]
    fn n32_uses_the_golden_variant_of_the_host_byte_order() {
        assert!(!uses_rgba_goldens(Config::N32, ColorType::BGRA8888));
        assert!(uses_rgba_goldens(Config::N32, ColorType::RGBA8888));
        for host in [ColorType::BGRA8888, ColorType::RGBA8888] {
            assert!(!uses_rgba_goldens(Config::Rgb565, host));
            assert!(!uses_rgba_goldens(Config::F16, host));
        }
    }

    #[test]
    fn packed_bytes_drop_row_padding() {
        let mut bm = Bitmap::new();
        let info = Config::Rgb565.image_info(ISize::new(3, 2));
        // 3 pixels * 2 bytes = 6 bytes per row, padded to 8.
        assert!(bm.try_alloc_pixels_info(&info, Some(8)));
        bm.erase_color(skia_rust_core::color::Color::WHITE);
        assert_eq!(packed_bytes(&bm), vec![0xff; 12]);
    }
}
