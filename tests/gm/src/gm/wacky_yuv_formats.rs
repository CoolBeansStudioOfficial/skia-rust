// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/wacky_yuv_formats.cpp (chrome/m156), the YUVSplitterGM and WackyYUVFormatsGM classes
//
// Only the WackyYUVFormatsGM configuration that runs on the CPU is registered: the generator path
// (`Type::kFromGenerator`) with no limited range, no target colour space, no subset and no cubic
// sampling. The other WackyYUVFormatsGM registrations need a GPU context, and YUVMakeColorSpaceGM
// needs one to create its images, so they are not registered (see the manifest reasons).

// GM ports mirror the C++ int-to-float conversions of image sizes and the byte-level planes.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
#![allow(clippy::too_many_lines, clippy::many_single_char_names)]

use std::sync::atomic::{AtomicU32, Ordering};

use crate::prelude::*;
use crate::tool_utils::{color_to_565, get_resource_as_image};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::{Color, ColorChannel, pm_color_set_argb};
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_priv::premultiply_argb_inline;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::half::float_to_half;
use skia_rust_core::image::Image;
use skia_rust_core::image_generator::ImageGenerator;
use skia_rust_core::image_info::{ImageInfo, YUVColorSpace};
use skia_rust_core::images;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::scalar::scalar_round_to_int;
use skia_rust_core::size::ISize;
use skia_rust_core::utils::text_utils::{self, Align};
use skia_rust_core::yuv_math::{color_matrix_rgb2yuv, color_matrix_yuv2rgb};
use skia_rust_core::yuva_info::{
    PlaneConfig, Siting, Subsampling, YUVAChannels, YUVAInfo, plane_dimensions_array,
};
use skia_rust_core::yuva_pixmaps::YUVAPixmaps;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::create_portable_typeface;

// Port of: gm/wacky_yuv_formats.cpp#L75-L79 (chrome/m156)
const TILE_WIDTH_HEIGHT: i32 = 128;
const LABEL_WIDTH: i32 = 64;
const LABEL_HEIGHT: i32 = 32;
const PAD: i32 = 1;

/// `kLast_YUVFormat + 1`: the number of [`YUVFormat`]s.
const NUM_YUV_FORMATS: usize = 10;

/// The YUV color spaces in `SkYUVColorSpace` order, `kJPEG_SkYUVColorSpace` to
/// `kLastEnum_SkYUVColorSpace`.
const YUV_COLOR_SPACES: [YUVColorSpace; 29] = [
    YUVColorSpace::JPEGFull,
    YUVColorSpace::Rec601Limited,
    YUVColorSpace::Rec709Full,
    YUVColorSpace::Rec709Limited,
    YUVColorSpace::BT2020_8BitFull,
    YUVColorSpace::BT2020_8BitLimited,
    YUVColorSpace::BT2020_10BitFull,
    YUVColorSpace::BT2020_10BitLimited,
    YUVColorSpace::BT2020_12BitFull,
    YUVColorSpace::BT2020_12BitLimited,
    YUVColorSpace::BT2020_16BitFull,
    YUVColorSpace::BT2020_16BitLimited,
    YUVColorSpace::FCCFull,
    YUVColorSpace::FCCLimited,
    YUVColorSpace::SMPTE240Full,
    YUVColorSpace::SMPTE240Limited,
    YUVColorSpace::YDZDXFull,
    YUVColorSpace::YDZDXLimited,
    YUVColorSpace::GBRFull,
    YUVColorSpace::GBRLimited,
    YUVColorSpace::YCgCo_8BitFull,
    YUVColorSpace::YCgCo_8BitLimited,
    YUVColorSpace::YCgCo_10BitFull,
    YUVColorSpace::YCgCo_10BitLimited,
    YUVColorSpace::YCgCo_12BitFull,
    YUVColorSpace::YCgCo_12BitLimited,
    YUVColorSpace::YCgCo_16BitFull,
    YUVColorSpace::YCgCo_16BitLimited,
    YUVColorSpace::Identity,
];

// Port of: gm/wacky_yuv_formats.cpp#L83-L111 (chrome/m156), `YUVFormat`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum YUVFormat {
    P016,
    P010,
    P016F,
    Y416,
    Ayuv,
    Y410,
    NV12,
    NV21,
    I420,
    YV12,
}

const ALL_YUV_FORMATS: [YUVFormat; NUM_YUV_FORMATS] = [
    YUVFormat::P016,
    YUVFormat::P010,
    YUVFormat::P016F,
    YUVFormat::Y416,
    YUVFormat::Ayuv,
    YUVFormat::Y410,
    YUVFormat::NV12,
    YUVFormat::NV21,
    YUVFormat::I420,
    YUVFormat::YV12,
];

// Port of: gm/wacky_yuv_formats.cpp#L115-L129 (chrome/m156), `has_alpha_channel`
fn has_alpha_channel(format: YUVFormat) -> bool {
    match format {
        YUVFormat::P016
        | YUVFormat::P010
        | YUVFormat::P016F
        | YUVFormat::NV12
        | YUVFormat::NV21
        | YUVFormat::I420
        | YUVFormat::YV12 => false,
        YUVFormat::Y416 | YUVFormat::Ayuv | YUVFormat::Y410 => true,
    }
}

// The pixels of one plane (`SkBitmap` in the C++), with its own row bytes.
// skia-rust: an owned byte buffer, so that the plane formats can be written directly.
#[derive(Clone, Debug)]
struct PlaneBuffer {
    info: ImageInfo,
    row_bytes: usize,
    pixels: Vec<u8>,
}

impl PlaneBuffer {
    // Port of: SkBitmap::allocPixels with the minimum row bytes.
    fn new(width: i32, height: i32, color_type: ColorType, alpha_type: AlphaType) -> Self {
        let info = ImageInfo::new(ISize::new(width, height), color_type, alpha_type, None);
        let row_bytes = info.min_row_bytes();
        let pixels = vec![0_u8; row_bytes * height as usize];
        Self {
            info,
            row_bytes,
            pixels,
        }
    }

    fn width(&self) -> i32 {
        self.info.width()
    }

    fn height(&self) -> i32 {
        self.info.height()
    }

    fn dimensions(&self) -> ISize {
        self.info.dimensions()
    }

    // The byte offset of pixel (x, y).
    fn offset(&self, x: i32, y: i32) -> usize {
        y as usize * self.row_bytes + x as usize * self.info.bytes_per_pixel()
    }

    // Port of: SkBitmap::getAddr8 (the read side; writes go through `set_u8`).
    fn get_u8(&self, x: i32, y: i32) -> u8 {
        self.pixels[self.offset(x, y)]
    }

    fn set_u8(&mut self, x: i32, y: i32, value: u8) {
        let offset = self.offset(x, y);
        self.pixels[offset] = value;
    }

    // Channel `index` of the pixel, as a native 16-bit value (`uint16_t* dst`).
    fn set_u16(&mut self, x: i32, y: i32, index: usize, value: u16) {
        let offset = self.offset(x, y) + 2 * index;
        self.pixels[offset..offset + 2].copy_from_slice(&value.to_ne_bytes());
    }

    // The whole pixel as a native 32-bit value (`*getAddr32(x, y) = value`).
    fn set_u32(&mut self, x: i32, y: i32, value: u32) {
        let offset = self.offset(x, y);
        self.pixels[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
    }

    // Channel `index` of the pixel, as a native 32-bit float (`RGBA_F32` pixels).
    fn set_f32(&mut self, x: i32, y: i32, index: usize, value: f32) {
        let offset = self.offset(x, y) + 4 * index;
        self.pixels[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
    }

    fn get_f32(&self, x: i32, y: i32, index: usize) -> f32 {
        let offset = self.offset(x, y) + 4 * index;
        f32::from_ne_bytes([
            self.pixels[offset],
            self.pixels[offset + 1],
            self.pixels[offset + 2],
            self.pixels[offset + 3],
        ])
    }
}

// All the planes we need to construct the various YUV formats.
// Port of: gm/wacky_yuv_formats.cpp#L224-L238 (chrome/m156), `PlaneData`
struct PlaneData {
    y_full: PlaneBuffer,
    u_full: PlaneBuffer,
    v_full: PlaneBuffer,
    a_full: PlaneBuffer,
    u_quarter: PlaneBuffer, // 2x2 downsampled U channel
    v_quarter: PlaneBuffer, // 2x2 downsampled V channel
    full: PlaneBuffer,
    quarter: PlaneBuffer, // 2x2 downsampled YUVA
}

// Port of: gm/wacky_yuv_formats.cpp#L131-L205 (chrome/m156), `YUVAPlanarConfig`
struct YUVAPlanarConfig {
    plane_config: PlaneConfig,
    subsampling: Subsampling,
    origin: EncodedOrigin,
}

impl YUVAPlanarConfig {
    fn new(format: YUVFormat, opaque: bool, origin: EncodedOrigin) -> Self {
        let (plane_config, subsampling) = match format {
            YUVFormat::P016 | YUVFormat::P010 | YUVFormat::P016F | YUVFormat::NV12 => (
                if opaque {
                    PlaneConfig::Y_UV
                } else {
                    PlaneConfig::Y_UV_A
                },
                Subsampling::S420,
            ),
            YUVFormat::Y416 | YUVFormat::Y410 => (
                if opaque {
                    PlaneConfig::UYV
                } else {
                    PlaneConfig::UYVA
                },
                Subsampling::S444,
            ),
            YUVFormat::Ayuv => (
                if opaque {
                    PlaneConfig::YUV
                } else {
                    PlaneConfig::YUVA
                },
                Subsampling::S444,
            ),
            YUVFormat::NV21 => (
                if opaque {
                    PlaneConfig::Y_VU
                } else {
                    PlaneConfig::Y_VU_A
                },
                Subsampling::S420,
            ),
            YUVFormat::I420 => (
                if opaque {
                    PlaneConfig::Y_U_V
                } else {
                    PlaneConfig::Y_U_V_A
                },
                Subsampling::S420,
            ),
            YUVFormat::YV12 => (
                if opaque {
                    PlaneConfig::Y_V_U
                } else {
                    PlaneConfig::Y_V_U_A
                },
                Subsampling::S420,
            ),
        };
        Self {
            plane_config,
            subsampling,
            origin,
        }
    }

    fn num_planes(&self) -> usize {
        skia_rust_core::yuva_info::num_planes(self.plane_config)
    }

    // Port of: gm/wacky_yuv_formats.cpp#L187-L199 (chrome/m156), `makeYUVAPixmaps`
    fn make_yuva_pixmaps(
        &self,
        dimensions: ISize,
        yuv_color_space: YUVColorSpace,
        bitmaps: &[PlaneBuffer],
    ) -> Option<YUVAPixmaps> {
        let info = YUVAInfo::new(
            dimensions,
            self.plane_config,
            self.subsampling,
            yuv_color_space,
            self.origin,
            (Siting::Centered, Siting::Centered),
        )?;
        let n = self.num_planes();
        if bitmaps.len() < n {
            return None;
        }
        // The pixmaps read the planes, so each borrows a copy of its plane's pixels.
        let mut buffers: Vec<Vec<u8>> = bitmaps[..n].iter().map(|b| b.pixels.clone()).collect();
        let pixmaps: Vec<Pixmap<'_>> = bitmaps[..n]
            .iter()
            .zip(buffers.iter_mut())
            .map(|(b, buf)| Pixmap::new(&b.info, buf.as_mut_slice(), b.row_bytes))
            .collect::<Option<Vec<_>>>()?;
        YUVAPixmaps::from_external_pixmaps(&info, &pixmaps)
    }
}

// Add a portion of a circle to 'path'. The points 'o1' and 'o2' are on the border of the circle.
// Port of: gm/wacky_yuv_formats.cpp#L240-L280 (chrome/m156), `add_arc`
fn add_arc(
    path: &mut PathBuilder,
    o1: Point,
    v1: Vector,
    o2: Point,
    v2: Vector,
    circles: Option<&mut Vec<Rect>>,
    take_long_way_round: bool,
) {
    let v3 = Vector::new(-v1.y, v1.x);
    let v4 = Vector::new(v2.y, -v2.x);
    let t = ((o2.x - o1.x) * v4.y - (o2.y - o1.y) * v4.x) / (v3.x * v4.y - v3.y * v4.x);
    let center = Point::new(o1.x + t * v3.x, o1.y + t * v3.y);
    let r = Rect::from_ltrb(center.x - t, center.y - t, center.x + t, center.y + t);
    if let Some(circles) = circles {
        circles.push(r);
    }

    let mut start_v = o1 - center;
    let mut end_v = o2 - center;
    start_v.normalize();
    end_v.normalize();

    let mut start_deg = radians_to_degrees(start_v.y.atan2(start_v.x));
    let mut end_deg = radians_to_degrees(end_v.y.atan2(end_v.x));

    start_deg += 360.0;
    start_deg %= 360.0;
    end_deg += 360.0;
    end_deg %= 360.0;
    if end_deg < start_deg {
        end_deg += 360.0;
    }

    let mut sweep_deg = (end_deg - start_deg).abs();
    if !take_long_way_round {
        sweep_deg -= 360.0;
    }
    path.arc_to(r, start_deg, sweep_deg, false);
}

// `SkRadiansToDegrees`, with `SK_ScalarPI` as the float literal.
fn radians_to_degrees(radians: f32) -> f32 {
    radians * (180.0 / std::f32::consts::PI)
}

// Port of: gm/wacky_yuv_formats.cpp#L282-L336 (chrome/m156), `create_splat`
fn create_splat(
    o: Point,
    inner_radius: f32,
    outer_radius: f32,
    ratio: f32,
    num_lobes: i32,
    mut circles: Option<&mut Vec<Rect>>,
) -> Path {
    if num_lobes <= 1 {
        return PathBuilder::new().detach();
    }

    let num_divisions = 2 * num_lobes;
    let full_lobe_degrees = 360.0_f32 / num_lobes as f32;
    let out_degrees = ratio * full_lobe_degrees / (ratio + 1.0);
    let inner_degrees = full_lobe_degrees / (ratio + 1.0);

    let mut outer_step = Matrix::default();
    outer_step.set_rotate(out_degrees, None::<Point>);
    let mut inner_step = Matrix::default();
    inner_step.set_rotate(inner_degrees, None::<Point>);

    let mut cur_v = Vector::new(0.0, 1.0);
    if let Some(circles) = circles.as_deref_mut() {
        circles.push(Rect::from_ltrb(
            o.x - inner_radius,
            o.y - inner_radius,
            o.x + inner_radius,
            o.y + inner_radius,
        ));
    }

    let mut p = PathBuilder::new();
    p.move_to(Point::new(
        o.x + inner_radius * cur_v.x,
        o.y + inner_radius * cur_v.y,
    ));
    for i in 0..num_divisions {
        let next_v;
        if i % 2 == 0 {
            next_v = outer_step.map_vector(cur_v);
            let top = Point::new(o.x + outer_radius * cur_v.x, o.y + outer_radius * cur_v.y);
            let next_top = Point::new(o.x + outer_radius * next_v.x, o.y + outer_radius * next_v.y);
            p.line_to(top);
            add_arc(
                &mut p,
                top,
                cur_v,
                next_top,
                next_v,
                circles.as_deref_mut(),
                true,
            );
        } else {
            next_v = inner_step.map_vector(cur_v);
            let bot = Point::new(o.x + inner_radius * cur_v.x, o.y + inner_radius * cur_v.y);
            let next_bot = Point::new(o.x + inner_radius * next_v.x, o.y + inner_radius * next_v.y);
            p.line_to(bot);
            add_arc(&mut p, bot, cur_v, next_bot, next_v, None, false);
        }
        cur_v = next_v;
    }
    p.close();
    p.detach()
}

// Port of: gm/wacky_yuv_formats.cpp#L338-L386 (chrome/m156), `make_bitmap`
//
// skia-rust: the C++ draws into a raster canvas over the bitmap's pixels. This draws into a raster
// surface and reads its pixels back, which is the same pixels for a raster surface.
fn make_bitmap(
    color_type: ColorType,
    path: &Path,
    circles: &[Rect],
    opaque: bool,
    pad_with_red: bool,
) -> Option<PlaneBuffer> {
    let green = color_to_565(Color::from(0xFF_B2_F0_68_u32));
    let blue = color_to_565(Color::from(0xFF_AD_A7_FC_u32));
    let yellow = color_to_565(Color::from(0xFF_FF_DD_75_u32));
    let magenta = color_to_565(Color::from(0xFF_FF_3C_D9_u32));
    let cyan = color_to_565(Color::from(0xFF_2D_ED_CD_u32));

    let width_height = TILE_WIDTH_HEIGHT + if pad_with_red { 2 * SUBSET_PADDING } else { 0 };
    let ii = ImageInfo::new(
        ISize::new(width_height, width_height),
        color_type,
        AlphaType::Premul,
        None,
    );
    let mut surf = surfaces::raster(&ii, None, None)?;
    let canvas = surf.canvas();
    if pad_with_red {
        canvas.clear(Color::RED);
        canvas.translate((SUBSET_PADDING as f32, SUBSET_PADDING as f32));
        canvas.clip_rect(
            Rect::from_wh(TILE_WIDTH_HEIGHT as f32, TILE_WIDTH_HEIGHT as f32),
            ClipOp::Intersect,
            false,
        );
    }
    canvas.clear(if opaque { green } else { Color::TRANSPARENT });

    let mut paint = Paint::default();
    paint.set_anti_alias(false); // serialize-8888 doesn't seem to work well w/ partial transparency
    paint.set_color(blue);
    canvas.draw_path(path, &paint);

    paint.set_blend_mode(BlendMode::Src);
    for (i, circle) in circles.iter().enumerate() {
        let color = match i % 3 {
            0 => yellow,
            1 => magenta,
            _ => cyan,
        };
        paint.set_color(color);
        paint.set_alpha(if opaque { 0xFF } else { 0x40 });
        let mut r = *circle;
        r.inset((r.width() / 4.0, r.height() / 4.0));
        canvas.draw_oval(r, &paint);
    }

    let mut pixels = vec![0_u8; ii.compute_byte_size(ii.min_row_bytes())];
    let row_bytes = ii.min_row_bytes();
    if !canvas.read_pixels(&ii, &mut pixels, row_bytes, (0, 0)) {
        return None;
    }
    Some(PlaneBuffer {
        info: ii,
        row_bytes,
        pixels,
    })
}

const SUBSET_PADDING: i32 = 8;

// Port of: gm/wacky_yuv_formats.cpp#L388-L398 (chrome/m156), `convert_rgba_to_yuva`
fn convert_rgba_to_yuva(mtx: &[f32; 20], col: Color) -> [u8; 4] {
    let r = col.r();
    let g = col.g();
    let b = col.b();
    let clamp = |v: i32| v.clamp(0, 255) as u8;
    [
        clamp(scalar_round_to_int(
            mtx[0] * f32::from(r) + mtx[1] * f32::from(g) + mtx[2] * f32::from(b) + mtx[4] * 255.0,
        )),
        clamp(scalar_round_to_int(
            mtx[5] * f32::from(r) + mtx[6] * f32::from(g) + mtx[7] * f32::from(b) + mtx[9] * 255.0,
        )),
        clamp(scalar_round_to_int(
            mtx[10] * f32::from(r)
                + mtx[11] * f32::from(g)
                + mtx[12] * f32::from(b)
                + mtx[14] * 255.0,
        )),
        col.a(),
    ]
}

// Port of: gm/wacky_yuv_formats.cpp#L399-L503 (chrome/m156), `extract_planes`
fn extract_planes(
    orig_bm: &PlaneBuffer,
    yuv_color_space: YUVColorSpace,
    origin: EncodedOrigin,
) -> Option<PlaneData> {
    let mut ii = orig_bm.info.clone();
    if origin.swaps_width_height() {
        ii = ii.with_wh(ii.height(), ii.width());
    }

    // Draw the original, through the inverse of the origin, into a bitmap of the oriented size.
    let mut surf = surfaces::raster(&ii, None, None)?;
    let matrix = origin.to_matrix_inverse(orig_bm.width(), orig_bm.height());
    let canvas = surf.canvas();
    canvas.concat(&matrix);
    let orig_image = plane_image(orig_bm)?;
    canvas.draw_image(&orig_image, (0.0, 0.0), None);
    let mut oriented_pixels = vec![0_u8; ii.compute_byte_size(ii.min_row_bytes())];
    let oriented_row_bytes = ii.min_row_bytes();
    if !canvas.read_pixels(&ii, &mut oriented_pixels, oriented_row_bytes, (0, 0)) {
        return None;
    }
    let oriented = Pixmap::new(&ii, &mut oriented_pixels, oriented_row_bytes)?;

    // The identity color space needs the JPEG planes, as in the C++.
    let yuv_color_space = if yuv_color_space == YUVColorSpace::Identity {
        YUVColorSpace::JPEGFull
    } else {
        yuv_color_space
    };

    let (w, h) = (ii.width(), ii.height());
    let gray =
        |width, height| PlaneBuffer::new(width, height, ColorType::Gray8, AlphaType::Unpremul);
    let mut planes = PlaneData {
        y_full: gray(w, h),
        u_full: gray(w, h),
        v_full: gray(w, h),
        a_full: PlaneBuffer::new(w, h, ColorType::Alpha8, AlphaType::Premul),
        u_quarter: gray(w / 2, h / 2),
        v_quarter: gray(w / 2, h / 2),
        full: PlaneBuffer::new(w, h, ColorType::RGBAF32, AlphaType::Unpremul),
        quarter: PlaneBuffer::new(w / 2, h / 2, ColorType::RGBAF32, AlphaType::Unpremul),
    };

    let mtx = color_matrix_rgb2yuv(yuv_color_space);
    for y in 0..oriented.height() {
        for x in 0..oriented.width() {
            let col = oriented.get_color((x, y));
            let yuva = convert_rgba_to_yuva(&mtx, col);
            planes.y_full.set_u8(x, y, yuva[0]);
            planes.u_full.set_u8(x, y, yuva[1]);
            planes.v_full.set_u8(x, y, yuva[2]);
            planes.a_full.set_u8(x, y, yuva[3]);
            // TODO: render in F32 rather than converting here
            planes.full.set_f32(x, y, 0, f32::from(yuva[0]) / 255.0);
            planes.full.set_f32(x, y, 1, f32::from(yuva[1]) / 255.0);
            planes.full.set_f32(x, y, 2, f32::from(yuva[2]) / 255.0);
            planes.full.set_f32(x, y, 3, f32::from(yuva[3]) / 255.0);
        }
    }

    for y in 0..oriented.height() / 2 {
        for x in 0..oriented.width() / 2 {
            let (x2, y2) = (2 * x, 2 * y);
            let samples = |p: &PlaneBuffer| {
                [
                    p.get_u8(x2, y2),
                    p.get_u8(x2 + 1, y2),
                    p.get_u8(x2, y2 + 1),
                    p.get_u8(x2 + 1, y2 + 1),
                ]
            };
            let sum = |s: [u8; 4]| s.iter().map(|&v| u32::from(v)).sum::<u32>();
            let y_accum = sum(samples(&planes.y_full));
            let u_accum = sum(samples(&planes.u_full));
            let v_accum = sum(samples(&planes.v_full));
            let a_accum = sum(samples(&planes.a_full));

            planes.u_quarter.set_u8(x, y, (u_accum as f32 / 4.0) as u8);
            planes.v_quarter.set_u8(x, y, (v_accum as f32 / 4.0) as u8);

            // TODO: render in F32 rather than converting here
            planes
                .quarter
                .set_f32(x, y, 0, y_accum as f32 / (4.0 * 255.0));
            planes
                .quarter
                .set_f32(x, y, 1, u_accum as f32 / (4.0 * 255.0));
            planes
                .quarter
                .set_f32(x, y, 2, v_accum as f32 / (4.0 * 255.0));
            planes
                .quarter
                .set_f32(x, y, 3, a_accum as f32 / (4.0 * 255.0));
        }
    }
    Some(planes)
}

// The `SkEncodedOrigin` with the value `n` (1 to 8), as the C++ enum's integer values are used.
fn origin_from_u32(n: u32) -> EncodedOrigin {
    match n {
        2 => EncodedOrigin::TopRight,
        3 => EncodedOrigin::BottomRight,
        4 => EncodedOrigin::BottomLeft,
        5 => EncodedOrigin::LeftTop,
        6 => EncodedOrigin::RightTop,
        7 => EncodedOrigin::RightBottom,
        8 => EncodedOrigin::LeftBottom,
        _ => EncodedOrigin::TopLeft,
    }
}

// Wraps `plane` as an image, for drawing it.
fn plane_image(plane: &PlaneBuffer) -> Option<Image> {
    images::raster_from_data(&plane.info, Data::new_copy(&plane.pixels), plane.row_bytes)
}

// Create some flavor of a 16bits/channel bitmap from a RGBA_F32 source.
// Port of: gm/wacky_yuv_formats.cpp#L533-L551 (chrome/m156), `make_16`
fn make_16(
    src: &PlaneBuffer,
    dst_ct: ColorType,
    channels: usize,
    convert: impl Fn(&mut [u16], &[f32]),
) -> PlaneBuffer {
    let mut result = PlaneBuffer::new(src.width(), src.height(), dst_ct, AlphaType::Unpremul);
    for y in 0..src.height() {
        for x in 0..src.width() {
            let src_pixel: [f32; 4] = [
                src.get_f32(x, y, 0),
                src.get_f32(x, y, 1),
                src.get_f32(x, y, 2),
                src.get_f32(x, y, 3),
            ];
            let mut dst_pixel = [0_u16; 4];
            convert(&mut dst_pixel[..channels], &src_pixel);
            for (i, value) in dst_pixel[..channels].iter().enumerate() {
                result.set_u16(x, y, i, *value);
            }
        }
    }
    result
}

// Port of: gm/wacky_yuv_formats.cpp#L553 (chrome/m156), `flt_2_uint16`
fn flt_2_uint16(flt: f32) -> u16 {
    // The C++ narrows the int to uint16_t, which wraps.
    (scalar_round_to_int(flt * 65535.0) as u32 & 0xFFFF) as u16
}

// Create a 2x2 downsampled bitmap. It is stored in an RG texture. It can optionally be uv (i.e.,
// NV12) or vu (i.e., NV21).
// Port of: gm/wacky_yuv_formats.cpp#L505-L531 (chrome/m156), `make_quarter_2_channel`
fn make_quarter_2_channel(
    full_y: &PlaneBuffer,
    quarter_u: &PlaneBuffer,
    quarter_v: &PlaneBuffer,
    uv: bool,
) -> PlaneBuffer {
    let mut result = PlaneBuffer::new(
        full_y.width() / 2,
        full_y.height() / 2,
        ColorType::R8G8UNorm,
        AlphaType::Unpremul,
    );
    for y in 0..full_y.height() / 2 {
        for x in 0..full_y.width() / 2 {
            let u8v = quarter_u.get_u8(x, y);
            let v8v = quarter_v.get_u8(x, y);
            let value = if uv {
                (u16::from(v8v) << 8) | u16::from(u8v)
            } else {
                (u16::from(u8v) << 8) | u16::from(v8v)
            };
            result.set_u16(x, y, 0, value);
        }
    }
    result
}

// Recombine the separate planes into some YUV format. Returns the planes, in order.
// Port of: gm/wacky_yuv_formats.cpp#L556-L700 (chrome/m156), `create_YUV`
fn create_yuv(planes: &PlaneData, yuv_format: YUVFormat, opaque: bool) -> Vec<PlaneBuffer> {
    let mut result: Vec<PlaneBuffer> = Vec::new();
    match yuv_format {
        YUVFormat::Y416 => {
            result.push(make_16(
                &planes.full,
                ColorType::R16G16B16A16UNorm,
                4,
                |dst, src| {
                    dst[0] = flt_2_uint16(src[1]); // U
                    dst[1] = flt_2_uint16(src[0]); // Y
                    dst[2] = flt_2_uint16(src[2]); // V
                    dst[3] = flt_2_uint16(src[3]); // A
                },
            ));
        }
        YUVFormat::Ayuv => {
            let mut yuva_full = PlaneBuffer::new(
                planes.y_full.width(),
                planes.y_full.height(),
                ColorType::RGBA8888,
                AlphaType::Unpremul,
            );
            for y in 0..planes.y_full.height() {
                for x in 0..planes.y_full.width() {
                    let yv = planes.y_full.get_u8(x, y);
                    let u = planes.u_full.get_u8(x, y);
                    let v = planes.v_full.get_u8(x, y);
                    let a = planes.a_full.get_u8(x, y);
                    // NOT premul!
                    // V and Y swapped to match RGBA layout
                    let c = pm_color_set_argb(a, v, u, yv);
                    yuva_full.set_u32(x, y, c);
                }
            }
            result.push(yuva_full);
        }
        YUVFormat::Y410 => {
            let mut yuva_full = PlaneBuffer::new(
                planes.y_full.width(),
                planes.y_full.height(),
                ColorType::RGBA1010102,
                AlphaType::Unpremul,
            );
            for y in 0..planes.y_full.height() {
                for x in 0..planes.y_full.width() {
                    let scale = |v: u8| scalar_round_to_int((f32::from(v) / 255.0) * 1023.0) as u32;
                    let yv = scale(planes.y_full.get_u8(x, y));
                    let u = scale(planes.u_full.get_u8(x, y));
                    let v = scale(planes.v_full.get_u8(x, y));
                    let a =
                        scalar_round_to_int((f32::from(planes.a_full.get_u8(x, y)) / 255.0) * 3.0)
                            as u32;
                    // NOT premul!
                    yuva_full.set_u32(x, y, (a << 30) | (v << 20) | (yv << 10) | u);
                }
            }
            result.push(yuva_full);
        }
        YUVFormat::P016 | YUVFormat::P010 => {
            let ten_bits = yuv_format == YUVFormat::P010;
            let mask = move |v: u16| if ten_bits { v & 0xFFC0 } else { v };
            result.push(make_16(&planes.full, ColorType::A16UNorm, 1, |dst, src| {
                dst[0] = mask(flt_2_uint16(src[0]));
            }));
            result.push(make_16(
                &planes.quarter,
                ColorType::R16G16UNorm,
                2,
                |dst, src| {
                    dst[0] = mask(flt_2_uint16(src[1]));
                    dst[1] = mask(flt_2_uint16(src[2]));
                },
            ));
            if !opaque {
                result.push(make_16(&planes.full, ColorType::A16UNorm, 1, |dst, src| {
                    dst[0] = mask(flt_2_uint16(src[3]));
                }));
            }
            return result;
        }
        YUVFormat::P016F => {
            result.push(make_16(&planes.full, ColorType::A16Float, 1, |dst, src| {
                dst[0] = float_to_half(src[0]);
            }));
            result.push(make_16(
                &planes.quarter,
                ColorType::R16G16Float,
                2,
                |dst, src| {
                    dst[0] = float_to_half(src[1]);
                    dst[1] = float_to_half(src[2]);
                },
            ));
            if !opaque {
                result.push(make_16(&planes.full, ColorType::A16Float, 1, |dst, src| {
                    dst[0] = float_to_half(src[3]);
                }));
            }
            return result;
        }
        YUVFormat::NV12 => {
            let uv_quarter =
                make_quarter_2_channel(&planes.y_full, &planes.u_quarter, &planes.v_quarter, true);
            result.push(planes.y_full.clone());
            result.push(uv_quarter);
        }
        YUVFormat::NV21 => {
            let vu_quarter =
                make_quarter_2_channel(&planes.y_full, &planes.u_quarter, &planes.v_quarter, false);
            result.push(planes.y_full.clone());
            result.push(vu_quarter);
        }
        YUVFormat::I420 => {
            result.push(planes.y_full.clone());
            result.push(planes.u_quarter.clone());
            result.push(planes.v_quarter.clone());
        }
        YUVFormat::YV12 => {
            result.push(planes.y_full.clone());
            result.push(planes.v_quarter.clone());
            result.push(planes.u_quarter.clone());
        }
    }
    if !opaque && !has_alpha_channel(yuv_format) {
        result.push(planes.a_full.clone());
    }
    result
}

// Converts a YUVA sample to a premultiplied RGBA pixel.
// Port of: tools/gpu/YUVUtils.cpp (chrome/m156), `convert_yuva_to_rgba`
fn convert_yuva_to_rgba(mtx: &[f32; 20], yuva: [u8; 4]) -> u32 {
    let y = yuva[0];
    let u = yuva[1];
    let v = yuva[2];
    let a = yuva[3];
    let clamp = |value: i32| value.clamp(0, 255) as u8;
    let r = clamp(scalar_round_to_int(
        mtx[0] * f32::from(y) + mtx[1] * f32::from(u) + mtx[2] * f32::from(v) + mtx[4] * 255.0,
    ));
    let g = clamp(scalar_round_to_int(
        mtx[5] * f32::from(y) + mtx[6] * f32::from(u) + mtx[7] * f32::from(v) + mtx[9] * 255.0,
    ));
    let b = clamp(scalar_round_to_int(
        mtx[10] * f32::from(y) + mtx[11] * f32::from(u) + mtx[12] * f32::from(v) + mtx[14] * 255.0,
    ));
    premultiply_argb_inline(u32::from(a), u32::from(r), u32::from(g), u32::from(b))
}

// The generator of the YUV image: it converts the planes to premultiplied RGBA when it is asked for
// pixels (`Generator` in tools/gpu/YUVUtils.cpp).
// Port of: tools/gpu/YUVUtils.cpp#L68-L140 (chrome/m156), `Generator`
struct YuvGenerator {
    pixmaps: YUVAPixmaps,
    info: ImageInfo,
    unique_id: u32,
    flattened: Option<Vec<u32>>,
}

static NEXT_GENERATOR_ID: AtomicU32 = AtomicU32::new(1);

impl YuvGenerator {
    fn new(pixmaps: YUVAPixmaps) -> Self {
        let dims = pixmaps.yuva_info().dimensions();
        Self {
            info: ImageInfo::new(dims, ColorType::N32, AlphaType::Premul, None),
            pixmaps,
            unique_id: NEXT_GENERATOR_ID.fetch_add(1, Ordering::Relaxed),
            flattened: None,
        }
    }

    // Port of: YUVUtils.cpp `look_up`: the channel of the plane at a normalized point.
    fn look_up(&self, norm_pt: Point, plane: usize, channel: ColorChannel) -> u8 {
        let plane_view = self.pixmaps.plane(plane);
        let pmap = plane_view.pixmap();
        let x = (norm_pt.x * pmap.width() as f32).floor() as i32;
        let y = (norm_pt.y * pmap.height() as f32).floor() as i32;
        let ii = pmap
            .info()
            .with_color_type(ColorType::RGBA8888)
            .with_dimensions(ISize::new(1, 1));
        let mut pixel_bytes = [0_u8; 4];
        let ok = pmap.read_pixels(&ii, &mut pixel_bytes, 4, (x, y));
        debug_assert!(ok);
        let pixel = u32::from_ne_bytes(pixel_bytes);
        let shift = channel_index(channel) * 8;
        ((pixel >> shift) & 0xff) as u8
    }

    // Port of: YUVUtils.cpp `onGetPixels`, which flattens the planes on first use.
    fn flatten(&mut self) {
        let mut out = vec![0_u32; (self.info.width() * self.info.height()) as usize];
        let mtx = color_matrix_yuv2rgb(self.pixmaps.yuva_info().yuv_color_space());
        let Some(locations) = self.pixmaps.to_yuva_locations() else {
            self.flattened = Some(out);
            return;
        };
        let om = self.pixmaps.yuva_info().inverse_origin_matrix();
        let mut norm_x = 1.0_f32 / self.info.width() as f32;
        let mut norm_y = 1.0_f32 / self.info.height() as f32;
        if self.pixmaps.yuva_info().origin().swaps_width_height() {
            std::mem::swap(&mut norm_x, &mut norm_y);
        }
        for y in 0..self.info.height() {
            for x in 0..self.info.width() {
                let mut xy1 = Point::new(x as f32 + 0.5, y as f32 + 0.5);
                xy1 = om.map_point(xy1);
                xy1.x *= norm_x;
                xy1.y *= norm_y;

                let mut yuva = [0_u8, 0, 0, 255];
                for (c, channel) in [YUVAChannels::Y, YUVAChannels::U, YUVAChannels::V]
                    .into_iter()
                    .enumerate()
                {
                    let loc = locations[channel as usize];
                    yuva[c] = self.look_up(xy1, loc.plane as usize, loc.channel);
                }
                let a_loc = locations[YUVAChannels::A as usize];
                if a_loc.plane >= 0 {
                    yuva[3] = self.look_up(xy1, a_loc.plane as usize, a_loc.channel);
                }
                // Making premul here.
                out[(y * self.info.width() + x) as usize] = convert_yuva_to_rgba(&mtx, yuva);
            }
        }
        self.flattened = Some(out);
    }
}

// Port of: SkColorChannel (`kR`=0, `kG`=1, `kB`=2, `kA`=3), as the shift of the channel's byte.
fn channel_index(channel: ColorChannel) -> u32 {
    match channel {
        ColorChannel::R => 0,
        ColorChannel::G => 1,
        ColorChannel::B => 2,
        ColorChannel::A => 3,
    }
}

impl ImageGenerator for YuvGenerator {
    fn info(&self) -> &ImageInfo {
        &self.info
    }

    fn unique_id(&self) -> u32 {
        self.unique_id
    }

    // Port of: YUVUtils.cpp `Generator::onGetPixels`
    fn on_get_pixels(&mut self, info: &ImageInfo, pixels: &mut [u8], row_bytes: usize) -> bool {
        if self.flattened.is_none() {
            self.flatten();
        }
        let flat = self.flattened.as_ref().map_or(&[][..], Vec::as_slice);
        let mut bytes: Vec<u8> = flat.iter().flat_map(|p| p.to_ne_bytes()).collect();
        let row_bytes_flat = self.info.min_row_bytes();
        let Some(src) = Pixmap::new(&self.info, &mut bytes, row_bytes_flat) else {
            return false;
        };
        src.read_pixels(info, pixels, row_bytes, (0, 0))
    }
}

// Port of: gm/wacky_yuv_formats.cpp#L762-L773 (chrome/m156), `yuv_to_rgb_colorfilter`
fn yuv_to_rgb_colorfilter() -> Option<skia_rust_core::color_filter::ColorFilter> {
    const JPEG_CONVERSION_MATRIX: [f32; 20] = [
        1.0,
        0.0,
        1.402,
        0.0,
        -180.0 / 255.0, //
        1.0,
        -0.344_136,
        -0.714_136,
        0.0,
        136.0 / 255.0, //
        1.0,
        1.772,
        0.0,
        0.0,
        -227.6 / 255.0, //
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
    ];
    color_filters::matrix_row_major(&JPEG_CONVERSION_MATRIX, Clamp::Yes)
}

// Port of: gm/wacky_yuv_formats.cpp#L702-L731 (chrome/m156), `draw_col_label`
fn draw_col_label(canvas: &Canvas, x: i32, yuv_color_space: usize, opaque: bool) {
    const NAMES: [&str; 29] = [
        "JPEG",
        "601",
        "709F",
        "709L",
        "2020_8F",
        "2020_8L",
        "2020_10F",
        "2020_10L",
        "2020_12F",
        "2020_12L",
        "2020_16F",
        "2020_16L",
        "FCCF",
        "FCCL",
        "SMPTE240F",
        "SMPTE240L",
        "YDZDXF",
        "YDZDXL",
        "GBRF",
        "GBRL",
        "YCGCO_8F",
        "YCGCO_8L",
        "YCGCO_10F",
        "YCGCO_10L",
        "YCGCO_12F",
        "YCGCO_12L",
        "YCGCO_16F",
        "YCGCO_16L",
        "Identity",
    ];
    let paint = Paint::default();
    let mut font = Font::from_size(
        create_portable_typeface(Some("Sans"), FontStyle::bold()),
        16.0,
    );
    font.set_edging(Edging::Alias);
    let (_, text_rect) =
        font.measure_text(NAMES[yuv_color_space].as_bytes(), TextEncoding::UTF8, None);
    let mut y = text_rect.height() as i32;
    text_utils::draw_string(
        canvas,
        NAMES[yuv_color_space],
        x as f32,
        y as f32,
        &font,
        &paint,
        Align::Center,
    );
    let label = if opaque { "Opaque" } else { "Transparent" };
    let (_, text_rect) = font.measure_text(label.as_bytes(), TextEncoding::UTF8, None);
    y += text_rect.height() as i32;
    text_utils::draw_string(
        canvas,
        label,
        x as f32,
        y as f32,
        &font,
        &paint,
        Align::Center,
    );
}

// Port of: gm/wacky_yuv_formats.cpp#L733-L760 (chrome/m156), `draw_row_label`
fn draw_row_label(canvas: &Canvas, y: i32, yuv_format: usize) {
    const NAMES: [&str; NUM_YUV_FORMATS] = [
        "P016", "P010", "P016F", "Y416", "Ayuv", "Y410", "NV12", "NV21", "I420", "YV12",
    ];
    let paint = Paint::default();
    let mut font = Font::from_size(
        create_portable_typeface(Some("Sans"), FontStyle::bold()),
        16.0,
    );
    font.set_edging(Edging::Alias);
    let (_, text_rect) = font.measure_text(NAMES[yuv_format].as_bytes(), TextEncoding::UTF8, None);
    let mut y = y as f32;
    y += (TILE_WIDTH_HEIGHT / 2) as f32 + text_rect.height() / 2.0;
    canvas.draw_simple_text(
        NAMES[yuv_format],
        TextEncoding::UTF8,
        (0.0, y),
        &font,
        &paint,
    );
}

// Port of: gm/wacky_yuv_formats.cpp#L791-L1049 (chrome/m156), `WackyYUVFormatsGM`, for the
// configuration that runs on the CPU: `Type::kFromGenerator`, with no limited range, no target
// colour space, no subset and no cubic sampling.
struct WackyYuvFormatsGm {
    original: [Option<PlaneBuffer>; 2],
    // Indexed as images[opaque][colorSpace][format].
    images: Vec<Option<Image>>,
}

impl WackyYuvFormatsGm {
    fn image_index(opaque: usize, cs: usize, format: usize) -> usize {
        (opaque * YUV_COLOR_SPACES.len() + cs) * NUM_YUV_FORMATS + format
    }

    fn image(&self, opaque: usize, cs: usize, format: usize) -> Option<&Image> {
        self.images[Self::image_index(opaque, cs, format)].as_ref()
    }

    // Port of: gm/wacky_yuv_formats.cpp#L837-L873 (chrome/m156), `createBitmaps`
    fn create_bitmaps(&mut self) {
        let origin = Point::new(
            TILE_WIDTH_HEIGHT as f32 / 2.0,
            TILE_WIDTH_HEIGHT as f32 / 2.0,
        );
        let outer_radius = TILE_WIDTH_HEIGHT as f32 / 2.0 - 20.0;
        let inner_radius = 20.0;
        {
            // transparent
            let mut circles = Vec::new();
            let path = create_splat(
                origin,
                inner_radius,
                outer_radius,
                1.0,
                5,
                Some(&mut circles),
            );
            self.original[0] = make_bitmap(ColorType::RGBA8888, &path, &circles, false, false);
        }
        {
            // opaque
            let mut circles = Vec::new();
            let path = create_splat(
                origin,
                inner_radius,
                outer_radius,
                1.0,
                7,
                Some(&mut circles),
            );
            self.original[1] = make_bitmap(ColorType::RGBA8888, &path, &circles, true, false);
        }
    }

    // Port of: gm/wacky_yuv_formats.cpp#L875-L913 (chrome/m156), `createImages`, for the CPU
    // generator path (`LazyYUVImage::refImage(kFromGenerator)`).
    fn create_images(&mut self) -> bool {
        let mut origin = 0_u32;
        for opaque in [false, true] {
            let Some(original) = self.original[usize::from(opaque)].clone() else {
                return false;
            };
            for (cs_index, &cs) in YUV_COLOR_SPACES.iter().enumerate() {
                if cs.is_limited_range() {
                    // `useLimitedRange` is false: the limited color spaces are skipped.
                    continue;
                }
                let Some(planes) = extract_planes(&original, cs, origin_from_u32(origin + 1))
                else {
                    return false;
                };
                for (f, &format) in ALL_YUV_FORMATS.iter().enumerate() {
                    let result_bms = create_yuv(&planes, format, opaque);
                    let planar_config =
                        YUVAPlanarConfig::new(format, opaque, origin_from_u32(origin + 1));
                    let pixmaps =
                        planar_config.make_yuva_pixmaps(original.dimensions(), cs, &result_bms);
                    let image = pixmaps.and_then(|pixmaps| {
                        images::deferred_from_generator(Some(Box::new(YuvGenerator::new(pixmaps))))
                    });
                    self.images[Self::image_index(usize::from(opaque), cs_index, f)] = image;
                }
                origin = (origin + 1) % 8;
            }
        }
        true
    }
}

impl GM for WackyYuvFormatsGm {
    fn name(&self) -> String {
        "wacky_yuv_formats_imggen".to_string()
    }

    // Port of: gm/wacky_yuv_formats.cpp#L912-L918 (chrome/m156), `getISize`
    fn size(&mut self) -> ISize {
        let num_cols = 2 * (YUV_COLOR_SPACES.len() as i32) / 2; // opacity x #-color-spaces/2
        let num_rows = 1 + NUM_YUV_FORMATS as i32; // original + #-yuv-formats
        let wh = TILE_WIDTH_HEIGHT; // no subset
        ISize::new(
            LABEL_WIDTH + num_cols * (wh + PAD),
            LABEL_HEIGHT + num_rows * (wh + PAD),
        )
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFCC_CCCC_u32)
    }

    // Port of: gm/wacky_yuv_formats.cpp#L940-L959 (chrome/m156), `onGpuSetup`, which creates the
    // images for the generator path on the CPU.
    fn on_gpu_setup(&mut self, _canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        self.create_bitmaps();
        if !self.create_images() {
            return DrawResult::Fail;
        }
        DrawResult::Ok
    }

    // Port of: gm/wacky_yuv_formats.cpp#L1000-L1049 (chrome/m156), `onDraw`
    fn on_draw(&mut self, canvas: &Canvas) {
        let cell_width = TILE_WIDTH_HEIGHT as f32;
        let cell_height = TILE_WIDTH_HEIGHT as f32;
        let Some(original0) = self.original[0].as_ref() else {
            return;
        };
        let src_rect = Rect::from_wh(original0.width() as f32, original0.height() as f32);
        let mut dst_rect =
            Rect::from_xywh(LABEL_WIDTH as f32, 0.0, src_rect.width(), src_rect.height());
        let sampling = SamplingOptions::new(FilterMode::Linear, MipmapMode::None);

        for (cs_index, &cs) in YUV_COLOR_SPACES.iter().enumerate() {
            if cs.is_limited_range() {
                continue;
            }
            let mut paint = Paint::default();
            if cs == YUVColorSpace::Identity {
                // The identity color space needs post processing to appear correctly
                paint.set_color_filter(yuv_to_rgb_colorfilter());
            }
            for opaque in [0_usize, 1] {
                dst_rect.offset_to((dst_rect.left, LABEL_HEIGHT as f32));
                draw_col_label(
                    canvas,
                    (dst_rect.left + cell_width / 2.0) as i32,
                    cs_index,
                    opaque == 1,
                );
                if let Some(image) = self.original[opaque].as_ref().and_then(plane_image) {
                    {
                        canvas.draw_image_rect_with_sampling_options(
                            &image,
                            Some((&src_rect, skia_rust_core::canvas::SrcRectConstraint::Fast)),
                            dst_rect,
                            SamplingOptions::default(),
                            &Paint::default(),
                        );
                    }
                }
                dst_rect.offset((0.0, cell_height + PAD as f32));
                for format in 0..NUM_YUV_FORMATS {
                    draw_row_label(canvas, dst_rect.top as i32, format);
                    if let Some(image) = self.image(opaque, cs_index, format) {
                        canvas.draw_image_rect_with_sampling_options(
                            image,
                            Some((&src_rect, skia_rust_core::canvas::SrcRectConstraint::Fast)),
                            dst_rect,
                            sampling,
                            &paint,
                        );
                    }
                    dst_rect.offset((0.0, cell_height + PAD as f32));
                }
                dst_rect.offset((cell_width + PAD as f32, 0.0));
            }
        }
    }
}

// Exercises SkColorMatrix_RGB2YUV for yuv colorspaces, showing the planes, and the
// resulting (recombined) images (gpu only for now).
//
// On raster the recombined images are null (`SkImages::TextureFromYUVAPixmaps` needs a GPU
// context), so only the planes of the last colour space are drawn.
// Port of: gm/wacky_yuv_formats.cpp#L1338-L1393 (chrome/m156), YUVSplitterGM
struct YuvSplitterGm {
    orig: Option<Image>,
}

impl GM for YuvSplitterGm {
    fn name(&self) -> String {
        "yuv_splitter".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1280, 768)
    }

    // Port of: gm/wacky_yuv_formats.cpp#L1346-L1348 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.orig = get_resource_as_image("images/mandrill_256.png");
    }

    // Port of: gm/wacky_yuv_formats.cpp#L1349-L1378 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let Some(orig) = self.orig.as_ref() else {
            return;
        };
        let ow = orig.width() as f32;
        canvas.translate((ow, 0.0));
        canvas.save();
        let mut last: Option<(Vec<Image>, YUVAInfo)> = None;
        for cs in [
            YUVColorSpace::REC709,
            YUVColorSpace::REC601,
            YUVColorSpace::JPEG,
            YUVColorSpace::BT2020,
        ] {
            // The C++ draws `TextureFromYUVAPixmaps`, which is null without a GPU context, and
            // then draws its difference (`draw_diff`), which is only reached for an image.
            last = make_yuva_planes_as_a8(orig, cs, Subsampling::S444);
            canvas.translate((ow, 0.0));
        }
        canvas.restore();
        canvas.translate((-ow, 0.0));
        if let Some((planes, info)) = last {
            let mut y = 0.0f32;
            for plane in planes.iter().take(info.num_planes()) {
                canvas.draw_image(plane, (0.0, y), None);
                y += plane.height() as f32;
            }
        }
    }
}

// Port of: tools/gpu/YUVUtils.cpp#L156-L205 (chrome/m156), sk_gpu_test::MakeYUVAPlanesAsA8 with no
// recording context: each plane is an A8 image, whose alpha is the plane's row of the RGB to YUV
// matrix applied to the source.
fn make_yuva_planes_as_a8(
    src: &Image,
    cs: YUVColorSpace,
    ss: Subsampling,
) -> Option<(Vec<Image>, YUVAInfo)> {
    let rgb_to_yuv = color_matrix_rgb2yuv(cs);

    let config = if src.is_opaque() {
        PlaneConfig::Y_U_V
    } else {
        PlaneConfig::Y_U_V_A
    };
    let (n, dims) = plane_dimensions_array(src.dimensions(), config, ss, EncodedOrigin::TopLeft);
    let mut planes = Vec::with_capacity(n);
    for (i, dim) in dims.iter().enumerate().take(n) {
        let info = ImageInfo::new(*dim, ColorType::Alpha8, AlphaType::Premul, None);
        let mut surf = surfaces::raster(&info, None, None)?;

        let mut paint = Paint::default();
        paint.set_blend_mode(BlendMode::Src);
        // Make a matrix with the ith row of rgbToYUV copied to the A row since we're drawing to A8.
        let mut m = [0.0f32; 20];
        m[15..20].copy_from_slice(&rgb_to_yuv[5 * i..5 * i + 5]);
        paint.set_color_filter(color_filters::matrix_row_major(&m, Clamp::Yes));
        surf.canvas().draw_image_rect_with_sampling_options(
            src,
            None,
            Rect::from_wh(dim.width as f32, dim.height as f32),
            SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
            &paint,
        );
        planes.push(surf.image_snapshot()?);
    }
    let info = YUVAInfo::new(
        src.dimensions(),
        config,
        ss,
        cs,
        EncodedOrigin::TopLeft,
        (Siting::Centered, Siting::Centered),
    )?;
    Some((planes, info))
}

// Port of: gm/wacky_yuv_formats.cpp#L1394 (chrome/m156), DEF_GM(return new YUVSplitterGM;)
crate::def_gm!(YUVSplitterGM, YuvSplitterGm { orig: None });

// Port of: gm/wacky_yuv_formats.cpp#L1081-L1085 (chrome/m156), the registration with
// `useLimitedRange=false, useTargetColorSpace=false, useSubset=false, useCubicSampling=false` and
// `Type::kFromGenerator`.
crate::def_gm!(
    #[ignore = "see notes/gm_wacky_yuv_formats_cpp_WackyYUVFormatsGM_imggen.md"]
    WackyYUVFormatsGMFromGenerator_ = "WackyYUVFormatsGM(/*useLimitedRange=*/false, /*useTargetColorSpace=*/false, /*useSubset=*/false, /*useCubicSampling=*/false, WackyYUVFormatsGM::Type::kFromGenerator)",
    WackyYuvFormatsGm {
        original: [None, None],
        images: vec![None; 2 * YUV_COLOR_SPACES.len() * NUM_YUV_FORMATS],
    }
);
