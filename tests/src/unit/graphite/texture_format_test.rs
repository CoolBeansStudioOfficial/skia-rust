// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/TextureFormatTest.cpp (chrome/m156)

#![cfg(test)]
// The tables below mirror the C++ expectation tables, and the helpers keep the C++ structure,
// signatures (`const Swizzle&`, `int` bit counts) and numeric casts of the test.
#![allow(
    clippy::too_many_lines,
    clippy::items_after_statements,
    clippy::format_push_string,
    clippy::too_many_arguments,
    clippy::similar_names,
    clippy::trivially_copy_pass_by_ref,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use std::num::NonZeroUsize;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::ColorChannelFlag;
use skia_rust_core::color_space::{ColorSpace, named_gamut, named_transfer_fn};
use skia_rust_core::color_space_xform_steps::ColorSpaceXformSteps;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::half::{float_to_half, half_to_float};
use skia_rust_core::texture_compression_type::TextureCompressionType;
use skia_rust_gpu::gpu::gpu_types::{Mipmapped, Protected, Renderable};
use skia_rust_gpu::gpu::swizzle::Swizzle;
use skia_rust_gpu::graphite::texture_format::{
    TEXTURE_FORMAT_COUNT, TextureFormat, are_color_type_and_format_compatible,
    read_swizzle_for_color_type, texture_format_auto_clamps, texture_format_bytes_per_block,
    texture_format_channel_mask, texture_format_color_type_info, texture_format_compression_type,
    texture_format_has_depth, texture_format_has_stencil, texture_format_is_depth_or_stencil,
    texture_format_is_floating_point, texture_format_is_multiplanar, texture_format_name,
    write_swizzle_for_color_type,
};
use skia_rust_gpu::graphite::texture_format_xfer_fn::TextureFormatXferFn;
use skia_rust_gpu::graphite::texture_info::texture_info_priv;
use skia_rust_gpu::graphite::wgpu::WgpuCaps;
use skia_rust_skcms::{srgb_inverse_transfer_function, srgb_transfer_function};

use crate::tools::tool_utils::{alphatype_name, colortype_name};
use crate::{Failure, Reporter, def_graphite_test_for_all_contexts, reporter_assert};

// Uncomment the SK_ABORT() to exit on first pixel mismatch to help debugging
macro_rules! stop_on_transfer_failure {
    () => {
        // SK_ABORT();
    };
}

// Types for defining color type and format behavior expectations
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ChannelDataType {
    UNorm,
    Signed,
    Float,
    FNorm,
    #[allow(non_camel_case_types)] // keeps Skia's name
    sRGB,
    XR,
    Pad,
}
use ChannelDataType::{FNorm, Float, Pad, Signed, UNorm, XR, sRGB};

#[derive(Clone, Copy, Debug)]
struct Channel {
    name: char, // rgbayuv01 and G (for gray)
    bits: i32,
    ty: ChannelDataType,
}

fn ch(name: char, bits: i32, ty: ChannelDataType) -> Channel {
    Channel { name, bits, ty }
}

struct ColorTypeExpectation {
    color_type: ColorType,
    read_swizzle: Swizzle,
    write_swizzle: Option<Swizzle>, // not set implies not renderable
}

fn color_type_expectation(
    color_type: ColorType,
    read_swizzle: Swizzle,
    write_swizzle: Option<Swizzle>,
) -> ColorTypeExpectation {
    ColorTypeExpectation {
        color_type,
        read_swizzle,
        write_swizzle,
    }
}

struct FormatExpectation {
    format: TextureFormat,
    channels: Vec<Channel>,
    compression_type: TextureCompressionType,

    // Not set implies transfers are disabled; it is composed with a ColorTypeExpectation's
    // read or write swizzle to produce the expected readback/upload swizzle.
    xfer_swizzle: Option<Swizzle>,
    // The first color type expectation is assumed to be the best fit.
    compatible_color_types: Vec<ColorTypeExpectation>,
}

impl FormatExpectation {
    // All of the expectations for the fixed properties of a TextureFormat are derived from its
    // fChannels definition.
    fn is_floating_point(&self) -> bool {
        self.has_type(Float) || self.has_type(FNorm)
    }
    fn has_depth(&self) -> bool {
        self.has_channel('d')
    }
    fn has_stencil(&self) -> bool {
        self.has_channel('s')
    }
    fn has_depth_or_stencil(&self) -> bool {
        self.has_depth() || self.has_stencil()
    }
    fn is_multiplanar(&self) -> bool {
        self.has_channel('y') && self.has_channel('u') && self.has_channel('v')
    }

    fn auto_clamps(&self) -> bool {
        // Auto clamping is derived from the type of the channel with the most bits
        let mut auto_clamp = true;
        let mut max_bit_size = 0;
        for c in &self.channels {
            if c.bits > max_bit_size {
                auto_clamp = c.ty == UNorm || c.ty == sRGB;
                max_bit_size = c.bits;
            }
        }
        auto_clamp
    }

    fn bytes_per_block(&self) -> i32 {
        if self.compression_type != TextureCompressionType::None {
            // At the moment, all supported compression types have the same bytes per block
            return 8;
        }

        let bit_count: i32 = self.channels.iter().map(|c| c.bits).sum();
        bit_count / 8
    }

    fn channel_mask(&self) -> u32 {
        let mut mask = 0;
        if self.has_channel('r') || self.has_channel('y') {
            mask |= ColorChannelFlag::RED.bits();
        }
        if self.has_channel('g') || self.has_channel('u') {
            mask |= ColorChannelFlag::GREEN.bits();
        }
        if self.has_channel('b') || self.has_channel('v') {
            mask |= ColorChannelFlag::BLUE.bits();
        }
        if self.has_channel('a') {
            mask |= ColorChannelFlag::ALPHA.bits();
        }
        // Other channels do not contribute to SkColorChannel mask
        mask
    }

    fn has_channel(&self, channel: char) -> bool {
        self.channels.iter().any(|c| c.name == channel)
    }
    fn has_type(&self, ty: ChannelDataType) -> bool {
        self.channels.iter().any(|c| c.ty == ty)
    }
}

type PixelData = [u8; 16]; // The largest texel/pixel size is RGBA32F = 16 bytes

fn channel_denominator(bits: i32) -> u32 {
    if bits == 32 {
        u32::MAX
    } else {
        (1u32 << bits) - 1
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors the C++ casts
fn channel_to_bits(channel: &Channel, mut value: f32) -> u32 {
    let mut ty = channel.ty;
    if ty == Pad {
        // Pad should only be used with 'x', which produces NaN in channel_to_float, so
        // replace `value` with the default 'x' bit pattern in gen_channel_values, and then
        // fall through to UNorm handling to adjust it to the right bit depth
        assert_eq!(channel.name, 'x');
        value = f32::from(0b0101u8) / 15.0;
        ty = UNorm;
    }

    match ty {
        sRGB | UNorm => {
            // sRGB data is stored in a non-linear gamma and automatically decodes to linear when
            // being sampled or rendered into. This means an SRGB_8888 image with a linear
            // SkColorSpace behaves like a regular 8888 image with an sRGB SkColorSpace. But
            // transfers between CPU and GPU assume the SkColorSpace is the same, so we need to
            // interpret the `v` as if it were an 8888 image with a linear SkColorSpace and map to
            // the sRGB encoding.
            if ty == sRGB {
                value = srgb_inverse_transfer_function().eval(value);
            }
            let denominator = channel_denominator(channel.bits);
            (value * denominator as f32).round() as u32
        }

        XR => {
            // See SkRP_opts::store_1010102_xr
            assert_eq!(channel.bits, 10);
            ((value * 510.0 + 384.0).round() as u32).min(1023)
        }

        // For simplicity fall through to Float, the Norm is just a hint about range
        FNorm | Float => {
            assert!(channel.bits == 16 || channel.bits == 32);
            if channel.bits == 16 {
                u32::from(float_to_half(value))
            } else {
                value.to_bits()
            }
        }

        Signed => {
            // This should only be used for 's' channels, which are already not reaching this code
            panic!("Should not be generating Signed values for pixel data");
        }

        Pad => unreachable!("handled above"),
    }
}

#[allow(clippy::cast_possible_truncation)] // mirrors the C++ casts
fn channel_to_float(channel: &Channel, bits: u32) -> f32 {
    match channel.ty {
        // first treat as unorm then apply gamma TF
        sRGB | UNorm => {
            let denominator = channel_denominator(channel.bits);
            let mut vf = bits as f32 * (1.0 / denominator as f32);
            if channel.ty == sRGB {
                vf = srgb_transfer_function().eval(vf);
            }
            vf
        }

        XR => (bits as f32 - 384.0) * (1.0 / 510.0),

        // Values are interpreted the same, values are in [0,1]
        FNorm | Float => {
            if channel.bits == 16 {
                half_to_float(bits as u16)
            } else {
                f32::from_bits(bits)
            }
        }

        Signed | Pad => f32::NAN, // No floating point printing
    }
}

fn swizzle_char(swizzle: &Swizzle, i: usize) -> char {
    swizzle
        .as_string()
        .chars()
        .nth(i)
        .expect("a swizzle has four channels")
}

fn apply_swizzle_to_name(dst_name: char, src_to_dst: &Swizzle) -> char {
    match dst_name {
        'r' => swizzle_char(src_to_dst, 0),
        'g' => swizzle_char(src_to_dst, 1),
        'b' => swizzle_char(src_to_dst, 2),
        'a' => swizzle_char(src_to_dst, 3),
        _ => dst_name,
    }
}

// Generate a unique value for `channelName` by applying `srcToDst` to the source channels.
fn gen_channel_value(name: char, load_src_swizzle: &Swizzle, src_channels: &[Channel]) -> f32 {
    let name = apply_swizzle_to_name(name, load_src_swizzle);

    // Try to look for an exact channel match, in which case we use the unique value corresponding
    // to that channel name.
    for src_channel in src_channels {
        if src_channel.name == name
            || (src_channel.name == 'G' && (name == 'r' || name == 'g' || name == 'b'))
        {
            let v: u32 = match src_channel.name {
                'r' => 0b0001,
                'g' => 0b0010,
                'b' => 0b0100,
                'a' => 0b1010, // `A` must additionally fit into 2 bit channel formats
                'x' => 0b0101,
                '0' => 0b0000,
                '1' => 0b1111,
                'G' => 0b1100, // arbitrary starting Gray value
                _ => {
                    // NOTE: 'd', 's', 'y', 'u', 'v' are valid channel names, but the formats that
                    // have those should not be participating in the read/write transfer testing.
                    panic!(
                        "Bad source channel name for pixel data generation: {}",
                        src_channel.name
                    );
                }
            };
            // Route through the source channel's bit representation to account for rounding
            #[allow(clippy::cast_precision_loss)] // v is at most 15
            return channel_to_float(src_channel, channel_to_bits(src_channel, v as f32 / 15.0));
        }
    }

    // We didn't find an exact channel match, so we either use default values or apply gray
    // handling
    match name {
        // Trivial default values
        'x' => f32::NAN,
        'r' | 'g' | 'b' | '0' => 0.0,
        'a' | '1' => 1.0,

        // Construct a gray value from r, g, and b values of the source
        'G' => {
            let r = gen_channel_value('r', load_src_swizzle, src_channels);
            let g = gen_channel_value('g', load_src_swizzle, src_channels);
            let b = gen_channel_value('b', load_src_swizzle, src_channels);

            // Since this is a linear sum, if we divided or multiplied by A after converting
            // through source's precision, we would arrive at the same value as if we applied
            // the premul/unpremul multiplication in gen_pixel_data applied to G
            0.2126 * r + 0.7142 * g + 0.0722 * b
        }

        _ => panic!("Bad dst channel name for pixel data generation: {name}"),
    }
}

// Copies `num_bits` bits of the channel value `src` into `dst` at `bit_offset`. `dst` should be
// zero-initialized before calling. Returns the bitOffset for the next channel.
// Port of: tests/graphite/TextureFormatTest.cpp#L211-L237 (chrome/m156) (`Src` = uint32_t)
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ uint8_t stores
fn copy_bits_to_pixel(num_bits: i32, mut bit_offset: i32, src: u32, dst: &mut PixelData) -> i32 {
    let mut src_bits_left = num_bits;
    while src_bits_left > 0 {
        let array_index = (bit_offset / 8) as usize;
        let array_bits_left = (array_index as i32 + 1) * 8 - bit_offset;
        let copy_bits = array_bits_left.min(src_bits_left);

        let channel_bit_shift = num_bits - src_bits_left;
        let array_bit_shift = 8 - array_bits_left;

        dst[array_index] |=
            (((src >> channel_bit_shift) & ((1 << copy_bits) - 1)) << array_bit_shift) as u8;
        src_bits_left -= copy_bits;
        bit_offset += copy_bits;
    }
    bit_offset
}

// Port of: tests/graphite/TextureFormatTest.cpp#L211-L237 (chrome/m156) (`Src` = PixelData)
fn copy_bits_from_pixel(num_bits: i32, mut bit_offset: i32, src: &PixelData, dst: &mut u32) -> i32 {
    let mut src_bits_left = num_bits;
    while src_bits_left > 0 {
        let array_index = (bit_offset / 8) as usize;
        let array_bits_left = (array_index as i32 + 1) * 8 - bit_offset;
        let copy_bits = array_bits_left.min(src_bits_left);

        let channel_bit_shift = num_bits - src_bits_left;
        let array_bit_shift = 8 - array_bits_left;

        *dst |= ((u32::from(src[array_index]) >> array_bit_shift) & ((1 << copy_bits) - 1))
            << channel_bit_shift;
        src_bits_left -= copy_bits;
        bit_offset += copy_bits;
    }
    bit_offset
}

fn needs_premul(src_alpha_type: AlphaType, dst_alpha_type: AlphaType) -> bool {
    dst_alpha_type == AlphaType::Premul
        && (src_alpha_type == AlphaType::Unpremul || src_alpha_type == AlphaType::Unknown)
}
fn needs_unpremul(src_alpha_type: AlphaType, dst_alpha_type: AlphaType) -> bool {
    src_alpha_type == AlphaType::Premul
        && (dst_alpha_type == AlphaType::Unpremul || dst_alpha_type == AlphaType::Unknown)
}

// Returns the number of set *bits* within PixelData.
fn gen_pixel_data_full(
    dst_channels: &[Channel],
    store_dst_swizzle: &Swizzle,
    dst_alpha_type: AlphaType,
    src_channels: &[Channel],
    load_src_swizzle: &Swizzle,
    src_alpha_type: AlphaType,
    for_input: bool,
) -> (PixelData, i32) {
    let mut pixel: PixelData = [0; 16]; // zero-initialize all bytes
    let mut bit_offset = 0;
    for dst_channel in dst_channels {
        let mut channel = *dst_channel;
        channel.name = apply_swizzle_to_name(dst_channel.name, store_dst_swizzle);
        let mut src_value = gen_channel_value(channel.name, load_src_swizzle, src_channels);

        // Handle alpha type conversion
        if for_input
            && ((channel.name == 'a' && src_alpha_type == AlphaType::Unknown)
                || (channel.name == '1' && dst_channel.name == 'a'))
        {
            // Fill junk alpha that is masked by a later read swizzle as if it were padding
            channel.ty = Pad;
            channel.name = 'x';
        } else if channel.name == 'a' && src_alpha_type == AlphaType::Opaque {
            src_value = 1.0;
            // NOTE: We don't force alpha = 1 when dstAlphaType = opaque, since that is
            // interpreted as a trust-the-user scenario and we assume input values are already
            // opaque and don't modify the alpha.
        } else if channel.name == 'r'
            || channel.name == 'g'
            || channel.name == 'b'
            || channel.name == 'G'
        {
            if needs_premul(src_alpha_type, dst_alpha_type) {
                // Must multiply other channels by the alpha value
                let src_alpha = gen_channel_value('a', load_src_swizzle, src_channels);
                src_value *= src_alpha;
            } else if needs_unpremul(src_alpha_type, dst_alpha_type) {
                // Must divide the other channels by the alpha value
                let src_alpha = gen_channel_value('a', load_src_swizzle, src_channels);
                src_value *= 1.0 / src_alpha;
            }
            // else we're the same alpha type on both sides, or a mix of opaque and unknown, in
            // which case premul vs. unpremul is a no-op.
        }

        let channel_value = channel_to_bits(&channel, src_value);
        bit_offset = copy_bits_to_pixel(channel.bits, bit_offset, channel_value, &mut pixel);
    }

    (pixel, bit_offset)
}

// No swizzling or data conversion variant for input data generation
fn gen_pixel_data(
    channels: &[Channel],
    read_swizzle: &Swizzle,
    alpha_type: AlphaType,
) -> (PixelData, i32) {
    gen_pixel_data_full(
        channels,
        &read_swizzle.invert(),
        alpha_type,
        channels,
        read_swizzle,
        alpha_type,
        /* forInput= */ true,
    )
}

fn channel_type_name(ty: ChannelDataType) -> &'static str {
    match ty {
        UNorm => "u",
        Signed => "s",
        Float => "f",
        FNorm => "fn",
        sRGB => "srgb",
        XR => "xr",
        Pad => "_",
    }
}

fn print_channel_names(channels: &[Channel]) -> Vec<String> {
    let mut names = vec![
        String::new(), // empty to align with channel values row label
        "raw".to_owned(),
    ];
    for c in channels {
        let type_name = channel_type_name(c.ty);
        names.push(format!("{}({}{})", c.name, type_name, c.bits));
    }
    names
}

fn print_channel_values(prefix: &str, channels: &[Channel], pixel: &PixelData) -> Vec<String> {
    let mut values = vec![prefix.to_owned()];

    values.push(String::new()); // Raw hex value
    for byte in pixel {
        values[1].push_str(&format!(" {byte:0>2x}"));
    }

    let mut bit_offset = 0;
    for c in channels {
        let mut channel_value = 0u32;
        bit_offset = copy_bits_from_pixel(c.bits, bit_offset, pixel, &mut channel_value);

        let mut binary = String::new();
        for i in 0..c.bits {
            binary.insert(
                0,
                if (channel_value >> i) & 1 != 0 {
                    '1'
                } else {
                    '0'
                },
            );
        }
        let vf = channel_to_float(c, channel_value);

        let numeric = match c.ty {
            Pad => "-".to_owned(),
            Signed => format!("{channel_value}"),

            sRGB | XR | UNorm => format!("0x{channel_value:x} / {vf:.2}"),

            FNorm | Float => format!("{vf:.4}"),
        };
        values.push(format!("{binary} ({numeric})"));
    }

    values
}

fn dump_string(s: &str, length: usize) {
    let str_len = s.len();
    assert!(str_len <= length);
    eprint!("{s:>width$}", width = length.min(str_len));
    eprint!("{}", " ".repeat(length - str_len));
}

#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn dump_pixel_comparison(
    input_name: &str,
    input_channels: &[Channel],
    input_pixel: &PixelData,
    output_name: &str,
    output_channels: &[Channel],
    expected_output_pixel: &PixelData,
    actual_output_pixel: &PixelData,
) {
    eprintln!("Transfer from {input_name} to {output_name}:");

    let input_channel_names = print_channel_names(input_channels);
    let input_channel_values = print_channel_values("Input", input_channels, input_pixel);

    let output_channel_names = print_channel_names(output_channels);
    let expected_channel_values =
        print_channel_values("Expected", output_channels, expected_output_pixel);
    let actual_channel_values =
        print_channel_values("Actual", output_channels, actual_output_pixel);

    let col_count = input_channel_names.len().max(output_channel_names.len());
    let rows = [
        input_channel_names,
        input_channel_values,
        output_channel_names,
        expected_channel_values,
        actual_channel_values,
    ];
    for row in &rows {
        for c in 0..col_count.min(row.len()) {
            if c == 0 {
                eprint!(" ");
            } else {
                eprint!(" | ");
            }
            let mut col_width = 0;
            for other_row in &rows {
                if c < other_row.len() {
                    col_width = col_width.max(other_row[c].len());
                }
            }
            dump_string(&row[c], col_width);
        }
        eprintln!();
    }
}

fn channel_tolerance(
    src_channels: &[Channel],
    src_at: AlphaType,
    dst_channels: &[Channel],
    dst_at: AlphaType,
) -> i32 {
    let mut src_has_srgb_or_gray = false;
    for c in src_channels {
        src_has_srgb_or_gray |= c.ty == sRGB || c.name == 'G';
    }
    let mut dst_has_srgb_or_gray = false;
    for c in dst_channels {
        dst_has_srgb_or_gray |= c.ty == sRGB || c.name == 'G';
    }
    // If the input or output data involves sRGB encoding/decoding or conversion to gray, or are
    // doing premul/unpremul math, we allow 1 bit of difference because of mismatches between how
    // SkRasterPipeline calculates channel values vs. the value generation in these tests.
    i32::from(
        src_has_srgb_or_gray
            || dst_has_srgb_or_gray
            || needs_premul(src_at, dst_at)
            || needs_unpremul(src_at, dst_at),
    )
}

fn compare_pixels(
    channels: &[Channel],
    expected: &PixelData,
    actual: &PixelData,
    channel_tolerance: i32,
    ignore_channel: Option<char>,
) -> bool {
    let mut bit_offset = 0;
    for c in channels {
        let mut actual_channel_bits = 0u32;
        let mut expected_channel_bits = 0u32;
        copy_bits_from_pixel(c.bits, bit_offset, expected, &mut expected_channel_bits);
        copy_bits_from_pixel(c.bits, bit_offset, actual, &mut actual_channel_bits);
        bit_offset += c.bits;

        let channel_diff = i64::from(actual_channel_bits) - i64::from(expected_channel_bits);
        if c.ty != Pad // ignore padding channels
            && !(ignore_channel.is_some() && Some(c.name) == ignore_channel) // ignore alpha channels
            && channel_diff.abs() > i64::from(channel_tolerance)
        {
            return false;
        }
    }
    true
}

fn transfer_data(
    xfer_fn: &TextureFormatXferFn,
    input_data: &PixelData,
    src_pixel_bits: i32,
    dst_pixel_bits: i32,
) -> PixelData {
    // First do a conversion on a 1x1 image, which will represent the expected output when
    // testing various width/height/rowbyte combinations for a solid input.
    let mut output_data: PixelData = [0; 16]; // zero-initialize for comparison stability on unwritten values.
    xfer_fn.run(1, 1, input_data, 16, &mut output_data, 16);

    assert_eq!(src_pixel_bits % 8, 0);
    assert_eq!(dst_pixel_bits % 8, 0);
    let src_pixel_bytes = (src_pixel_bits / 8) as usize;
    let dst_pixel_bytes = (dst_pixel_bits / 8) as usize;

    const MAX_DIM: usize = 32; // > 128-bit SIMD processing for 8-bit formats
    let mut src = vec![0u8; MAX_DIM * MAX_DIM * 16];
    let mut dst = vec![0u8; MAX_DIM * MAX_DIM * 16];

    let fill_image = |src: &mut [u8], w: usize, h: usize, row_bytes: usize| {
        let mut data_start = 0;
        for x in 0..w {
            src[x * src_pixel_bytes..(x + 1) * src_pixel_bytes]
                .copy_from_slice(&input_data[..src_pixel_bytes]);
        }
        for _y in 1..h {
            src.copy_within(data_start..data_start + row_bytes, data_start + row_bytes);
            data_start += row_bytes;
        }
    };
    let check_image =
        |dst: &[u8], output_data: &mut PixelData, w: usize, h: usize, row_bytes: usize| {
            let mut data_start = 0;
            let mut px_data: PixelData = [0; 16]; // zero-initialize all bytes
            for y in 0..h {
                for x in 0..w {
                    px_data[..dst_pixel_bytes].copy_from_slice(
                        &dst[data_start + x * dst_pixel_bytes
                            ..data_start + (x + 1) * dst_pixel_bytes],
                    );
                    if px_data != *output_data {
                        eprintln!("Pixel mismatch at ({x}, {y}) in bulk transfer.");
                        *output_data = px_data; // propagate this error out for pixel dumping
                        return false;
                    }
                }
                data_start += row_bytes;
            }
            true
        };

    // RasterPipeline isn't the fastest in debug builds and transfer_data is called ~70k times
    // in this unit test, so only test a specific width and height.
    const WIDTH: usize = 17; // > 16 to test padding logic for 128-bit SIMD processing
    const HEIGHT: usize = 7; // > 1 for multirow testing, but low to help performance

    for input_dense in [true, false] {
        let src_row_bytes = (if input_dense { WIDTH } else { MAX_DIM }) * src_pixel_bytes;
        fill_image(&mut src, WIDTH, HEIGHT, src_row_bytes);
        for output_dense in [true, false] {
            let dst_row_bytes = (if output_dense { WIDTH } else { MAX_DIM }) * dst_pixel_bytes;
            xfer_fn.run(WIDTH, HEIGHT, &src, src_row_bytes, &mut dst, dst_row_bytes);
            if !check_image(&dst, &mut output_data, WIDTH, HEIGHT, dst_row_bytes) {
                return output_data;
            }
        }
    }
    output_data
}

// Define the channel layout for every SkColorType for use in generating and validating the
// result of transferring data to or from a texture format.
struct ColorTypeChannels {
    color_type: ColorType,
    effective_swizzle: Swizzle, // Derivable from channel mask
    channels: Vec<Channel>,
}

static COLOR_TYPE_CHANNELS: LazyLock<Vec<ColorTypeChannels>> = LazyLock::new(|| {
    vec![
        ColorTypeChannels {
            color_type: ColorType::Alpha8,
            effective_swizzle: Swizzle::new("000a"),
            channels: vec![ch('a', 8, UNorm)],
        },
        ColorTypeChannels {
            color_type: ColorType::RGB565,
            effective_swizzle: Swizzle::new("rgb1"),
            channels: vec![ch('b', 5, UNorm), ch('g', 6, UNorm), ch('r', 5, UNorm)],
        },
        ColorTypeChannels {
            color_type: ColorType::ARGB4444,
            effective_swizzle: Swizzle::new("rgba"),
            channels: vec![
                ch('a', 4, UNorm),
                ch('b', 4, UNorm),
                ch('g', 4, UNorm),
                ch('r', 4, UNorm),
            ],
        },
        ColorTypeChannels {
            color_type: ColorType::RGBA8888,
            effective_swizzle: Swizzle::new("rgba"),
            channels: vec![
                ch('r', 8, UNorm),
                ch('g', 8, UNorm),
                ch('b', 8, UNorm),
                ch('a', 8, UNorm),
            ],
        },
        ColorTypeChannels {
            color_type: ColorType::RGB888x,
            effective_swizzle: Swizzle::new("rgb1"),
            channels: vec![
                ch('r', 8, UNorm),
                ch('g', 8, UNorm),
                ch('b', 8, UNorm),
                ch('x', 8, Pad),
            ],
        },
        ColorTypeChannels {
            color_type: ColorType::BGRA8888,
            effective_swizzle: Swizzle::new("rgba"),
            channels: vec![
                ch('b', 8, UNorm),
                ch('g', 8, UNorm),
                ch('r', 8, UNorm),
                ch('a', 8, UNorm),
            ],
        },
        ColorTypeChannels {
            color_type: ColorType::RGBA1010102,
            effective_swizzle: Swizzle::new("rgba"),
            channels: vec![
                ch('r', 10, UNorm),
                ch('g', 10, UNorm),
                ch('b', 10, UNorm),
                ch('a', 2, UNorm),
            ],
        },
        ColorTypeChannels {
            color_type: ColorType::BGRA1010102,
            effective_swizzle: Swizzle::new("rgba"),
            channels: vec![
                ch('b', 10, UNorm),
                ch('g', 10, UNorm),
                ch('r', 10, UNorm),
                ch('a', 2, UNorm),
            ],
        },
        ColorTypeChannels {
            color_type: ColorType::RGB101010x,
            effective_swizzle: Swizzle::new("rgb1"),
            channels: vec![
                ch('r', 10, UNorm),
                ch('g', 10, UNorm),
                ch('b', 10, UNorm),
                ch('x', 2, Pad),
            ],
        },
        ColorTypeChannels {
            color_type: ColorType::BGR101010x,
            effective_swizzle: Swizzle::new("rgb1"),
            channels: vec![
                ch('b', 10, UNorm),
                ch('g', 10, UNorm),
                ch('r', 10, UNorm),
                ch('x', 2, Pad),
            ],
        },
        ColorTypeChannels {
            color_type: ColorType::BGR101010xXR,
            effective_swizzle: Swizzle::new("rgb1"),
            channels: vec![
                ch('b', 10, XR),
                ch('g', 10, XR),
                ch('r', 10, XR),
                ch('x', 2, Pad),
            ],
        },
        ColorTypeChannels {
            color_type: ColorType::BGRA10101010XR,
            effective_swizzle: Swizzle::new("rgba"),
            channels: vec![
                ch('x', 6, Pad),
                ch('b', 10, XR),
                ch('x', 6, Pad),
                ch('g', 10, XR),
                ch('x', 6, Pad),
                ch('r', 10, XR),
                ch('x', 6, Pad),
                ch('a', 10, XR),
            ],
        },
        ColorTypeChannels {
            color_type: ColorType::RGBA10x6,
            effective_swizzle: Swizzle::new("rgba"),
            channels: vec![
                ch('x', 6, Pad),
                ch('r', 10, UNorm),
                ch('x', 6, Pad),
                ch('g', 10, UNorm),
                ch('x', 6, Pad),
                ch('b', 10, UNorm),
                ch('x', 6, Pad),
                ch('a', 10, UNorm),
            ],
        },
        ColorTypeChannels {
            color_type: ColorType::Gray8,
            effective_swizzle: Swizzle::new("rrr1"),
            channels: vec![ch('G', 8, UNorm)],
        },
        ColorTypeChannels {
            color_type: ColorType::RGBAF16Norm,
            effective_swizzle: Swizzle::new("rgba"),
            channels: vec![
                ch('r', 16, FNorm),
                ch('g', 16, FNorm),
                ch('b', 16, FNorm),
                ch('a', 16, FNorm),
            ],
        },
        ColorTypeChannels {
            color_type: ColorType::RGBAF16,
            effective_swizzle: Swizzle::new("rgba"),
            channels: vec![
                ch('r', 16, Float),
                ch('g', 16, Float),
                ch('b', 16, Float),
                ch('a', 16, Float),
            ],
        },
        ColorTypeChannels {
            color_type: ColorType::RGBF16F16F16x,
            effective_swizzle: Swizzle::new("rgb1"),
            channels: vec![
                ch('r', 16, Float),
                ch('g', 16, Float),
                ch('b', 16, Float),
                ch('x', 16, Pad),
            ],
        },
        ColorTypeChannels {
            color_type: ColorType::RGBAF32,
            effective_swizzle: Swizzle::new("rgba"),
            channels: vec![
                ch('r', 32, Float),
                ch('g', 32, Float),
                ch('b', 32, Float),
                ch('a', 32, Float),
            ],
        },
        ColorTypeChannels {
            color_type: ColorType::R8G8UNorm,
            effective_swizzle: Swizzle::new("rg01"),
            channels: vec![ch('r', 8, UNorm), ch('g', 8, UNorm)],
        },
        ColorTypeChannels {
            color_type: ColorType::A16Float,
            effective_swizzle: Swizzle::new("000a"),
            channels: vec![ch('a', 16, Float)],
        },
        ColorTypeChannels {
            color_type: ColorType::R16Float,
            effective_swizzle: Swizzle::new("r001"),
            channels: vec![ch('r', 16, Float)],
        },
        ColorTypeChannels {
            color_type: ColorType::R16G16Float,
            effective_swizzle: Swizzle::new("rg01"),
            channels: vec![ch('r', 16, Float), ch('g', 16, Float)],
        },
        ColorTypeChannels {
            color_type: ColorType::A16UNorm,
            effective_swizzle: Swizzle::new("000a"),
            channels: vec![ch('a', 16, UNorm)],
        },
        ColorTypeChannels {
            color_type: ColorType::R16UNorm,
            effective_swizzle: Swizzle::new("r001"),
            channels: vec![ch('r', 16, UNorm)],
        },
        ColorTypeChannels {
            color_type: ColorType::R16G16UNorm,
            effective_swizzle: Swizzle::new("rg01"),
            channels: vec![ch('r', 16, UNorm), ch('g', 16, UNorm)],
        },
        ColorTypeChannels {
            color_type: ColorType::R16G16B16A16UNorm,
            effective_swizzle: Swizzle::new("rgba"),
            channels: vec![
                ch('r', 16, UNorm),
                ch('g', 16, UNorm),
                ch('b', 16, UNorm),
                ch('a', 16, UNorm),
            ],
        },
        ColorTypeChannels {
            color_type: ColorType::SRGBA8888,
            effective_swizzle: Swizzle::new("rgba"),
            channels: vec![
                ch('r', 8, sRGB),
                ch('g', 8, sRGB),
                ch('b', 8, sRGB),
                ch('a', 8, UNorm),
            ],
        },
        ColorTypeChannels {
            color_type: ColorType::R8UNorm,
            effective_swizzle: Swizzle::new("r001"),
            channels: vec![ch('r', 8, UNorm)],
        },
    ]
});

static EXPECTATIONS: LazyLock<Vec<FormatExpectation>> = LazyLock::new(|| {
    vec![
        FormatExpectation {
            format: TextureFormat::Unsupported,
            channels: vec![],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![],
        },
        FormatExpectation {
            format: TextureFormat::R8,
            channels: vec![ch('r', 8, UNorm)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("r001")),
            compatible_color_types: vec![
                color_type_expectation(ColorType::R8UNorm, Swizzle::rgba(), Some(Swizzle::rgba())),
                color_type_expectation(
                    ColorType::Alpha8,
                    Swizzle::new("000r"),
                    Some(Swizzle::new("a000")),
                ),
                color_type_expectation(ColorType::Gray8, Swizzle::new("rrra"), None),
            ],
        },
        FormatExpectation {
            format: TextureFormat::R16,
            channels: vec![ch('r', 16, UNorm)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("r001")),
            compatible_color_types: vec![
                color_type_expectation(ColorType::R16UNorm, Swizzle::rgba(), Some(Swizzle::rgba())),
                color_type_expectation(
                    ColorType::A16UNorm,
                    Swizzle::new("000r"),
                    Some(Swizzle::new("a000")),
                ),
            ],
        },
        FormatExpectation {
            format: TextureFormat::R16F,
            channels: vec![ch('r', 16, Float)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("r001")),
            compatible_color_types: vec![
                color_type_expectation(ColorType::R16Float, Swizzle::rgba(), Some(Swizzle::rgba())),
                color_type_expectation(
                    ColorType::A16Float,
                    Swizzle::new("000r"),
                    Some(Swizzle::new("a000")),
                ),
            ],
        },
        FormatExpectation {
            format: TextureFormat::R32F,
            channels: vec![ch('r', 32, Float)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![color_type_expectation(
                ColorType::R16Float,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::A8,
            channels: vec![ch('a', 8, UNorm)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("000a")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::Alpha8,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::RG8,
            channels: vec![ch('r', 8, UNorm), ch('g', 8, UNorm)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rg01")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::R8G8UNorm,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::RG16,
            channels: vec![ch('r', 16, UNorm), ch('g', 16, UNorm)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rg01")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::R16G16UNorm,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::RG16F,
            channels: vec![ch('r', 16, Float), ch('g', 16, Float)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rg01")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::R16G16Float,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::RG32F,
            channels: vec![ch('r', 32, Float), ch('g', 32, Float)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![color_type_expectation(
                ColorType::R16G16Float,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::RGB8,
            channels: vec![ch('r', 8, UNorm), ch('g', 8, UNorm), ch('b', 8, UNorm)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgb1")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGB888x,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::BGR8,
            channels: vec![ch('b', 8, UNorm), ch('g', 8, UNorm), ch('r', 8, UNorm)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgb1")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGB888x,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::B5_G6_R5,
            channels: vec![ch('b', 5, UNorm), ch('g', 6, UNorm), ch('r', 5, UNorm)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgb1")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGB565,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::R5_G6_B5,
            channels: vec![ch('r', 5, UNorm), ch('g', 6, UNorm), ch('b', 5, UNorm)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgb1")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGB565,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::RGB16,
            channels: vec![ch('r', 16, UNorm), ch('g', 16, UNorm), ch('b', 16, UNorm)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgb1")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::R16G16B16A16UNorm,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::RGB16F,
            channels: vec![ch('r', 16, Float), ch('g', 16, Float), ch('b', 16, Float)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgb1")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGBF16F16F16x,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::RGB32F,
            channels: vec![ch('r', 32, Float), ch('g', 32, Float), ch('b', 32, Float)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgb1")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGBAF32,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::RGB8_sRGB,
            channels: vec![ch('r', 8, sRGB), ch('g', 8, sRGB), ch('b', 8, sRGB)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgb1")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::SRGBA8888,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::BGR10_XR,
            channels: vec![
                ch('b', 10, XR),
                ch('g', 10, XR),
                ch('r', 10, XR),
                ch('x', 2, Pad),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgb1")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::BGR101010xXR,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::RGBA8,
            channels: vec![
                ch('r', 8, UNorm),
                ch('g', 8, UNorm),
                ch('b', 8, UNorm),
                ch('a', 8, UNorm),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgba")),
            compatible_color_types: vec![
                color_type_expectation(ColorType::RGBA8888, Swizzle::rgba(), Some(Swizzle::rgba())),
                color_type_expectation(ColorType::BGRA8888, Swizzle::rgba(), Some(Swizzle::rgba())),
                color_type_expectation(ColorType::RGB888x, Swizzle::rgb1(), None),
            ],
        },
        FormatExpectation {
            format: TextureFormat::RGBA16,
            channels: vec![
                ch('r', 16, UNorm),
                ch('g', 16, UNorm),
                ch('b', 16, UNorm),
                ch('a', 16, UNorm),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgba")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::R16G16B16A16UNorm,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::RGBA16F,
            channels: vec![
                ch('r', 16, Float),
                ch('g', 16, Float),
                ch('b', 16, Float),
                ch('a', 16, Float),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgba")),
            compatible_color_types: vec![
                color_type_expectation(ColorType::RGBAF16, Swizzle::rgba(), Some(Swizzle::rgba())),
                color_type_expectation(
                    ColorType::RGBAF16Norm,
                    Swizzle::rgba(),
                    Some(Swizzle::rgba()),
                ),
                color_type_expectation(ColorType::RGBF16F16F16x, Swizzle::rgb1(), None),
            ],
        },
        FormatExpectation {
            format: TextureFormat::RGBA32F,
            channels: vec![
                ch('r', 32, Float),
                ch('g', 32, Float),
                ch('b', 32, Float),
                ch('a', 32, Float),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgba")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGBAF32,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::RGB10_A2,
            channels: vec![
                ch('r', 10, UNorm),
                ch('g', 10, UNorm),
                ch('b', 10, UNorm),
                ch('a', 2, UNorm),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgba")),
            compatible_color_types: vec![
                color_type_expectation(
                    ColorType::RGBA1010102,
                    Swizzle::rgba(),
                    Some(Swizzle::rgba()),
                ),
                color_type_expectation(ColorType::RGB101010x, Swizzle::rgb1(), None),
                color_type_expectation(
                    ColorType::BGRA1010102,
                    Swizzle::rgba(),
                    Some(Swizzle::rgba()),
                ),
                color_type_expectation(ColorType::BGR101010x, Swizzle::rgb1(), None),
            ],
        },
        FormatExpectation {
            format: TextureFormat::RGBA10x6,
            channels: vec![
                ch('x', 6, Pad),
                ch('r', 10, UNorm),
                ch('x', 6, Pad),
                ch('g', 10, UNorm),
                ch('x', 6, Pad),
                ch('b', 10, UNorm),
                ch('x', 6, Pad),
                ch('a', 10, UNorm),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgba")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGBA10x6,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::RGBA8_sRGB,
            channels: vec![
                ch('r', 8, sRGB),
                ch('g', 8, sRGB),
                ch('b', 8, sRGB),
                ch('a', 8, UNorm),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgba")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::SRGBA8888,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::BGRA8,
            channels: vec![
                ch('b', 8, UNorm),
                ch('g', 8, UNorm),
                ch('r', 8, UNorm),
                ch('a', 8, UNorm),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgba")),
            compatible_color_types: vec![
                color_type_expectation(ColorType::BGRA8888, Swizzle::rgba(), Some(Swizzle::rgba())),
                color_type_expectation(ColorType::RGBA8888, Swizzle::rgba(), Some(Swizzle::rgba())),
                color_type_expectation(ColorType::RGB888x, Swizzle::rgb1(), None),
            ],
        },
        FormatExpectation {
            format: TextureFormat::BGR10_A2,
            channels: vec![
                ch('b', 10, UNorm),
                ch('g', 10, UNorm),
                ch('r', 10, UNorm),
                ch('a', 2, UNorm),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgba")),
            compatible_color_types: vec![
                color_type_expectation(
                    ColorType::BGRA1010102,
                    Swizzle::rgba(),
                    Some(Swizzle::rgba()),
                ),
                color_type_expectation(ColorType::BGR101010x, Swizzle::rgb1(), None),
                color_type_expectation(
                    ColorType::RGBA1010102,
                    Swizzle::rgba(),
                    Some(Swizzle::rgba()),
                ),
                color_type_expectation(ColorType::RGB101010x, Swizzle::rgb1(), None),
            ],
        },
        FormatExpectation {
            format: TextureFormat::BGRA8_sRGB,
            channels: vec![
                ch('b', 8, sRGB),
                ch('g', 8, sRGB),
                ch('r', 8, sRGB),
                ch('a', 8, UNorm),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgba")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::SRGBA8888,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::ABGR4,
            channels: vec![
                ch('a', 4, UNorm),
                ch('b', 4, UNorm),
                ch('g', 4, UNorm),
                ch('r', 4, UNorm),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgba")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::ARGB4444,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::ARGB4,
            channels: vec![
                ch('a', 4, UNorm),
                ch('r', 4, UNorm),
                ch('g', 4, UNorm),
                ch('b', 4, UNorm),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgba")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::ARGB4444,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::BGRA10x6_XR,
            channels: vec![
                ch('x', 6, Pad),
                ch('b', 10, XR),
                ch('x', 6, Pad),
                ch('g', 10, XR),
                ch('x', 6, Pad),
                ch('r', 10, XR),
                ch('x', 6, Pad),
                ch('a', 10, XR),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: Some(Swizzle::new("rgba")),
            compatible_color_types: vec![color_type_expectation(
                ColorType::BGRA10101010XR,
                Swizzle::rgba(),
                Some(Swizzle::rgba()),
            )],
        },
        FormatExpectation {
            format: TextureFormat::RGB8_ETC2,
            channels: vec![ch('r', 10, UNorm), ch('g', 8, UNorm), ch('b', 8, UNorm)],
            compression_type: TextureCompressionType::ETC2_RGB8_UNORM,
            xfer_swizzle: None,
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGB888x,
                Swizzle::rgba(),
                None,
            )],
        },
        FormatExpectation {
            format: TextureFormat::RGB8_ETC2_sRGB,
            channels: vec![ch('r', 10, sRGB), ch('g', 8, sRGB), ch('b', 8, sRGB)],
            compression_type: TextureCompressionType::ETC2_RGB8_UNORM,
            xfer_swizzle: None,
            compatible_color_types: vec![color_type_expectation(
                ColorType::SRGBA8888,
                Swizzle::rgba(),
                None,
            )],
        },
        FormatExpectation {
            format: TextureFormat::RGB8_BC1,
            channels: vec![ch('r', 10, UNorm), ch('g', 8, UNorm), ch('b', 8, UNorm)],
            compression_type: TextureCompressionType::BC1_RGB8_UNORM,
            xfer_swizzle: None,
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGB888x,
                Swizzle::rgba(),
                None,
            )],
        },
        FormatExpectation {
            format: TextureFormat::RGBA8_BC1,
            channels: vec![
                ch('r', 10, UNorm),
                ch('g', 8, UNorm),
                ch('b', 8, UNorm),
                ch('a', 8, UNorm),
            ],
            compression_type: TextureCompressionType::BC1_RGBA8_UNORM,
            xfer_swizzle: None,
            compatible_color_types: vec![
                color_type_expectation(ColorType::RGBA8888, Swizzle::rgba(), None),
                color_type_expectation(ColorType::RGB888x, Swizzle::rgb1(), None),
            ],
        },
        FormatExpectation {
            format: TextureFormat::RGBA8_BC1_sRGB,
            channels: vec![
                ch('r', 10, sRGB),
                ch('g', 8, sRGB),
                ch('b', 8, sRGB),
                ch('a', 8, UNorm),
            ],
            compression_type: TextureCompressionType::BC1_RGBA8_UNORM,
            xfer_swizzle: None,
            compatible_color_types: vec![color_type_expectation(
                ColorType::SRGBA8888,
                Swizzle::rgba(),
                None,
            )],
        },
        FormatExpectation {
            format: TextureFormat::YUV8_P2_420,
            channels: vec![ch('y', 8, UNorm), ch('u', 8, UNorm), ch('v', 8, UNorm)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGB888x,
                Swizzle::rgba(),
                None,
            )],
        },
        FormatExpectation {
            format: TextureFormat::YUV8_P3_420,
            channels: vec![ch('y', 8, UNorm), ch('u', 8, UNorm), ch('v', 8, UNorm)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGB888x,
                Swizzle::rgba(),
                None,
            )],
        },
        FormatExpectation {
            format: TextureFormat::YUV10x6_P2_420,
            channels: vec![
                ch('y', 10, UNorm),
                ch('x', 6, Pad),
                ch('u', 10, UNorm),
                ch('x', 6, Pad),
                ch('v', 10, UNorm),
                ch('x', 6, Pad),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGBA10x6,
                Swizzle::rgba(),
                None,
            )],
        },
        FormatExpectation {
            format: TextureFormat::YUV8_P2_422,
            channels: vec![ch('y', 8, UNorm), ch('u', 8, UNorm), ch('v', 8, UNorm)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGB888x,
                Swizzle::rgba(),
                None,
            )],
        },
        FormatExpectation {
            format: TextureFormat::YUV8_P3_422,
            channels: vec![ch('y', 8, UNorm), ch('u', 8, UNorm), ch('v', 8, UNorm)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGB888x,
                Swizzle::rgba(),
                None,
            )],
        },
        FormatExpectation {
            format: TextureFormat::YUV10x6_P2_422,
            channels: vec![
                ch('y', 10, UNorm),
                ch('x', 6, Pad),
                ch('u', 10, UNorm),
                ch('x', 6, Pad),
                ch('v', 10, UNorm),
                ch('x', 6, Pad),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGBA10x6,
                Swizzle::rgba(),
                None,
            )],
        },
        FormatExpectation {
            format: TextureFormat::YUV8_P2_444,
            channels: vec![ch('y', 8, UNorm), ch('u', 8, UNorm), ch('v', 8, UNorm)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGB888x,
                Swizzle::rgba(),
                None,
            )],
        },
        FormatExpectation {
            format: TextureFormat::YUV8_P3_444,
            channels: vec![ch('y', 8, UNorm), ch('u', 8, UNorm), ch('v', 8, UNorm)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGB888x,
                Swizzle::rgba(),
                None,
            )],
        },
        FormatExpectation {
            format: TextureFormat::YUV10x6_P2_444,
            channels: vec![
                ch('y', 10, UNorm),
                ch('x', 6, Pad),
                ch('u', 10, UNorm),
                ch('x', 6, Pad),
                ch('v', 10, UNorm),
                ch('x', 6, Pad),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![color_type_expectation(
                ColorType::RGBA10x6,
                Swizzle::rgba(),
                None,
            )],
        },
        FormatExpectation {
            format: TextureFormat::External,
            channels: vec![
                ch('r', 8, UNorm),
                ch('g', 8, UNorm),
                ch('b', 8, UNorm),
                ch('a', 8, UNorm),
            ],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![
                color_type_expectation(ColorType::RGBA8888, Swizzle::rgba(), None),
                color_type_expectation(ColorType::RGB888x, Swizzle::rgb1(), None),
            ],
        },
        FormatExpectation {
            format: TextureFormat::S8,
            channels: vec![ch('s', 8, Signed)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![],
        },
        FormatExpectation {
            format: TextureFormat::D16,
            channels: vec![ch('d', 16, UNorm)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![],
        },
        FormatExpectation {
            format: TextureFormat::D32F,
            channels: vec![ch('d', 32, Float)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![],
        },
        FormatExpectation {
            format: TextureFormat::D24_S8,
            channels: vec![ch('d', 24, UNorm), ch('s', 8, Signed)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![],
        },
        FormatExpectation {
            format: TextureFormat::D32F_S8,
            channels: vec![ch('d', 32, Float), ch('s', 8, Signed)],
            compression_type: TextureCompressionType::None,
            xfer_swizzle: None,
            compatible_color_types: vec![],
        },
    ]
});

// Match convention used with WrapTexture, where unknown alpha is forced to opaque via swizzle
fn adjust_swizzle_for_alphatype(read_swizzle: &Swizzle, at: AlphaType) -> Swizzle {
    if at == AlphaType::Unknown {
        Swizzle::concat(read_swizzle, &Swizzle::rgb1())
    } else {
        *read_swizzle
    }
}

fn raster_pipeline_allowed_in_transfer(
    src: &[Channel],
    dst: &[Channel],
    cs: &ColorSpaceXformSteps,
) -> bool {
    // Colorspace conversions have to happen in raster pipeline
    if cs.flags.mask() != 0 {
        return true;
    }

    if src.len() != dst.len() {
        // Converting between RGB and RGBA shouldn't require raster pipeline,
        // but larger channel swizzlings such as RGBA to A do.
        if (src.len() != 3 && src.len() != 4) || (dst.len() != 3 && dst.len() != 4) {
            return true;
        }
    }

    let summarize = |channels: &[Channel]| {
        let mut max_channel_bit_depth = 0;
        let mut primary_type = UNorm;
        let mut holds_luminance = false;
        for c in channels {
            holds_luminance = c.name == 'G';
            if c.ty != Pad {
                max_channel_bit_depth = max_channel_bit_depth.max(c.bits);
                primary_type = primary_type.max(c.ty);
            }
        }
        (max_channel_bit_depth, primary_type, holds_luminance)
    };

    let (src_bits, src_type, src_lum) = summarize(src);
    let (dst_bits, dst_type, dst_lum) = summarize(dst);

    // Data bit representation has to go through raster pipeline
    if src_bits != dst_bits || src_type != dst_type {
        return true;
    }

    // Computing a luminance value has to go through raster pipeline
    if dst_lum && !src_lum && (src.len() != 1 || src[0].name != 'a') {
        return true;
    }
    false
}

#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn validate_optimal_xfer_fn(
    r: &mut Reporter,
    xfer_fn: &TextureFormatXferFn,
    cs: &ColorSpaceXformSteps,
    ignore_channel: Option<char>,
    src_name: &str,
    src_channels: &[Channel],
    src_data: &PixelData,
    dst_name: &str,
    dst_channels: &[Channel],
    dst_data: &PixelData,
) {
    // If the transferred data remains unmodified, confirm that the transfer function detected
    // that it could have been an identity transfer. We skip that due to red herrings when
    // there's a colorspace (which would force raster pipeline always, but because of our RGB->BGR
    // synthetic space it can sometimes leave computed values unmodified). We skip when there's a
    // channel change (which can sometimes appear as an identity if certain channels remain 0). We
    // skip the case when it's an ignore-src + force-opaque, because TextureFormatXferfn will
    // choose that for alpha-only color types with unknown alpha but when the source data is
    // alpha-only + opaque it looks equivalent.
    let identity_red_herring = cs.flags.mask() != 0
        || src_channels.len() != dst_channels.len()
        || xfer_fn.is_ignore_src_force_opaque();
    if !identity_red_herring && src_data == dst_data {
        reporter_assert!(
            r,
            xfer_fn.is_identity(),
            "Expected identity xfer fn between {} -> {}",
            src_name,
            dst_name
        );
    }

    // The flip-side, if `xferFn` claims its the identity, then the data should match as well.
    if xfer_fn.is_identity()
        && !compare_pixels(
            dst_channels,
            src_data,
            dst_data,
            /* channelTolerance= */ 0,
            ignore_channel,
        )
    {
        dump_pixel_comparison(
            src_name,
            src_channels,
            src_data,
            dst_name,
            dst_channels,
            src_data,
            dst_data,
        );
        reporter_assert!(
            r,
            false,
            "Xfer fn between {} -> {} claimed identity but was not",
            src_name,
            dst_name
        );
        stop_on_transfer_failure!();
    }

    // Otherwise, check that the transfer function is only relying on the raster pipeline when
    // necessary. And if it is using raster pipeline, then it's not mixing the SIMD xfer ops.
    if xfer_fn.uses_raster_pipeline() {
        reporter_assert!(
            r,
            raster_pipeline_allowed_in_transfer(src_channels, dst_channels, cs),
            "Transfer between {} -> {} unexpected uses raster pipeline",
            src_name,
            dst_name
        );
        if xfer_fn.uses_xfer_ops() {
            // The only time it's okay to mix xfer ops and RP is for handling kDrop/kPadAlpha
            // since the format is not representable with raster pipeline.
            reporter_assert!(
                r,
                xfer_fn.is_drop_or_pad_alpha(),
                "Transfer between {} -> {} mixes transfer methods",
                src_name,
                dst_name
            );
        }
    }
}

const ALPHA_TYPES: [AlphaType; 4] = [
    AlphaType::Unknown,
    AlphaType::Opaque,
    AlphaType::Premul,
    AlphaType::Unpremul,
];

fn test_format_transfers(
    r: &mut Reporter,
    texture_format: &FormatExpectation,
    texture_ct: &ColorTypeExpectation,
    src_at: AlphaType,
    dst_at: AlphaType,
    apply_cs: bool,
) {
    // When transferring to CPU->GPU, we want to apply the textureCT's write swizzle, but if that
    // is undefined because rendering is disabled, just use RGBA.
    let write_swizzle = texture_ct.write_swizzle.unwrap_or_else(Swizzle::rgba);

    // Adjust the R8 channel to be 'G' for gray-storing textures so that gen_pixel_data includes
    // any conversion to or from luminance.
    let is_red_only = texture_format.channels.len() == 1 && texture_format.channels[0].name == 'r';
    let mut expected_texture_channels = texture_format.channels.clone();
    if texture_ct.color_type == ColorType::Gray8 {
        assert!(is_red_only);
        expected_texture_channels[0].name = 'G';
    }

    // This colorspace is constructed to have a linear gamma curve and an identity gamut transform
    // so that any operations that it applies are just multiplies by 0 or 1. Anything else adds
    // minor bit variations to processed data, making checking expectations harder.
    let src_cs = ColorSpace::new_rgb(&named_transfer_fn::LINEAR, &named_gamut::XYZ)
        .expect("a valid color space");
    let dst_cs = if apply_cs {
        src_cs.with_color_spin()
    } else {
        src_cs.clone()
    };
    let cs_steps = ColorSpaceXformSteps::new(Some(&src_cs), src_at, Some(&dst_cs), dst_at);

    let gpu_label = format!(
        "GPU format {} as {}",
        texture_format_name(texture_format.format),
        colortype_name(texture_ct.color_type)
    );
    // Transfering from srcCT into a GPU textureFormat interpreted as textureCT
    for src in COLOR_TYPE_CHANNELS.iter() {
        // Adjust srcAT as if they had created a valid SkBitmap, which may mean skipping. Since we
        // test every srcAT possibility, if the srcAT changes, we skip to avoid running extra
        // work.
        let Some(valid_src_at) = src.color_type.validate_alpha_type(src_at) else {
            continue;
        };
        if valid_src_at != src_at {
            continue;
        }
        // Adjust the read swizzle to account for unknown alpha type semantics
        let read_swizzle = adjust_swizzle_for_alphatype(&texture_ct.read_swizzle, dst_at);
        let xfer_fn = TextureFormatXferFn::make_cpu_to_gpu(
            src.color_type,
            &cs_steps,
            texture_format.format,
            read_swizzle,
        );
        reporter_assert!(
            r,
            texture_format.xfer_swizzle.is_some() == xfer_fn.is_some()
        );

        if texture_format.xfer_swizzle.is_some()
            && let Some(xfer_fn) = &xfer_fn
        {
            let ct_label = format!("CPU colortype {}", colortype_name(src.color_type));
            let (cpu_pixel, cpu_pixel_bits) =
                gen_pixel_data(&src.channels, &Swizzle::rgba(), src_at);

            // The expected GPU value is formed by applying the source colortype's effective
            // swizzle (i.e. fill in missing channels), and the format's compatible colortype's
            // write swizzle to the channel definition of format.
            let mut load_src = src.effective_swizzle;
            if apply_cs {
                // The colorspace conversion is constructed to be a spin via gamut transform
                // matrix, so the end effect should match a rotation swizzle on R,G,B (ignoring
                // alpha).
                load_src = Swizzle::concat(&load_src, &Swizzle::new("gbra"));
            }

            let (expected_gpu_pixel, gpu_pixel_bits) = gen_pixel_data_full(
                &expected_texture_channels,
                &write_swizzle,
                dst_at,
                &src.channels,
                &load_src,
                valid_src_at,
                /* forInput= */ false,
            );
            let actual_gpu_pixel =
                transfer_data(xfer_fn, &cpu_pixel, cpu_pixel_bits, gpu_pixel_bits);

            // The data transfer can skip alpha handling if we know that all samples/reads are
            // going to override it with 1.0 anyways. Testing the final readSwizzle includes both
            // color type semantics that induce the swizzle and requesting unknown alpha type.
            let mut ignore_channel = None;
            if swizzle_char(&read_swizzle, 3) == '1' {
                // If the format stores alpha in r, we need to ignore the r value.
                ignore_channel = Some(if is_red_only { 'r' } else { 'a' });
            }

            // Confirm the transfer is as optimized as possible
            validate_optimal_xfer_fn(
                r,
                xfer_fn,
                &cs_steps,
                ignore_channel,
                &ct_label,
                &src.channels,
                &cpu_pixel,
                &gpu_label,
                &expected_texture_channels,
                &expected_gpu_pixel,
            );

            let tol = channel_tolerance(
                &src.channels,
                valid_src_at,
                &expected_texture_channels,
                dst_at,
            );
            if !compare_pixels(
                &expected_texture_channels,
                &expected_gpu_pixel,
                &actual_gpu_pixel,
                tol,
                ignore_channel,
            ) {
                dump_pixel_comparison(
                    &ct_label,
                    &src.channels,
                    &cpu_pixel,
                    &gpu_label,
                    &texture_format.channels,
                    &expected_gpu_pixel,
                    &actual_gpu_pixel,
                );
                reporter_assert!(
                    r,
                    false,
                    "Pixel mismatch uploading from {}, alpha {} -> {}{}",
                    ct_label,
                    alphatype_name(valid_src_at),
                    alphatype_name(dst_at),
                    if apply_cs {
                        " with colorspace conversion"
                    } else {
                        ""
                    }
                );
                stop_on_transfer_failure!();
            }
        }
    }

    // Transfering from a GPU textureFormat interpreted as textureCT into dstCT
    for dst in COLOR_TYPE_CHANNELS.iter() {
        // Adjust dstAT as if they had created a valid SkBitmap, which may mean skipping. Since we
        // test every dstAT possibility, if the dstAT changes, we skip to avoid running extra
        // work.
        let Some(valid_dst_at) = dst.color_type.validate_alpha_type(dst_at) else {
            continue;
        };
        if valid_dst_at != dst_at {
            continue;
        }
        // Adjust the read swizzle to account for unknown alpha type semantics
        let read_swizzle = adjust_swizzle_for_alphatype(&texture_ct.read_swizzle, src_at);
        let xfer_fn = TextureFormatXferFn::make_gpu_to_cpu(
            texture_format.format,
            read_swizzle,
            &cs_steps,
            dst.color_type,
        );
        reporter_assert!(
            r,
            texture_format.xfer_swizzle.is_some() == xfer_fn.is_some()
        );

        if let (Some(xfer_swizzle), Some(xfer_fn)) = (&texture_format.xfer_swizzle, &xfer_fn) {
            let ct_label = format!("CPU colortype {}", colortype_name(dst.color_type));
            let (gpu_pixel, gpu_pixel_bits) =
                gen_pixel_data(&expected_texture_channels, &texture_ct.read_swizzle, src_at);

            // The expected CPU value is formed by applying the TextureFormat's implicit transfer
            // swizzle (i.e. fill in missing channels), its compatible colortype's read swizzle
            // to the channel definition of the dst color type.
            let mut load_src = Swizzle::concat(xfer_swizzle, &read_swizzle);
            if apply_cs {
                load_src = Swizzle::concat(&load_src, &Swizzle::new("gbra")); // See above
            }
            let (expected_cpu_pixel, cpu_pixel_bits) = gen_pixel_data_full(
                &dst.channels,
                &Swizzle::rgba(),
                dst_at,
                &expected_texture_channels,
                &load_src,
                src_at,
                /* forInput= */ false,
            );
            // Confirm the transfer is as optimized as possible
            validate_optimal_xfer_fn(
                r,
                xfer_fn,
                &cs_steps,
                /* ignoreChannel= */ None,
                &gpu_label,
                &expected_texture_channels,
                &gpu_pixel,
                &ct_label,
                &dst.channels,
                &expected_cpu_pixel,
            );

            let actual_cpu_pixel =
                transfer_data(xfer_fn, &gpu_pixel, gpu_pixel_bits, cpu_pixel_bits);

            let tol = channel_tolerance(&expected_texture_channels, src_at, &dst.channels, dst_at);
            if !compare_pixels(
                &dst.channels,
                &expected_cpu_pixel,
                &actual_cpu_pixel,
                tol,
                None,
            ) {
                dump_pixel_comparison(
                    &gpu_label,
                    &texture_format.channels,
                    &gpu_pixel,
                    &ct_label,
                    &dst.channels,
                    &expected_cpu_pixel,
                    &actual_cpu_pixel,
                );
                reporter_assert!(
                    r,
                    false,
                    "Pixel mismatch reading back to {}, alpha {} -> {}{}",
                    ct_label,
                    alphatype_name(src_at),
                    alphatype_name(dst_at),
                    if apply_cs {
                        " with colorspace conversion"
                    } else {
                        ""
                    }
                );
                stop_on_transfer_failure!();
            }
        }
    }
}

fn run_texture_format_test(r: &mut Reporter, caps: &WgpuCaps, format: TextureFormat) {
    let mut found_expectation = false;
    // skiatest::ReporterContext: the context names are prepended to the failures' messages.
    let outer_context = r.context().map(str::to_owned);
    let scoped = |inner: &str| match &outer_context {
        Some(outer) => format!("{outer}, {inner}"),
        None => inner.to_owned(),
    };
    for e in EXPECTATIONS.iter() {
        if e.format != format {
            continue;
        }

        // Should only find it once
        reporter_assert!(
            r,
            !found_expectation,
            "Format expectation listed multiple times"
        );
        found_expectation = true;

        r.set_context(Some(scoped(&format!(
            "Format {}",
            texture_format_name(format)
        ))));

        // Found the expectation for the requested format. Check fixed properties first.
        reporter_assert!(
            r,
            e.compression_type == texture_format_compression_type(format)
        );
        reporter_assert!(
            r,
            e.bytes_per_block() == texture_format_bytes_per_block(format)
        );
        reporter_assert!(r, e.channel_mask() == texture_format_channel_mask(format));
        reporter_assert!(
            r,
            e.has_depth_or_stencil() == texture_format_is_depth_or_stencil(format)
        );
        reporter_assert!(r, e.has_depth() == texture_format_has_depth(format));
        reporter_assert!(r, e.has_stencil() == texture_format_has_stencil(format));
        reporter_assert!(
            r,
            e.is_multiplanar() == texture_format_is_multiplanar(format)
        );
        reporter_assert!(r, e.auto_clamps() == texture_format_auto_clamps(format));
        reporter_assert!(
            r,
            e.is_floating_point() == texture_format_is_floating_point(format)
        );

        // Verify compatible color types
        let (base_color_type, _) = texture_format_color_type_info(format);
        if base_color_type == ColorType::Unknown {
            reporter_assert!(r, e.compatible_color_types.is_empty());
        } else {
            // Should be the first listed compatible color type
            reporter_assert!(r, !e.compatible_color_types.is_empty());
            reporter_assert!(
                r,
                e.compatible_color_types
                    .first()
                    .is_some_and(|first| first.color_type == base_color_type)
            );
        }

        for c in 0..=ColorType::LAST_ENUM as i32 {
            let ct = ColorType::from_i32(c).expect("a valid color type");

            r.set_context(Some(scoped(&format!(
                "Format {}, color type {}\n",
                texture_format_name(format),
                colortype_name(ct)
            ))));

            let mut found_color_expectation = false;
            for ec in &e.compatible_color_types {
                if ec.color_type == ct {
                    // Expected to be compatible (and should only find it once)
                    reporter_assert!(
                        r,
                        !found_color_expectation,
                        "Color type listed multiple times: {}",
                        colortype_name(ec.color_type)
                    );
                    found_color_expectation = true;

                    // Check swizzles and transfers here, the rest of the color type checks
                    // happen outside the loop based on `foundColorExpectation`.
                    let actual_read_swizzle = read_swizzle_for_color_type(ct, format);
                    reporter_assert!(
                        r,
                        ec.read_swizzle == actual_read_swizzle,
                        "actual {} vs. expected {}",
                        actual_read_swizzle.as_string(),
                        ec.read_swizzle.as_string()
                    );

                    let actual_write_swizzle = write_swizzle_for_color_type(ct, format);
                    if let Some(expected_write_swizzle) = &ec.write_swizzle {
                        reporter_assert!(r, actual_write_swizzle.is_some());
                        reporter_assert!(
                            r,
                            Some(*expected_write_swizzle) == actual_write_swizzle,
                            "actual {} vs. expected {}",
                            actual_write_swizzle
                                .as_ref()
                                .map_or_else(|| "null".to_owned(), Swizzle::as_string),
                            expected_write_swizzle.as_string()
                        );
                    } else {
                        reporter_assert!(r, actual_write_swizzle.is_none());
                        // This is a proxy for "the format can represent CT, and there are some
                        // formats that can render CT, but this format does not render w/ CT".
                        let renderable_info = caps.get_default_sampled_texture_info(
                            ct,
                            Mipmapped::No,
                            Protected::No,
                            Renderable::Yes,
                        );
                        reporter_assert!(
                            r,
                            format != texture_info_priv::view_format(&renderable_info)
                        );
                    }

                    // Test sampled and readable copy info
                    let sampled_info = caps.get_default_sampled_texture_info(
                        ct,
                        Mipmapped::No,
                        Protected::No,
                        Renderable::No,
                    );
                    if sampled_info.is_valid()
                        && texture_info_priv::view_format(&sampled_info) == format
                    {
                        reporter_assert!(r, caps.is_texturable(&sampled_info, false));
                        reporter_assert!(r, caps.is_readable(&sampled_info, false));
                        let copy_info =
                            caps.get_texture_info_for_sampled_copy(&sampled_info, Mipmapped::No);
                        reporter_assert!(r, copy_info.is_valid());
                    }

                    let readable_info =
                        caps.get_default_readable_texture_info(format, Protected::No);
                    if readable_info.is_valid()
                        && texture_info_priv::view_format(&readable_info) == format
                    {
                        reporter_assert!(r, caps.is_readable(&readable_info, false));
                        let copy_info = caps.get_texture_info_for_readable_copy(&readable_info);
                        reporter_assert!(r, copy_info.is_valid());
                    }

                    let storage_info =
                        caps.get_default_readable_storage_texture_info(format, Protected::No);
                    if storage_info.is_valid()
                        && texture_info_priv::view_format(&storage_info) == format
                    {
                        reporter_assert!(r, caps.is_storage(&storage_info));
                        reporter_assert!(r, caps.is_readable(&storage_info, false));
                    }

                    // Test all combinations of alpha type x 2 (texture vs cpu) and whether or not
                    // colorspace conversions are handled in the transfer.
                    for apply_cs in [false, true] {
                        for src_at in ALPHA_TYPES {
                            for dst_at in ALPHA_TYPES {
                                test_format_transfers(r, e, ec, src_at, dst_at, apply_cs);
                            }
                        }
                    }
                }
            }

            // If we found an expectation, it should be detected as compatible (and false
            // otherwise)
            let actual_compatible = are_color_type_and_format_compatible(ct, format);
            reporter_assert!(
                r,
                found_color_expectation == actual_compatible,
                "actual ({}) vs expected ({})",
                i32::from(actual_compatible),
                i32::from(found_color_expectation)
            );
        }
    }

    r.set_context(outer_context.clone());
    // All formats should have expectations
    reporter_assert!(
        r,
        found_expectation,
        "Missing expectation for {}",
        texture_format_name(format)
    );
}

def_graphite_test_for_all_contexts!(TextureFormatTest, |r, context| {
    // Port of: tests/graphite/TextureFormatTest.cpp#L1393-L1397 (chrome/m156)
    // The formats are independent of each other, and the raster pipeline is slow in debug builds
    // (the C++ test makes ~70k transfers per context), so the port checks them on several
    // threads, each with its own reporter, and reports their failures together.
    let caps = context.wgpu_caps();
    let next_format = AtomicUsize::new(0);
    let thread_count = thread::available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .min(8);
    let failures: Vec<Failure> = thread::scope(|scope| {
        let workers: Vec<_> = (0..thread_count)
            .map(|_| {
                scope.spawn(|| {
                    let mut reporter = Reporter::new("TextureFormatTest");
                    loop {
                        let i = next_format.fetch_add(1, Ordering::Relaxed);
                        if i >= TEXTURE_FORMAT_COUNT {
                            break;
                        }
                        run_texture_format_test(&mut reporter, caps, TextureFormat::ALL[i]);
                    }
                    reporter.failures().to_vec()
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("a format worker thread"))
            .collect()
    });
    for failure in failures {
        r.report_failed(failure);
    }
});
