// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skresources/include/SkResources.h (`ImageAsset`, `ImageDecodeStrategy`,
// `MultiFrameImageAsset`, `ExternalTrackAsset`), modules/skresources/src/SkResources.cpp (chrome/m156)

use std::cell::RefCell;
use std::rc::Rc;

use skia_rust_codec::codec::Codec;
use skia_rust_codec::codecs;
use skia_rust_core::data::Data;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::stream::MemoryStream;
use skia_rust_raster::pixmap_draw::ImageScalePixels;

use crate::anim_codec_player::AnimCodecPlayer;

/// Whether decoded frames are decoded up front, or on demand (`ImageDecodeStrategy`).
// Port of: modules/skresources/include/SkResources.h#L72-L75 (chrome/m156) (`ImageDecodeStrategy`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageDecodeStrategy {
    /// Frames decode when they are drawn (`kLazyDecode`).
    #[default]
    LazyDecode,
    /// Frames decode when they are read, and large frames are scaled down (`kPreDecode`).
    PreDecode,
}

/// How an image is scaled to the size of its layer (`ImageAsset::SizeFit`).
// Port of: modules/skresources/include/SkResources.h#L44-L51 (chrome/m156) (`ImageAsset::SizeFit`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeFit {
    /// Stretches the image to the layer (`kFill`).
    Fill,
    /// Scales to fit, aligned to the start (`kStart`).
    Start,
    /// Scales to fit, centered (`kCenter`).
    Center,
    /// Scales to fit, aligned to the end (`kEnd`).
    End,
    /// The image is not scaled (`kNone`).
    None,
}

/// The payload of one frame of an image asset (`ImageAsset::FrameData`).
// Port of: modules/skresources/include/SkResources.h#L53-L60 (chrome/m156) (`ImageAsset::FrameData`)
#[derive(Debug, Clone)]
pub struct FrameData {
    /// The image of the frame.
    pub image: Option<Image>,
    /// How the image is sampled.
    pub sampling: SamplingOptions,
    /// The matrix that places the image.
    pub matrix: Matrix,
    /// How the image is scaled to its layer.
    pub scaling: SizeFit,
}

/// An image of a layer, which may have several frames (`ImageAsset`).
// Port of: modules/skresources/include/SkResources.h#L24-L70 (chrome/m156) (`class ImageAsset`)
#[doc(alias = "skresources::ImageAsset")]
pub trait ImageAsset {
    /// True if the image asset is animated (`isMultiFrame`).
    #[doc(alias = "isMultiFrame")]
    fn is_multi_frame(&self) -> bool;

    /// The image of the frame at `t` seconds, relative to the in-point of the layer (`getFrame`).
    /// Deprecated in favour of [`ImageAsset::get_frame_data`]; an asset implements one of them.
    // Port of: modules/skresources/src/SkResources.cpp#L16-L18 (chrome/m156) (`ImageAsset::getFrame`)
    #[doc(alias = "getFrame")]
    fn get_frame(&self, _t: f32) -> Option<Image> {
        None
    }

    /// The payload of the frame at `t` seconds (`getFrameData`). The default is the legacy
    /// behaviour: the image of [`ImageAsset::get_frame`], linearly sampled and centered.
    // Port of: modules/skresources/src/SkResources.cpp#L20-L28 (chrome/m156) (`ImageAsset::getFrameData`)
    #[doc(alias = "getFrameData")]
    fn get_frame_data(&self, t: f32) -> FrameData {
        // legacy behavior
        FrameData {
            image: self.get_frame(t),
            sampling: SamplingOptions::new(FilterMode::Linear, MipmapMode::Nearest),
            matrix: Matrix::new_identity(),
            scaling: SizeFit::Center,
        }
    }
}

/// An audio track of a layer, controlled by the animation (`ExternalTrackAsset`).
// Port of: modules/skresources/include/SkResources.h#L186-L197 (chrome/m156) (`class ExternalTrackAsset`)
#[doc(alias = "skresources::ExternalTrackAsset")]
pub trait ExternalTrackAsset {
    /// Playback control, for each seek of the animation. Negative times stop the playback, which
    /// is outside the span of the layer (`seek`).
    fn seek(&self, t: f32);
}

/// An animated or still image, decoded from encoded data by the codecs (`MultiFrameImageAsset`).
// Port of: modules/skresources/include/SkResources.h#L77-L105 (chrome/m156) (`class MultiFrameImageAsset`)
#[doc(alias = "skresources::MultiFrameImageAsset")]
pub struct MultiFrameImageAsset {
    player: RefCell<AnimCodecPlayer>,
    /// The last generated frame, for still images (`fCachedFrame`).
    cached_frame: RefCell<Option<Image>>,
    strategy: ImageDecodeStrategy,
}

/// The largest area of a decoded frame, in pixels, before it is scaled down (`kMaxArea`).
// Port of: modules/skresources/src/SkResources.cpp#L118-L118 (chrome/m156) (`kMaxArea` in `generateFrame`)
const MAX_FRAME_AREA: usize = 2048 * 2048;

impl MultiFrameImageAsset {
    /// The asset of encoded `data`, or `None` if no codec recognises it (`Make(sk_sp<SkData>)`).
    // Port of: modules/skresources/src/SkResources.cpp#L142-L148 (chrome/m156) (`MultiFrameImageAsset::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(data: Data, strategy: ImageDecodeStrategy) -> Option<Rc<Self>> {
        // SkCodec::MakeFromData
        let codec = codecs::make_codec_from_stream(MemoryStream::make(Some(data))).ok()?;
        Some(Self::make_from_codec(codec, strategy))
    }

    /// The asset of an existing codec (`Make(std::unique_ptr<SkCodec>)`).
    // Port of: modules/skresources/src/SkResources.cpp#L150-L155 (chrome/m156) (`MultiFrameImageAsset::Make`, codec overload)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make_from_codec(codec: Codec<'static>, strategy: ImageDecodeStrategy) -> Rc<Self> {
        Rc::new(Self {
            player: RefCell::new(AnimCodecPlayer::new(codec)),
            cached_frame: RefCell::new(None),
            strategy,
        })
    }

    /// The duration of the animation, in milliseconds (`duration`).
    // Port of: modules/skresources/src/SkResources.cpp#L162-L162 (chrome/m156) (`MultiFrameImageAsset::duration`)
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // durations are small enough for the f32 mantissa
    pub fn duration(&self) -> f32 {
        self.player.borrow().duration() as f32
    }

    /// Decodes the frame at `t` seconds (`generateFrame`).
    // Port of: modules/skresources/src/SkResources.cpp#L164-L199 (chrome/m156) (`MultiFrameImageAsset::generateFrame`)
    fn generate_frame(&self, t: f32) -> Option<Image> {
        let mut player = self.player.borrow_mut();
        // The conversion saturates, where Skia's `static_cast` is undefined for negative times.
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // saturating cast
        let msec = (t * 1000.0) as u32;
        player.seek(msec);
        let frame = player.get_frame();
        match frame {
            Some(frame)
                if self.strategy == ImageDecodeStrategy::PreDecode && frame.is_lazy_generated() =>
            {
                // The multi-frame decoder should never return lazy images.
                decode_frame(&frame)
            }
            other => other,
        }
    }
}

/// Decodes a lazy frame up front. A frame larger than [`MAX_FRAME_AREA`] is scaled down to that
/// area, with linear filtering. `None` if the frame cannot be decoded (the `decode` lambda of
/// `generateFrame`).
// Port of: modules/skresources/src/SkResources.cpp#L165-L189 (chrome/m156) (`generateFrame::decode`)
fn decode_frame(image: &Image) -> Option<Image> {
    let image_area = usize::try_from(image.width() * image.height()).unwrap_or(0);
    if image_area > MAX_FRAME_AREA {
        // When the image is too large, decode and scale down to a reasonable size.
        #[allow(clippy::cast_precision_loss)] // areas are far below the f32 mantissa limit
        let scale = (MAX_FRAME_AREA as f32 / image_area as f32).sqrt();
        // The float dimensions are truncated to int, as in SkImageInfo::MakeN32Premul.
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)] // Skia's conversion
        let info = ImageInfo::new_n32_premul(
            (
                (scale * image.width() as f32) as i32,
                (scale * image.height() as f32) as i32,
            ),
            None,
        );
        let row_bytes = info.min_row_bytes();
        let mut pixels = vec![0_u8; info.compute_byte_size(row_bytes)];
        let sampling = SamplingOptions::new(FilterMode::Linear, MipmapMode::Nearest);
        let scaled = Pixmap::new(&info, &mut pixels, row_bytes)
            .is_some_and(|mut dst| image.scale_pixels(&mut dst, &sampling));
        if !scaled {
            return None;
        }
        images::raster_from_data(&info, Data::new_copy(&pixels), row_bytes)
    } else {
        // When the image size is OK, just force-decode.
        image.to_raster_image()
    }
}

impl std::fmt::Debug for MultiFrameImageAsset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiFrameImageAsset")
            .field("strategy", &self.strategy)
            .finish_non_exhaustive()
    }
}

impl ImageAsset for MultiFrameImageAsset {
    // Port of: modules/skresources/src/SkResources.cpp#L157-L160 (chrome/m156) (`MultiFrameImageAsset::isMultiFrame`)
    fn is_multi_frame(&self) -> bool {
        self.player.borrow().duration() > 0
    }

    // Port of: modules/skresources/src/SkResources.cpp#L201-L209 (chrome/m156) (`MultiFrameImageAsset::getFrame`)
    fn get_frame(&self, t: f32) -> Option<Image> {
        // For static images we can reuse the cached frame (which includes the optional
        // pre-decode step).
        if self.cached_frame.borrow().is_none() || self.is_multi_frame() {
            let frame = self.generate_frame(t);
            *self.cached_frame.borrow_mut() = frame;
        }
        self.cached_frame.borrow().clone()
    }
}
