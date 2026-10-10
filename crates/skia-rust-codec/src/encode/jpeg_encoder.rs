// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/encode/SkJpegEncoder.h, src/encode/SkJpegEncoderImpl.cpp (the RGB path),
// src/encode/SkJPEGWriteUtility.cpp (the error and destination handling, which the libjpeg port
// replaces with `Result` and a `Vec<u8>`), and the metadata segments of `SkJpegMetadataEncoder`
// (`AppendXMPStandard`, `AppendICC`), with the skia-safe API shape (`jpeg_encoder::{encode,
// encode_pixmap, encode_image, Options, Downsample, AlphaOption}`).
//
// Not ported, each returns `false` / `None` rather than encoding differently:
// - `EncodeYUVAPixmaps` / `MakeYUV`: the YUVA pixmap type is not in skia-rust-core.
// - `Options::origin`: the EXIF segment is written by `SkExif::WriteExif`, which this crate does
//   not port (`exif.rs` only parses).
// - the gainmap encoder (`SkJpegGainmapEncoder.cpp`), which is outside this port.

use std::io;
use std::sync::{Arc, Mutex};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::convert_pixels::convert_pixels;
use skia_rust_core::data::Data;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_info_priv::{color_type_is_alpha_only, color_type_num_channels};
use skia_rust_core::pixmap::Pixmap;
use skia_rust_libjpeg::{ColorSpace as JpegColorSpace, Compress};

use crate::encode::icc::write_icc_profile;

/// `kXMPMarker` (APP1) and `kXMPStandardSig` (`SkJpegConstants.h`).
const XMP_MARKER: u8 = 0xE1;
const XMP_STANDARD_SIG: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
/// `kICCMarker` (APP2) and `kICCSig` (`SkJpegConstants.h`).
const ICC_MARKER: u8 = 0xE2;
const ICC_SIG: &[u8] = b"ICC_PROFILE\0";

/// Port of `SkJpegEncoder::Downsample`: the chroma subsampling. The default is 4:2:0.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub enum Downsample {
    /// Reduction by two in both directions (`k420`), the libjpeg default.
    #[default]
    BothDirections,
    /// Reduction by two horizontally (`k422`).
    Horizontal,
    /// No downsampling (`k444`).
    No,
}

/// Port of `SkJpegEncoder::AlphaOption`: how an input with alpha is made opaque.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub enum AlphaOption {
    /// Ignore the alpha channel and treat the image as opaque (`kIgnore`).
    #[default]
    Ignore,
    /// Blend the pixels onto black before encoding (`kBlendOnBlack`).
    BlendOnBlack,
}

/// Port of `SkJpegEncoder::Options` (SkJpegEncoder.h): quality 0..=100, the subsampling, the
/// alpha handling, and optional XMP and origin metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// The quality, `0..=100`. Default 100.
    pub quality: u32,
    /// The chroma subsampling. Default 4:2:0.
    pub downsample: Downsample,
    /// The alpha handling. Default `Ignore`.
    pub alpha_option: AlphaOption,
    /// The XMP packet, written as a standard XMP APP1 segment.
    pub xmp_metadata: Option<String>,
    /// The orientation. Not supported: see the module comment.
    pub origin: Option<EncodedOrigin>,
}

impl Default for Options {
    /// The defaults of `SkJpegEncoder::Options`.
    fn default() -> Self {
        Self {
            quality: 100,
            downsample: Downsample::BothDirections,
            alpha_option: AlphaOption::Ignore,
            xmp_metadata: None,
            origin: None,
        }
    }
}

/// How the rows of the source reach the compressor (`SkJpegEncoderMgr::initializeRGB`).
struct RowSource {
    /// The `in_color_space` and the components per pixel the compressor is given.
    in_cs: JpegColorSpace,
    components: i32,
    /// The destination of the colour transform (`fDstInfo`, one row), or `None` when the source rows
    /// are passed to the compressor unchanged.
    convert: Option<ImageInfo>,
}

/// `SkJpegEncoderMgr::initializeRGB`: the colour space and conversion for `src`, or `None` for a
/// source the encoder does not accept.
fn row_source(src_info: &ImageInfo, options: &Options) -> Option<RowSource> {
    let src_ct = src_info.color_type();
    let src_alpha = src_info.alpha_type();
    let apply_premul =
        options.alpha_option == AlphaOption::BlendOnBlack && src_alpha == AlphaType::Unpremul;
    if src_ct == ColorType::RGB888x {
        return Some(RowSource {
            in_cs: JpegColorSpace::ExtRgbx,
            components: 4,
            convert: None,
        });
    }
    if !apply_premul && src_ct == ColorType::RGBA8888 {
        return Some(RowSource {
            in_cs: JpegColorSpace::ExtRgba,
            components: 4,
            convert: None,
        });
    }
    if !apply_premul && src_ct == ColorType::BGRA8888 {
        return Some(RowSource {
            in_cs: JpegColorSpace::ExtBgra,
            components: 4,
            convert: None,
        });
    }
    // A colour conversion is needed.
    match color_type_num_channels(src_ct) {
        1 => {
            // Alpha8 is encoded as grayscale; other alpha-only types have no sensible encoding.
            if color_type_is_alpha_only(src_ct) && src_ct != ColorType::Alpha8 {
                return None;
            }
            Some(RowSource {
                in_cs: JpegColorSpace::Grayscale,
                components: 1,
                convert: None,
            })
        }
        3 => Some(RowSource {
            in_cs: JpegColorSpace::ExtRgbx,
            components: 4,
            convert: Some(ImageInfo::new(
                (src_info.width(), 1),
                ColorType::RGB888x,
                AlphaType::Unpremul,
                None,
            )),
        }),
        4 => {
            let dst_at = if apply_premul {
                AlphaType::Premul
            } else {
                src_alpha
            };
            Some(RowSource {
                in_cs: JpegColorSpace::ExtRgba,
                components: 4,
                convert: Some(ImageInfo::new(
                    (src_info.width(), 1),
                    ColorType::RGBA8888,
                    dst_at,
                    None,
                )),
            })
        }
        _ => None,
    }
}

/// `SkJpegMetadataEncoder::AppendXMPStandard` and `AppendICC`, in that order.
fn metadata_segments(src: &Pixmap<'_>, options: &Options) -> Vec<(u8, Vec<u8>)> {
    let mut segments = Vec::new();
    if let Some(xmp) = &options.xmp_metadata {
        let mut body = XMP_STANDARD_SIG.to_vec();
        body.extend_from_slice(xmp.as_bytes());
        segments.push((XMP_MARKER, body));
    }
    let cs = src.color_space();
    if let Some(icc) = write_icc_profile(cs.as_ref()) {
        // The profile is written as one segment: "1 of 1".
        let mut body = ICC_SIG.to_vec();
        body.push(1);
        body.push(1);
        body.extend_from_slice(&icc);
        segments.push((ICC_MARKER, body));
    }
    segments
}

/// The JPEG encoder: `SkJpegEncoderImpl`. [`make`] starts the compressor and writes the metadata;
/// [`JpegEncoder::encode_rows`] feeds it rows. The finished file goes to the sink after the last
/// row is encoded.
pub struct JpegEncoder<'a> {
    src: Pixmap<'a>,
    source: RowSource,
    cinfo: Compress,
    out: Arc<Mutex<Vec<u8>>>,
    curr_row: usize,
}

impl std::fmt::Debug for JpegEncoder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JpegEncoder")
            .field("curr_row", &self.curr_row)
            .finish_non_exhaustive()
    }
}

impl JpegEncoder<'_> {
    /// Port of `SkEncoder::encodeRows` for the JPEG encoder: encodes up to `num_rows` further rows
    /// of the source, and finishes the file after the last one. Returns `false` for a
    /// non-positive count, for a call after the last row, or on a compressor error.
    #[doc(alias = "encodeRows")]
    pub fn encode_rows(&mut self, num_rows: i32) -> bool {
        let height = usize::try_from(self.src.info().height()).unwrap_or(0);
        if num_rows <= 0 || self.curr_row >= height {
            return false;
        }
        let n = usize::try_from(num_rows)
            .unwrap_or(0)
            .min(height - self.curr_row);
        for _ in 0..n {
            let Some(row) = self.src_row(self.curr_row) else {
                return false;
            };
            match self.cinfo.write_scanlines(&[row.as_slice()]) {
                Ok(1) => {}
                _ => return false,
            }
            self.curr_row += 1;
        }
        if self.curr_row == height {
            // SkJpegEncoderImpl::onEncodeRows: jpeg_finish_compress after the last row.
            if self.cinfo.finish_compress().is_err() {
                return false;
            }
            let bytes = self.cinfo.take_output();
            match self.out.lock() {
                Ok(mut out) => out.extend_from_slice(&bytes),
                Err(_) => return false,
            }
        }
        true
    }

    /// The row `y` as the compressor takes it: the source bytes, or the colour-transformed row.
    fn src_row(&self, y: usize) -> Option<Vec<u8>> {
        let info = self.src.info();
        let width = usize::try_from(info.width()).unwrap_or(0);
        let bytes_per_row = width * info.bytes_per_pixel();
        let pixels = self.src.addr()?;
        let start = y.checked_mul(self.src.row_bytes())?;
        let src_row = pixels.get(start..start + bytes_per_row)?;
        match &self.source.convert {
            None => Some(src_row.to_vec()),
            Some(dst_info) => {
                // SkJpegEncoderMgr::colorTransformProc: SkConvertPixels of one row.
                let src_row_info = ImageInfo::new(
                    (info.width(), 1),
                    info.color_type(),
                    info.alpha_type(),
                    None,
                );
                let dst_rb = dst_info.min_row_bytes();
                let mut converted = vec![0u8; dst_rb];
                if !convert_pixels(
                    dst_info,
                    &mut converted,
                    dst_rb,
                    &src_row_info,
                    src_row,
                    bytes_per_row,
                ) {
                    return None;
                }
                Some(converted)
            }
        }
    }
}

/// Port of `SkJpegEncoder::Make` (SkJpegEncoderImpl.cpp#L433-L441): the encoder for `src`, with
/// its metadata written and the compressor started. The file goes to `out`.
// Port of: src/encode/SkJpegEncoderImpl.cpp#L433-L441 (chrome/m156)
#[must_use]
pub fn make<'a>(
    out: Arc<Mutex<Vec<u8>>>,
    src: Pixmap<'a>,
    options: &Options,
) -> Option<JpegEncoder<'a>> {
    if options.origin.is_some() {
        return None;
    }
    let info = src.info();
    let width = u32::try_from(info.width()).ok()?;
    let height = u32::try_from(info.height()).ok()?;
    if width == 0 || height == 0 || src.addr().is_none() {
        return None;
    }
    let source = row_source(info, options)?;
    let segments = metadata_segments(&src, options);

    let mut cinfo = Compress::new();
    cinfo.set_image(width, height, source.in_cs, source.components);
    cinfo.set_defaults().ok()?;
    // SkJpegEncoderImpl.cpp#L160-L195: the chroma sampling factors (not for one component).
    if source.components != 1 {
        let (h, v) = match options.downsample {
            Downsample::BothDirections => (2, 2),
            Downsample::Horizontal => (2, 1),
            Downsample::No => (1, 1),
        };
        cinfo.set_component_sampling(0, h, v).ok()?;
    }
    // SkJpegEncoderImpl::initializeCommon: optimized Huffman tables, the quality, the header.
    cinfo.set_optimize_coding(true).ok()?;
    cinfo
        .set_quality(i32::try_from(options.quality).ok()?, true)
        .ok()?;
    cinfo.start_compress(true).ok()?;
    for (marker, body) in &segments {
        cinfo.write_marker(*marker, body).ok()?;
    }
    Some(JpegEncoder {
        src,
        source,
        cinfo,
        out,
        curr_row: 0,
    })
}

/// The JPEG bytes of `src` in one go: `make`, then `encode_rows` for every row.
fn encode_to_vec(src: &Pixmap<'_>, options: &Options) -> Option<Vec<u8>> {
    let view = Pixmap::new_readonly(src.info(), src.addr()?, src.row_bytes())?;
    let out = Arc::new(Mutex::new(Vec::new()));
    let height = src.info().height();
    let mut encoder = make(Arc::clone(&out), view, options)?;
    if !encoder.encode_rows(height) {
        return None;
    }
    drop(encoder);
    let bytes = out.lock().ok()?.clone();
    Some(bytes)
}

/// Encodes the pixels of `src` as a JPEG into `writer`. Returns `true` on success, and `false`
/// for an invalid or unsupported pixmap, unsupported options, or a failed write.
///
/// Port of `SkJpegEncoder::Encode(SkWStream*, const SkPixmap&, const Options&)`.
#[doc(alias = "SkJpegEncoder::Encode")]
pub fn encode<W: io::Write>(pixmap: &Pixmap<'_>, writer: &mut W, options: &Options) -> bool {
    match encode_to_vec(pixmap, options) {
        Some(bytes) => writer.write_all(&bytes).is_ok(),
        None => false,
    }
}

/// Returns the JPEG bytes of `src`, or `None` if the encoding fails.
///
/// Port of `SkJpegEncoder::Encode(const SkPixmap&, const Options&)`.
#[doc(alias = "SkJpegEncoder::Encode")]
#[must_use]
pub fn encode_pixmap(src: &Pixmap<'_>, options: &Options) -> Option<Data> {
    encode_to_vec(src, options).map(|bytes| Data::new_copy(&bytes))
}

/// Returns the JPEG bytes of an image's pixels, or `None` if they cannot be read or the encoding
/// fails. Raster images only: the pixels come from the legacy bitmap, as in
/// `SkJpegEncoder::Encode(GrDirectContext*, const SkImage*, const Options&)` with no GPU context.
// Port of: src/encode/SkJpegEncoderImpl.cpp#L422-L431 (chrome/m156)
#[doc(alias = "SkJpegEncoder::Encode")]
#[must_use]
pub fn encode_image(img: &Image, options: &Options) -> Option<Data> {
    let bitmap = img.as_legacy_bitmap()?;
    let pixmap = bitmap.peek_pixels()?;
    encode_pixmap(&pixmap, options)
}
