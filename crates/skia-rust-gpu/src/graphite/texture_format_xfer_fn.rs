// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/TextureFormatXferFn.h, src/gpu/graphite/TextureFormatXferFn.cpp

//! `TextureFormatXferFn`: converts pixel data between a CPU color type and the bit layout of a
//! texture format, for uploads and readbacks.
//!
//! A transfer is up to three steps: bit manipulations of the raw texture data (`preOps`), a raster
//! pipeline (or an `SkOpts` swizzler) between color types, and bit manipulations into the raw
//! texture data (`postOps`).
//!
//! The bit manipulations work on 16-byte vectors in Skia (`skvx`), holding several pixels, and
//! keep the vector between iterations so a partial tail vector carries lanes over from the
//! previous one. The port emulates those vectors lane for lane, so every byte written is the same
//! as in C++.
//!
//! skia-rust: Skia builds the raster pipeline once (in an arena owned by the function) and patches
//! the memory contexts per run. The port's pipelines borrow their arena, so the function keeps
//! the recipe (color types and modifiers, in Skia's order) and builds the same stages at the
//! start of each [`TextureFormatXferFn::run`].

use std::sync::Arc;

use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::color::ColorChannelFlag;
use skia_rust_core::color_space_xform_steps::ColorSpaceXformSteps;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::half::HALF_1;
use skia_rust_core::image_info_priv::{color_type_channel_flags, color_type_is_alpha_only};
use skia_rust_core::raster_pipeline::{
    MemSlot, MemView, MemoryBindings, MemoryCtx, RasterPipeline, Stage,
};
use skia_rust_core::texture_compression_type::TextureCompressionType;
use skia_rust_simd::swizzle as opts;

use crate::gpu::swizzle::Swizzle;
use crate::graphite::texture_format::{
    FormatXferOp, TextureFormat, preferred_texture_formats, texture_format_bytes_per_block,
    texture_format_color_type_info, texture_format_compression_type,
};

use TextureFormat as TF;

// This is intentionally distinct from FormatXferOp: it splits the ops by conversion direction.
// Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L36-L42 (chrome/m156)
/// `kDropAlpha`: `FormatXferOp::kDropAlpha` for CPU->GPU conversion.
const DROP_ALPHA: u8 = 0x1;
/// `kPadAlpha`: `FormatXferOp::kDropAlpha` for GPU->CPU conversion.
const PAD_ALPHA: u8 = 0x2;
/// `kSwapRB`: behaves the same for either conversion direction.
const SWAP_RB: u8 = 0x4;
/// `kForceOpaque`: `RGBx` handling to avoid raster pipeline is the same in both directions.
const FORCE_OPAQUE: u8 = 0x8;
/// `kIgnoreSrc`: the output value does not depend on the src, will be either 0 or 1.
const IGNORE_SRC: u8 = 0x10;

/// The 16-byte SIMD vector width every row function works with.
const VECTOR_BYTES: usize = 16;

/// An unsigned lane type of a row-function vector (`uint8_t`, `uint16_t`, `uint32_t`).
trait Lane: Copy + Default + PartialEq + Send + Sync + 'static {
    const SIZE: usize;
    fn read(bytes: &[u8]) -> Self;
    fn write(self, bytes: &mut [u8]);
    fn bits(self) -> u32;
    fn from_bits(bits: u32) -> Self;
}

macro_rules! impl_lane {
    ($t:ty) => {
        impl Lane for $t {
            const SIZE: usize = std::mem::size_of::<$t>();

            fn read(bytes: &[u8]) -> Self {
                <$t>::from_ne_bytes(bytes[..Self::SIZE].try_into().expect("lane bytes"))
            }

            fn write(self, bytes: &mut [u8]) {
                bytes[..Self::SIZE].copy_from_slice(&self.to_ne_bytes());
            }

            fn bits(self) -> u32 {
                u32::from(self)
            }

            #[allow(clippy::cast_possible_truncation)] // lanes hold values of their own width
            fn from_bits(bits: u32) -> Self {
                bits as $t
            }
        }
    };
}
impl_lane!(u8);
impl_lane!(u16);
impl_lane!(u32);

/// A `skvx::Vec<16 / sizeof(Px), Px>`, kept as its bytes so `memcpy` in and out is exact.
#[derive(Clone, Copy)]
struct PixelVec {
    bytes: [u8; VECTOR_BYTES],
}

impl PixelVec {
    fn lane<Px: Lane>(&self, i: usize) -> Px {
        Px::read(&self.bytes[i * Px::SIZE..])
    }

    fn set_lane<Px: Lane>(&mut self, i: usize, v: Px) {
        v.write(&mut self.bytes[i * Px::SIZE..]);
    }

    fn splat<Px: Lane>(&mut self, v: Px) {
        for i in 0..VECTOR_BYTES / Px::SIZE {
            self.set_lane(i, v);
        }
    }

    // skvx::shuffle<idx...>(pixel): lane i of the result is lane idx[i] of the input.
    fn shuffle<Px: Lane>(&mut self, idx: &[usize]) {
        let src = *self;
        for (i, &from) in idx.iter().enumerate() {
            self.set_lane::<Px>(i, src.lane(from));
        }
    }
}

/// `XferRowFn`: converts `width` pixels of `src` into `dst`.
type XferRowFn = Box<dyn Fn(&[u8], &mut [u8], usize) + Send + Sync>;

/// `create_xfer_row_fn`: applies `apply_pixel` to `n` pixels per 16-byte vector. A `src_bpp` of 0
/// ignores the source and starts each vector from `zero_value`.
// Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L46-L105 (chrome/m156)
fn create_xfer_row_fn<Px: Lane>(
    n: usize,
    src_bpp: usize,
    dst_bpp: usize,
    opaque_alpha: Px,
    zero_value: Px,
    apply_pixel: impl Fn(&mut PixelVec, Px) + Send + Sync + 'static,
) -> XferRowFn {
    if src_bpp != 0 {
        // The vector should be sufficient to hold N src and dst pixels, and match at least one
        debug_assert!(VECTOR_BYTES >= src_bpp * n && VECTOR_BYTES >= dst_bpp * n);
        debug_assert!(VECTOR_BYTES == src_bpp * n || VECTOR_BYTES == dst_bpp * n);

        Box::new(move |src: &[u8], dst: &mut [u8], mut width: usize| {
            let src_bpp_n = n * src_bpp;
            let dst_bpp_n = n * dst_bpp;

            let mut pixel = PixelVec {
                bytes: [0; VECTOR_BYTES],
            };
            let mut s = 0;
            let mut d = 0;
            while width >= n {
                pixel.bytes[..src_bpp_n].copy_from_slice(&src[s..s + src_bpp_n]);
                apply_pixel(&mut pixel, opaque_alpha);
                dst[d..d + dst_bpp_n].copy_from_slice(&pixel.bytes[..dst_bpp_n]);

                width -= n;
                s += src_bpp_n;
                d += dst_bpp_n;
            }

            if width > 0 {
                // Process tail that is less than a full vector
                debug_assert!(width < n);
                pixel.bytes[..width * src_bpp].copy_from_slice(&src[s..s + width * src_bpp]);
                apply_pixel(&mut pixel, opaque_alpha);
                dst[d..d + width * dst_bpp].copy_from_slice(&pixel.bytes[..width * dst_bpp]);
            }
        })
    } else {
        // Ignore the source in favor of loading zeroValue into pixel each time
        debug_assert!(VECTOR_BYTES >= dst_bpp * n);

        Box::new(move |_src: &[u8], dst: &mut [u8], mut width: usize| {
            let dst_bpp_n = n * dst_bpp;

            let mut pixel = PixelVec {
                bytes: [0; VECTOR_BYTES],
            };
            let mut d = 0;
            while width >= n {
                pixel.splat(zero_value);
                apply_pixel(&mut pixel, opaque_alpha);
                dst[d..d + dst_bpp_n].copy_from_slice(&pixel.bytes[..dst_bpp_n]);

                width -= n;
                d += dst_bpp_n;
            }

            if width > 0 {
                // Process tail that is less than a full vector
                debug_assert!(width < n);
                pixel.splat(zero_value);
                apply_pixel(&mut pixel, opaque_alpha);
                dst[d..d + width * dst_bpp].copy_from_slice(&pixel.bytes[..width * dst_bpp]);
            }
        })
    }
}

/// `apply_ops_packed<Px, N, Ops, RShift, BShift, RBBits>`: each lane holds one pixel.
// Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L107-L132 (chrome/m156)
fn apply_ops_packed<Px: Lane>(
    pixel: &mut PixelVec,
    opaque_alpha: Px,
    ops: u8,
    r_shift: u32,
    b_shift: u32,
    rb_bits: u32,
) {
    debug_assert!(
        ops & (PAD_ALPHA | DROP_ALPHA) == 0,
        "Packed formats do not drop/pad alpha"
    );
    debug_assert!(
        r_shift < b_shift,
        "Shifts are not in the correct significance order"
    );
    debug_assert!(r_shift + rb_bits <= b_shift, "Low swap channel overflows");
    debug_assert!(
        b_shift + rb_bits <= u32::try_from(Px::SIZE * 8).unwrap_or(32),
        "Hi swap channel overflows"
    );

    for i in 0..VECTOR_BYTES / Px::SIZE {
        let mut p = pixel.lane::<Px>(i).bits();
        if ops & SWAP_RB != 0 {
            let channel_mask = (1u32 << rb_bits) - 1;
            let r_mask = channel_mask << r_shift;
            let b_mask = channel_mask << b_shift;

            p = (p & !(r_mask | b_mask)) // Preserve non-RB bits
                | ((p & r_mask) << (b_shift - r_shift)) // Move red (lo) to blue (hi) position
                | ((p & b_mask) >> (b_shift - r_shift)); // Move blue (hi) to red (lo) position
        }

        if ops & FORCE_OPAQUE != 0 {
            // This assumes the opaqueAlpha value will have 0 bits set in the R,G,B channels.
            p |= opaque_alpha.bits();
        }
        pixel.set_lane(i, Px::from_bits(p));
    }
}

/// `xfer_rows_packed<Px, RShift, BShift, RBBits>`.
// Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L138-L163 (chrome/m156)
fn xfer_rows_packed<Px: Lane>(
    ops: u8,
    r_shift: u32,
    b_shift: u32,
    rb_bits: u32,
    opaque_alpha: Px,
    zero_value: Px,
) -> XferRowFn {
    let bpp = Px::SIZE;
    let n = VECTOR_BYTES / Px::SIZE; // Fit to 128-bit/16-byte SIMD

    match ops {
        // The expected combination of ExtendedFormatXferOps
        x if x == SWAP_RB
            || x == IGNORE_SRC
            || x == FORCE_OPAQUE
            || x == FORCE_OPAQUE | SWAP_RB
            || x == FORCE_OPAQUE | IGNORE_SRC =>
        {
            let pixel_ops = ops & !IGNORE_SRC;
            create_xfer_row_fn::<Px>(
                n,
                if ops & IGNORE_SRC != 0 { 0 } else { bpp },
                bpp,
                opaque_alpha,
                zero_value,
                move |pixel, opaque| {
                    apply_ops_packed(pixel, opaque, pixel_ops, r_shift, b_shift, rb_bits);
                },
            )
        }
        _ => panic!("Unsupported ExtendedFormatXferOps combination: {ops}"),
    }
}

/// `apply_ops_by_channel<Cx, N, CPow2, Ops>`: each lane holds one channel; `n` pixels of
/// `c_pow2` lanes.
// Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L169-L240 (chrome/m156)
fn apply_ops_by_channel<Cx: Lane>(
    pixel: &mut PixelVec,
    opaque_alpha: Cx,
    ops: u8,
    n: usize,
    c_pow2: usize,
) {
    debug_assert!(c_pow2 == 1 || c_pow2 == 2 || c_pow2 == 4);

    if ops & PAD_ALPHA != 0 && c_pow2 == 4 {
        // Padding alpha moves 3-channel source data (in the first N*3 values) to 4 channels. 3*N+1
        // holds undefined data after loading, which is set to the opaque alpha value. Then the
        // slots are shuffled to spread out each pixel's R, G and B and insert the alpha.
        let a = 3 * n;
        pixel.set_lane(a, opaque_alpha);

        match n {
            4 => pixel.shuffle::<Cx>(&[0, 1, 2, a, 3, 4, 5, a, 6, 7, 8, a, 9, 10, 11, a]),
            2 => pixel.shuffle::<Cx>(&[0, 1, 2, a, 3, 4, 5, a]),
            // No shuffling needed for N=1, since pixel == shuffle<0,1,2,A>(pixel)
            _ => debug_assert_eq!(n, 1),
        }
    }

    // Ops that assume 4 channels can be applied between kPadAlpha and kDropAlpha.
    if ops & SWAP_RB != 0 && c_pow2 == 4 {
        // For all 3 and 4 channel formats, it is assumed that R and B are in channels 0 and 2.
        match n {
            4 => pixel.shuffle::<Cx>(&[2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15]),
            2 => pixel.shuffle::<Cx>(&[2, 1, 0, 3, 6, 5, 4, 7]),
            _ => {
                debug_assert_eq!(n, 1);
                pixel.shuffle::<Cx>(&[2, 1, 0, 3]);
            }
        }
    }

    // 2-channel formats are incompatible with forcing opaque since there's never an alpha channel
    if ops & FORCE_OPAQUE != 0 && c_pow2 != 2 {
        if c_pow2 == 1 {
            // For single-channel formats, the opaque alpha gets splatted over the whole vector
            pixel.splat(opaque_alpha);
        } else {
            // Between kPadAlpha and kDropAlpha, it is assumed that alpha is always channel 3
            for p in 0..n {
                pixel.set_lane(4 * p + 3, opaque_alpha);
            }
        }
    }

    if ops & DROP_ALPHA != 0 && c_pow2 == 4 {
        // Dropping alpha shuffles the R, G and B values of the 4-channel source data into the
        // first N*3 slots. The remaining N slots are ignored by the final memcpy.
        match n {
            4 => pixel.shuffle::<Cx>(&[0, 1, 2, 4, 5, 6, 8, 9, 10, 12, 13, 14, 14, 14, 14, 14]),
            2 => pixel.shuffle::<Cx>(&[0, 1, 2, 4, 5, 6, 6, 6]),
            // No shuffling needed for N=1, since pixel == shuffle<0,1,2,_>(pixel)
            _ => debug_assert_eq!(n, 1),
        }
    }
}

/// `xfer_rows_by_channel<Cx, C>`.
// Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L248-L291 (chrome/m156)
fn xfer_rows_by_channel<Cx: Lane>(
    c: usize,
    ops: u8,
    opaque_alpha: Cx,
    zero_value: Cx,
) -> XferRowFn {
    const SUPPORTED: [u8; 11] = [
        DROP_ALPHA,
        DROP_ALPHA | SWAP_RB,
        DROP_ALPHA | IGNORE_SRC,
        PAD_ALPHA,
        PAD_ALPHA | SWAP_RB,
        PAD_ALPHA | IGNORE_SRC,
        SWAP_RB,
        IGNORE_SRC,
        FORCE_OPAQUE,
        FORCE_OPAQUE | SWAP_RB,
        FORCE_OPAQUE | IGNORE_SRC,
    ];

    debug_assert!((1..=4).contains(&c));
    let c_pow2 = c.next_power_of_two();
    let n = VECTOR_BYTES / (c_pow2 * Cx::SIZE); // Fit to 128-bit/16-byte SIMD

    let mut src_bpp = c * Cx::SIZE;
    let mut dst_bpp = c * Cx::SIZE;
    if ops & DROP_ALPHA != 0 {
        // Going from 4-channel src to the 3-channel format
        debug_assert_eq!(c, 3);
        src_bpp = c_pow2 * Cx::SIZE;
    } else if ops & PAD_ALPHA != 0 {
        // Going from the 3-channel format to 4-channel dst
        debug_assert_eq!(c, 3);
        dst_bpp = c_pow2 * Cx::SIZE;
    }

    assert!(
        SUPPORTED.contains(&ops),
        "Unsupported ExtendedFormatXferOps combination: {ops}"
    );

    let pixel_ops = ops & !IGNORE_SRC;
    create_xfer_row_fn::<Cx>(
        n,
        if ops & IGNORE_SRC != 0 { 0 } else { src_bpp },
        dst_bpp,
        opaque_alpha,
        zero_value,
        move |pixel, opaque| apply_ops_by_channel(pixel, opaque, pixel_ops, n, c_pow2),
    )
}

/// `get_xfer_row_fn`: the bit manipulations `ops` for `format`.
// Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L293-L386 (chrome/m156)
fn get_xfer_row_fn(format: TextureFormat, ops: u8) -> XferRowFn {
    const FLOAT_BITS_1: u32 = 0x3f80_0000; // SkFloat2Bits isn't constexpr

    // 10 bits for _XR formats that encodes 1.0 and 0.0 given their extended normalization range.
    const XR_ONE: u32 = 0x37e;
    const XR_ZERO: u32 = 0x180;

    debug_assert_eq!(FLOAT_BITS_1, 1.0f32.to_bits());

    debug_assert!(ops != 0); // For now, assume we only call into this if we have work to do.

    match format {
        // Packed formats operate on a primitive that holds the entire pixel value
        TF::B5_G6_R5 | TF::R5_G6_B5 => xfer_rows_packed::<u16>(ops, 0, 11, 5, 0, 0),

        TF::ABGR4 | TF::ARGB4 => xfer_rows_packed::<u16>(ops, 4, 12, 4, 0xF, 0),

        TF::RGB10_A2 | TF::BGR10_A2 => xfer_rows_packed::<u32>(ops, 0, 20, 10, 0b11 << 30, 0),

        // NOTE: opaqueAlpha doesn't matter here since it's just BGRx data, but to match SkRP's
        // handling of BGR10A2_XR, treat the last 2 bits as regular unorm.
        TF::BGR10_XR => xfer_rows_packed::<u32>(
            ops,
            0,
            20,
            10,
            0b11 << 30,
            (XR_ZERO << 20) | (XR_ZERO << 10) | XR_ZERO,
        ),

        // The remaining formats can be operated on with each channel as a primitive
        TF::R8 | TF::A8 => xfer_rows_by_channel::<u8>(1, ops, 0xFF, 0),

        TF::R16 => xfer_rows_by_channel::<u16>(1, ops, 0xFFFF, 0),

        TF::R16F => xfer_rows_by_channel::<u16>(1, ops, HALF_1, 0),

        TF::RG8 => xfer_rows_by_channel::<u8>(2, ops, 0xFF, 0),

        // (sic) Skia's opaque value for RG16; 2-channel formats never force opaque.
        TF::RG16 => xfer_rows_by_channel::<u16>(2, ops, 0xFF, 0),

        TF::RG16F => xfer_rows_by_channel::<u16>(2, ops, HALF_1, 0),

        TF::RG32F => xfer_rows_by_channel::<u32>(2, ops, FLOAT_BITS_1, 0),

        TF::RGB8_sRGB | TF::RGB8 | TF::BGR8 => xfer_rows_by_channel::<u8>(3, ops, 0xFF, 0),

        TF::RGB16 => xfer_rows_by_channel::<u16>(3, ops, 0xFFFF, 0),

        TF::RGB16F => xfer_rows_by_channel::<u16>(3, ops, HALF_1, 0),

        TF::RGB32F => xfer_rows_by_channel::<u32>(3, ops, FLOAT_BITS_1, 0),

        TF::RGBA8 | TF::RGBA8_sRGB | TF::BGRA8 | TF::BGRA8_sRGB => {
            xfer_rows_by_channel::<u8>(4, ops, 0xFF, 0)
        }

        // Each channel is the 10 real bits, with the least significant 6 padding bits.
        TF::RGBA10x6 => xfer_rows_by_channel::<u16>(4, ops, 0xFFC0, 0),

        // Like RGBA10x6 except the constants have to fit the extended range.
        #[allow(clippy::cast_possible_truncation)] // the XR constants fit 10 bits
        TF::BGRA10x6_XR => {
            xfer_rows_by_channel::<u16>(4, ops, (XR_ONE << 6) as u16, (XR_ZERO << 6) as u16)
        }

        TF::RGBA16 => xfer_rows_by_channel::<u16>(4, ops, 0xFFFF, 0),

        TF::RGBA16F => xfer_rows_by_channel::<u16>(4, ops, HALF_1, 0),

        TF::RGBA32F => xfer_rows_by_channel::<u32>(4, ops, FLOAT_BITS_1, 0),

        _ => {
            // Remaining cases are compressed, multiplanar, or non-color so shouldn't be reached.
            debug_assert!(
                texture_format_color_type_info(format)
                    .1
                    .contains(FormatXferOp::DISABLED)
            );
            unreachable!("no transfer for {format:?}")
        }
    }
}

// --- Functions for collapsing equivalent colortypes and formats into fewer xfer ops

// Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L412-L432 (chrome/m156)
fn account_for_luminance(
    src_ct: ColorType,
    has_color_space_transform: bool,
    dst_ct: &mut ColorType,
) -> bool {
    if *dst_ct == ColorType::Gray8 {
        if src_ct != ColorType::Gray8 || has_color_space_transform {
            // Luminance must be calculated by raster pipeline before any srcToDst swizzle, so pull
            // it out to its own RPModifier. The op stores the value in the alpha channel, so that
            // needs to be dstCT that SkRasterPipeline sees.
            *dst_ct = ColorType::Alpha8;
            return true;
        } // else leave it as gray->gray
    } else if src_ct == ColorType::Gray8
        && *dst_ct == ColorType::R8UNorm
        && !has_color_space_transform
    {
        // The source gray value can just be copied to the red channel, so adjust this to be
        // gray->"gray" for a no-op copy
        *dst_ct = ColorType::Gray8;
        return false;
    } // else trivially no extra luminance needs to be calculated, leave dstCT as-is
    false
}

// Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L434-L444 (chrome/m156)
fn rgbx_to_rgba(ct: &mut ColorType) -> bool {
    match *ct {
        ColorType::RGB888x => *ct = ColorType::RGBA8888,
        ColorType::RGB101010x => *ct = ColorType::RGBA1010102,
        ColorType::RGBF16F16F16x => *ct = ColorType::RGBAF16,
        ColorType::BGR101010x => *ct = ColorType::BGRA1010102,
        // No colortype consolidation possible by switching to a kForceOpaque op instead.
        _ => return false,
    }
    true
}

// Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L446-L458 (chrome/m156)
fn bgra_to_rgba(ct: &mut ColorType) -> bool {
    match *ct {
        ColorType::BGRA8888 => *ct = ColorType::RGBA8888,
        ColorType::BGRA1010102 => *ct = ColorType::RGBA1010102,
        ColorType::BGR101010x => *ct = ColorType::RGB101010x,
        // NOTE: For now there's no RGBA version of the _XR color types, so they can't be swapped
        _ => return false,
    }
    true
}

// Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L460-L462 (chrome/m156)
fn gray_adjusted_channels(ct: ColorType) -> u32 {
    if ct == ColorType::Gray8 {
        ColorChannelFlag::RGB.bits()
    } else {
        color_type_channel_flags(ct).bits()
    }
}

// `swizzle[i]`.
fn swz(swizzle: Swizzle, i: usize) -> char {
    swizzle
        .as_string()
        .chars()
        .nth(i)
        .expect("a swizzle has four components")
}

// Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L464-L476 (chrome/m156)
fn swizzle_adjusted_channels(mut channels: u32, swizzle: Swizzle) -> u32 {
    let mut remove_channel_if_constant = |flag: ColorChannelFlag, index: usize| {
        let c = swz(swizzle, index);
        if c == '0' || c == '1' {
            channels &= !flag.bits();
        }
    };
    remove_channel_if_constant(ColorChannelFlag::RED, 0);
    remove_channel_if_constant(ColorChannelFlag::GREEN, 1);
    remove_channel_if_constant(ColorChannelFlag::BLUE, 2);
    remove_channel_if_constant(ColorChannelFlag::ALPHA, 3);
    channels
}

// `SkColorTypeIsAlwaysOpaque`.
fn color_type_is_always_opaque(ct: ColorType) -> bool {
    color_type_channel_flags(ct).bits() & ColorChannelFlag::ALPHA.bits() == 0
}

/// `optimize_transfer<TextureIsDst>`: simplifies the transfer, returning the extended ops and
/// whether luminance must be computed.
// Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L478-L677 (chrome/m156)
#[allow(clippy::too_many_lines)] // one function, as in Skia
fn optimize_transfer(
    texture_is_dst: bool,
    cpu_ct: &mut ColorType,
    tex_base_ct: &mut ColorType,
    tex_read_swizzle: &mut Swizzle,
    cs_steps: &mut ColorSpaceXformSteps,
    xfer_ops: FormatXferOp,
) -> (u8, bool) {
    // Aliases for color types that are based on the transfer direction:
    // srcCT = TextureIsDst ? cpuCT : texBaseCT; dstCT = TextureIsDst ? texBaseCT : cpuCT;
    macro_rules! src_ct {
        () => {
            if texture_is_dst {
                &mut *cpu_ct
            } else {
                &mut *tex_base_ct
            }
        };
    }
    macro_rules! dst_ct {
        () => {
            if texture_is_dst {
                &mut *tex_base_ct
            } else {
                &mut *cpu_ct
            }
        };
    }

    let mut final_ops: u8 = 0;

    // First, adjust texBaseCT to match colortype semantics that can be inferred from swizzle for
    // alpha-only and red-only color types (hopefully creating a no-op transfer).
    {
        let mut adjusted_base = *tex_base_ct;
        if *tex_read_swizzle == Swizzle::new("000r") || *tex_read_swizzle == Swizzle::new("0001") {
            // Red -> Alpha so shift the texture's "base" colortype to be the its alpha type
            match adjusted_base {
                ColorType::R8UNorm => adjusted_base = ColorType::Alpha8,
                ColorType::R16UNorm => adjusted_base = ColorType::A16UNorm,
                ColorType::R16Float => adjusted_base = ColorType::A16Float,
                _ => {} // Go through regular RP + swizzle flow
            }
        } else if (*tex_read_swizzle == Swizzle::new("rrra")
            || *tex_read_swizzle == Swizzle::new("rrr1"))
            && adjusted_base == ColorType::R8UNorm
        {
            // Red -> Gray so shift to kGray.
            adjusted_base = ColorType::Gray8;
        }

        if adjusted_base != *tex_base_ct {
            *tex_base_ct = adjusted_base;
            // Must preserve any forced opacity in the swizzle
            if color_type_channel_flags(adjusted_base).bits() & ColorChannelFlag::ALPHA.bits() != 0
                && swz(*tex_read_swizzle, 3) == '1'
            {
                *tex_read_swizzle = Swizzle::rgb1();
            } else {
                *tex_read_swizzle = Swizzle::rgba();
            }
        }
    }

    // Second, remove normalized floating point semantics, since historically we don't enforce
    // them during transfers (for better or worse).
    {
        if *src_ct!() == ColorType::RGBAF16Norm {
            *src_ct!() = ColorType::RGBAF16;
        }
        if *dst_ct!() == ColorType::RGBAF16Norm {
            *dst_ct!() = ColorType::RGBAF16;
        }
    }

    // Third, discard colorspace conversions if there's no RGB data that would be adjusted
    {
        if cs_steps.is_needed()
            && (color_type_is_alpha_only(*src_ct!()) || color_type_is_alpha_only(*dst_ct!()))
        {
            *cs_steps = ColorSpaceXformSteps::default();
        }
    }

    // Fourth, switch swizzle components back to their default if the format doesn't have them
    {
        let tex_channels = color_type_channel_flags(*tex_base_ct).bits();
        let c = [
            swz(*tex_read_swizzle, 0),
            swz(*tex_read_swizzle, 1),
            swz(*tex_read_swizzle, 2),
            swz(*tex_read_swizzle, 3),
        ];
        let has = |flag: ColorChannelFlag| tex_channels & flag.bits() != 0;
        *tex_read_swizzle = Swizzle::from_chars(
            if has(ColorChannelFlag::RED) {
                c[0]
            } else {
                'r'
            },
            if has(ColorChannelFlag::GREEN) {
                c[1]
            } else {
                'g'
            },
            if has(ColorChannelFlag::BLUE) {
                c[2]
            } else {
                'b'
            },
            if has(ColorChannelFlag::ALPHA) {
                c[3]
            } else {
                'a'
            },
        );
    }

    // Fifth, if the channels are disjoint between the src and the dst, then the dst values can
    // ignore the source, at which point we reset everything else to the identity and flag what
    // value is required by the dst (1 if the dst is alpha, 0 if it's RGB/gray).
    {
        let cpu_channels = gray_adjusted_channels(*cpu_ct);
        let gpu_channels =
            swizzle_adjusted_channels(gray_adjusted_channels(*tex_base_ct), *tex_read_swizzle);

        if gpu_channels & cpu_channels == 0 {
            final_ops |= IGNORE_SRC;
            if xfer_ops.contains(FormatXferOp::DROP_ALPHA) {
                // We're skipping the final xfer op handling, but we still have to make sure to
                // drop the alpha channel for 3-channel formats
                if texture_is_dst {
                    final_ops |= DROP_ALPHA;
                } else {
                    // Normally this would be kPadAlpha, but we're ignoring the src texture data,
                    // so instead it can just use kForceOpaque
                    final_ops |= FORCE_OPAQUE;
                }
            } else if !color_type_is_always_opaque(*dst_ct!()) {
                final_ops |= FORCE_OPAQUE;
            }

            // Disable everything else for the transfer
            *tex_read_swizzle = Swizzle::rgba();
            let dst = *dst_ct!();
            *src_ct!() = dst;
            *cs_steps = ColorSpaceXformSteps::default();

            return (final_ops, /* compute_luminance= */ false);
        }
    }

    // Sixth, handle masking any unknown alpha bits if the dst doesn't already mask them. This must
    // happen *after* checking for channel overlap to optimize to the kIgnoreSrc case.
    {
        let cpu_masks_alpha = rgbx_to_rgba(cpu_ct);
        let gpu_masks_alpha = rgbx_to_rgba(tex_base_ct) || swz(*tex_read_swizzle, 3) == '1';
        let src_has_junk_alpha = if texture_is_dst {
            cpu_masks_alpha
        } else {
            gpu_masks_alpha
        };
        let dst_stores_alpha =
            (color_type_channel_flags(*dst_ct!()).bits() & ColorChannelFlag::ALPHA.bits() != 0)
                && !(if texture_is_dst {
                    gpu_masks_alpha
                } else {
                    cpu_masks_alpha
                });

        if src_has_junk_alpha {
            cs_steps.flags.premul = false;
            cs_steps.flags.unpremul = false;
        }

        if dst_stores_alpha && src_has_junk_alpha {
            final_ops |= FORCE_OPAQUE;
        }

        // Reset any opacity forcing in the swizzle since it's handled by kForceOpaque.
        if swz(*tex_read_swizzle, 3) == '1' {
            *tex_read_swizzle = Swizzle::from_chars(
                swz(*tex_read_swizzle, 0),
                swz(*tex_read_swizzle, 1),
                swz(*tex_read_swizzle, 2),
                'a',
            );
        }
    }

    // Seventh, lift gray/luminance calculation out of colortype so that its placement in the
    // raster pipeline ops list can be controlled (vs. attached to a store).
    let src = *src_ct!();
    let compute_luminance = account_for_luminance(src, cs_steps.is_needed(), dst_ct!());

    // Eighth, consolidate red/blue swaps present in colortype, swizzle, and xferOps into just ops
    {
        // Any swaps from the texture's base color type, swizzle, and xfer ops can always be
        // combined since those operations are grouped together, regardless of `TextureIsDst`.
        let mut num_rb_swaps = 0;
        if swz(*tex_read_swizzle, 0) == 'b' && swz(*tex_read_swizzle, 2) == 'r' {
            // Remove the swap in the swizzle (moving towards a no-op swizzle).
            *tex_read_swizzle = Swizzle::concat(tex_read_swizzle, &Swizzle::bgra());
            num_rb_swaps += 1;
        }
        if bgra_to_rgba(tex_base_ct) {
            num_rb_swaps += 1;
        }
        if xfer_ops.contains(FormatXferOp::SWAP_RB) {
            num_rb_swaps += 1;
        }

        // If there is not any RGB-dependent calculation between CPU and GPU data, we can also
        // consolidate the swap from cpu color type.
        #[allow(clippy::nonminimal_bool)] // Skia's expression, kept as written
        let no_rgb_dependent_calculation = !compute_luminance
            && !(cs_steps.flags.gamut_transform
                || cs_steps.flags.src_ootf
                || cs_steps.flags.dst_ootf);
        if no_rgb_dependent_calculation {
            let mut swapped_ct = *cpu_ct;
            if bgra_to_rgba(&mut swapped_ct) && swapped_ct == *tex_base_ct {
                num_rb_swaps += 1;
                *cpu_ct = swapped_ct;
            }
        }

        // An even number of swaps is a no-op; an odd number of swaps is the same as one swap. The
        // swap can be skipped if the source data is known to have the same values in R and B, or
        // if R and B will be discarded.
        let src = *src_ct!();
        let src_has_rb_data = src != ColorType::Gray8 && !color_type_is_alpha_only(src);
        let dst_keeps_rb_data = !color_type_is_alpha_only(*dst_ct!()) || compute_luminance;
        if (num_rb_swaps & 1) != 0 && src_has_rb_data && dst_keeps_rb_data {
            final_ops |= SWAP_RB;
        }
    }

    if xfer_ops.contains(FormatXferOp::DROP_ALPHA) {
        if texture_is_dst {
            // On CPU->GPU conversion, FormatXferOp::kDropAlpha actually drops the alpha bits
            final_ops |= DROP_ALPHA;
        } else {
            final_ops |= PAD_ALPHA;
        }
        // Remove kForceOpaque as it's redundant with kDropAlpha/kPadAlpha
        final_ops &= !FORCE_OPAQUE;
    }

    (final_ops, compute_luminance)
}

/// One `RPModifier` of `RPOps::Make`, in the order it is applied.
#[derive(Clone, Copy, Debug)]
enum RpModifier {
    // SkColorSpaceXformSteps (copied into the arena in Skia).
    Steps(ColorSpaceXformSteps),
    // BT709Luminance.
    Luminance(bool),
    // Swizzle (with any kSwapRB / kForceOpaque folded in).
    Swizzle(Swizzle),
}

impl RpModifier {
    // `explicit operator bool` of each modifier.
    fn is_enabled(&self) -> bool {
        match self {
            Self::Steps(steps) => steps.is_needed(),
            Self::Luminance(enabled) => *enabled,
            Self::Swizzle(swizzle) => *swizzle != Swizzle::rgba(),
        }
    }

    // `isOnlyPremul` in RPOps::Make.
    fn is_only_premul(&self) -> bool {
        match self {
            Self::Steps(steps) => {
                let flags = steps.flags;
                flags.premul
                    && !(flags.unpremul || flags.linearize || flags.gamut_transform || flags.encode)
            }
            _ => !self.is_enabled(),
        }
    }

    // `apply(SkRasterPipeline*)`.
    fn apply<'a>(&self, rp: &mut RasterPipeline<'a>, alloc: &'a ArenaAlloc) {
        match self {
            Self::Steps(steps) => steps.apply_to_pipeline(rp, alloc),
            // Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L397-L401 (chrome/m156)
            Self::Luminance(enabled) => {
                if *enabled {
                    rp.append(Stage::Bt709LuminanceOrLumaToAlpha);
                } // else no op needed
            }
            Self::Swizzle(swizzle) => swizzle.apply(rp),
        }
    }
}

/// `SwizzlerFn` (`SkOpts::Swizzle_8888_u32`).
type SwizzlerFn = fn(&mut [u8], &[u8], usize);

/// `TextureFormatXferFn::RPOps`: the color type conversion step.
#[derive(Debug)]
struct RpOps {
    src_color_type: ColorType,
    dst_color_type: ColorType,
    // Empty when the swizzler handles the conversion (`fRP.empty()`).
    modifiers: Vec<RpModifier>,
    // exclusive to the raster pipeline if this matches a case in SkSwizzler
    swizzler: Option<SwizzlerFn>,
    src_bpp: usize,
    dst_bpp: usize,
}

impl RpOps {
    /// `RPOps::Make(srcCT, dstCT, xferOps, rpModifiers...)`: `None` for an identity conversion.
    // Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L751-L829 (chrome/m156)
    fn make(
        src_color_type: ColorType,
        dst_color_type: ColorType,
        xfer_ops: &mut u8,
        rp_modifiers: &[RpModifier],
    ) -> Option<Arc<RpOps>> {
        if src_color_type == dst_color_type && rp_modifiers.iter().all(|m| !m.is_enabled()) {
            return None; // Identity conversion
        }

        // Luminance has to be calculated before the texture's swizzle so it's pulled out manually,
        // so we don't want to encounter it where the appendStore() also computes luminance.
        debug_assert_ne!(dst_color_type, ColorType::Gray8);

        let mut ops = RpOps {
            src_color_type,
            dst_color_type,
            modifiers: Vec::new(),
            swizzler: None,
            src_bpp: src_color_type.bytes_per_pixel(),
            dst_bpp: dst_color_type.bytes_per_pixel(),
        };

        // Match SkConvertPixels swizzle_or_premul and delegate to SkOpts for certain combinations.
        let is_8888 = |ct: ColorType| ct == ColorType::RGBA8888 || ct == ColorType::BGRA8888;
        if is_8888(src_color_type) && is_8888(dst_color_type) {
            // 32-bit unorm8 colors that can be swizzled with a premul go to SkOpts.
            if rp_modifiers.iter().all(RpModifier::is_only_premul) {
                debug_assert!(src_color_type == dst_color_type && *xfer_ops & FORCE_OPAQUE == 0);
                if *xfer_ops & SWAP_RB != 0 {
                    ops.swizzler = Some(opts::rgba_to_bgra_premul); // swap RB and premultiply
                    *xfer_ops &= !SWAP_RB;
                } else {
                    ops.swizzler = Some(opts::rgba_to_rgba_premul); // just premultiply
                }
                return Some(Arc::new(ops));
            } // else fall through to use raster pipeline
        }

        for &modifier in rp_modifiers {
            let mut modifier = modifier;
            if let RpModifier::Swizzle(swizzle) = &mut modifier {
                // kSwapRB and kForceOpaque fold into the swizzle (grouped with the swizzle
                // modifier, given the order of modifiers passed into Make).
                if *xfer_ops & SWAP_RB != 0 {
                    *swizzle = Swizzle::concat(swizzle, &Swizzle::bgra());
                    *xfer_ops &= !SWAP_RB;
                }
                if *xfer_ops & FORCE_OPAQUE != 0 {
                    *swizzle = Swizzle::concat(swizzle, &Swizzle::rgb1());
                    *xfer_ops &= !FORCE_OPAQUE;
                }
            }
            ops.modifiers.push(modifier);
        }

        Some(Arc::new(ops))
    }

    // The pipeline Skia builds in RPOps::Make: load, the modifiers in order, store.
    // Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L797-L828 (chrome/m156)
    fn build<'a>(&self, alloc: &'a ArenaAlloc) -> RasterPipeline<'a> {
        let mut rp = RasterPipeline::new();
        rp.append_load(self.src_color_type, MemoryCtx::new(MemSlot(0)));
        for modifier in &self.modifiers {
            modifier.apply(&mut rp, alloc);
        }
        rp.append_store(self.dst_color_type, MemoryCtx::new(MemSlot(1)));
        rp
    }

    // `fRP.empty()`.
    fn rp_is_empty(&self) -> bool {
        self.swizzler.is_some()
    }
}

/// Converts pixel data between a color type and a texture format's layout.
#[doc(alias = "skgpu::graphite::TextureFormatXferFn")]
#[derive(Clone, Debug)]
pub struct TextureFormatXferFn {
    // Whichever of pre_ops or post_ops is non-identity determines the side of the conversion for
    // which `format` defines the raw data format.
    format: TextureFormat,

    // 1. Bit manipulations to convert raw data to src ct
    pre_ops: u8,
    // 2. Raster pipeline to convert to dst ct
    rp: Option<Arc<RpOps>>,
    // 3. Bit manipulations to convert dst ct to raw data
    post_ops: u8,
}

/// How [`TextureFormatXferFn::run`] walks the rows.
struct RowPlan {
    row_invoke_count: usize,
    width: usize,
    height: usize,
    src_stride: isize,
    dst_stride: isize,
}

impl TextureFormatXferFn {
    // Port of: src/gpu/graphite/TextureFormatXferFn.h#L103-L110 (chrome/m156)
    fn new(format: TextureFormat, pre_ops: u8, rp: Option<Arc<RpOps>>, post_ops: u8) -> Self {
        // At least one direction should not add extra conversion operations
        debug_assert!(pre_ops == 0 || post_ops == 0);
        Self {
            format,
            pre_ops,
            rp,
            post_ops,
        }
    }

    /// `MakeCpuToGpu`: converts CPU data of `src_ct` to the bit layout of `dst_format`, applying
    /// the color space and alpha type conversion between loading and writing.
    // Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L681-L705 (chrome/m156)
    #[doc(alias = "MakeCpuToGpu")]
    #[must_use]
    pub fn make_cpu_to_gpu(
        mut src_ct: ColorType,
        cs_steps: &ColorSpaceXformSteps,
        dst_format: TextureFormat,
        mut dst_read_swizzle: Swizzle,
    ) -> Option<Self> {
        let (mut base_ct, xfer_ops) = texture_format_color_type_info(dst_format);
        if xfer_ops.contains(FormatXferOp::DISABLED) {
            return None;
        }

        let mut cs_steps_optimized = *cs_steps;
        let (mut post_ops, luminance) = optimize_transfer(
            /* texture_is_dst= */ true,
            &mut src_ct,
            &mut base_ct,
            &mut dst_read_swizzle,
            &mut cs_steps_optimized,
            xfer_ops,
        );

        // The CPU -> GPU transform is:
        //  SkRP{load(srcCT) -> csSteps? -> luminance? -> srcToDst(dstReadSwizzle^-1)? ->
        //       store(baseCT)}? -> postOps(baseCT->TF)?
        let rp = RpOps::make(
            src_ct,
            base_ct,
            &mut post_ops,
            &[
                RpModifier::Steps(cs_steps_optimized),
                RpModifier::Luminance(luminance),
                RpModifier::Swizzle(dst_read_swizzle.invert()),
            ],
        );
        Some(Self::new(dst_format, 0, rp, post_ops))
    }

    /// `MakeGpuToCpu`: converts data read back from a `src_format` texture to `dst_ct`.
    // Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L707-L737 (chrome/m156)
    #[doc(alias = "MakeGpuToCpu")]
    #[must_use]
    pub fn make_gpu_to_cpu(
        mut src_format: TextureFormat,
        mut src_read_swizzle: Swizzle,
        cs_steps: &ColorSpaceXformSteps,
        mut dst_ct: ColorType,
    ) -> Option<Self> {
        let (mut base_ct, xfer_ops) = texture_format_color_type_info(src_format);
        if xfer_ops.contains(FormatXferOp::DISABLED) {
            return None;
        }

        let mut cs_steps_optimized = *cs_steps;
        let (mut pre_ops, luminance) = optimize_transfer(
            /* texture_is_dst= */ false,
            &mut dst_ct,
            &mut base_ct,
            &mut src_read_swizzle,
            &mut cs_steps_optimized,
            xfer_ops,
        );
        if pre_ops & IGNORE_SRC != 0 {
            // The source format is ignored, so pick a format that matches `dst_ct`. Actual GPU
            // support doesn't matter; this ensures the CPU data is initialized correctly.
            src_format = preferred_texture_formats(dst_ct)[0];
        }

        // The GPU -> CPU transform is:
        //  preOps(TF->baseCT)? -> SkRP{load(baseCT) -> srcToDst(srcReadSwizzle)? -> csSteps? ->
        //                              luminance? -> store(dstCT)}?
        let rp = RpOps::make(
            base_ct,
            dst_ct,
            &mut pre_ops,
            &[
                RpModifier::Swizzle(src_read_swizzle),
                RpModifier::Steps(cs_steps_optimized),
                RpModifier::Luminance(luminance),
            ],
        );
        Some(Self::new(src_format, pre_ops, rp, 0))
    }

    /// `MakeIdentity`: moves data already in `format`'s layout (compressed formats included).
    // Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L739-L749 (chrome/m156)
    #[doc(alias = "MakeIdentity")]
    #[must_use]
    pub fn make_identity(format: TextureFormat) -> Option<Self> {
        let (_, xfer_ops) = texture_format_color_type_info(format);
        if xfer_ops.contains(FormatXferOp::DISABLED)
            && texture_format_compression_type(format) == TextureCompressionType::None
        {
            return None;
        } // else allow compressed formats through for identity conversion uploads

        Some(Self::new(format, 0, None, 0))
    }

    /// `isIdentity()`: true if the conversion is a memcpy.
    // Port of: src/gpu/graphite/TextureFormatXferFn.h#L57-L59 (chrome/m156)
    #[doc(alias = "isIdentity")]
    #[must_use]
    pub fn is_identity(&self) -> bool {
        !(self.pre_ops != 0 || self.post_ops != 0 || self.rp.is_some())
    }

    /// `usesRasterPipeline()` (test utility).
    #[doc(alias = "usesRasterPipeline")]
    #[must_use]
    pub fn uses_raster_pipeline(&self) -> bool {
        self.rp.is_some()
    }

    /// `usesXferOps()` (test utility).
    #[doc(alias = "usesXferOps")]
    #[must_use]
    pub fn uses_xfer_ops(&self) -> bool {
        (self.pre_ops | self.post_ops) != 0
    }

    /// `isIgnoreSrcForceOpaque()` (test utility).
    // Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L971-L974 (chrome/m156)
    #[doc(alias = "isIgnoreSrcForceOpaque")]
    #[must_use]
    pub fn is_ignore_src_force_opaque(&self) -> bool {
        let ops = self.pre_ops | self.post_ops;
        ops == (IGNORE_SRC | FORCE_OPAQUE)
    }

    /// `isDropOrPadAlpha()` (test utility).
    // Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L976-L979 (chrome/m156)
    #[doc(alias = "isDropOrPadAlpha")]
    #[must_use]
    pub fn is_drop_or_pad_alpha(&self) -> bool {
        let ops = self.pre_ops | self.post_ops;
        ops == PAD_ALPHA || ops == DROP_ALPHA
    }

    /// `getRowInvokeCount()`: how many times to invoke the row functions, possibly adjusting the
    /// width and height of the transfer, and the strides the raster pipeline uses.
    // Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L831-L896 (chrome/m156)
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // SkTo<int> strides
    fn get_row_invoke_count(
        &self,
        width: usize,
        height: usize,
        src_row_bytes: usize,
        dst_row_bytes: usize,
    ) -> RowPlan {
        let mut plan = RowPlan {
            row_invoke_count: 0,
            width,
            height,
            src_stride: 0,
            dst_stride: 0,
        };
        let dense_transfer;
        let width_sz = width;
        if let Some(rp) = &self.rp {
            // NOTE: kept exactly as Skia has it: a raster pipeline whose row bytes are whole
            // pixels goes row by row, and one whose row bytes are not is treated as dense.
            if
            // Mixed transfer methods are always row-by-row to minimize intermediate storage
            ((self.pre_ops | self.post_ops) != 0)
                // Swizzler has to go row by row if the src and dst strides aren't dense
                || (rp.swizzler.is_some()
                    && (width_sz * rp.src_bpp != src_row_bytes
                        || width_sz * rp.dst_bpp != dst_row_bytes))
                // RasterPipeline operates in pixel units so its built-in row handling can be used
                // if the src and dst strides are multiples of the pixel sizes
                || (rp.swizzler.is_none()
                    && src_row_bytes.is_multiple_of(rp.src_bpp)
                    && dst_row_bytes.is_multiple_of(rp.dst_bpp))
            {
                // Set the strides to 0 when the control loop will be handling the src/dst pointers
                plan.src_stride = 0;
                plan.dst_stride = 0;
                dense_transfer = false;
            } else {
                // RasterPipeline or the Swizzler function can be invoked just once
                plan.src_stride = (src_row_bytes / rp.src_bpp) as i32 as isize;
                plan.dst_stride = (dst_row_bytes / rp.dst_bpp) as i32 as isize;
                dense_transfer = true;
            }
        } else {
            // There's only extended xfer ops, so the src and dst bpp's can be derived from the
            // format.
            let bpp = texture_format_bytes_per_block(self.format).cast_unsigned() as usize;
            if self.pre_ops & PAD_ALPHA != 0 {
                let src_stride = width_sz * bpp;
                let dst_stride = width_sz * (bpp + bpp / 3);
                dense_transfer = src_stride == src_row_bytes && dst_stride == dst_row_bytes;
            } else if self.post_ops & DROP_ALPHA != 0 {
                let src_stride = width_sz * (bpp + bpp / 3);
                let dst_stride = width_sz * bpp;
                dense_transfer = src_stride == src_row_bytes && dst_stride == dst_row_bytes;
            } else {
                let stride = width_sz * bpp;
                dense_transfer = stride == src_row_bytes && stride == dst_row_bytes;
            }
        }

        if dense_transfer {
            if self.rp.as_ref().is_some_and(|rp| !rp.rp_is_empty()) {
                // RasterPipeline manages its own dense transfer and needs to see the original width
                // and height values, since it uses the strides to adjust pointers
                plan.row_invoke_count = 1; // one RP invocation over width x height pixels
                return plan;
            }
            // When not using the raster pipeline, the dense transfer happens by treating the data
            // as a single row that is width*height long, if that still fits into an int.
            debug_assert!(self.rp.as_ref().is_none_or(|rp| rp.swizzler.is_some()));
            if (i32::MAX as usize) / height > width {
                plan.width = width * height;
                plan.height = 1;
                plan.row_invoke_count = 1; // one "width*height" invocation
                return plan;
            } // otherwise fall through to do a row-by-row transfer
        }

        // If we're here, it can't be dense, so the transfer will be row-by-row
        plan.row_invoke_count = height;
        plan.height = 1;
        plan
    }

    /// `run()`: applies the transfer to a `width` x `height` block of pixels in `src`, writing to
    /// `dst`; rows advance by `src_row_bytes` and `dst_row_bytes`. For compressed formats, the
    /// width and height are in blocks.
    ///
    /// # Panics
    /// If `src` or `dst` are too small for the transfer.
    // Port of: src/gpu/graphite/TextureFormatXferFn.cpp#L898-L969 (chrome/m156)
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // pixel strides
    pub fn run(
        &self,
        width: usize,
        height: usize,
        src: &[u8],
        src_row_bytes: usize,
        dst: &mut [u8],
        dst_row_bytes: usize,
    ) {
        debug_assert!(width >= 1 && height >= 1);
        let plan = self.get_row_invoke_count(width, height, src_row_bytes, dst_row_bytes);
        let width = plan.width;
        let height = plan.height;

        let mut temp_row_storage: Vec<u8> = Vec::new(); // empty if not mixing transfer methods
        let alloc = ArenaAlloc::new();
        let mut row_fns: Vec<RowFn<'_>> = Vec::with_capacity(2); // At most 2 actions per row
        if self.pre_ops != 0 {
            // `src` is definitively the texture
            if let Some(rp) = &self.rp {
                // A temporary buffer equal to srcBpp*width holds the output of the preOps, used
                // as the source of data for SkRasterPipeline (executed per row).
                temp_row_storage.resize(rp.src_bpp * width, 0);
            }
            row_fns.push(RowFn::Xfer(get_xfer_row_fn(self.format, self.pre_ops)));
        }

        if let Some(rp) = &self.rp {
            if let Some(swizzler) = rp.swizzler {
                row_fns.push(RowFn::Swizzler(swizzler));
            } else {
                row_fns.push(RowFn::Pipeline {
                    pipeline: rp.build(&alloc),
                    height,
                    src_stride: plan.src_stride,
                    dst_stride: plan.dst_stride,
                });
            }
        }

        if self.post_ops != 0 {
            // `dst` is definitively the texture
            if let Some(rp) = &self.rp {
                // A temporary buffer equal to dstBpp*width holds the output of the
                // SkRasterPipeline conversion, used as the input to postOps (executed per row).
                temp_row_storage.resize(rp.dst_bpp * width, 0);
            }
            row_fns.push(RowFn::Xfer(get_xfer_row_fn(self.format, self.post_ops)));
        }

        if row_fns.is_empty() {
            // Identity conversion function still needs to move the data
            let bpp = texture_format_bytes_per_block(self.format).cast_unsigned() as usize;
            row_fns.push(RowFn::Memcpy(bpp));
        }

        for y in 0..plan.row_invoke_count {
            // Always start by processing `src`
            let input = &src[y * src_row_bytes..];
            let output = &mut dst[y * dst_row_bytes..];
            match row_fns.as_slice() {
                [only] => only.call(input, output, width),
                [first, second] => {
                    // The first writes the temporary row, which is the input of the second
                    first.call(input, &mut temp_row_storage, width);
                    second.call(&temp_row_storage, output, width);
                }
                _ => unreachable!("at most 2 actions per row"),
            }
        }
    }
}

/// One per-row action of [`TextureFormatXferFn::run`].
enum RowFn<'a> {
    Xfer(XferRowFn),
    Swizzler(SwizzlerFn),
    Pipeline {
        pipeline: RasterPipeline<'a>,
        height: usize,
        src_stride: isize,
        dst_stride: isize,
    },
    Memcpy(usize),
}

impl RowFn<'_> {
    fn call(&self, src: &[u8], dst: &mut [u8], width: usize) {
        match self {
            Self::Xfer(f) => f(src, dst, width),
            Self::Swizzler(f) => f(dst, src, width),
            Self::Pipeline {
                pipeline,
                height,
                src_stride,
                dst_stride,
            } => {
                // NOTE: When height != 1, this invocation actually processes the entire image.
                // Otherwise src and dst have been offset by y.
                let mut mem = MemoryBindings::new()
                    .with(MemSlot(0), MemView::read(src).with_stride(*src_stride))
                    .with(MemSlot(1), MemView::write(dst).with_stride(*dst_stride));
                pipeline.run(0, 0, width, *height, &mut mem);
            }
            Self::Memcpy(bpp) => dst[..bpp * width].copy_from_slice(&src[..bpp * width]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opaque_steps() -> ColorSpaceXformSteps {
        ColorSpaceXformSteps::default()
    }

    #[test]
    fn identity_and_disabled() {
        assert!(
            TextureFormatXferFn::make_identity(TF::RGBA8)
                .unwrap()
                .is_identity()
        );
        assert!(TextureFormatXferFn::make_identity(TF::RGBA8_BC1).is_some());
        assert!(TextureFormatXferFn::make_identity(TF::D16).is_none());
        let rgba = TextureFormatXferFn::make_cpu_to_gpu(
            ColorType::RGBA8888,
            &opaque_steps(),
            TF::RGBA8,
            Swizzle::rgba(),
        )
        .unwrap();
        assert!(rgba.is_identity());
    }

    #[test]
    fn rgbx_drops_alpha_into_rgb8() {
        let f = TextureFormatXferFn::make_cpu_to_gpu(
            ColorType::RGB888x,
            &opaque_steps(),
            TF::RGB8,
            Swizzle::rgba(),
        )
        .unwrap();
        assert!(f.is_drop_or_pad_alpha());
        // Five pixels: one full 4-pixel vector and a tail.
        let src: Vec<u8> = (0..20).collect();
        let mut dst = vec![0u8; 15];
        f.run(5, 1, &src, 20, &mut dst, 15);
        assert_eq!(dst, [0, 1, 2, 4, 5, 6, 8, 9, 10, 12, 13, 14, 16, 17, 18]);
    }

    #[test]
    fn bgr8_readback_pads_and_swaps() {
        let f = TextureFormatXferFn::make_gpu_to_cpu(
            TF::BGR8,
            Swizzle::rgba(),
            &opaque_steps(),
            ColorType::RGB888x,
        )
        .unwrap();
        assert!(!f.uses_raster_pipeline());
        let src = [1u8, 2, 3, 4, 5, 6];
        let mut dst = vec![0u8; 8];
        f.run(2, 1, &src, 6, &mut dst, 8);
        assert_eq!(dst, [3, 2, 1, 0xFF, 6, 5, 4, 0xFF]);
    }

    #[test]
    fn packed_565_swap() {
        let f = TextureFormatXferFn::make_cpu_to_gpu(
            ColorType::RGB565,
            &opaque_steps(),
            TF::R5_G6_B5,
            Swizzle::rgba(),
        )
        .unwrap();
        let px: u16 = 0xF801; // R5 = 0b11111, G6 = 0, B5 = 1
        let src = px.to_ne_bytes();
        let mut dst = [0u8; 2];
        f.run(1, 1, &src, 2, &mut dst, 2);
        assert_eq!(u16::from_ne_bytes(dst), 0x081F);
    }

    #[test]
    fn f16_upload_uses_the_raster_pipeline() {
        let f = TextureFormatXferFn::make_cpu_to_gpu(
            ColorType::RGBAF16,
            &opaque_steps(),
            TF::RGBA8,
            Swizzle::rgba(),
        )
        .unwrap();
        assert!(f.uses_raster_pipeline());
        assert!(!f.uses_xfer_ops());
        // Two rows of one pixel each, with padding in the source rows.
        let px: [u16; 4] = [HALF_1, 0x3800, 0, HALF_1]; // 1, 0.5, 0, 1
        let mut src = Vec::new();
        for _ in 0..2 {
            for c in px {
                src.extend_from_slice(&c.to_ne_bytes());
            }
            src.extend_from_slice(&[0; 8]);
        }
        let mut dst = [0u8; 8];
        f.run(1, 2, &src, 16, &mut dst, 4);
        assert_eq!(dst, [255, 128, 0, 255, 255, 128, 0, 255]);
    }

    #[test]
    fn disjoint_channels_ignore_the_source() {
        // Reading an RG8 texture into Alpha8: the channels are disjoint, so the result is opaque
        // regardless of the source.
        let f = TextureFormatXferFn::make_gpu_to_cpu(
            TF::RG8,
            Swizzle::rgba(),
            &opaque_steps(),
            ColorType::Alpha8,
        )
        .unwrap();
        assert!(f.is_ignore_src_force_opaque());
        assert!(!f.uses_raster_pipeline());
        let src = [7u8; 10];
        let mut dst = [0u8; 5];
        f.run(5, 1, &src, 10, &mut dst, 5);
        assert_eq!(dst, [0xFF; 5]);
    }
}
