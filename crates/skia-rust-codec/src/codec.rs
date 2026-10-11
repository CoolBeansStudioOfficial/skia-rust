// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/codec/SkCodec.h (chrome/m156), src/codec/SkCodec.cpp#L171-L1101 (chrome/m156)
// Ported from: include/codec/SkCodec.h, src/codec/SkCodec.cpp
//
// Not ported yet in this layer: the callback path of handleFrameIndex (SkAndroidCodec), the decode
// memory budget (getImage, fDecodeBudget), YUVA
// planes, incremental decoding (startIncrementalDecode and incrementalDecode), the decoder
// registry's Register, getImage and the gainmap/HDR accessors. Those arrive with the codecs and
// the lazy-image wave that use them.

//! The codec base: a [`Codec`] wraps a stream and a decoder ([`CodecImpl`]), and does the
//! checks, colour setup and bookkeeping that every decoder shares.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::gainmap_info::GainmapInfo;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::{Matrix, ScaleToFit};
use skia_rust_core::rect::{Contains, IRect, Rect, RoundOut};
use skia_rust_core::size::ISize;
use skia_rust_core::stream::Stream;
use skia_rust_core::yuva_pixmaps::{SupportedDataTypes, YUVAPixmapInfo, YUVAPixmaps};
use skia_rust_skcms::{
    AlphaFormat, IccProfile, PixelFormat, approximately_equal_profiles, srgb_profile,
};

use crate::codec_animation::{Blend, DisposalMethod};
use crate::codec_priv::{select_xform_format, valid_alpha};
use crate::encoded_info::{Alpha, Color, EncodedInfo};
use crate::frame_holder::{Frame, FrameHolder};
use crate::sampler::{self, Sampler};

/// Port of `SkCodec::ZeroInitialized`: whether the destination was zeroed by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[doc(alias = "SkCodec::ZeroInitialized")]
pub enum ZeroInitialized {
    /// Port of `kYes_ZeroInitialized`.
    Yes,
    /// Port of `kNo_ZeroInitialized`.
    #[default]
    No,
}

/// Port of `SkCodec::SkScanlineOrder`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "SkScanlineOrder")]
pub enum ScanlineOrder {
    /// Port of `kTopDown_SkScanlineOrder`.
    TopDown,
    /// Port of `kBottomUp_SkScanlineOrder`.
    BottomUp,
}

/// Port of `SkCodec::SelectionPolicy`: which kind of image to prefer when a format holds both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[doc(alias = "SkCodec::SelectionPolicy")]
pub enum SelectionPolicy {
    /// Port of `kPreferStillImage`.
    #[default]
    PreferStillImage,
    /// Port of `kPreferAnimation`.
    PreferAnimation,
}

/// Port of `SkCodec::Result`: the outcome of a decode call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "SkCodec::Result")]
pub enum Result {
    /// Port of `kSuccess`.
    Success,
    /// Port of `kIncompleteInput`.
    IncompleteInput,
    /// Port of `kErrorInInput`.
    ErrorInInput,
    /// Port of `kInvalidConversion`.
    InvalidConversion,
    /// Port of `kInvalidScale`.
    InvalidScale,
    /// Port of `kInvalidParameters`.
    InvalidParameters,
    /// Port of `kInvalidInput`.
    InvalidInput,
    /// Port of `kCouldNotRewind`.
    CouldNotRewind,
    /// Port of `kInternalError`.
    InternalError,
    /// Port of `kUnimplemented`.
    Unimplemented,
    /// Port of `kOutOfMemory`.
    OutOfMemory,
}

impl Result {
    /// Port of `SkCodec::ResultToString`.
    // Port of: src/codec/SkCodec.cpp#L919-L946 (chrome/m156)
    #[doc(alias = "ResultToString")]
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::IncompleteInput => "incomplete input",
            Self::ErrorInInput => "error in input",
            Self::InvalidConversion => "invalid conversion",
            Self::InvalidScale => "invalid scale",
            Self::InvalidParameters => "invalid parameters",
            Self::InvalidInput => "invalid input",
            Self::CouldNotRewind => "could not rewind",
            Self::InternalError => "internal error",
            Self::Unimplemented => "unimplemented",
            Self::OutOfMemory => "out of memory",
        }
    }
}

/// Port of `SkCodec::Options`, without the decode budget and the frame-prior bookkeeping that
/// animation needs (`fPriorFrame` is kept, as the frame-index checks read it).
#[derive(Debug, Clone, PartialEq, Eq)]
#[doc(alias = "SkCodec::Options")]
pub struct Options {
    /// Port of `fZeroInitialized`.
    pub zero_initialized: ZeroInitialized,
    /// Port of `fSubset`: decode only this part of the image.
    pub subset: Option<IRect>,
    /// Port of `fFrameIndex`.
    pub frame_index: i32,
    /// Port of `fPriorFrame`: [`NO_FRAME`] when there is none.
    pub prior_frame: i32,
}

/// Port of `SkCodec::kNoFrame`.
pub const NO_FRAME: i32 = -1;

impl Default for Options {
    // Port of: include/codec/SkCodec.h#L336-L430 (the Options defaults)
    fn default() -> Self {
        Self {
            zero_initialized: ZeroInitialized::No,
            subset: None,
            frame_index: 0,
            prior_frame: NO_FRAME,
        }
    }
}

/// How the colour transform is applied. Port of `SkCodec::XformTime`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum XformTime {
    No,
    Palette,
    DecodeRow,
}

/// The state every codec shares. Port of the data members of `SkCodec`.
pub struct CodecBase<'a> {
    encoded_info: EncodedInfo,
    src_xform_format: Option<PixelFormat>,
    stream: Option<Box<dyn Stream + Send + 'a>>,
    needs_rewind: bool,
    origin: EncodedOrigin,
    dst_info: ImageInfo,
    options: Options,
    curr_scanline: i32,
    xform_time: XformTime,
    dst_profile: Option<IccProfile>,
    /// Whether the destination profile is the source profile itself (`fDstProfile ==
    /// fEncodedInfo.profile()` in C++). skcms skips the colour conversion when both profiles are
    /// the same object, which pointer identity decides, so the transform gets one reference for
    /// both.
    dst_is_src_profile: bool,
    dst_xform_format: Option<PixelFormat>,
    dst_xform_alpha_format: AlphaFormat,
    /// Port of `fStartedIncrementalDecode`: set by a successful start, cleared by a rewind.
    started_incremental_decode: bool,
}

impl std::fmt::Debug for CodecBase<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodecBase")
            .field("encoded_info", &self.encoded_info)
            .field("origin", &self.origin)
            .field("curr_scanline", &self.curr_scanline)
            .finish_non_exhaustive()
    }
}

impl<'a> CodecBase<'a> {
    /// Port of `SkCodec::dimensions`.
    #[must_use]
    pub fn dimensions(&self) -> ISize {
        ISize::new(self.encoded_info.width(), self.encoded_info.height())
    }

    /// Port of `SkCodec::getOrigin`, for the codec implementations.
    #[must_use]
    pub(crate) fn origin(&self) -> EncodedOrigin {
        self.origin
    }

    /// Port of `SkCodec::stream`: the stream the codec reads from.
    pub fn stream(&mut self) -> Option<&mut (dyn Stream + Send + 'a)> {
        self.stream.as_deref_mut()
    }

    /// Port of `SkCodec::getEncodedData` for the codec's own stream: the stream's data if it has
    /// some, otherwise a read of a duplicate of the stream. `None` if there is no stream, or it
    /// has neither data nor a length.
    // Port of: src/codec/SkCodec.cpp#L1087-L1100 (chrome/m156)
    pub fn encoded_data(&mut self) -> Option<Data> {
        let stream = self.stream.as_deref_mut()?;
        if let Some(data) = stream.get_data() {
            return Some(data);
        }
        let mut duplicate = stream.duplicate()?;
        if !duplicate.has_length() {
            return None;
        }
        let size = duplicate.get_length();
        Data::from_stream(&mut *duplicate, size)
    }

    /// Port of `SkCodec::rewindStream`.
    // Port of: src/codec/SkCodec.cpp#L321-L328 (chrome/m156)
    pub fn rewind_stream(&mut self) -> bool {
        match self.stream.as_deref_mut() {
            Some(stream) => stream.rewind(),
            None => true,
        }
    }

    /// Port of `SkCodec::rewindIfNeeded`: rewinds before a decode that follows another one.
    // Port of: src/codec/SkCodec.cpp#L330-L345 (chrome/m156)
    fn rewind_if_needed(&mut self, imp: &mut dyn CodecImpl) -> bool {
        let needs_rewind = self.needs_rewind;
        self.needs_rewind = true;
        if !needs_rewind {
            return true;
        }
        self.curr_scanline = -1;
        // startIncrementalDecode will need to be called before incrementalDecode.
        self.started_incremental_decode = false;
        imp.on_rewind(self)
    }

    /// Port of `SkCodec::dstInfo`.
    #[must_use]
    pub fn dst_info(&self) -> &ImageInfo {
        &self.dst_info
    }

    /// Port of `SkCodec::options`.
    #[must_use]
    pub fn options(&self) -> &Options {
        &self.options
    }

    /// Port of `SkCodec::getEncodedInfo`.
    #[must_use]
    pub fn encoded_info(&self) -> &EncodedInfo {
        &self.encoded_info
    }

    /// Port of `SkCodec::currScanline`.
    #[must_use]
    pub fn curr_scanline(&self) -> i32 {
        self.curr_scanline
    }

    /// Port of `SkCodec::colorXform`: whether a colour transform was set up.
    #[must_use]
    pub fn color_xform(&self) -> bool {
        self.xform_time != XformTime::No
    }

    /// Port of `SkCodec::xformOnDecode`.
    #[must_use]
    pub fn xform_on_decode(&self) -> bool {
        self.xform_time == XformTime::DecodeRow
    }

    /// Port of `SkCodec::applyColorXform`: converts `count` decoded pixels from the source format
    /// to the destination format.
    // Port of: src/codec/SkCodec.cpp#L892-L898 (chrome/m156)
    pub fn apply_color_xform(&self, dst: &mut [u8], src: &[u8], count: usize) {
        let (Some(src_format), Some(dst_format)) = (self.src_xform_format, self.dst_xform_format)
        else {
            return;
        };
        let src_profile = self.encoded_info.profile();
        // A destination that is the source profile is the same object, as in C++.
        let dst_profile = if self.dst_is_src_profile {
            src_profile
        } else {
            self.dst_profile.as_ref()
        };
        // Port of: SkAssertResult(skcms_Transform(...)) (src/codec/SkCodec.cpp#L895-L897)
        let transformed = skia_rust_skcms::transform(
            src,
            src_format,
            AlphaFormat::Unpremul,
            src_profile,
            dst,
            dst_format,
            self.dst_xform_alpha_format,
            dst_profile,
            count,
        );
        debug_assert!(transformed);
    }

    /// Port of `SkCodec::initializeColorXform`: sets up the destination colour format and
    /// reports whether the conversion is supported.
    // Port of: src/codec/SkCodec.cpp#L840-L890 (chrome/m156)
    fn initialize_color_xform(
        &mut self,
        imp: &dyn CodecImpl,
        dst_info: &ImageInfo,
        encoded_alpha: Alpha,
        src_is_opaque: bool,
    ) -> bool {
        self.xform_time = XformTime::No;
        let mut needs_color_xform = false;
        if imp.uses_color_xform() {
            let dst_ct = dst_info.color_type();
            if matches!(
                dst_ct,
                ColorType::RGBAF16 | ColorType::RGBA1010102 | ColorType::BGR101010xXR
            ) {
                needs_color_xform = true;
                if let Some(cs) = dst_info.color_space() {
                    self.dst_profile = Some(cs.to_profile());
                    self.dst_is_src_profile = false;
                } else {
                    // Use the source profile to avoid conversion (or sRGB when there is none). The
                    // transform gets the source profile by reference, as C++ passes the same pointer.
                    self.dst_profile = None;
                    self.dst_is_src_profile = true;
                }
            } else if let Some(cs) = dst_info.color_space() {
                let dst_profile = cs.to_profile();
                let src_profile = self
                    .encoded_info
                    .profile()
                    .unwrap_or_else(|| srgb_profile());
                if !approximately_equal_profiles(src_profile, &dst_profile) {
                    needs_color_xform = true;
                }
                self.dst_profile = Some(dst_profile);
                self.dst_is_src_profile = false;
            }
        }

        if !imp.conversion_supported(self, dst_info, src_is_opaque, needs_color_xform) {
            return false;
        }

        if needs_color_xform {
            self.xform_time = if self.encoded_info.color() != Color::Palette
                || dst_info.color_type() == ColorType::RGBAF16
            {
                XformTime::DecodeRow
            } else {
                XformTime::Palette
            };
            let Some(format) =
                select_xform_format(dst_info.color_type(), self.xform_time == XformTime::Palette)
            else {
                return false;
            };
            self.dst_xform_format = Some(format);
            self.dst_xform_alpha_format =
                if encoded_alpha == Alpha::Unpremul && dst_info.alpha_type() == AlphaType::Premul {
                    AlphaFormat::PremulAsEncoded
                } else {
                    AlphaFormat::Unpremul
                };
        }
        true
    }
}

/// The decoder-specific half of a codec. Port of the virtual methods of `SkCodec`.
///
/// Each method receives the shared [`CodecBase`] explicitly, where C++ would call `this->foo()`.
/// Defaults match `SkCodec`'s own defaults.
pub trait CodecImpl: Send {
    /// Port of `onGetEncodedFormat`.
    fn on_get_encoded_format(&self) -> EncodedImageFormat;

    /// Port of `onGetPixels`: decodes into `dst` (rows `row_bytes` apart) and reports how many
    /// rows were decoded.
    fn on_get_pixels(
        &mut self,
        base: &mut CodecBase<'_>,
        info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
        rows_decoded: &mut i32,
    ) -> Result;

    /// Port of `onQueryYUVAInfo`: the YUVA layout this image decodes to, when the codec can
    /// decode to planes (the default: it cannot).
    // Port of: include/codec/SkCodec.h#L880-L881 (chrome/m156)
    fn on_query_yuva_info(
        &self,
        _base: &CodecBase<'_>,
        _supported: &SupportedDataTypes,
    ) -> Option<YUVAPixmapInfo> {
        None
    }

    /// Port of `onGetYUVAPlanes`: decodes into the planes of `pixmaps`, which `on_query_yuva_info`
    /// laid out (the default: unimplemented).
    // Port of: include/codec/SkCodec.h#L883 (chrome/m156)
    fn on_get_yuva_planes(
        &mut self,
        _base: &mut CodecBase<'_>,
        _pixmaps: &mut YUVAPixmaps,
    ) -> Result {
        Result::Unimplemented
    }

    /// Port of `onRewind`. The default rewinds the stream.
    fn on_rewind(&mut self, base: &mut CodecBase<'_>) -> bool {
        base.rewind_stream()
    }

    /// Port of `onDimensionsSupported`: whether a scaled size is decodable. Defaults to false.
    fn on_dimensions_supported(&self, _base: &CodecBase<'_>, _dim: ISize) -> bool {
        false
    }

    /// Port of `onGetValidSubset`: adjusts `subset` to one the codec can decode, and returns
    /// whether a subset decode is supported. Defaults to false.
    fn on_get_valid_subset(&self, _base: &CodecBase<'_>, _subset: &mut IRect) -> bool {
        false
    }

    /// Port of `onGetScanlineOrder`.
    fn on_get_scanline_order(&self) -> ScanlineOrder {
        ScanlineOrder::TopDown
    }

    /// Port of `onGetScaledDimensions`: the size the codec scales to for `desired_scale`. Defaults
    /// to the codec's own dimensions (no native scaling).
    fn on_get_scaled_dimensions(&self, base: &CodecBase<'_>, _desired_scale: f32) -> ISize {
        base.dimensions()
    }

    /// Port of `getSampler`: the sampler that writes sampled rows, or `None` when the codec has no
    /// sampler yet and `create_if_necessary` is false. Defaults to `None`.
    fn on_get_sampler(
        &mut self,
        _base: &CodecBase<'_>,
        _create_if_necessary: bool,
    ) -> Option<&mut dyn Sampler> {
        None
    }

    /// Port of `usesColorXform`.
    fn uses_color_xform(&self) -> bool {
        true
    }

    /// Port of `conversionSupported`, which a decoder may override. The default is
    /// `SkCodec::conversionSupported`.
    fn conversion_supported(
        &self,
        base: &CodecBase<'_>,
        dst: &ImageInfo,
        src_is_opaque: bool,
        needs_color_xform: bool,
    ) -> bool {
        let _ = needs_color_xform;
        default_conversion_supported(base.encoded_info.color(), dst, src_is_opaque)
    }

    /// Port of `onStartScanlineDecode`. Defaults to `Unimplemented`.
    fn on_start_scanline_decode(
        &mut self,
        _base: &mut CodecBase<'_>,
        _dst_info: &ImageInfo,
        _options: &Options,
    ) -> Result {
        Result::Unimplemented
    }

    /// Port of `onGetScanlines`: decodes up to `count` rows and returns how many it produced.
    fn on_get_scanlines(
        &mut self,
        _base: &mut CodecBase<'_>,
        _dst: &mut [u8],
        _count: i32,
        _row_bytes: usize,
    ) -> i32 {
        0
    }

    /// Port of `onSkipScanlines`.
    fn on_skip_scanlines(&mut self, _base: &mut CodecBase<'_>, _count: i32) -> bool {
        false
    }

    /// Port of `onSupportsIncrementalDecode`. Defaults to false.
    fn on_supports_incremental_decode(&self, _dst: &ImageInfo) -> bool {
        false
    }

    /// Port of `onStartIncrementalDecode`. The destination is passed again on every
    /// [`CodecImpl::on_incremental_decode`] call, so an implementation keeps only the sizes and
    /// progress it needs. Defaults to `Unimplemented`.
    fn on_start_incremental_decode(
        &mut self,
        _base: &mut CodecBase<'_>,
        _dst_info: &ImageInfo,
        _dst: &mut [u8],
        _row_bytes: usize,
        _options: &Options,
    ) -> Result {
        Result::Unimplemented
    }

    /// Port of `onIncrementalDecode`: decodes as much as the input allows into `dst`, the same
    /// buffer as the start call, and reports the rows it wrote. Defaults to `Unimplemented`.
    fn on_incremental_decode(
        &mut self,
        _base: &mut CodecBase<'_>,
        _dst: &mut [u8],
        _rows_decoded: &mut i32,
    ) -> Result {
        Result::Unimplemented
    }

    /// Port of `onGetFrameCount`: the number of frames. Defaults to 1 (a still image).
    // Port of: include/codec/SkCodec.h#L967-L969 (chrome/m156)
    fn on_get_frame_count(&mut self) -> i32 {
        1
    }

    /// Port of `onGetFrameInfo`: fills `info` (when given) for frame `index`, and reports whether
    /// the frame exists. Defaults to false.
    // Port of: include/codec/SkCodec.h#L971-L973 (chrome/m156)
    fn on_get_frame_info(&self, _index: i32, _info: Option<&mut FrameInfo>) -> bool {
        false
    }

    /// Port of `onGetRepetitionCount`. Defaults to 0.
    // Port of: include/codec/SkCodec.h#L975-L977 (chrome/m156)
    fn on_get_repetition_count(&mut self) -> i32 {
        0
    }

    /// Port of `onIsAnimated`. Defaults to [`IsAnimated::No`].
    // Port of: include/codec/SkCodec.h#L979-L981 (chrome/m156)
    fn on_is_animated(&mut self) -> IsAnimated {
        IsAnimated::No
    }

    /// Port of `getFrameHolder`: the frames of an animated image. Defaults to `None`.
    // Port of: include/codec/SkCodec.h#L1046-L1048 (chrome/m156)
    fn frame_holder(&self) -> Option<&dyn FrameHolder> {
        None
    }

    /// Port of the virtual `getEncodedData`. The default reads the codec's own stream, through
    /// [`CodecBase::encoded_data`].
    // Port of: src/codec/SkCodec.cpp#L1087-L1100 (chrome/m156)
    fn on_get_encoded_data(&mut self, base: &mut CodecBase<'_>) -> Option<Data> {
        base.encoded_data()
    }

    /// Port of the virtual `onGetGainmapInfo(SkGainmapInfo*)`: whether the image has a gainmap,
    /// and its parameters when `info` is given. Formats without gainmaps answer false.
    // Port of: include/codec/SkCodec.h#L848 (chrome/m156)
    fn on_get_gainmap_info(&self, _info: Option<&mut GainmapInfo>) -> bool {
        false
    }

    /// Port of the virtual `onGetGainmapCodec(SkGainmapInfo*, std::unique_ptr<SkCodec>*)`: whether
    /// the image has a gainmap, its parameters, and, when `want_codec` is set, the codec of the
    /// gainmap image. Formats without gainmaps answer `(false, None)`.
    // Port of: include/codec/SkCodec.h#L847 (chrome/m156)
    fn on_get_gainmap_codec(
        &mut self,
        _info: Option<&mut GainmapInfo>,
        _want_codec: bool,
    ) -> (bool, Option<Codec<'static>>) {
        (false, None)
    }

    /// Port of the virtual `onGetGainmapInfo(SkGainmapInfo*, std::unique_ptr<SkStream>*)`, which
    /// only JPEG implements: the gainmap parameters, and the stream of the encoded gainmap image.
    /// Other formats answer `None`.
    // Port of: include/codec/SkCodec.h#L852 (chrome/m156)
    fn on_get_gainmap_info_stream(
        &self,
        _info: Option<&mut GainmapInfo>,
    ) -> Option<Box<skia_rust_core::stream::MemoryStream>> {
        None
    }
}

/// Port of `SkCodec::FrameInfo`: what a client needs to know about one frame of an animation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[doc(alias = "SkCodec::FrameInfo")]
pub struct FrameInfo {
    /// Port of `fRequiredFrame`: the earliest frame this one can be blended with, or
    /// [`NO_FRAME`].
    pub required_frame: i32,
    /// Port of `fDuration`: the milliseconds to show the frame.
    pub duration: i32,
    /// Port of `fFullyReceived`: whether the end of the frame is in the stream.
    pub fully_received: bool,
    /// Port of `fAlphaType`.
    pub alpha_type: AlphaType,
    /// Port of `fHasAlphaWithinBounds`.
    pub has_alpha_within_bounds: bool,
    /// Port of `fDisposalMethod`.
    pub disposal_method: DisposalMethod,
    /// Port of `fBlend`.
    pub blend: Blend,
    /// Port of `fFrameRect`: the rectangle the frame updates.
    pub frame_rect: IRect,
}

impl Default for FrameInfo {
    // Port of: include/codec/SkCodec.h (SkCodec::FrameInfo's member initializers)
    fn default() -> Self {
        Self {
            required_frame: NO_FRAME,
            duration: 0,
            fully_received: false,
            alpha_type: AlphaType::Unknown,
            has_alpha_within_bounds: false,
            disposal_method: DisposalMethod::Keep,
            blend: Blend::SrcOver,
            frame_rect: IRect::from_xywh(0, 0, 0, 0),
        }
    }
}

/// Port of `SkCodec::kRepetitionCountInfinite`.
pub const REPETITION_COUNT_INFINITE: i32 = -1;

/// Port of `SkCodec::IsAnimated`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "SkCodec::IsAnimated")]
pub enum IsAnimated {
    /// Port of `kYes`.
    Yes,
    /// Port of `kNo`.
    No,
    /// Port of `kUnknown`: more frames may still arrive.
    Unknown,
}

/// Port of `SkCodec::conversionSupported` (the default, used by the codecs that do not
/// override it).
// Port of: src/codec/SkCodec.cpp#L296-L319 (chrome/m156)
/// The frame `index` of the codec's frame holder, copied out. `None` if the codec has no frames or
/// no frame has that index.
fn frame_at(imp: &dyn CodecImpl, index: i32) -> Option<Frame> {
    imp.frame_holder()?.get_frame(index).cloned()
}

/// Port of `zero_rect`: clears the part of `dst` that `prev_rect` covers, to transparent black,
/// for a frame of a `kRestoreBGColor` required frame. When the destination is scaled, the rectangle
/// is mapped to the destination with `Rect2Rect`, as in the C++.
// Port of: src/codec/SkCodec.cpp#L356-L382 (chrome/m156)
// `SkRect::Make` takes scalars, so the frame sizes are converted to `f32` as the C++ does.
#[allow(clippy::cast_precision_loss)]
fn zero_rect(
    info: &ImageInfo,
    dst: &mut [u8],
    row_bytes: usize,
    src_dimensions: ISize,
    prev_rect: IRect,
) -> bool {
    let dimensions = info.dimensions();
    let mut prev_rect = prev_rect;
    if dimensions != src_dimensions {
        // The rectangle of the previous frame, scaled to the destination, rounded out.
        let src = Rect::from_wh(src_dimensions.width as f32, src_dimensions.height as f32);
        let dst_rect = Rect::from_wh(dimensions.width as f32, dimensions.height as f32);
        let Some(map) = Matrix::rect_2_rect(src, dst_rect, ScaleToFit::Fill) else {
            return false;
        };
        let (as_rect, mapped) = map.map_rect(Rect::from_irect(prev_rect));
        if !mapped {
            return false;
        }
        prev_rect = as_rect.round_out();
    }

    let left = prev_rect.left().max(0);
    let top = prev_rect.top().max(0);
    let right = prev_rect.right().min(dimensions.width);
    let bottom = prev_rect.bottom().min(dimensions.height);
    if left >= right || top >= bottom {
        // Nothing to zero, due to scaling or bad frame rect.
        return true;
    }
    let rect = IRect::from_ltrb(left, top, right, bottom);

    let erase_info = info.with_dimensions(rect.size());
    let bpp = info.bytes_per_pixel();
    let offset = usize::try_from(rect.x()).unwrap_or(0) * bpp
        + usize::try_from(rect.y()).unwrap_or(0) * row_bytes;
    let Some(erase_dst) = dst.get_mut(offset..) else {
        return false;
    };
    sampler::fill(
        &erase_info,
        erase_dst,
        row_bytes,
        crate::codec::ZeroInitialized::No,
    );
    true
}

fn default_conversion_supported(encoded: Color, dst: &ImageInfo, src_is_opaque: bool) -> bool {
    if !valid_alpha(dst.alpha_type(), src_is_opaque) {
        return false;
    }
    match dst.color_type() {
        ColorType::RGBA8888
        | ColorType::BGRA8888
        | ColorType::RGBAF16
        | ColorType::BGRA10101010XR => true,
        ColorType::BGR101010xXR | ColorType::RGB565 => src_is_opaque,
        ColorType::Gray8 => encoded == Color::Gray && src_is_opaque,
        ColorType::Alpha8 => encoded == Color::XAlpha,
        _ => false,
    }
}

/// A decoder for one encoded image. Port of `SkCodec`.
pub struct Codec<'a> {
    base: CodecBase<'a>,
    imp: Box<dyn CodecImpl + 'a>,
}

impl std::fmt::Debug for Codec<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Codec")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}

impl<'a> Codec<'a> {
    /// Port of the `SkCodec` constructor: a codec for `encoded`, reading from `stream`, decoded
    /// by `imp`.
    // Port of: src/codec/SkCodec.cpp#L264-L273 (chrome/m156)
    #[must_use]
    pub fn new(
        encoded: EncodedInfo,
        imp: Box<dyn CodecImpl + 'a>,
        stream: Option<Box<dyn Stream + Send + 'a>>,
        origin: EncodedOrigin,
        src_xform_format: Option<PixelFormat>,
    ) -> Self {
        Self {
            base: CodecBase {
                encoded_info: encoded,
                src_xform_format,
                stream,
                needs_rewind: false,
                origin,
                dst_info: ImageInfo::new_unknown(None),
                options: Options::default(),
                curr_scanline: -1,
                xform_time: XformTime::No,
                dst_profile: None,
                dst_is_src_profile: false,
                dst_xform_format: None,
                dst_xform_alpha_format: AlphaFormat::Unpremul,
                started_incremental_decode: false,
            },
            imp,
        }
    }

    /// Port of `SkCodec::getInfo`: the image's natural info (sRGB unless a profile says otherwise).
    #[doc(alias = "getInfo")]
    #[must_use]
    pub fn info(&self) -> ImageInfo {
        self.base.encoded_info.make_image_info()
    }

    /// Port of `SkCodec::dimensions`.
    #[must_use]
    pub fn dimensions(&self) -> ISize {
        self.base.dimensions()
    }

    /// Port of `SkCodec::bounds`.
    #[must_use]
    pub fn bounds(&self) -> IRect {
        IRect::from_wh(
            self.base.encoded_info.width(),
            self.base.encoded_info.height(),
        )
    }

    /// Port of `SkCodec::getOrigin`.
    #[must_use]
    pub fn origin(&self) -> EncodedOrigin {
        self.base.origin
    }

    /// Port of `SkCodec::getEncodedFormat`.
    #[must_use]
    pub fn encoded_format(&self) -> EncodedImageFormat {
        self.imp.on_get_encoded_format()
    }

    /// Port of `SkCodec::getEncodedInfo`.
    #[must_use]
    pub fn encoded_info(&self) -> &EncodedInfo {
        &self.base.encoded_info
    }

    /// Whether the image has a gainmap, and its parameters when `info` is given
    /// (`onGetGainmapInfo`, reached through `SkAndroidCodec::getAndroidGainmap`).
    // Port of: include/codec/SkCodec.h#L848 (chrome/m156), the caller of the virtual hook
    pub(crate) fn get_gainmap_info(&self, info: Option<&mut GainmapInfo>) -> bool {
        self.imp.on_get_gainmap_info(info)
    }

    /// Whether the image has a gainmap, its parameters, and the codec of the gainmap image when
    /// `want_codec` is set (`onGetGainmapCodec`, reached through
    /// `SkAndroidCodec::getGainmapAndroidCodec`).
    // Port of: include/codec/SkCodec.h#L847 (chrome/m156), the caller of the virtual hook
    pub(crate) fn get_gainmap_codec(
        &mut self,
        info: Option<&mut GainmapInfo>,
        want_codec: bool,
    ) -> (bool, Option<Codec<'static>>) {
        self.imp.on_get_gainmap_codec(info, want_codec)
    }

    /// The gainmap parameters and the stream of the gainmap image, when the image has a gainmap
    /// (`onGetGainmapInfo` with a stream, reached through `SkAndroidCodec::getAndroidGainmap`).
    // Port of: include/codec/SkCodec.h#L852 (chrome/m156), the caller of the virtual hook
    pub(crate) fn get_gainmap_info_stream(
        &self,
        info: Option<&mut GainmapInfo>,
    ) -> Option<Box<skia_rust_core::stream::MemoryStream>> {
        self.imp.on_get_gainmap_info_stream(info)
    }

    /// Port of `SkCodec::getScanlineOrder`.
    #[must_use]
    pub fn scanline_order(&self) -> ScanlineOrder {
        self.imp.on_get_scanline_order()
    }

    /// Port of `SkCodec::getValidSubset`: adjusts `subset` to a decodable one. Returns false if
    /// subset decoding is not supported by this codec.
    #[must_use]
    pub fn get_valid_subset(&self, subset: &mut IRect) -> bool {
        self.imp.on_get_valid_subset(&self.base, subset)
    }

    /// Port of `SkCodec::getEncodedData`: the encoded bytes the codec reads. A memory stream
    /// shares its data; any other stream with a length is read again from a duplicate of it.
    /// Returns `None` if the codec has no stream, or the stream has no data and no length.
    // Port of: src/codec/SkCodec.cpp#L1087-L1100 (chrome/m156)
    #[doc(alias = "getEncodedData")]
    pub fn encoded_data(&mut self) -> Option<Data> {
        self.imp.on_get_encoded_data(&mut self.base)
    }

    /// Whether the next decode will rewind the stream first: the state `SkCodec::fNeedsRewind`
    /// holds. The tests read it through `SkCodecPriv::needsRewind` (tests/CodecTest.cpp#L2609-L2612),
    /// a friend of `SkCodec`.
    #[doc(hidden)]
    #[must_use]
    pub fn needs_rewind(&self) -> bool {
        self.base.needs_rewind
    }

    /// Port of `SkCodec::getFrameCount`: the number of frames. A codec that reads its input
    /// incrementally may report more frames later.
    // Port of: include/codec/SkCodec.h#L670-L672 (chrome/m156)
    #[doc(alias = "getFrameCount")]
    pub fn get_frame_count(&mut self) -> i32 {
        self.imp.on_get_frame_count()
    }

    /// Port of `SkCodec::getFrameInfo(index, info)`: the description of frame `index`, or `None`
    /// if there is no such frame (or the codec does not report frames).
    // Port of: include/codec/SkCodec.h#L763-L768 (chrome/m156)
    #[doc(alias = "getFrameInfo")]
    #[must_use]
    pub fn get_frame_info(&self, index: i32) -> Option<FrameInfo> {
        if index < 0 {
            return None;
        }
        let mut info = FrameInfo::default();
        self.imp
            .on_get_frame_info(index, Some(&mut info))
            .then_some(info)
    }

    /// Port of `SkCodec::getFrameInfo()`: the description of every frame. Empty for a still
    /// image, and for a codec that does not report its frames.
    // Port of: src/codec/SkCodec.cpp#L900-L917 (chrome/m156)
    #[doc(alias = "getFrameInfo")]
    pub fn frame_infos(&mut self) -> Vec<FrameInfo> {
        let frame_count = self.get_frame_count();
        if frame_count <= 0 {
            return Vec::new();
        }
        if frame_count == 1 && !self.imp.on_get_frame_info(0, None) {
            // Not animated.
            return Vec::new();
        }
        (0..frame_count)
            .map(|i| {
                let mut info = FrameInfo::default();
                let found = self.imp.on_get_frame_info(i, Some(&mut info));
                debug_assert!(found);
                info
            })
            .collect()
    }

    /// Port of `SkCodec::getRepetitionCount`: how many times to play the animation after the
    /// first play, or [`REPETITION_COUNT_INFINITE`].
    // Port of: include/codec/SkCodec.h#L803-L805 (chrome/m156)
    #[doc(alias = "getRepetitionCount")]
    pub fn get_repetition_count(&mut self) -> i32 {
        self.imp.on_get_repetition_count()
    }

    /// Port of `SkCodec::isAnimated`.
    // Port of: include/codec/SkCodec.h#L835 (chrome/m156)
    #[doc(alias = "isAnimated")]
    pub fn is_animated(&mut self) -> IsAnimated {
        self.imp.on_is_animated()
    }

    /// Port of `SkCodec::getPixels(info, pixels, rowBytes, options)`. Decodes the image into
    /// Port of `SkCodec::queryYUVAInfo`: the YUVA layout the image decodes to, or `None` when it
    /// has no YUV planes or they do not use a data type in `supported_data_types`.
    // Port of: src/codec/SkCodec.cpp#L277-L284 (chrome/m156)
    #[doc(alias = "SkCodec::queryYUVAInfo")]
    #[must_use]
    pub fn query_yuva_info(
        &self,
        supported_data_types: &SupportedDataTypes,
    ) -> Option<YUVAPixmapInfo> {
        let info = self
            .imp
            .on_query_yuva_info(&self.base, supported_data_types)?;
        info.is_supported(supported_data_types).then_some(info)
    }

    /// Port of `SkCodec::getYUVAPlanes`: decodes into the planes of `pixmaps`, which must come
    /// from a successful [`query_yuva_info`](Self::query_yuva_info).
    // Port of: src/codec/SkCodec.cpp#L286-L294 (chrome/m156)
    #[doc(alias = "SkCodec::getYUVAPlanes")]
    pub fn get_yuva_planes(&mut self, pixmaps: &mut YUVAPixmaps) -> Result {
        if !pixmaps.is_valid() {
            return Result::InvalidInput;
        }
        if !self.base.rewind_if_needed(self.imp.as_mut()) {
            return Result::CouldNotRewind;
        }
        self.imp.on_get_yuva_planes(&mut self.base, pixmaps)
    }

    /// `dst`, which holds `info.height()` rows `row_bytes` apart.
    // Port of: src/codec/SkCodec.cpp#L491-L499 and #L502-L564 (getPixels, getPixelsBudgeted),
    // with the decode budget left out.
    pub fn get_pixels(
        &mut self,
        info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: Option<&Options>,
    ) -> Result {
        let default_options = Options::default();
        let options = options.unwrap_or(&default_options);
        if info.color_type() == ColorType::Unknown {
            return Result::InvalidConversion;
        }
        if dst.is_empty() {
            return Result::InvalidParameters;
        }
        if row_bytes < info.min_row_bytes() {
            return Result::InvalidParameters;
        }

        if let Some(subset) = options.subset {
            let mut valid = subset;
            if !self.imp.on_get_valid_subset(&self.base, &mut valid) || valid != subset {
                return Result::Unimplemented;
            }
        }

        let frame_index_result = self.handle_frame_index(info, dst, row_bytes, options);
        if frame_index_result != Result::Success {
            return frame_index_result;
        }

        if !self.dimensions_supported(info.dimensions()) {
            return Result::InvalidScale;
        }

        self.base.dst_info = info.clone();
        self.base.options = options.clone();

        let mut rows_decoded = 0;
        let result = self.imp.on_get_pixels(
            &mut self.base,
            info,
            dst,
            row_bytes,
            options,
            &mut rows_decoded,
        );

        if (result == Result::IncompleteInput || result == Result::ErrorInInput)
            && rows_decoded != info.height()
        {
            // The rest of the image is filled, so the subset no longer applies.
            self.base.options.subset = None;
            self.fill_incomplete_image(
                info,
                dst,
                row_bytes,
                options.zero_initialized,
                info.height(),
                rows_decoded,
            );
        }
        result
    }

    /// Port of `SkCodec::getImage(info, options)`: decodes the image into a new raster image,
    /// rotated upright when the codec's origin says so. Without `info`, the codec's own info is
    /// used, with width and height swapped for a rotated origin, as the C++ `getImage()` overload
    /// does. The decode memory budget is not ported.
    ///
    /// # Errors
    /// The decode result, if the pixels could not be decoded (`IncompleteInput` and
    /// `ErrorInInput` still produce an image in Skia, and so here they return `Ok`).
    // Port of: src/codec/SkCodec.cpp#L566-L615 (getImage, both overloads; chrome/m156)
    #[doc(alias = "getImage")]
    pub fn get_image<'o>(
        &mut self,
        info: impl Into<Option<ImageInfo>>,
        options: impl Into<Option<&'o Options>>,
    ) -> std::result::Result<skia_rust_core::image::Image, Result> {
        let info = info.into().unwrap_or_else(|| {
            let info = self.info();
            if self.origin().swaps_width_height() {
                info.with_wh(info.height(), info.width())
            } else {
                info
            }
        });
        let default_options = Options::default();
        let options = options.into().unwrap_or(&default_options);

        let row_bytes = info.min_row_bytes();
        let mut storage = vec![0u8; info.compute_byte_size(row_bytes)];
        let origin = self.origin();
        let mut result = Result::InternalError;
        {
            let Some(mut pixmap) =
                skia_rust_core::pixmap::Pixmap::new(&info, &mut storage, row_bytes)
            else {
                return Err(Result::InternalError);
            };
            let decoded = crate::codec_image_generator::orient_decode(
                &mut pixmap,
                origin,
                |pm: &mut skia_rust_core::pixmap::Pixmap<'_>| {
                    let pm_info = pm.info().clone();
                    let pm_row_bytes = pm.row_bytes();
                    let Some(dst) = pm.writable_addr() else {
                        return false;
                    };
                    result = self.get_pixels(&pm_info, dst, pm_row_bytes, Some(options));
                    matches!(
                        result,
                        Result::Success | Result::IncompleteInput | Result::ErrorInInput
                    )
                },
            );
            if !decoded {
                return Err(result);
            }
        }

        let mut bitmap = skia_rust_core::bitmap::Bitmap::new();
        if !bitmap.install_pixels(&info, storage, row_bytes) {
            return Err(Result::InternalError);
        }
        bitmap.set_immutable();
        skia_rust_core::images::raster_from_bitmap(&bitmap).ok_or(Result::InternalError)
    }

    /// Port of `SkCodec::handleFrameIndex`, without the `getPixelsFn` callback that
    /// `SkAndroidCodec` passes (not ported). Frame 0 sets up the colour conversion. A later frame
    /// first decodes its required frame into `dst` (when it has one), clears the rectangle of a
    /// `kRestoreBGColor` required frame, and then sets up the colour conversion for the frame.
    /// A caller with no destination passes an empty `dst`: a required frame then fails with
    /// `InvalidParameters`, as Skia's `getPixels(nullptr, ...)` does.
    // Port of: src/codec/SkCodec.cpp#L386-L476 (chrome/m156)
    pub(crate) fn handle_frame_index(
        &mut self,
        info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: &Options,
    ) -> Result {
        if !self.base.rewind_if_needed(self.imp.as_mut()) {
            return Result::CouldNotRewind;
        }

        let index = options.frame_index;
        if index == 0 {
            let alpha = self.base.encoded_info.alpha();
            let opaque = self.base.encoded_info.opaque();
            return if self
                .base
                .initialize_color_xform(self.imp.as_ref(), info, alpha, opaque)
            {
                Result::Success
            } else {
                Result::InvalidConversion
            };
        }

        if index < 0 {
            return Result::InvalidParameters;
        }
        if options.subset.is_some() {
            return Result::InvalidParameters;
        }
        if index >= self.imp.on_get_frame_count() {
            return Result::IncompleteInput;
        }

        // The frame is copied out, so that decoding the required frame can borrow `self`.
        let Some(frame) = frame_at(self.imp.as_ref(), index) else {
            return Result::InternalError;
        };

        let required_frame = frame.required_frame();
        if required_frame != NO_FRAME {
            // Decode the earlier frame if necessary.
            let prepped = if options.prior_frame == NO_FRAME {
                let mut prev_frame_options = options.clone();
                prev_frame_options.frame_index = required_frame;
                let result = self.get_pixels(info, dst, row_bytes, Some(&prev_frame_options));
                if result != Result::Success {
                    return result;
                }
                match frame_at(self.imp.as_ref(), required_frame) {
                    Some(prepped) => prepped,
                    None => return Result::InternalError,
                }
            } else {
                // Check for a valid frame as a starting point.
                if options.prior_frame < required_frame || options.prior_frame >= index {
                    return Result::InvalidParameters;
                }
                match frame_at(self.imp.as_ref(), options.prior_frame) {
                    Some(prepped) => prepped,
                    None => return Result::InternalError,
                }
            };

            match prepped.disposal_method() {
                DisposalMethod::RestorePrevious => return Result::InvalidParameters,
                DisposalMethod::RestoreBgColor => {
                    // A frame after the required frame covers the cleared rectangle, so only the
                    // required frame itself needs clearing.
                    if prepped.frame_id() == required_frame {
                        let prepped_rect = prepped.frame_rect();
                        if !zero_rect(info, dst, row_bytes, self.base.dimensions(), prepped_rect) {
                            return Result::InternalError;
                        }
                    }
                }
                DisposalMethod::Keep => {}
            }
        }

        if self.base.initialize_color_xform(
            self.imp.as_ref(),
            info,
            frame.reported_alpha(),
            !frame.has_alpha(),
        ) {
            Result::Success
        } else {
            Result::InvalidConversion
        }
    }

    /// Port of `SkCodec::dimensionsSupported`: whether `dim` is the codec's own size or a scaled
    /// size it can decode to directly.
    #[must_use]
    pub fn dimensions_supported(&self, dim: ISize) -> bool {
        dim == self.base.dimensions() || self.imp.on_dimensions_supported(&self.base, dim)
    }

    /// Port of `SkCodec::getScaledDimensions`: the size the codec suggests for `desired_scale`. Only
    /// downscales are native; a scale of one or more returns the codec's own dimensions.
    // Port of: include/codec/SkCodec.h#L276-L289 (chrome/m156)
    #[must_use]
    pub fn get_scaled_dimensions(&self, desired_scale: f32) -> ISize {
        // Negative and zero scales are errors.
        if desired_scale <= 0.0 {
            return ISize::new(0, 0);
        }
        // Upscaling is not supported. Return the original size if the client requests an upscale.
        if desired_scale >= 1.0 {
            return self.base.dimensions();
        }
        self.imp.on_get_scaled_dimensions(&self.base, desired_scale)
    }

    /// Port of `SkCodec::getSampler`: the sampler of the current decode, created on demand when
    /// `create_if_necessary` is true.
    // Port of: include/codec/SkCodec.h#L1120 (getSampler)
    pub(crate) fn get_sampler(&mut self, create_if_necessary: bool) -> Option<&mut dyn Sampler> {
        self.imp.on_get_sampler(&self.base, create_if_necessary)
    }

    /// Port of `SkCodec::outputScanline`: the output row for an input (encoded) row.
    // Port of: src/codec/SkCodec.cpp#L763-L779 (chrome/m156)
    #[must_use]
    pub fn output_scanline(&self, input_scanline: i32) -> i32 {
        match self.imp.on_get_scanline_order() {
            ScanlineOrder::TopDown => input_scanline,
            ScanlineOrder::BottomUp => self.base.encoded_info.height() - input_scanline - 1,
        }
    }

    /// Port of `SkCodec::nextScanline`: the output row the next scanline decode produces.
    // Port of: include/codec/SkCodec.h#L647 (nextScanline)
    #[must_use]
    pub fn next_scanline(&self) -> i32 {
        self.output_scanline(self.base.curr_scanline)
    }

    /// Port of `SkCodec::startScanlineDecode(info, options)`.
    // Port of: src/codec/SkCodec.cpp#L673-L724 (chrome/m156)
    pub fn start_scanline_decode(&mut self, info: &ImageInfo, options: Option<&Options>) -> Result {
        self.base.curr_scanline = -1;
        let default_options = Options::default();
        let options = options.unwrap_or(&default_options);

        if let Some(subset) = options.subset {
            let size = IRect::from_wh(info.width(), info.height());
            if !size.contains(&subset) {
                return Result::InvalidInput;
            }
            // Only full-height subsets are supported for scanline decoding.
            if subset.top() != 0 || subset.height() != info.height() {
                return Result::InvalidInput;
            }
        }

        if options.frame_index != 0 {
            return Result::Unimplemented;
        }

        let frame_index_result = self.handle_frame_index(info, &mut [], 0, options);
        if frame_index_result != Result::Success {
            return frame_index_result;
        }

        if !self.dimensions_supported(info.dimensions()) {
            return Result::InvalidScale;
        }

        let result = self
            .imp
            .on_start_scanline_decode(&mut self.base, info, options);
        if result != Result::Success {
            return result;
        }

        self.base.curr_scanline = 0;
        self.base.dst_info = info.clone();
        self.base.options = options.clone();
        Result::Success
    }

    /// Port of `SkCodec::getScanlines`: decodes `count` rows into `dst` (`row_bytes` apart).
    // Port of: src/codec/SkCodec.cpp#L726-L743 (chrome/m156)
    pub fn get_scanlines(&mut self, dst: &mut [u8], count: i32, row_bytes: usize) -> i32 {
        if self.base.curr_scanline < 0 {
            return 0;
        }
        let height = self.base.dst_info.height();
        if count <= 0 || self.base.curr_scanline + count > height {
            return 0;
        }
        // Not in Skia's `getScanlines`, which trusts its caller: a row stride shorter than one row,
        // or a destination too small for `count` rows, would panic on slice indexing below. Zero
        // lines decoded is the only failure value this signature can carry, so report that.
        // A sampled decode writes rows of the sampler's width, which is narrower than the native
        // destination, so the check uses the width the sampler fills. The stride only separates
        // rows, so a single line (`count == 1`) takes any `row_bytes`, as Skia's does.
        let row_width = match self.imp.on_get_sampler(&self.base, false) {
            Some(sampler) => sampler.fill_width(),
            None => self.base.dst_info.width(),
        };
        let min_row_bytes = self.base.dst_info.with_wh(row_width, 1).min_row_bytes();
        let Ok(count_usize) = usize::try_from(count) else {
            return 0;
        };
        if (count_usize > 1 && row_bytes < min_row_bytes)
            || (count_usize - 1)
                .checked_mul(row_bytes)
                .and_then(|n| n.checked_add(min_row_bytes))
                .is_none_or(|needed| dst.len() < needed)
        {
            return 0;
        }

        let lines_decoded = self
            .imp
            .on_get_scanlines(&mut self.base, dst, count, row_bytes);
        if lines_decoded < count {
            let info = self.base.dst_info.clone();
            let zero = self.base.options.zero_initialized;
            self.fill_incomplete_image(&info, dst, row_bytes, zero, count, lines_decoded);
        }
        self.base.curr_scanline += count;
        lines_decoded
    }

    /// Port of `SkCodec::skipScanlines`.
    // Port of: src/codec/SkCodec.cpp#L745-L761 (chrome/m156)
    pub fn skip_scanlines(&mut self, count: i32) -> bool {
        if self.base.curr_scanline < 0 {
            return false;
        }
        let height = self.base.dst_info.height();
        if count < 0 || self.base.curr_scanline + count > height {
            return false;
        }
        let result = self.imp.on_skip_scanlines(&mut self.base, count);
        self.base.curr_scanline += count;
        result
    }

    /// Port of `SkCodec::startIncrementalDecode`: prepares to decode into `dst` in parts.
    ///
    /// The destination is only borrowed for this call, so the decode keeps no reference to it:
    /// every [`Codec::incremental_decode`] call takes the destination again, and other calls
    /// (such as [`Codec::frame_infos`] or [`Codec::get_pixels`]) may run between those calls.
    /// Like `SkCodec`, [`Codec::get_pixels`] and a rewind end the started decode.
    ///
    /// # Errors
    /// The `Result` that stopped the start: `Unimplemented` for a codec without incremental
    /// decoding, `InvalidParameters` for a bad subset or destination, and the frame, scale or
    /// decoder failure otherwise.
    // Port of: src/codec/SkCodec.cpp#L618-L671 (chrome/m156)
    #[doc(alias = "startIncrementalDecode")]
    pub fn start_incremental_decode(
        &mut self,
        info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        options: Option<&Options>,
    ) -> Result {
        self.base.started_incremental_decode = false;
        if !self.imp.on_supports_incremental_decode(info) {
            return Result::Unimplemented;
        }
        if info.color_type() == ColorType::Unknown {
            return Result::InvalidConversion;
        }
        if dst.is_empty() {
            return Result::InvalidParameters;
        }

        let default_options = Options::default();
        let options = options.unwrap_or(&default_options);
        if let Some(subset) = options.subset {
            let size = IRect::from_wh(info.width(), info.height());
            if !size.contains(&subset) {
                return Result::InvalidParameters;
            }
            let top = subset.top();
            let bottom = subset.bottom();
            if top < 0 || top >= info.height() || top >= bottom || bottom > info.height() {
                return Result::InvalidParameters;
            }
        }

        let frame_index_result = self.handle_frame_index(info, &mut *dst, row_bytes, options);
        if frame_index_result != Result::Success {
            return frame_index_result;
        }

        if !self.dimensions_supported(info.dimensions()) {
            return Result::InvalidScale;
        }

        self.base.dst_info = info.clone();
        self.base.options = options.clone();
        let result =
            self.imp
                .on_start_incremental_decode(&mut self.base, info, dst, row_bytes, options);
        self.base.started_incremental_decode = result == Result::Success;
        result
    }

    /// Port of `SkCodec::incrementalDecode`: decodes as much of the started decode as the input
    /// allows into `dst`, which must be the destination given to
    /// [`Codec::start_incremental_decode`].
    ///
    /// Returns the result and, only for [`Result::IncompleteInput`], the number of rows written.
    /// `Success` means every requested row is decoded. `IncompleteInput` means the decode may be
    /// resumed once more input is available. Without a started decode the result is
    /// `InvalidParameters`.
    // Port of: include/codec/SkCodec.h#L521-L526 (incrementalDecode)
    #[doc(alias = "incrementalDecode")]
    pub fn incremental_decode(&mut self, dst: &mut [u8]) -> (Result, Option<usize>) {
        let (result, rows_decoded) = self.incremental_decode_rows(dst);
        let rows = if result == Result::IncompleteInput {
            usize::try_from(rows_decoded).ok()
        } else {
            None
        };
        (result, rows)
    }

    /// Port of `SkCodec::incrementalDecode` with the rows as `int`, for the codecs that drive it
    /// (sampled and ICO decodes need the count for filling).
    // Port of: include/codec/SkCodec.h#L521-L526 (incrementalDecode)
    pub(crate) fn incremental_decode_rows(&mut self, dst: &mut [u8]) -> (Result, i32) {
        if !self.base.started_incremental_decode {
            return (Result::InvalidParameters, 0);
        }
        let mut rows_decoded = 0;
        let result = self
            .imp
            .on_incremental_decode(&mut self.base, dst, &mut rows_decoded);
        (result, rows_decoded)
    }

    /// Port of the `onSupportsIncrementalDecode` virtual, for a codec that wraps this one (the ICO
    /// codec asks each embedded codec).
    // Port of: src/codec/SkCodec.h (onSupportsIncrementalDecode, called on an embedded codec)
    pub(crate) fn supports_incremental_decode_imp(&self, dst: &ImageInfo) -> bool {
        self.imp.on_supports_incremental_decode(dst)
    }

    /// Port of `SkCodec::fillIncompleteImage`: writes zeros over the rows a decode did not
    /// produce, unless the caller zeroed the destination.
    // Port of: src/codec/SkCodec.cpp#L781-L797 (chrome/m156)
    // Line counts are non-negative, so the cast to usize is exact.
    #[allow(clippy::cast_sign_loss)]
    pub(crate) fn fill_incomplete_image(
        &mut self,
        info: &ImageInfo,
        dst: &mut [u8],
        row_bytes: usize,
        zero_init: ZeroInitialized,
        lines_requested: i32,
        lines_decoded: i32,
    ) {
        if zero_init == ZeroInitialized::Yes {
            return;
        }
        let lines_remaining = lines_requested - lines_decoded;
        // Port of `sampler->fillWidth()` when the codec has a sampler, else the subset or image width.
        let fill_width = match self.imp.on_get_sampler(&self.base, false) {
            Some(sampler) => sampler.fill_width(),
            None => match self.base.options.subset {
                Some(subset) => subset.width(),
                None => info.width(),
            },
        };
        let fill_offset = if self.imp.on_get_scanline_order() == ScanlineOrder::BottomUp {
            0
        } else {
            lines_decoded as usize * row_bytes
        };
        let fill_info = info.with_wh(fill_width, lines_remaining);
        sampler::fill(
            &fill_info,
            &mut dst[fill_offset..],
            row_bytes,
            ZeroInitialized::No,
        );
    }

    /// Port of `SkCodec::MakeFromStream(stream, decoders)` for a single decoder list: sniffs the
    /// format from the first bytes and hands the stream to the matching decoder.
    ///
    /// # Errors
    /// The `Result` describes why no codec was made: `InvalidInput` never occurs (a stream is
    /// always given), `CouldNotRewind` if the format could not be checked or rewound,
    /// `IncompleteInput` if the stream ended before a format could be recognised, and
    /// `Unimplemented` if no decoder recognised the format.
    // Port of: src/codec/SkCodec.cpp#L177-L250 (chrome/m156)
    #[doc(alias = "MakeFromStream")]
    pub fn make_from_stream(
        stream: Box<dyn Stream + Send + 'a>,
        decoders: &[crate::codecs::Decoder],
    ) -> std::result::Result<Codec<'a>, Result> {
        let mut stream = stream;
        let mut buffer = [0u8; MIN_BUFFERED_BYTES_NEEDED];
        let mut bytes_read = stream.peek(&mut buffer);
        // Port of: src/codec/SkCodec.cpp#L208-L222 (peek failed, so read and rewind)
        if bytes_read == 0 {
            bytes_read = stream.read(&mut buffer);
            if !stream.rewind() {
                return Err(Result::CouldNotRewind);
            }
        }

        for decoder in decoders {
            if (decoder.is_format)(&buffer[..bytes_read]) {
                return (decoder.make_from_stream)(stream);
            }
        }

        if bytes_read < MIN_BUFFERED_BYTES_NEEDED {
            Err(Result::IncompleteInput)
        } else {
            Err(Result::Unimplemented)
        }
    }
}

/// Port of `SkCodec::MinBufferedBytesNeeded`.
pub const MIN_BUFFERED_BYTES_NEEDED: usize = 32;
