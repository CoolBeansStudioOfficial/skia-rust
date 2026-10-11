// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/encode/SkJpegGainmapEncoder.cpp and include/private/SkJpegGainmapEncoder.h
// (chrome/m156): the HDRGM encoder, which writes a base image and a gainmap image as one
// Multi-Picture file, each with its Exif, XMP, ICC and ISO 21496-1 metadata.
//
// The metadata segments are written by the JPEG encoder from an explicit segment list, as
// `SkJpegEncoderImpl::MakeRGB` does. The Multi-Picture segments are written here, as the C++
// `MakeMPF` does.

use std::io;
use std::sync::{Arc, Mutex};

use skia_rust_core::color::Color4f;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::data::Data;
use skia_rust_core::gainmap_info::{BaseImageType, GainmapInfo};
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::stream::{DynamicMemoryWStream, WStream};

use crate::encode::icc::write_icc_profile;
use crate::encode::jpeg_encoder::{self, Options};
use crate::jpeg_constants::{
    JPEG_MARKER_START_OF_SCAN, JPEG_SEGMENT_PARAMETER_LENGTH_SIZE, MPF_MARKER,
};
use crate::jpeg_multi_picture::MultiPictureParameters;
use crate::jpeg_segment_scan::JpegSegmentScanner;

/// A metadata segment: its marker code and its parameters (`SkJpegMetadataEncoder::Segment`).
type Segment = (u8, Vec<u8>);

// Port of: src/codec/SkJpegConstants.h#L46-L47 (chrome/m156), `kXMPMarker`, `kXMPStandardSig`
const XMP_MARKER: u8 = 0xE1;
const XMP_STANDARD_SIG: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
// Port of: src/codec/SkJpegConstants.h#L55-L56 (chrome/m156), `kExifMarker`, `kExifSig`
const EXIF_MARKER: u8 = 0xE1;
const EXIF_SIG: &[u8] = b"Exif\0";
// Port of: src/codec/SkJpegConstants.h#L30-L37 (chrome/m156), `kICCMarker`, `kICCSig`
const ICC_MARKER: u8 = 0xE2;
const ICC_SIG: &[u8] = b"ICC_PROFILE\0";
// Port of: src/codec/SkJpegConstants.h#L63-L64 (chrome/m156), `kISOGainmapMarker`, `kISOGainmapSig`
const ISO_GAINMAP_MARKER: u8 = 0xE2;
const ISO_GAINMAP_SIG: &[u8] = b"urn:iso:std:iso:ts:21496:-1\0";

/// Port of `is_single_channel`: all three channels of `c` are equal.
// Port of: src/encode/SkJpegGainmapEncoder.cpp#L25 (chrome/m156)
#[allow(clippy::float_cmp)] // exact equality, as the C++ compares the channels
fn is_single_channel(c: Color4f) -> bool {
    c.r == c.g && c.g == c.b
}

/// Writes one scalar attribute (`write_scalar_attr`).
// Port of: src/encode/SkJpegGainmapEncoder.cpp#L34-L41 (chrome/m156)
fn write_scalar_attr(s: &mut DynamicMemoryWStream, attrib: &str, value: f32) {
    s.write_text("        ");
    s.write_text(attrib);
    s.write_text("=\"");
    s.write_scalar_as_text(value);
    s.write_text("\"\n");
}

/// Writes a scalar attribute only if all channels of `value` are equal.
// Port of: src/encode/SkJpegGainmapEncoder.cpp#L43-L50 (chrome/m156), maybe_write_scalar_attr
fn maybe_write_scalar_attr(s: &mut DynamicMemoryWStream, attrib: &str, value: Color4f) {
    if !is_single_channel(value) {
        return;
    }
    write_scalar_attr(s, attrib, value.r);
}

/// Writes a float3 attribute as a list only if not all channels of `value` are equal.
// Port of: src/encode/SkJpegGainmapEncoder.cpp#L52-L76 (chrome/m156), maybe_write_float3_attr
fn maybe_write_float3_attr(s: &mut DynamicMemoryWStream, attrib: &str, value: Color4f) {
    if is_single_channel(value) {
        return;
    }
    s.write_text("      <");
    s.write_text(attrib);
    s.write_text(">\n");
    s.write_text("        <rdf:Seq>\n");
    s.write_text("          <rdf:li>");
    s.write_scalar_as_text(value.r);
    s.write_text("</rdf:li>\n");
    s.write_text("          <rdf:li>");
    s.write_scalar_as_text(value.g);
    s.write_text("</rdf:li>\n");
    s.write_text("          <rdf:li>");
    s.write_scalar_as_text(value.b);
    s.write_text("</rdf:li>\n");
    s.write_text("        </rdf:Seq>\n");
    s.write_text("      </");
    s.write_text(attrib);
    s.write_text(">\n");
}

/// Generates the XMP metadata of an HDRGM gainmap image.
// Port of: src/encode/SkJpegGainmapEncoder.cpp#L33-L125 (chrome/m156), get_gainmap_image_xmp_metadata
fn get_gainmap_image_xmp_metadata(info: &GainmapInfo) -> Data {
    let mut s = DynamicMemoryWStream::new();
    let k_log2 = 2.0f32.ln();
    let gain_map_min = Color4f {
        r: info.gainmap_ratio_min.r.ln() / k_log2,
        g: info.gainmap_ratio_min.g.ln() / k_log2,
        b: info.gainmap_ratio_min.b.ln() / k_log2,
        a: 1.0,
    };
    let gain_map_max = Color4f {
        r: info.gainmap_ratio_max.r.ln() / k_log2,
        g: info.gainmap_ratio_max.g.ln() / k_log2,
        b: info.gainmap_ratio_max.b.ln() / k_log2,
        a: 1.0,
    };
    let gamma = Color4f {
        r: 1.0 / info.gainmap_gamma.r,
        g: 1.0 / info.gainmap_gamma.g,
        b: 1.0 / info.gainmap_gamma.b,
        a: 1.0,
    };

    s.write_text("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"XMP Core 5.5.0\">\n");
    s.write_text("  <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n");
    s.write_text("    <rdf:Description rdf:about=\"\"\n");
    s.write_text("        xmlns:hdrgm=\"http://ns.adobe.com/hdr-gain-map/1.0/\"\n");
    s.write_text("        hdrgm:Version=\"1.0\"\n");
    maybe_write_scalar_attr(&mut s, "hdrgm:GainMapMin", gain_map_min);
    maybe_write_scalar_attr(&mut s, "hdrgm:GainMapMax", gain_map_max);
    maybe_write_scalar_attr(&mut s, "hdrgm:Gamma", gamma);
    maybe_write_scalar_attr(&mut s, "hdrgm:OffsetSDR", info.epsilon_sdr);
    maybe_write_scalar_attr(&mut s, "hdrgm:OffsetHDR", info.epsilon_hdr);
    write_scalar_attr(
        &mut s,
        "hdrgm:HDRCapacityMin",
        info.display_ratio_sdr.ln() / k_log2,
    );
    write_scalar_attr(
        &mut s,
        "hdrgm:HDRCapacityMax",
        info.display_ratio_hdr.ln() / k_log2,
    );
    match info.base_image_type {
        BaseImageType::Sdr => {
            s.write_text("        hdrgm:BaseRenditionIsHDR=\"False\">\n");
        }
        BaseImageType::Hdr => {
            s.write_text("        hdrgm:BaseRenditionIsHDR=\"True\">\n");
        }
    }

    // The vector parameters that cannot be written as scalars.
    maybe_write_float3_attr(&mut s, "hdrgm:GainMapMin", gain_map_min);
    maybe_write_float3_attr(&mut s, "hdrgm:GainMapMax", gain_map_max);
    maybe_write_float3_attr(&mut s, "hdrgm:Gamma", gamma);
    maybe_write_float3_attr(&mut s, "hdrgm:OffsetSDR", info.epsilon_sdr);
    maybe_write_float3_attr(&mut s, "hdrgm:OffsetHDR", info.epsilon_hdr);
    s.write_text("    </rdf:Description>\n");
    s.write_text("  </rdf:RDF>\n");
    s.write_text("</x:xmpmeta>");
    s.detach_as_data()
}

/// Generates the `GContainer` metadata of a base image with a JPEG gainmap.
// Port of: src/encode/SkJpegGainmapEncoder.cpp#L128-L160 (chrome/m156), get_base_image_xmp_metadata
fn get_base_image_xmp_metadata(gainmap_item_length: i32) -> Data {
    let mut s = DynamicMemoryWStream::new();
    s.write_text(
        "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"Adobe XMP Core 5.1.2\">\n\
         \x20 <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
         \x20   <rdf:Description\n\
         \x20       xmlns:Container=\"http://ns.google.com/photos/1.0/container/\"\n\
         \x20       xmlns:Item=\"http://ns.google.com/photos/1.0/container/item/\"\n\
         \x20       xmlns:hdrgm=\"http://ns.adobe.com/hdr-gain-map/1.0/\"\n\
         \x20       hdrgm:Version=\"1.0\">\n\
         \x20     <Container:Directory>\n\
         \x20       <rdf:Seq>\n\
         \x20         <rdf:li rdf:parseType=\"Resource\">\n\
         \x20           <Container:Item\n\
         \x20            Item:Semantic=\"Primary\"\n\
         \x20            Item:Mime=\"image/jpeg\"/>\n\
         \x20         </rdf:li>\n\
         \x20         <rdf:li rdf:parseType=\"Resource\">\n\
         \x20           <Container:Item\n\
         \x20            Item:Semantic=\"GainMap\"\n\
         \x20            Item:Mime=\"image/jpeg\"\n\
         \x20            Item:Length=\"",
    );
    s.write_dec_as_text(gainmap_item_length);
    s.write_text(
        "\"/>\n\
         \x20         </rdf:li>\n\
         \x20       </rdf:Seq>\n\
         \x20     </Container:Directory>\n\
         \x20   </rdf:Description>\n\
         \x20 </rdf:RDF>\n\
         </x:xmpmeta>\n",
    );
    s.detach_as_data()
}

/// The Exif segment's parameters: the signature, a padding byte, and an Exif TIFF structure that
/// holds only the sub-IFD pointer of the Ultra HDR version tag.
// Port of: src/encode/SkJpegGainmapEncoder.cpp#L173-L217 (chrome/m156), get_exif_params
fn get_exif_params() -> Vec<u8> {
    const K_ENDIAN_BIG: [u8; 4] = [b'M', b'M', 0, 42];
    const K_TYPE_UNSIGNED_LONG: u16 = 4;
    const K_TYPE_UNDEFINED: u16 = 7;
    let mut s = Vec::new();

    s.extend_from_slice(EXIF_SIG);
    s.push(0);

    s.extend_from_slice(&K_ENDIAN_BIG);
    s.extend_from_slice(&8u32.to_be_bytes()); // Offset of index IFD

    // The index IFD.
    {
        let index_ifd_number_of_tags: u16 = 1;
        s.extend_from_slice(&index_ifd_number_of_tags.to_be_bytes());

        let sub_ifd_offset_tag: u16 = 0x8769;
        let sub_ifd_count: u32 = 1;
        let sub_ifd_offset: u32 = 26;
        s.extend_from_slice(&sub_ifd_offset_tag.to_be_bytes());
        s.extend_from_slice(&K_TYPE_UNSIGNED_LONG.to_be_bytes());
        s.extend_from_slice(&sub_ifd_count.to_be_bytes());
        s.extend_from_slice(&sub_ifd_offset.to_be_bytes());

        let index_ifd_next_ifd_offset: u32 = 0;
        s.extend_from_slice(&index_ifd_next_ifd_offset.to_be_bytes());
    }

    // The sub-IFD.
    {
        let sub_ifd_number_of_tags: u16 = 1;
        s.extend_from_slice(&sub_ifd_number_of_tags.to_be_bytes());

        let version_tag: u16 = 0x9000;
        let version_count: u32 = 4;
        let version: [u8; 4] = *b"0232";
        s.extend_from_slice(&version_tag.to_be_bytes());
        s.extend_from_slice(&K_TYPE_UNDEFINED.to_be_bytes());
        s.extend_from_slice(&version_count.to_be_bytes());
        s.extend_from_slice(&version);

        let sub_ifd_next_ifd_offset: u32 = 0;
        s.extend_from_slice(&sub_ifd_next_ifd_offset.to_be_bytes());
    }

    s
}

/// The Multi-Picture segment for image `image_number` of `mp_params`, with its marker and length
/// (`get_mpf_segment`).
// Port of: src/encode/SkJpegGainmapEncoder.cpp#L219-L230 (chrome/m156), get_mpf_segment
// The C++ writes the length bytes as uint8_t, which keeps the low byte; the casts mirror that.
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ uint8_t conversions
fn get_mpf_segment(mp_params: &MultiPictureParameters, image_number: u32) -> Vec<u8> {
    let segment_parameters = mp_params.serialize(image_number);
    let mp_parameter_length = JPEG_SEGMENT_PARAMETER_LENGTH_SIZE + segment_parameters.size();
    let mut s = Vec::with_capacity(4 + segment_parameters.size());
    s.push(0xFF);
    s.push(MPF_MARKER as u8);
    s.push(((mp_parameter_length / 256) & 0xFF) as u8);
    s.push((mp_parameter_length % 256) as u8);
    s.extend_from_slice(segment_parameters.as_bytes());
    s
}

/// The ISO 21496-1 segment parameters: the signature, then the data (`get_iso_gainmap_segment_params`).
// Port of: src/encode/SkJpegGainmapEncoder.cpp#L232-L237 (chrome/m156), get_iso_gainmap_segment_params
fn get_iso_gainmap_segment_params(data: &[u8]) -> Vec<u8> {
    let mut s = Vec::with_capacity(ISO_GAINMAP_SIG.len() + data.len());
    s.extend_from_slice(ISO_GAINMAP_SIG);
    s.extend_from_slice(data);
    s
}

/// Adds the XMP standard segment for `xmp`, if there is any (`SkJpegMetadataEncoder::AppendXMPStandard`).
// Port of: src/encode/SkJpegEncoderImpl.cpp#L478-L488 (chrome/m156), AppendXMPStandard
fn append_xmp_standard(segments: &mut Vec<Segment>, xmp: Option<&[u8]>) {
    let Some(xmp) = xmp else {
        return;
    };
    let mut body = XMP_STANDARD_SIG.to_vec();
    body.extend_from_slice(xmp);
    segments.push((XMP_MARKER, body));
}

/// Adds the ICC segment for `color_space`, if it has a profile
/// (`SkJpegMetadataEncoder::AppendICC`). The profile is written as one segment: "1 of 1".
// Port of: src/encode/SkJpegEncoderImpl.cpp#L460-L476 (chrome/m156), AppendICC
fn append_icc(segments: &mut Vec<Segment>, color_space: Option<&ColorSpace>) {
    let Some(icc) = write_icc_profile(color_space) else {
        return;
    };
    let mut body = ICC_SIG.to_vec();
    body.push(1);
    body.push(1);
    body.extend_from_slice(&icc);
    segments.push((ICC_MARKER, body));
}

/// Encodes `pm` as a JPEG with the given metadata segments and returns the bytes
/// (`encode_to_data`).
// Port of: src/encode/SkJpegGainmapEncoder.cpp#L162-L171 (chrome/m156), encode_to_data
fn encode_to_data(pm: &Pixmap<'_>, options: &Options, segments: &[Segment]) -> Option<Vec<u8>> {
    let out = Arc::new(Mutex::new(Vec::new()));
    let view = Pixmap::new_readonly(pm.info(), pm.addr()?, pm.row_bytes())?;
    let mut encoder = jpeg_encoder::make_with_segments(Arc::clone(&out), view, options, segments)?;
    if !encoder.encode_rows(pm.info().height()) {
        return None;
    }
    drop(encoder);
    let bytes = out.lock().ok()?.clone();
    Some(bytes)
}

/// The offset in `image` at which the Multi-Picture segment goes, or 0 on failure
/// (`mp_segment_offset`).
// Port of: src/encode/SkJpegGainmapEncoder.cpp#L330-L348 (chrome/m156), mp_segment_offset
fn mp_segment_offset(image: &[u8]) -> usize {
    // Scan the image until the StartOfScan marker.
    let mut scan = JpegSegmentScanner::new(JPEG_MARKER_START_OF_SCAN);
    scan.on_bytes(image);
    if !scan.is_done() {
        return 0;
    }

    // According to CIPA DC-007 section 5.1, the MP Extensions follow the Exif Attributes in APP2.
    // In practice, that is rarely obeyed, and makes the file dangerous for less robust editors
    // (see b/355642172). Instead, the MP segment goes just before the StartOfScan marker.
    scan.segments().last().map_or(0, |segment| segment.offset)
}

/// The Multi-Picture image number of the `i`th image. There are only a few images, so the count
/// always fits.
fn image_number(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

/// Writes the images, as one Multi-Picture file (`SkJpegGainmapEncoder::MakeMPF`). Returns `None`
/// when an image's header cannot be scanned.
// Port of: src/encode/SkJpegGainmapEncoder.cpp#L350-L402 (chrome/m156), MakeMPF
#[doc(alias = "SkJpegGainmapEncoder::MakeMPF")]
#[must_use]
pub fn make_mpf(images: &[&[u8]]) -> Option<Vec<u8>> {
    if images.is_empty() {
        return Some(Vec::new());
    }
    let image_count = images.len();

    // The offset into each image at which the MP segment will be written.
    let mut mp_segment_offsets = vec![0usize; image_count];

    // Populate the MP parameters (image sizes and offsets).
    let mut mp_params = MultiPictureParameters::new(image_count);
    let mut cumulative_size = 0usize;
    for (i, image) in images.iter().enumerate() {
        let number = image_number(i);
        // The offset into each image where the MP parameters go.
        mp_segment_offsets[i] = mp_segment_offset(image);
        if mp_segment_offsets[i] == 0 {
            return None;
        }

        // The size of the MPF segment is added to the image size. The contents of the segment are
        // wrong until the offsets are set, but the size is right.
        let image_size = image.len() + get_mpf_segment(&mp_params, number).len();
        mp_params.images[i].data_offset =
            MultiPictureParameters::get_image_data_offset(cumulative_size, mp_segment_offsets[0]);
        mp_params.images[i].size = u32::try_from(image_size).unwrap_or(u32::MAX);
        cumulative_size += image_size;
    }

    // Write the images.
    let mut out = Vec::with_capacity(cumulative_size);
    for (i, image) in images.iter().enumerate() {
        let split = mp_segment_offsets[i];
        // Up to the MP segment, then the MP segment, then the rest of the image.
        out.extend_from_slice(&image[..split]);
        out.extend_from_slice(&get_mpf_segment(&mp_params, image_number(i)));
        out.extend_from_slice(&image[split..]);
    }
    Some(out)
}

/// Encodes a base image and a gainmap image as one HDRGM Multi-Picture JPEG, written to `dst`
/// (`SkJpegGainmapEncoder::EncodeHDRGM`). Returns `false` when an image cannot be encoded or
/// written.
// Port of: src/encode/SkJpegGainmapEncoder.cpp#L239-L327 (chrome/m156), EncodeHDRGM
#[doc(alias = "SkJpegGainmapEncoder::EncodeHDRGM")]
pub fn encode_hdrgm<W: io::Write>(
    dst: &mut W,
    base: &Pixmap<'_>,
    base_options: &Options,
    gainmap: &Pixmap<'_>,
    gainmap_options: &Options,
    gainmap_info: &GainmapInfo,
) -> bool {
    let include_ultra_hdr_v1 = gainmap_info.is_ultra_hdr_v1_compatible();

    // All images have the same minimal Exif metadata.
    let exif_params = get_exif_params();

    // Encode the gainmap image.
    let gainmap_data = {
        let mut segments: Vec<Segment> = Vec::new();

        // Start with Exif metadata. The MPF segment is inserted after this.
        segments.push((EXIF_MARKER, exif_params.clone()));

        // The XMP metadata of the gainmap, for an Ultra HDR v1 gainmap.
        if include_ultra_hdr_v1 {
            let xmp = get_gainmap_image_xmp_metadata(gainmap_info);
            append_xmp_standard(&mut segments, Some(xmp.as_bytes()));
        }

        // The ICC profile of the alternate colour space, if it is used.
        if let Some(math_color_space) = gainmap_info.gainmap_math_color_space.as_ref() {
            append_icc(&mut segments, Some(math_color_space));
        }

        // The ISO 21496-1 metadata.
        let iso = get_iso_gainmap_segment_params(gainmap_info.serialize().as_bytes());
        segments.push((ISO_GAINMAP_MARKER, iso));

        match encode_to_data(gainmap, gainmap_options, &segments) {
            Some(data) => data,
            None => return false,
        }
    };

    // Encode the base image.
    let base_data = {
        let mut segments: Vec<Segment> = Vec::new();

        // Start with Exif metadata. The MPF segment is inserted after this.
        segments.push((EXIF_MARKER, exif_params));

        // The GContainer XMP. The gainmap image's size includes the MPF segment for image 1 of a
        // 2-image file.
        if include_ultra_hdr_v1 {
            let mp_params = MultiPictureParameters::new(2);
            let gainmap_image_size = gainmap_data.len() + get_mpf_segment(&mp_params, 1).len();
            let xmp =
                get_base_image_xmp_metadata(i32::try_from(gainmap_image_size).unwrap_or(i32::MAX));
            append_xmp_standard(&mut segments, Some(xmp.as_bytes()));
        }

        // The ICC profile metadata of the base image's colour space.
        let base_color_space = base.color_space();
        append_icc(&mut segments, base_color_space.as_ref());

        // The ISO 21496-1 version metadata.
        let iso = get_iso_gainmap_segment_params(GainmapInfo::serialize_version().as_bytes());
        segments.push((ISO_GAINMAP_MARKER, iso));

        match encode_to_data(base, base_options, &segments) {
            Some(data) => data,
            None => return false,
        }
    };

    // Combine them into a Multi-Picture file.
    let Some(file) = make_mpf(&[&base_data, &gainmap_data]) else {
        return false;
    };
    dst.write_all(&file).is_ok()
}
