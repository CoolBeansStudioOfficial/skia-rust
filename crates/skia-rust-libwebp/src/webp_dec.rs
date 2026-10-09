// Copyright 2010 Google Inc. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the COPYING file. Port by The skia-rust Authors.

//! Port of the still-image path of libwebp's `src/dec/webp_dec.c` (`ParseHeadersInternal`,
//! `DecodeInto`) and of `WebPGetFeatures` for a single frame.
//!
//! Handled: the RIFF container with an optional `VP8X` header, `ALPH` for lossy frames, and the
//! `VP8 ` (lossy) and `VP8L` (lossless) image chunks, as well as bare VP8 and VP8L bitstreams.
//! Animation (`ANIM`/`ANMF`) needs the demuxer, which is not ported yet; those files return
//! [`Status::UnsupportedFeature`].

use crate::alpha;
use crate::io::{Io, Status};
use crate::lossless::CspMode;
use crate::output;
use crate::vp8_dec::{self, Crop};
use crate::vp8l;

/// Port of `WebPGetFeatures` for one image: the size and whether the image has alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "WebPBitstreamFeatures")]
pub struct Features {
    pub width: i32,
    pub height: i32,
    pub has_alpha: bool,
    pub is_lossless: bool,
}

/// The image chunks of a file: `(alpha_payload, image_kind, image_payload)`.
struct Chunks<'a> {
    alpha: Option<&'a [u8]>,
    image: Option<(bool, &'a [u8])>, // (is_lossless, payload)
    vp8x_alpha: bool,
    animated: bool,
}

/// Reads the chunks of a RIFF/WEBP file, or interprets the data as a bare bitstream.
fn parse_chunks(data: &[u8]) -> Result<Chunks<'_>, Status> {
    let mut out = Chunks {
        alpha: None,
        image: None,
        vp8x_alpha: false,
        animated: false,
    };
    if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        // Parsing is bounded by the RIFF size, and stops after the image chunk (libwebp ignores
        // whatever follows, such as trailing metadata or padding).
        let riff_size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let riff_end = riff_size.saturating_add(8).min(data.len());
        let mut pos = 12usize;
        while pos + 8 <= riff_end {
            if out.image.is_some() {
                break;
            }
            let fourcc = &data[pos..pos + 4];
            let size =
                u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
                    as usize;
            let start = pos + 8;
            let end = start.checked_add(size).ok_or(Status::BitstreamError)?;
            if end > riff_end {
                return Err(Status::NotEnoughData);
            }
            let payload = &data[start..end];
            match fourcc {
                b"VP8X" => {
                    if payload.len() < 10 {
                        return Err(Status::BitstreamError);
                    }
                    out.vp8x_alpha = payload[0] & 0x10 != 0;
                    out.animated = payload[0] & 0x02 != 0;
                }
                b"ALPH" => out.alpha = Some(payload),
                b"VP8 " => {
                    if out.image.is_none() {
                        out.image = Some((false, payload));
                    }
                }
                b"VP8L" => {
                    if out.image.is_none() {
                        out.image = Some((true, payload));
                    }
                }
                b"ANIM" | b"ANMF" => out.animated = true,
                _ => {}
            }
            pos = end + (size & 1);
        }
    } else if data.len() >= 5 && vp8l::check_signature(data) {
        out.image = Some((true, data));
    } else {
        out.image = Some((false, data));
    }
    if out.animated {
        return Err(Status::UnsupportedFeature);
    }
    if out.image.is_none() {
        return Err(Status::BitstreamError);
    }
    Ok(out)
}

/// Port of `WebPGetFeatures` for a single-frame file.
///
/// # Errors
///
/// Returns `BitstreamError` when the container or the image header does not parse.
#[doc(alias = "WebPGetFeatures")]
pub fn get_features(data: &[u8]) -> Result<Features, Status> {
    let chunks = parse_chunks(data)?;
    let (is_lossless, payload) = chunks.image.ok_or(Status::BitstreamError)?;
    if is_lossless {
        let (width, height, has_alpha) = vp8l::get_info(payload).ok_or(Status::BitstreamError)?;
        Ok(Features {
            width,
            height,
            has_alpha,
            is_lossless,
        })
    } else {
        let (width, height) =
            vp8_dec::get_info(payload, payload.len()).ok_or(Status::BitstreamError)?;
        Ok(Features {
            width,
            height,
            has_alpha: chunks.alpha.is_some() || chunks.vp8x_alpha,
            is_lossless,
        })
    }
}

/// Decodes a still WebP image into `out` in colour space `mode` (`WebPDecode` with default
/// options: full frame, no scaling, fancy upsampling). Returns the image size.
///
/// # Errors
///
/// Returns the `Status` of the first parse or decode failure.
#[doc(alias = "WebPDecode")]
pub fn decode(
    data: &[u8],
    mode: CspMode,
    out: &mut [u8],
    out_stride: usize,
) -> Result<(i32, i32), Status> {
    let chunks = parse_chunks(data)?;
    let (is_lossless, payload) = chunks.image.ok_or(Status::BitstreamError)?;
    if is_lossless {
        return crate::decode_vp8l(payload, mode, out, out_stride);
    }
    let (width, height) =
        vp8_dec::get_info(payload, payload.len()).ok_or(Status::BitstreamError)?;
    let planes = vp8_dec::decode(
        payload,
        Crop {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        },
    )?;
    let alpha_plane = match chunks.alpha {
        None => None,
        Some(alph) => {
            let mut dec = alpha::alpha_init(alph, width, height, (0, width, 0, height))
                .ok_or(Status::BitstreamError)?;
            if !alpha::alpha_decode(&mut dec, 0, height) {
                return Err(dec.status());
            }
            Some(dec.plane().to_vec())
        }
    };
    let mut io = Io::new(out, out_stride, mode, width, height);
    let crop = Crop {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };
    let mb_h = planes.mb_h;
    if !output::emit_frame(&planes, alpha_plane.as_deref(), &mut io, crop, mb_h) {
        return Err(Status::UnsupportedFeature);
    }
    Ok((width, height))
}
