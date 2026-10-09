// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ResourceTypes.h

//! Graphite's resource types (`ResourceTypes.h`).
//!
//! `BindBufferInfo` (which points at a `Buffer`) comes with the buffer managers.

use bitflags::bitflags;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;

use crate::graphite::graphite_types::SampleCount;

/// `SamplesToKey`: the 3-bit key for a sample count (1, 2, 4, 8, 16 → 0..=4).
// Port of: src/gpu/graphite/ResourceTypes.h#L34-L48 (chrome/m156)
#[doc(alias = "SamplesToKey")]
#[must_use]
pub const fn samples_to_key(num_samples: SampleCount) -> u32 {
    match num_samples {
        SampleCount::One => 0,
        SampleCount::Two => 1,
        SampleCount::Four => 2,
        SampleCount::Eight => 3,
        SampleCount::Sixteen => 4,
    }
}

/// `KeyToSamples`: the inverse of [`samples_to_key`].
// Port of: src/gpu/graphite/ResourceTypes.h#L49-L52 (chrome/m156)
#[doc(alias = "KeyToSamples")]
#[must_use]
pub const fn key_to_samples(key_bits: u32) -> SampleCount {
    debug_assert!(key_bits <= 4);
    match key_bits {
        0 => SampleCount::One,
        1 => SampleCount::Two,
        2 => SampleCount::Four,
        3 => SampleCount::Eight,
        _ => SampleCount::Sixteen,
    }
}

/// `kNumSampleKeyBits`.
// Port of: src/gpu/graphite/ResourceTypes.h#L53 (chrome/m156)
pub const NUM_SAMPLE_KEY_BITS: u32 = 3;

/// The strategy that a renderpass and/or pipeline use to access the current dst pixel when
/// blending.
// Port of: src/gpu/graphite/ResourceTypes.h#L58-L67 (chrome/m156)
#[doc(alias = "skgpu::graphite::DstReadStrategy")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DstReadStrategy {
    /// No dst read.
    #[doc(alias = "kNoneRequired")]
    NoneRequired,
    /// Copy the dst into a texture first.
    #[doc(alias = "kTextureCopy")]
    TextureCopy,
    /// Sample the dst texture directly.
    #[doc(alias = "kTextureSample")]
    TextureSample,
    /// Read the dst as a render-pass input.
    #[doc(alias = "kReadFromInput")]
    ReadFromInput,
    /// Framebuffer fetch.
    #[doc(alias = "kFramebufferFetch")]
    FramebufferFetch,
}

impl DstReadStrategy {
    /// `kLast`.
    pub const LAST: Self = Self::FramebufferFetch;
}

/// `kDstReadStrategyCount`.
pub const DST_READ_STRATEGY_COUNT: usize = DstReadStrategy::LAST as usize + 1;

/// The load operation used when a render pass begins.
// Port of: src/gpu/graphite/ResourceTypes.h#L72-L79 (chrome/m156)
#[doc(alias = "skgpu::graphite::LoadOp")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LoadOp {
    /// Load the existing contents.
    #[doc(alias = "kLoad")]
    Load,
    /// Clear.
    #[doc(alias = "kClear")]
    Clear,
    /// Contents are undefined.
    #[doc(alias = "kDiscard")]
    Discard,
}

/// `kLoadOpCount`.
pub const LOAD_OP_COUNT: usize = LoadOp::Discard as usize + 1;

/// The store operation used when a render pass ends.
// Port of: src/gpu/graphite/ResourceTypes.h#L84-L90 (chrome/m156)
#[doc(alias = "skgpu::graphite::StoreOp")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StoreOp {
    /// Store the results.
    #[doc(alias = "kStore")]
    Store,
    /// Discard the results.
    #[doc(alias = "kDiscard")]
    Discard,
}

/// `kStoreOpCount`.
pub const STORE_OP_COUNT: usize = StoreOp::Discard as usize + 1;

/// What a GPU buffer will be used for.
// Port of: src/gpu/graphite/ResourceTypes.h#L95-L111 (chrome/m156)
#[doc(alias = "skgpu::graphite::BufferType")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum BufferType {
    /// Vertex data.
    #[doc(alias = "kVertex")]
    Vertex,
    /// Index data.
    #[doc(alias = "kIndex")]
    Index,
    /// CPU → GPU transfer.
    #[doc(alias = "kXferCpuToGpu")]
    XferCpuToGpu,
    /// GPU → CPU transfer.
    #[doc(alias = "kXferGpuToCpu")]
    XferGpuToCpu,
    /// Uniforms.
    #[doc(alias = "kUniform")]
    Uniform,
    /// Storage.
    #[doc(alias = "kStorage")]
    Storage,
    /// Query results.
    #[doc(alias = "kQuery")]
    Query,
    /// Indirect draw/dispatch arguments (GPU only).
    #[doc(alias = "kIndirect")]
    Indirect,
    /// Vertex data written by compute (GPU only).
    #[doc(alias = "kVertexStorage")]
    VertexStorage,
    /// Index data written by compute (GPU only).
    #[doc(alias = "kIndexStorage")]
    IndexStorage,
}

/// `kBufferTypeCount`.
pub const BUFFER_TYPE_COUNT: usize = BufferType::IndexStorage as usize + 1;

/// Data layout requirements on host-shareable buffer contents.
// Port of: src/gpu/graphite/ResourceTypes.h#L116-L123 (chrome/m156)
#[doc(alias = "skgpu::graphite::Layout")]
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Layout {
    /// The layout of an uninitialised `UniformOffsetCalculator`.
    #[default]
    #[doc(alias = "kInvalid")]
    Invalid = 0,
    /// OpenGL's std140 uniform block layout.
    #[doc(alias = "kStd140")]
    Std140,
    /// std140 with half-precision uniforms stored as 16-bit floats.
    #[doc(alias = "kStd140_F16")]
    Std140F16,
    /// std430 (shader storage buffers).
    #[doc(alias = "kStd430")]
    Std430,
    /// std430 with half-precision uniforms stored as 16-bit floats.
    #[doc(alias = "kStd430_F16")]
    Std430F16,
    /// Metal's layout (`vec3` takes the size of a `vec4`).
    #[doc(alias = "kMetal")]
    Metal,
}

impl Layout {
    /// The layout's name, for diagnostics.
    // Port of: src/gpu/graphite/ResourceTypes.h#L125-L136 (chrome/m156)
    #[doc(alias = "LayoutString")]
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Std140 => "std140",
            Self::Std140F16 => "std140-f16",
            Self::Std430 => "std430",
            Self::Std430F16 => "std430-f16",
            Self::Metal => "metal",
            Self::Invalid => "invalid",
        }
    }
}

/// The intended access pattern over resource memory (a hint for choosing the memory type).
// Port of: src/gpu/graphite/ResourceTypes.h#L144-L154 (chrome/m156)
#[doc(alias = "skgpu::graphite::AccessPattern")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AccessPattern {
    /// GPU-only memory; GPU-private memory is preferred where available.
    #[doc(alias = "kGpuOnly")]
    GpuOnly,
    /// CPU visible, e.g. for read-back or as a copy/upload source.
    #[doc(alias = "kHostVisible")]
    HostVisible,
    /// Used to debug GPU-only buffers.
    #[doc(alias = "kGpuOnlyCopySrc")]
    GpuOnlyCopySrc,
}

/// Whether a GPU buffer sub-allocation is cleared to 0 before use.
// Port of: src/gpu/graphite/ResourceTypes.h#L160-L163 (chrome/m156)
#[doc(alias = "skgpu::graphite::ClearBuffer")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClearBuffer {
    /// Not cleared.
    #[doc(alias = "kNo")]
    No = 0,
    /// Cleared.
    #[doc(alias = "kYes")]
    Yes = 1,
}

/// Must the contents of the resource be preserved after a render pass.
// Port of: src/gpu/graphite/ResourceTypes.h#L169-L172 (chrome/m156)
#[doc(alias = "skgpu::graphite::Discardable")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Discardable {
    /// Preserved.
    #[doc(alias = "kNo")]
    No = 0,
    /// May be discarded.
    #[doc(alias = "kYes")]
    Yes = 1,
}

/// Whether Graphite owns a resource or wraps a client's object.
// Port of: src/gpu/graphite/ResourceTypes.h#L174-L177 (chrome/m156)
#[doc(alias = "skgpu::graphite::Ownership")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Ownership {
    /// Created and owned by Graphite.
    #[doc(alias = "kOwned")]
    Owned,
    /// Wraps a client object.
    #[doc(alias = "kWrapped")]
    Wrapped,
}

/// Uniquely identifies the type of resource that is cached with a `GraphiteResourceKey`.
// Port of: src/gpu/graphite/ResourceTypes.h#L180 (chrome/m156)
pub type ResourceType = u32;

/// Can the resource be held by multiple users at the same time?
// Port of: src/gpu/graphite/ResourceTypes.h#L186-L190 (chrome/m156)
#[doc(alias = "skgpu::graphite::Shareable")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Shareable {
    /// Visible in the resource cache once all its usage refs are dropped.
    #[doc(alias = "kNo")]
    #[default]
    No,
    /// Visible to other recorders, but acts like `No` within a recording.
    #[doc(alias = "kScratch")]
    Scratch,
    /// Always visible in the resource cache.
    #[doc(alias = "kYes")]
    Yes,
}

/// How texture memory is arranged on the GPU.
// Port of: src/gpu/graphite/ResourceTypes.h#L211-L214 (chrome/m156)
#[doc(alias = "skgpu::graphite::Tiling")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Tiling {
    /// Driver-chosen layout.
    #[doc(alias = "kOptimal")]
    Optimal,
    /// Row-major layout.
    #[doc(alias = "kLinear")]
    Linear,
}

bitflags! {
    /// Coarse ways in which a texture can be used, or the usages a format supports.
    // Port of: src/gpu/graphite/ResourceTypes.h#L218-L229 (chrome/m156)
    #[doc(alias = "skgpu::graphite::TextureUsage")]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct TextureUsage: u8 {
        /// Can be used as a rendering attachment.
        const RENDER = 0x01;
        /// Can be rendered with MSAA-render-to-single-sampled functionality.
        const MSRTSS = 0x02;
        /// Can be sampled within a shader with linear filtering.
        const SAMPLE = 0x04;
        /// Can be copied into another texture or buffer.
        const COPY_SRC = 0x08;
        /// Can be the copy target of another texture or buffer.
        const COPY_DST = 0x10;
        /// Can be read and written to in a compute pipeline.
        const STORAGE = 0x20;
        /// Can be written to directly from host memory.
        const HOST_COPY = 0x40;
        /// Can be read in a shader (point sampling or texel fetch).
        const READ = 0x80;
    }
}

/// Immutable-sampler data a backend can attach to a [`SamplerDesc`].
// Port of: src/gpu/graphite/ResourceTypes.h#L231-L238 (chrome/m156)
#[doc(alias = "skgpu::graphite::ImmutableSamplerInfo")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ImmutableSamplerInfo {
    /// YCbCr conversion information (at most `MAX_NUM_CONVERSION_INFO_BITS` bits).
    pub non_format_ycbcr_conversion_info: u32,
    /// Known or external format numerical representation.
    pub format: u64,
}

/// Describes how a texture is sampled.
// Port of: src/gpu/graphite/ResourceTypes.h#L243-L358 (chrome/m156)
#[doc(alias = "skgpu::graphite::SamplerDesc")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SamplerDesc {
    desc: u32,
    format: u32,
    external_format_most_significant_bits: u32,
}

// SkNextLog2(n) for the small constants below.
const fn next_log2(n: u32) -> u32 {
    32 - (n - 1).leading_zeros()
}

impl SamplerDesc {
    /// `kNumTileModeBits`.
    pub const NUM_TILE_MODE_BITS: u32 = next_log2(TileMode::Decal as u32 + 1);
    /// `kNumFilterModeBits`.
    pub const NUM_FILTER_MODE_BITS: u32 = next_log2(FilterMode::Linear as u32 + 1);
    /// `kNumMipmapModeBits`.
    pub const NUM_MIPMAP_MODE_BITS: u32 = next_log2(MipmapMode::Linear as u32 + 1);
    /// `kMaxNumConversionInfoBits`.
    pub const MAX_NUM_CONVERSION_INFO_BITS: u32 =
        32 - Self::NUM_FILTER_MODE_BITS - Self::NUM_MIPMAP_MODE_BITS - Self::NUM_TILE_MODE_BITS;

    /// `kTileModeXShift`.
    pub const TILE_MODE_X_SHIFT: u32 = 0;
    /// `kTileModeYShift`.
    pub const TILE_MODE_Y_SHIFT: u32 = Self::TILE_MODE_X_SHIFT + Self::NUM_TILE_MODE_BITS;
    /// `kFilterModeShift`.
    pub const FILTER_MODE_SHIFT: u32 = Self::TILE_MODE_Y_SHIFT + Self::NUM_TILE_MODE_BITS;
    /// `kMipmapModeShift`.
    pub const MIPMAP_MODE_SHIFT: u32 = Self::FILTER_MODE_SHIFT + Self::NUM_FILTER_MODE_BITS;
    /// `kImmutableSamplerInfoShift`.
    pub const IMMUTABLE_SAMPLER_INFO_SHIFT: u32 =
        Self::MIPMAP_MODE_SHIFT + Self::NUM_MIPMAP_MODE_BITS;

    /// `kInt32sNeededKnownFormat`.
    pub const INT32S_NEEDED_KNOWN_FORMAT: usize = 2;
    /// `kInt32sNeededExternalFormat`.
    pub const INT32S_NEEDED_EXTERNAL_FORMAT: usize = 3;

    /// `SamplerDesc(samplingOptions, tileMode)`.
    // Port of: src/gpu/graphite/ResourceTypes.h#L246-L247 (chrome/m156)
    #[must_use]
    pub fn new(sampling_options: &SamplingOptions, tile_mode: TileMode) -> Self {
        Self::new_with_tile_modes(
            sampling_options,
            (tile_mode, tile_mode),
            ImmutableSamplerInfo::default(),
        )
    }

    /// `SamplerDesc(samplingOptions, tileModes, info)`.
    // Port of: src/gpu/graphite/ResourceTypes.h#L249-L271 (chrome/m156)
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // mirrors the C++ casts
    #[must_use]
    pub fn new_with_tile_modes(
        sampling_options: &SamplingOptions,
        tile_modes: (TileMode, TileMode),
        info: ImmutableSamplerInfo,
    ) -> Self {
        let desc = ((tile_modes.0 as u32) << Self::TILE_MODE_X_SHIFT)
            | ((tile_modes.1 as u32) << Self::TILE_MODE_Y_SHIFT)
            | ((sampling_options.filter as u32) << Self::FILTER_MODE_SHIFT)
            | ((sampling_options.mipmap as u32) << Self::MIPMAP_MODE_SHIFT)
            | (info.non_format_ycbcr_conversion_info << Self::IMMUTABLE_SAMPLER_INFO_SHIFT);

        // Cubic sampling is handled in a shader, with the actual texture sampled by with NN,
        // but that is what a cubic SkSamplingOptions is set to if you ignore 'cubic'.
        debug_assert!(
            !sampling_options.use_cubic
                || (sampling_options.filter == FilterMode::Nearest
                    && sampling_options.mipmap == MipmapMode::None)
        );
        debug_assert_eq!(
            info.non_format_ycbcr_conversion_info >> Self::MAX_NUM_CONVERSION_INFO_BITS,
            0
        );

        Self {
            desc,
            format: info.format as u32,
            external_format_most_significant_bits: (info.format >> 32) as u32,
        }
    }

    /// `SamplerDesc(desc, format, extFormatMSB)`.
    // Port of: src/gpu/graphite/ResourceTypes.h#L276-L279 (chrome/m156)
    #[must_use]
    pub const fn from_raw(desc: u32, format: u32, ext_format_msb: u32) -> Self {
        Self {
            desc,
            format,
            external_format_most_significant_bits: ext_format_msb,
        }
    }

    // Port of: src/gpu/graphite/ResourceTypes.h#L288-L299 (chrome/m156)
    /// `tileModeX()`.
    #[must_use]
    pub fn tile_mode_x(&self) -> TileMode {
        tile_mode_from_bits((self.desc >> Self::TILE_MODE_X_SHIFT) & 0b11)
    }

    /// `tileModeY()`.
    #[must_use]
    pub fn tile_mode_y(&self) -> TileMode {
        tile_mode_from_bits((self.desc >> Self::TILE_MODE_Y_SHIFT) & 0b11)
    }

    /// `filterMode()`.
    #[must_use]
    pub fn filter_mode(&self) -> FilterMode {
        filter_mode_from_bits((self.desc >> Self::FILTER_MODE_SHIFT) & 0b01)
    }

    /// `mipmap()`.
    #[must_use]
    pub fn mipmap(&self) -> MipmapMode {
        mipmap_mode_from_bits((self.desc >> Self::MIPMAP_MODE_SHIFT) & 0b11)
    }

    // Port of: src/gpu/graphite/ResourceTypes.h#L300-L304 (chrome/m156)
    /// `desc()`.
    #[must_use]
    pub fn desc(&self) -> u32 {
        self.desc
    }

    /// `format()`.
    #[must_use]
    pub fn format(&self) -> u32 {
        self.format
    }

    /// `externalFormatMSBs()`.
    #[doc(alias = "externalFormatMSBs")]
    #[must_use]
    pub fn external_format_msbs(&self) -> u32 {
        self.external_format_most_significant_bits
    }

    /// `isImmutable()`.
    #[must_use]
    pub fn is_immutable(&self) -> bool {
        (self.desc >> Self::IMMUTABLE_SAMPLER_INFO_SHIFT) != 0
    }

    /// `usesExternalFormat()`.
    #[must_use]
    pub fn uses_external_format(&self) -> bool {
        (self.desc >> Self::IMMUTABLE_SAMPLER_INFO_SHIFT) & 0b1 != 0
    }

    /// `samplingOptions()`: the hardware sampling options (bicubic becomes nearest).
    // Port of: src/gpu/graphite/ResourceTypes.h#L306-L313 (chrome/m156)
    #[must_use]
    pub fn sampling_options(&self) -> SamplingOptions {
        let filter = filter_mode_from_bits((self.desc >> Self::FILTER_MODE_SHIFT) & 0b01);
        let mipmap = mipmap_mode_from_bits((self.desc >> Self::MIPMAP_MODE_SHIFT) & 0b11);
        SamplingOptions::new(filter, mipmap)
    }

    /// `immutableSamplerInfo()`.
    // Port of: src/gpu/graphite/ResourceTypes.h#L315-L318 (chrome/m156)
    #[must_use]
    pub fn immutable_sampler_info(&self) -> ImmutableSamplerInfo {
        ImmutableSamplerInfo {
            non_format_ycbcr_conversion_info: self.desc() >> Self::IMMUTABLE_SAMPLER_INFO_SHIFT,
            format: (u64::from(self.external_format_msbs()) << 32) | u64::from(self.format()),
        }
    }

    /// `asSpan()`: 1, 2 or 3 words depending on whether the sampler is immutable and whether it
    /// uses an external format.
    // Port of: src/gpu/graphite/ResourceTypes.h#L320-L323 (chrome/m156)
    #[must_use]
    pub fn as_span(&self) -> Vec<u32> {
        let words = [
            self.desc,
            self.format,
            self.external_format_most_significant_bits,
        ];
        let len = 1 + usize::from(self.is_immutable()) + usize::from(self.uses_external_format());
        words[..len].to_vec()
    }
}

// static_cast<SkTileMode>(bits)
fn tile_mode_from_bits(bits: u32) -> TileMode {
    match bits {
        0 => TileMode::Clamp,
        1 => TileMode::Repeat,
        2 => TileMode::Mirror,
        _ => TileMode::Decal,
    }
}

// static_cast<SkFilterMode>(bits)
fn filter_mode_from_bits(bits: u32) -> FilterMode {
    if bits == 0 {
        FilterMode::Nearest
    } else {
        FilterMode::Linear
    }
}

// static_cast<SkMipmapMode>(bits); 3 is not a valid SkMipmapMode and never stored.
fn mipmap_mode_from_bits(bits: u32) -> MipmapMode {
    match bits {
        0 => MipmapMode::None,
        1 => MipmapMode::Nearest,
        _ => MipmapMode::Linear,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampler_desc_layout() {
        assert_eq!(SamplerDesc::TILE_MODE_Y_SHIFT, 2);
        assert_eq!(SamplerDesc::FILTER_MODE_SHIFT, 4);
        assert_eq!(SamplerDesc::MIPMAP_MODE_SHIFT, 5);
        assert_eq!(SamplerDesc::IMMUTABLE_SAMPLER_INFO_SHIFT, 7);
        assert_eq!(SamplerDesc::MAX_NUM_CONVERSION_INFO_BITS, 27);

        let opts = SamplingOptions::new(FilterMode::Linear, MipmapMode::Nearest);
        let d = SamplerDesc::new_with_tile_modes(
            &opts,
            (TileMode::Mirror, TileMode::Decal),
            ImmutableSamplerInfo::default(),
        );
        assert_eq!(d.desc(), 2 | (3 << 2) | (1 << 4) | (1 << 5));
        assert_eq!(d.tile_mode_x(), TileMode::Mirror);
        assert_eq!(d.tile_mode_y(), TileMode::Decal);
        assert_eq!(d.filter_mode(), FilterMode::Linear);
        assert_eq!(d.mipmap(), MipmapMode::Nearest);
        assert_eq!(d.as_span().len(), 1);

        let ext = SamplerDesc::new_with_tile_modes(
            &opts,
            (TileMode::Clamp, TileMode::Clamp),
            ImmutableSamplerInfo {
                non_format_ycbcr_conversion_info: 1,
                format: 0x1_0000_0002,
            },
        );
        assert_eq!(ext.as_span(), [ext.desc(), 2, 1]);
        assert_eq!(ext.immutable_sampler_info().format, 0x1_0000_0002);
    }

    #[test]
    fn sample_keys_round_trip() {
        for s in [
            SampleCount::One,
            SampleCount::Two,
            SampleCount::Four,
            SampleCount::Eight,
            SampleCount::Sixteen,
        ] {
            assert_eq!(key_to_samples(samples_to_key(s)), s);
        }
    }
}
