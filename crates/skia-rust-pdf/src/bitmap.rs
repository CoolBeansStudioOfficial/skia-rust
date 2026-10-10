// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFBitmap.{h,cpp} (chrome/m156)

//! `SkPDFBitmap`: an image as an Image XObject. A JPEG that the decoder understands is embedded
//! as it is (DCT); anything else is written as deflated pixels with a soft mask for the alpha,
//! or re-encoded as a JPEG when it is opaque and the metadata asks for a lossy encoding.

use std::hash::{Hash, Hasher};

use skia_rust_codec::encode::icc::write_icc_profile_from_profile;
use skia_rust_codec::encoded_info::Color as EncodedColor;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::{DynamicMemoryWStream, NullWStream, WStream};
use skia_rust_skcms::{IccProfile, get_input_channel_count};

use crate::deflate::DeflateWStream;
use crate::document::DocHandle;
use crate::metadata::CompressionLevel;
use crate::types::{PdfArray, PdfDict, PdfIndirectReference, PdfUnion};

/// `SkPDFIccProfileKey`: an ICC profile and the number of channels of the image it is for.
// Port of: src/pdf/SkPDFBitmap.h#L26-L40 (chrome/m156)
#[doc(alias = "SkPDFIccProfileKey")]
#[derive(Debug, Clone)]
pub struct IccProfileKey {
    /// `fData`.
    pub data: Data,
    /// `fChannels`.
    pub channels: i32,
}

impl PartialEq for IccProfileKey {
    fn eq(&self, that: &Self) -> bool {
        self.channels == that.channels && self.data.equals(Some(&that.data))
    }
}

impl Eq for IccProfileKey {}

impl Hash for IccProfileKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.channels.hash(state);
        self.data.as_bytes().hash(state);
    }
}

/// The packed `SkColor` (`0xAARRGGBB`) at `(x, y)` of a `BGRA_8888` pixmap, read from the bytes
/// so that the byte order of the host does not matter.
fn bgra_color_at(bm: &Pixmap<'_>, x: i32, y: i32) -> u32 {
    let row_bytes = bm.row_bytes();
    let pixels = bm.addr().expect("a pixmap with pixels");
    let offset = y as usize * row_bytes + x as usize * 4;
    u32::from_le_bytes([
        pixels[offset],
        pixels[offset + 1],
        pixels[offset + 2],
        pixels[offset + 3],
    ])
}

/// It is necessary to average the color component of transparent pixels with their surrounding
/// neighbors since the PDF renderer may separately re-sample the alpha and color channels when
/// the image is not displayed at its native resolution. Since an alpha of zero gives no
/// information about the color component, the pathological case is a white image with sharp
/// transparency bounds - the color channel goes to black, and the should-be-transparent pixels
/// are rendered as grey because of the separate soft mask and color resizing. e.g.:
/// gm/bitmappremul.cpp
// Port of: src/pdf/SkPDFBitmap.cpp#L40-L66 (get_neighbor_avg_color, chrome/m156)
fn get_neighbor_avg_color(bm: &Pixmap<'_>, x_orig: i32, y_orig: i32) -> u32 {
    debug_assert_eq!(ColorType::BGRA8888, bm.color_type());
    let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
    // Clamp the range to the edge of the bitmap.
    let ymin = 0.max(y_orig - 1);
    let ymax = (y_orig + 1).min(bm.height() - 1);
    let xmin = 0.max(x_orig - 1);
    let xmax = (x_orig + 1).min(bm.width() - 1);
    for y in ymin..=ymax {
        for x in xmin..=xmax {
            let color = bgra_color_at(bm, x, y);
            if color != 0 {
                r += (color >> 16) & 0xFF;
                g += (color >> 8) & 0xFF;
                b += color & 0xFF;
                n += 1;
            }
        }
    }
    if n > 0 {
        // SkColorSetRGB(SkToU8(r / n), SkToU8(g / n), SkToU8(b / n))
        0xFF00_0000 | ((r / n) << 16) | ((g / n) << 8) | (b / n)
    } else {
        0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamFormat {
    Dct,
    Flate,
    Uncompressed,
}

// Port of: src/pdf/SkPDFBitmap.cpp#L77-L106 (emit_image_stream, chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ template
fn emit_image_stream(
    doc: &DocHandle,
    reference: PdfIndirectReference,
    content: &[u8],
    size: ISize,
    color_space: PdfUnion,
    s_mask: PdfIndirectReference,
    length: usize,
    format: StreamFormat,
) {
    if !reference.is_valid() {
        return;
    }
    let mut pdf_dict = PdfDict::new(Some("XObject"));
    pdf_dict.insert_name("Subtype", "Image");
    pdf_dict.insert_int("Width", size.width);
    pdf_dict.insert_int("Height", size.height);
    pdf_dict.insert_union("ColorSpace", color_space);
    if s_mask.is_valid() {
        pdf_dict.insert_ref("SMask", s_mask);
    }
    pdf_dict.insert_int("BitsPerComponent", 8);
    match format {
        StreamFormat::Dct => pdf_dict.insert_name("Filter", "DCTDecode"),
        StreamFormat::Flate => pdf_dict.insert_name("Filter", "FlateDecode"),
        StreamFormat::Uncompressed => {}
    }
    if format == StreamFormat::Dct {
        pdf_dict.insert_int("ColorTransform", 0);
    }
    pdf_dict.insert_int_usize("Length", length);
    doc.emit_stream(&pdf_dict, content, reference);
}

fn stream_format(compression_level: CompressionLevel) -> StreamFormat {
    if compression_level == CompressionLevel::None {
        StreamFormat::Uncompressed
    } else {
        StreamFormat::Flate
    }
}

// Port of: src/pdf/SkPDFBitmap.cpp#L108-L152 (do_deflated_alpha, chrome/m156)
fn do_deflated_alpha(pm: &Pixmap<'_>, doc: &DocHandle, reference: PdfIndirectReference) -> usize {
    let compression_level = doc.metadata().compression_level;
    let format = stream_format(compression_level);
    let mut buffer = DynamicMemoryWStream::new();
    // The writes of Skia, in the same chunks.
    let mut chunks: Vec<Vec<u8>> = Vec::new();
    if ColorType::Alpha8 == pm.color_type() {
        debug_assert_eq!(pm.row_bytes(), pm.width() as usize);
        let count = (pm.width() * pm.height()) as usize;
        chunks.push(pm.addr().expect("pixels")[..count].to_vec());
    } else {
        debug_assert_eq!(pm.alpha_type(), AlphaType::Unpremul);
        debug_assert_eq!(pm.color_type(), ColorType::BGRA8888);
        debug_assert_eq!(pm.row_bytes(), pm.width() as usize * 4);
        let total = (pm.height() * pm.width()) as usize;
        let pixels = pm.addr().expect("pixels");

        let mut byte_buffer = [0u8; 4092];
        let mut dst = 0usize;
        for i in 0..total {
            // `0xFF & (*ptr++ >> SK_BGRA_A32_SHIFT)`: the alpha, the fourth byte.
            byte_buffer[dst] = pixels[i * 4 + 3];
            dst += 1;
            if dst == byte_buffer.len() {
                chunks.push(byte_buffer.to_vec());
                dst = 0;
            }
        }
        chunks.push(byte_buffer[..dst].to_vec());
    }
    if format == StreamFormat::Flate {
        let mut deflate = DeflateWStream::new(Some(&mut buffer), compression_level as i32, false);
        for chunk in &chunks {
            deflate.write(chunk);
        }
        deflate.finalize();
    } else {
        for chunk in &chunks {
            buffer.write(chunk);
        }
    }

    let length = buffer.bytes_written();
    let data = buffer.detach_as_vector();
    emit_image_stream(
        doc,
        reference,
        &data,
        pm.dimensions(),
        PdfUnion::name("DeviceGray"),
        PdfIndirectReference::default(),
        length,
        format,
    );
    length
}

// Port of: src/pdf/SkPDFBitmap.cpp#L154-L178 (write_icc_profile, chrome/m156)
fn write_icc_profile(doc: &DocHandle, icc: Data, channels: i32) -> PdfUnion {
    let key = IccProfileKey {
        data: icc.clone(),
        channels,
    };
    let existing = doc.with(|d| d.icc_profile_map.get(&key).copied());
    let icc_stream_ref = if let Some(reference) = existing {
        reference
    } else {
        let mut icc_stream_dict = PdfDict::new(None);
        icc_stream_dict.insert_int("N", channels);
        let reference = doc.stream_out(Some(icc_stream_dict), icc.as_bytes(), true);
        doc.with(|d| d.icc_profile_map.insert(key, reference));
        reference
    };

    let mut icc_pdf = PdfArray::new();
    icc_pdf.append_name("ICCBased");
    icc_pdf.append_ref(icc_stream_ref);
    PdfUnion::object(Box::new(icc_pdf))
}

// Port of: src/pdf/SkPDFBitmap.cpp#L180-L187 (icc_channel_mismatch, chrome/m156)
fn icc_channel_mismatch(icc_profile: Option<&IccProfile>, expected_channels: i32) -> bool {
    let mut icc_channels = -1;
    if let Some(icc_profile) = icc_profile {
        icc_channels = get_input_channel_count(icc_profile);
    }
    0 < icc_channels && expected_channels != icc_channels
}

/// `SkWriteICCProfile(&profile, "")` wrapped in data.
fn icc_data_for_profile(profile: &IccProfile) -> Option<Data> {
    write_icc_profile_from_profile(profile, "").map(Data::new_from_vec)
}

// Port of: src/pdf/SkPDFBitmap.cpp#L189-L297 (do_deflated_image, chrome/m156)
fn do_deflated_image(
    pm: &Pixmap<'_>,
    doc: &DocHandle,
    is_opaque: bool,
    reference: PdfIndirectReference,
) -> usize {
    let compression_level = doc.metadata().compression_level;
    let format = stream_format(compression_level);
    let mut dynamic = DynamicMemoryWStream::new();
    let mut write_only = NullWStream::new();
    let mut color_space = PdfUnion::name("DeviceGray");
    let channels;
    let length;
    {
        let buffer: &mut dyn WStream = if reference.is_valid() {
            &mut dynamic
        } else {
            &mut write_only
        };
        // A deflater, or a pass-through to the buffer.
        let mut chunks: Vec<Vec<u8>> = Vec::new();
        match pm.color_type() {
            ColorType::Alpha8 => {
                channels = 1;
                // `fill_stream(stream, '\x00', width * height)`
                let n = (pm.width() * pm.height()) as usize;
                for _ in 0..n / 4096 {
                    chunks.push(vec![0u8; 4096]);
                }
                chunks.push(vec![0u8; n % 4096]);
            }
            ColorType::Gray8 => {
                channels = 1;
                debug_assert!(is_opaque);
                debug_assert_eq!(pm.row_bytes(), pm.width() as usize);
                let n = (pm.width() * pm.height()) as usize;
                chunks.push(pm.addr().expect("pixels")[..n].to_vec());
            }
            _ => {
                color_space = PdfUnion::name("DeviceRGB");
                channels = 3;
                debug_assert_eq!(pm.alpha_type(), AlphaType::Unpremul);
                debug_assert_eq!(pm.color_type(), ColorType::BGRA8888);
                debug_assert_eq!(pm.row_bytes(), pm.width() as usize * 4);
                let mut byte_buffer = [0u8; 3072];
                let mut dst = 0usize;
                for y in 0..pm.height() {
                    for x in 0..pm.width() {
                        let mut color = bgra_color_at(pm, x, y);
                        if (color >> 24) == 0 {
                            color = get_neighbor_avg_color(pm, x, y);
                        }
                        byte_buffer[dst] = ((color >> 16) & 0xFF) as u8;
                        byte_buffer[dst + 1] = ((color >> 8) & 0xFF) as u8;
                        byte_buffer[dst + 2] = (color & 0xFF) as u8;
                        dst += 3;
                        if dst == byte_buffer.len() {
                            chunks.push(byte_buffer.to_vec());
                            dst = 0;
                        }
                    }
                }
                chunks.push(byte_buffer[..dst].to_vec());
            }
        }
        if format == StreamFormat::Flate {
            let mut deflate = DeflateWStream::new(Some(buffer), compression_level as i32, false);
            for chunk in &chunks {
                deflate.write(chunk);
            }
            deflate.finalize();
            drop(deflate);
        } else {
            for chunk in &chunks {
                buffer.write(chunk);
            }
        }
        length = buffer.bytes_written();
    }

    if let Some(color_space_ref) = pm.color_space() {
        let icc_profile = color_space_ref.to_profile();
        if !icc_channel_mismatch(Some(&icc_profile), channels) {
            if let Some(icc_data) = icc_data_for_profile(&icc_profile) {
                color_space = write_icc_profile(doc, icc_data, channels);
            }
        }
    }

    let mut s_mask = PdfIndirectReference::default();
    if !is_opaque && reference.is_valid() {
        s_mask = doc.reserve_ref();
    }
    let data = dynamic.detach_as_vector();
    emit_image_stream(
        doc,
        reference,
        &data,
        pm.dimensions(),
        color_space,
        s_mask,
        length,
        format,
    );
    let mut length = length;
    if !is_opaque {
        length += do_deflated_alpha(pm, doc, s_mask);
    }
    length
}

// Port of: src/pdf/SkPDFBitmap.cpp#L299-L371 (do_jpeg, chrome/m156)
fn do_jpeg(
    data: Data,
    image_color_space: Option<&ColorSpace>,
    doc: &DocHandle,
    size: ISize,
    reference: PdfIndirectReference,
) -> usize {
    if !reference.is_valid() {
        return data.size();
    }

    let Some(decode_jpeg) = doc.metadata().jpeg_decoder else {
        return 0;
    };
    let Some(codec) = decode_jpeg(data.clone()) else {
        return 0;
    };

    let jpeg_size = codec.dimensions();
    let encoded_info = codec.encoded_info();
    let jpeg_color_type = encoded_info.color();
    let exif_orientation = codec.origin();

    let yuv = jpeg_color_type == EncodedColor::YUV;
    let good_color_type = yuv || jpeg_color_type == EncodedColor::Gray;
    if jpeg_size != size // Safety check.
        || !good_color_type
        || EncodedOrigin::TopLeft != exif_orientation
    {
        return 0;
    }

    let channels = if yuv { 3 } else { 1 };
    let mut color_space = if yuv {
        PdfUnion::name("DeviceRGB")
    } else {
        PdfUnion::name("DeviceGray")
    };

    if let (Some(encoded_icc_profile_data), false) = (
        encoded_info.profile_data(),
        icc_channel_mismatch(encoded_info.profile(), channels),
    ) {
        color_space = write_icc_profile(
            doc,
            Data::new_copy(encoded_icc_profile_data),
            channels,
        );
    } else if let Some(codec_icc_profile) = encoded_info
        .profile()
        .filter(|p| !icc_channel_mismatch(Some(p), channels))
    {
        if let Some(codec_icc_data) = icc_data_for_profile(codec_icc_profile) {
            color_space = write_icc_profile(doc, codec_icc_data, channels);
        }
    } else if let Some(image_color_space) = image_color_space {
        let image_icc_profile = image_color_space.to_profile();
        if !icc_channel_mismatch(Some(&image_icc_profile), channels) {
            if let Some(image_icc_data) = icc_data_for_profile(&image_icc_profile) {
                color_space = write_icc_profile(doc, image_icc_data, channels);
            }
        }
    }

    emit_image_stream(
        doc,
        reference,
        data.as_bytes(),
        jpeg_size,
        color_space,
        PdfIndirectReference::default(),
        data.size(),
        StreamFormat::Dct,
    );
    data.size()
}

// Port of: src/pdf/SkPDFBitmap.cpp#L373-L396 (to_pixels, chrome/m156)
fn to_pixels(image: &Image) -> Bitmap {
    let mut bm = Bitmap::new();
    let (w, h) = (image.width(), image.height());
    match image.color_type() {
        ColorType::Alpha8 => {
            bm.alloc_pixels_info(&ImageInfo::new_a8((w, h)), None);
        }
        ColorType::Gray8 => {
            bm.alloc_pixels_info(
                &ImageInfo::new((w, h), ColorType::Gray8, AlphaType::Opaque, None),
                None,
            );
        }
        _ => {
            // TODO: makeColorSpace(sRGB) or actually tag the images
            let at = if bm.is_opaque() {
                AlphaType::Opaque
            } else {
                AlphaType::Unpremul
            };
            bm.alloc_pixels_info(
                &ImageInfo::new((w, h), ColorType::BGRA8888, at, image.color_space()),
                None,
            );
        }
    }
    // TODO: support GPU images in PDFs
    let read = bm
        .peek_pixels_mut()
        .is_some_and(|mut pixmap| image.read_pixels_to_pixmap(&mut pixmap, (0, 0)));
    if !read {
        bm.erase_color(skia_rust_core::color::Color::from_argb(0xFF, 0, 0, 0));
    }
    bm
}

// Port of: src/pdf/SkPDFBitmap.cpp#L398-L428 (serialize_image, chrome/m156)
fn serialize_image(
    img: &Image,
    encoding_quality: i32,
    doc: &DocHandle,
    reference: PdfIndirectReference,
) -> usize {
    debug_assert!(encoding_quality >= 0);
    let dimensions = img.dimensions();

    if let Some(data) = img.ref_encoded_data() {
        let size = do_jpeg(data, img.color_space().as_ref(), doc, dimensions, reference);
        if size != 0 {
            return size;
        }
    }
    let bm = to_pixels(img);
    let pm = bm.pixmap();
    let encode_jpeg = doc.metadata().jpeg_encoder;

    let is_opaque = pm.is_opaque() || pm.compute_is_opaque();
    if let Some(encode_jpeg) = encode_jpeg
        && encoding_quality <= 100
        && is_opaque
    {
        let mut stream = DynamicMemoryWStream::new();
        if encode_jpeg(&mut stream, &pm, encoding_quality) {
            let size = do_jpeg(
                stream.detach_as_data(),
                pm.color_space().as_ref(),
                doc,
                dimensions,
                reference,
            );
            if size != 0 {
                return size;
            }
        }
    }
    do_deflated_image(&pm, doc, is_opaque, reference)
}

/// `SkPDFSerializeImageSize`: the number of bytes the image takes in the document, without
/// writing it.
// Port of: src/pdf/SkPDFBitmap.cpp#L432-L434 (chrome/m156)
#[doc(alias = "SkPDFSerializeImageSize")]
#[must_use]
pub fn serialize_image_size(img: &Image, doc: &DocHandle, encoding_quality: i32) -> usize {
    serialize_image(img, encoding_quality, doc, PdfIndirectReference::default())
}

/// `SkPDFSerializeImage`: serializes an image as an Image XObject. A `encoding_quality` over 100
/// means lossless.
// Port of: src/pdf/SkPDFBitmap.cpp#L436-L452 (chrome/m156)
#[doc(alias = "SkPDFSerializeImage")]
#[must_use]
pub fn serialize_image_xobject(
    img: &Image,
    doc: &DocHandle,
    encoding_quality: i32,
) -> PdfIndirectReference {
    let reference = doc.reserve_ref();
    serialize_image(img, encoding_quality, doc, reference);
    reference
}
