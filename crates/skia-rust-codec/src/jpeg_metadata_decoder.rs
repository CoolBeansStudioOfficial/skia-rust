// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkJpegMetadataDecoderImpl.cpp and src/codec/SkJpegMetadataDecoderImpl.h
// (chrome/m156), the gainmap parts: the marker list, the metadata readers (`read_metadata`), the
// XMP, ISO 21496-1 and MPF lookups, and `findGainmapImage`, which finds and parses the gainmap
// image of a JPEG.
//
// The `SkCodecPrintf` diagnostics are not ported: they are debug output only.

use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::data::Data;
use skia_rust_core::gainmap_info::GainmapInfo;

use crate::exif;
use crate::jpeg_constants::{
    JPEG_MARKER_CODE_SIZE, JPEG_MARKER_END_OF_IMAGE, JPEG_MARKER_START_OF_SCAN,
};
use crate::jpeg_multi_picture::MultiPictureParameters;
use crate::jpeg_segment_scan::{JpegSegment, JpegSegmentScanner};
use crate::jpeg_source_mgr::JpegSourceMgr;
use crate::jpeg_xmp::make_xmp;
use crate::xmp::Xmp;

// Port of: src/codec/SkJpegConstants.h#L46-L47 (chrome/m156), `kXMPMarker`
const XMP_MARKER: u32 = 0xE0 + 1;
// Port of: src/codec/SkJpegConstants.h#L55-L56 (chrome/m156), `kExifMarker`, `kExifSig`
const EXIF_MARKER: u32 = 0xE0 + 1;
const EXIF_SIG: &[u8] = b"Exif\0";
// Port of: src/codec/SkJpegConstants.h#L30-L37 (chrome/m156), `kICCMarker`, `kICCSig`
const ICC_MARKER: u32 = 0xE0 + 2;
const ICC_SIG: &[u8] = b"ICC_PROFILE\0";
// Port of: src/codec/SkJpegConstants.h#L59-L60 (chrome/m156), `kMpfMarker`
const MPF_MARKER: u32 = 0xE0 + 2;
// Port of: src/codec/SkJpegConstants.h#L63-L64 (chrome/m156), `kISOGainmapMarker`, `kISOGainmapSig`
const ISO_GAINMAP_MARKER: u32 = 0xE0 + 2;
const ISO_GAINMAP_SIG: &[u8] = b"urn:iso:std:iso:ts:21496:-1\0";

/// One saved marker segment: its marker code and its parameters (`SkJpegMarker`).
// Port of: src/codec/SkJpegMetadataDecoderImpl.h (SkJpegMarker)
#[derive(Clone, Debug)]
pub struct JpegMarker {
    /// The marker code (`fMarker`).
    pub marker: u32,
    /// The segment's parameters, without the marker code and length (`fData`).
    pub data: Data,
}

/// The metadata of a JPEG, read from its saved APP1 and APP2 markers
/// (`SkJpegMetadataDecoderImpl`).
// Port of: src/codec/SkJpegMetadataDecoderImpl.h (SkJpegMetadataDecoderImpl)
#[derive(Clone, Debug, Default)]
pub struct JpegMetadataDecoder {
    marker_list: Vec<JpegMarker>,
}

// Port of: src/codec/SkJpegMetadataDecoderImpl.cpp#L292-L390 (chrome/m156), `marker_has_signature`
// and `read_metadata`: the parts of a metadata value that the segments of one marker hold.
//
// Metadata too big for one segment comes in parts, each with a big-endian index and count of
// `bytes_in_index` bytes. The parts are stitched together. Return None if parts are absent,
// duplicated, or disagree on their count.
fn read_metadata(
    marker_list: &[JpegMarker],
    target_marker: u32,
    signature: &[u8],
    signature_padding: usize,
    bytes_in_index: usize,
) -> Option<Data> {
    // The total size of the header: signature, padding, index and count.
    let header_size = signature.len() + signature_padding + 2 * bytes_in_index;

    // A map from part index to the data in each part.
    let mut parts: Vec<Option<Data>> = Vec::new();
    // Running total of the data in all parts.
    let mut parts_total_size = 0usize;
    let mut found_part_count: u32 = 0;
    let mut expected_part_count: u32 = 0;

    for marker in marker_list {
        // Skip segments that do not have the right marker or signature.
        if marker.marker != target_marker
            || marker.data.size() <= signature.len()
            || !marker.data.as_bytes().starts_with(signature)
        {
            continue;
        }

        // Skip segments that are too small to include the index and count.
        let data_length = marker.data.size();
        if data_length <= header_size {
            continue;
        }

        // Read this part's index and count as big-endian (or 1 and 1 without an index).
        let data = marker.data.as_bytes();
        let mut part_index: u32 = 0;
        let mut part_count: u32 = 0;
        if bytes_in_index == 0 {
            part_index = 1;
            part_count = 1;
        } else {
            let offset = signature.len() + signature_padding;
            for i in 0..bytes_in_index {
                part_index = (part_index << 8) + u32::from(data[offset + i]);
                part_count = (part_count << 8) + u32::from(data[offset + bytes_in_index + i]);
            }
        }

        // A part count of 0 is invalid.
        if part_count == 0 {
            return None;
        }

        // The indices must be in the range 1, ..., count.
        if part_index == 0 || part_index > part_count {
            return None;
        }

        // The first marker encountered sets the expected part count.
        if expected_part_count == 0 {
            expected_part_count = part_count;
            parts = vec![None; expected_part_count as usize];
        }

        // A count that does not match the expected count fails.
        if part_count != expected_part_count {
            return None;
        }

        // A reference to this part's data, after the header.
        let part_data = Data::new_subset(&marker.data, header_size, data_length - header_size);

        // Fail if duplicates are found.
        let slot = &mut parts[part_index as usize - 1];
        if slot.is_some() {
            return None;
        }

        // Save the part in its slot.
        parts_total_size += part_data.size();
        *slot = Some(part_data);
        found_part_count += 1;

        // Stop as soon as all of the parts are found.
        if found_part_count == expected_part_count {
            break;
        }
    }

    // No metadata is not an error.
    if expected_part_count == 0 {
        return None;
    }

    // Fail if not all of the parts are present.
    if found_part_count != expected_part_count {
        return None;
    }

    // Stitch the parts together.
    let mut result = Vec::with_capacity(parts_total_size);
    for part in parts.into_iter().flatten() {
        result.extend_from_slice(part.as_bytes());
    }
    Some(Data::new_from_vec(result))
}

// Port of: src/codec/SkJpegMetadataDecoderImpl.cpp#L35-L43 (chrome/m156), the collection of the
// APP1 parameters in `getXmpMetadata`.
fn xmp_app1_params(marker_list: &[JpegMarker]) -> Vec<Data> {
    marker_list
        .iter()
        .filter(|marker| marker.marker == XMP_MARKER)
        .map(|marker| marker.data.clone())
        .collect()
}

impl JpegMetadataDecoder {
    /// Port of `SkJpegMetadataDecoderImpl(SkJpegMarkerList)`: the decoder over saved markers.
    // Port of: src/codec/SkJpegMetadataDecoderImpl.cpp#L461-L465 (chrome/m156), the list
    // constructor
    #[must_use]
    pub fn from_marker_list(marker_list: Vec<JpegMarker>) -> Self {
        Self { marker_list }
    }

    /// Port of `SkJpegMetadataDecoderImpl(sk_sp<const SkData>)`: scans the header of the JPEG in
    /// `data` and keeps its APP1 and APP2 segments.
    // Port of: src/codec/SkJpegMetadataDecoderImpl.cpp (chrome/m156), the data constructor
    #[must_use]
    pub fn from_data(data: &Data) -> Self {
        let mut scan = JpegSegmentScanner::new(JPEG_MARKER_START_OF_SCAN);
        scan.on_bytes(data.as_bytes());
        if scan.had_error() || !scan.is_done() {
            return Self::default();
        }
        let mut marker_list = Vec::new();
        for segment in scan.segments() {
            // Save the APP1 and APP2 parameters (which include Exif, XMP, ICC, and MPF).
            if u32::from(segment.marker) != XMP_MARKER && u32::from(segment.marker) != ICC_MARKER {
                continue;
            }
            let Some(parameters) = JpegSegmentScanner::get_parameters(data, segment) else {
                continue;
            };
            marker_list.push(JpegMarker {
                marker: u32::from(segment.marker),
                data: parameters,
            });
        }
        Self { marker_list }
    }

    /// The XMP metadata of the APP1 segments, if it parses (`getXmpMetadata`).
    // Port of: src/codec/SkJpegMetadataDecoderImpl.cpp#L35-L43 (chrome/m156), getXmpMetadata
    #[must_use]
    pub fn xmp_metadata(&self) -> Option<Xmp> {
        make_xmp(&xmp_app1_params(&self.marker_list))
    }

    /// The Exif metadata (`getExifMetadata`).
    // Port of: src/codec/SkJpegMetadataDecoderImpl.cpp#L441-L449 (chrome/m156), getExifMetadata
    #[must_use]
    pub fn exif_metadata(&self) -> Option<Data> {
        read_metadata(&self.marker_list, EXIF_MARKER, EXIF_SIG, 1, 0)
    }

    /// The ICC profile data, stitched from its parts (`getICCProfileData`).
    // Port of: src/codec/SkJpegMetadataDecoderImpl.cpp#L451-L459 (chrome/m156), getICCProfileData
    #[must_use]
    pub fn icc_profile_data(&self) -> Option<Data> {
        read_metadata(&self.marker_list, ICC_MARKER, ICC_SIG, 0, 1)
    }

    /// The ISO 21496-1 gainmap metadata (`getISOGainmapMetadata`).
    // Port of: src/codec/SkJpegMetadataDecoderImpl.cpp#L461-L469 (chrome/m156),
    // getISOGainmapMetadata
    #[must_use]
    pub fn iso_gainmap_metadata(&self) -> Option<Data> {
        read_metadata(&self.marker_list, ISO_GAINMAP_MARKER, ISO_GAINMAP_SIG, 0, 0)
    }

    /// Whether the image might have a gainmap image: all supported gainmap formats require MPF
    /// (`mightHaveGainmapImage`).
    // Port of: src/codec/SkJpegMetadataDecoderImpl.cpp#L471-L478 (chrome/m156),
    // mightHaveGainmapImage
    #[must_use]
    pub fn might_have_gainmap_image(&self) -> bool {
        self.find_mp_params(None).is_some()
    }

    // Port of: src/codec/SkJpegMetadataDecoderImpl.cpp#L45-L91 (chrome/m156), find_mp_params.
    // Searches the MPF markers for parameters that parse. With a source manager, also returns the
    // segment that the parameters came from.
    fn find_mp_params(
        &self,
        source_mgr: Option<&mut JpegSourceMgr<'_>>,
    ) -> Option<(MultiPictureParameters, Option<JpegSegment>)> {
        let mut mp_params = None;
        let mut skipped_segment_count = 0usize;

        // Search the libjpeg segments until one parses as MP parameters, counting the MPF markers
        // skipped to get there.
        for marker in &self.marker_list {
            if marker.marker != MPF_MARKER {
                continue;
            }
            mp_params = MultiPictureParameters::make(&marker.data);
            if mp_params.is_some() {
                break;
            }
            skipped_segment_count += 1;
        }
        let mp_params = mp_params?;

        // Without a source manager, the segment is not looked up.
        let Some(source_mgr) = source_mgr else {
            return Some((mp_params, None));
        };

        // Find the scanner segment that corresponds to the libjpeg marker.
        let mut skipped = skipped_segment_count;
        for segment in source_mgr.get_all_segments().to_vec() {
            if u32::from(segment.marker) != MPF_MARKER {
                continue;
            }
            if skipped == 0 {
                return Some((mp_params, Some(segment)));
            }
            skipped -= 1;
        }
        None
    }

    /// Finds the gainmap image of the JPEG and its gainmap parameters: first among the
    /// Multi-Picture images, then at the location the container XMP suggests. Returns the
    /// gainmap image's data and its parameters (`findGainmapImage`).
    // Port of: src/codec/SkJpegMetadataDecoderImpl.cpp#L176-L247 (chrome/m156), findGainmapImage
    #[must_use]
    pub fn find_gainmap_image(
        &self,
        source_mgr: &mut JpegSourceMgr<'_>,
    ) -> Option<(Data, GainmapInfo)> {
        let base_exif_data = self.exif_metadata();
        let mut base_exif = exif::Metadata::default();
        exif::parse(&mut base_exif, base_exif_data.as_ref().map(Data::as_bytes));
        let xmp = self.xmp_metadata();

        // Whether a supported ISO 21496-1 gainmap version is present in the base image.
        let iso_gainmap_present = GainmapInfo::parse_version(self.iso_gainmap_metadata().as_ref());

        // Whether the Adobe HDR gainmap is indicated in the base image.
        let adobe_gainmap_present = xmp
            .as_ref()
            .is_some_and(|xmp| xmp.get_gainmap_info_adobe(None));

        // The container XMP's location. Its offset is relative to the end of the primary image's
        // EndOfImage marker, when the last segment is one.
        let mut container_gainmap_offset = 0usize;
        let mut container_gainmap_size = 0usize;
        if let Some((offset, size)) = xmp.as_ref().and_then(Xmp::container_gainmap_location) {
            container_gainmap_offset = offset;
            container_gainmap_size = size;
            if let Some(last) = source_mgr.get_all_segments().last().copied()
                && last.marker == JPEG_MARKER_END_OF_IMAGE
            {
                container_gainmap_offset += last.offset + JPEG_MARKER_CODE_SIZE;
            }
        }

        // First, search the Multi-Picture images. Skia's debug assertions, which check that the
        // container's location names the same image, are not ported.
        if let Some((mp_params, Some(mp_params_segment))) =
            self.find_mp_params(Some(&mut *source_mgr))
        {
            for mp_image_index in 1..mp_params.images.len() {
                let image = mp_params.images[mp_image_index];
                let mp_image_offset = MultiPictureParameters::get_image_absolute_offset(
                    image.data_offset,
                    mp_params_segment.offset,
                );
                if let Some(found) = extract_gainmap(
                    source_mgr,
                    mp_image_offset,
                    image.size as usize,
                    iso_gainmap_present,
                    adobe_gainmap_present,
                    base_exif.hdr_headroom,
                ) {
                    return Some(found);
                }
            }
        }

        // Next, try the location suggested by the container XMP.
        if container_gainmap_offset != 0 {
            return extract_gainmap(
                source_mgr,
                container_gainmap_offset,
                container_gainmap_size,
                false,
                adobe_gainmap_present,
                None,
            );
        }
        None
    }
}

// Attempt to extract a gainmap image from an offset and size in the decoder's source. The image
// is a gainmap only if its metadata gives the rendering parameters.
// Port of: src/codec/SkJpegMetadataDecoderImpl.cpp#L96-L174 (chrome/m156), extract_gainmap
fn extract_gainmap(
    decoder_source: &mut JpegSourceMgr<'_>,
    offset: usize,
    size: usize,
    base_image_has_iso_version: bool,
    base_image_has_adobe_xmp: bool,
    base_image_apple_hdr_headroom: Option<f32>,
) -> Option<(Data, GainmapInfo)> {
    // The data of this image.
    let image_data = decoder_source.get_subset_data(offset, size)?;

    // Parse the potential gainmap image's metadata.
    let metadata_decoder = JpegMetadataDecoder::from_data(&image_data);

    // If this image identifies itself as a gainmap, then `info` is populated.
    let mut did_populate_info = false;
    let mut info = GainmapInfo::default();

    // ISO 21496-1 gainmap metadata.
    if base_image_has_iso_version {
        did_populate_info =
            GainmapInfo::parse(metadata_decoder.iso_gainmap_metadata().as_ref(), &mut info);
        if did_populate_info && info.gainmap_math_color_space.is_some() {
            // The gainmap math colour space is that of the gainmap image's ICC profile. When it is
            // missing or does not parse, the base image's primaries are used (None).
            info.gainmap_math_color_space = metadata_decoder
                .icc_profile_data()
                .and_then(|icc| skia_rust_skcms::parse(icc.as_bytes()))
                .and_then(|profile| ColorSpace::make(&profile));
        }
    }

    if !did_populate_info {
        // The Adobe and Apple gainmap metadata require XMP. Parse it now.
        let xmp = metadata_decoder.xmp_metadata()?;

        // The Adobe gainmap metadata is checked only when the base image specifies it.
        if base_image_has_adobe_xmp {
            did_populate_info = xmp.get_gainmap_info_adobe(Some(&mut info));
        }

        // The Apple gainmap metadata does not require anything specific from the base image.
        if !did_populate_info && let Some(headroom) = base_image_apple_hdr_headroom {
            did_populate_info = xmp.get_gainmap_info_apple(headroom, &mut info);
        }
    }

    // None of the formats identified this image as a gainmap.
    if !did_populate_info {
        return None;
    }
    Some((image_data, info))
}
