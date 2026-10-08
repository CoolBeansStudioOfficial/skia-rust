// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkReadBuffer.h, src/core/SkReadBuffer.cpp (the subset below)

//! `SkReadBuffer`: deserialization of what `SkBinaryWriteBuffer` wrote.
//!
//! Ported: the cursor (`skip`, `available`, `isValid`, `validate`), the version check,
//! `readInt`/`readUInt`/`read32`/`read32LE`/`readScalar`, `readPad32`, `readByteArray`,
//! `skipByteArray`, `readScalarArray`, `readString`, `readMatrix`, `readPath`, the flattenable
//! readers for path effects and mask filters, and the empty case of `readTypeface`. Not ported:
//! the deserial procs, the factory array, typeface-table and image readers, the recursion limit,
//! and every read of a type that is not ported yet (paints, ...).
//!
//! skia-rust: `SkReadBuffer` requires its memory to be 4-byte aligned because it reads words in
//! place; here the words are read from the bytes of a slice, so only the offsets are checked.

use crate::alpha_type::AlphaType;
use crate::bitmap::Bitmap;
use crate::blend_mode::BlendMode;
use crate::color::Color;
use crate::color::Color4f;
use crate::color_space_priv::srgb_singleton;
use crate::data::Data;
use crate::flattenable::FlattenableRegistry;
use crate::image::{Image, RequiredProperties};
use crate::images;
use crate::mask_filter::MaskFilter;
use crate::matrix::Matrix;
use crate::paint::{Cap, Join, Paint, Style};
use crate::path::Path;
use crate::path_effect::PathEffect;
use crate::picture_priv::{VERSION_ANISOTROPIC_FILTER, VERSION_SK_BLENDER_IN_SK_PAINT};
use crate::point::Point;
use crate::rect::{IRect, Rect};
use crate::rrect::RRect;
use crate::sampling_options::{CubicResampler, FilterMode, MipmapMode, SamplingOptions};
use crate::serial_procs::DeserialProcs;
use crate::stream::MemoryStream;
use crate::typeface::Typeface;
use crate::write_buffer::{CUSTOM_BLEND_MODE_SENTINEL, FLAT_HAS_EFFECTS};

/// The image flags of `SkWriteBufferImageFlags`: the subset rect, the mipmaps, and unpremultiplied.
// Port of: src/core/SkWriteBuffer.h#L167-L174 (chrome/m156)
const IMAGE_FLAG_HAS_SUBSET: u32 = 1 << 8;
const IMAGE_FLAG_HAS_MIPMAP: u32 = 1 << 9;
const IMAGE_FLAG_UNPREMUL: u32 = 1 << 10;

/// `SkReadBuffer::MakeEmptyImage(1, 1)`: the image that stands for an image that could not be
/// read. Skia's is a lazy image whose generator fails, which draws nothing; this is a transparent
/// 1x1 raster image. It draws nothing with source-over, and it is recorded like any image, so
/// a picture that has it keeps its op count. (Blend modes that keep the destination where the
/// source is transparent draw differently.)
// Port of: src/core/SkReadBuffer.cpp#L40-L49 (chrome/m156), MakeEmptyImage
fn make_empty_image() -> Image {
    let mut bitmap = Bitmap::new();
    bitmap.alloc_n32_pixels((1, 1), None);
    bitmap.erase_color(Color::TRANSPARENT);
    // A 1x1 bitmap with pixels always makes an image.
    images::raster_from_bitmap(&bitmap).expect("a 1x1 raster image")
}

/// Rounds `x` up to a multiple of 4 (`SkAlign4`), wrapping like the unsigned arithmetic of C++.
fn align4(x: usize) -> usize {
    x.wrapping_add(3) & !3
}

/// Unpacks the word that `pack_v68` made (`unpack_v68`): the anti-alias, dither, blend mode, cap,
/// join and style go into `paint`, and the flat flags are returned. `safe` is cleared if a field
/// is out of range (`SkSafeRange`).
// Port of: src/core/SkPaintPriv.cpp#L234-L256 (chrome/m156)
fn unpack_v68(paint: &mut Paint, packed: u32, safe: &mut bool) -> u8 {
    paint.set_anti_alias(packed & 1 != 0);
    paint.set_dither(packed & 2 != 0);
    let mode = (packed >> 8) & 0xFF;
    if mode != u32::from(CUSTOM_BLEND_MODE_SENTINEL) {
        // The sentinel stands for a custom blender, which is read with the effects.
        match i32::try_from(mode).ok().and_then(BlendMode::from_i32) {
            Some(blend_mode) => {
                paint.set_blend_mode(blend_mode);
            }
            None => *safe = false,
        }
    }
    let cap = match (packed >> 16) & 0x3 {
        0 => Cap::Butt,
        1 => Cap::Round,
        2 => Cap::Square,
        _ => {
            *safe = false;
            Cap::Butt
        }
    };
    paint.set_stroke_cap(cap);
    let join = match (packed >> 18) & 0x3 {
        0 => Join::Miter,
        1 => Join::Round,
        2 => Join::Bevel,
        _ => {
            *safe = false;
            Join::Miter
        }
    };
    paint.set_stroke_join(join);
    let style = match (packed >> 20) & 0x3 {
        0 => Style::Fill,
        1 => Style::Stroke,
        2 => Style::StrokeAndFill,
        _ => {
            *safe = false;
            Style::Fill
        }
    };
    paint.set_style(style);
    // The old filter quality bits (22..24) are skipped.
    // The flat flags are the top byte.
    u8::try_from(packed >> 24).unwrap_or(0)
}

/// Reads primitives from a memory block of 4-byte words (`SkReadBuffer`). Reading past the end,
/// or any [`validate`](Self::validate) with a false condition, makes the buffer invalid for good.
// Port of: src/core/SkReadBuffer.h#L55-L286 (chrome/m156)
#[doc(alias = "SkReadBuffer")]
#[derive(Clone, Debug, Default)]
pub struct ReadBuffer<'a> {
    data: &'a [u8],
    /// `fCurr - fBase`: the current position.
    curr: usize,
    version: u32,
    error: bool,
    /// The names read so far (`fFlattenableDict`); the index of a name is its position plus one.
    flattenable_names: Vec<String>,
    /// `fProcs`: how the typefaces (and later the images) are read back.
    deserial_procs: DeserialProcs,
    /// `fTFArray`: the typefaces that index references name, in order (1 is the first).
    typeface_array: Vec<Typeface>,
    /// `fFactoryArray`: the factories that an index references, by name, in order (1 is the
    /// first). Empty when the flattenables are read by name.
    factory_names: Vec<String>,
}

impl<'a> ReadBuffer<'a> {
    /// A buffer reading `data` (`SkReadBuffer(data, size)`). The size must be a multiple of 4,
    /// or the buffer is invalid.
    #[must_use]
    pub fn new(data: &'a [u8]) -> ReadBuffer<'a> {
        let mut buffer = ReadBuffer::default();
        buffer.set_memory(data);
        buffer
    }

    /// `SkReadBuffer(data, size)` followed by `setDeserialProcs(procs)`: a buffer that reads
    /// the typefaces of `data` with `procs`.
    // Port of: src/core/SkReadBuffer.h (setDeserialProcs, chrome/m156)
    #[must_use]
    pub fn with_deserial_procs(data: &'a [u8], deserial_procs: DeserialProcs) -> ReadBuffer<'a> {
        let mut buffer = ReadBuffer::new(data);
        buffer.deserial_procs = deserial_procs;
        buffer
    }

    /// `setTypefaceArray(array, count)`: the typefaces that the index arm of a typeface refers to.
    // Port of: src/core/SkReadBuffer.h (setTypefaceArray, chrome/m156)
    #[doc(alias = "setTypefaceArray")]
    pub fn set_typeface_array(&mut self, typefaces: Vec<Typeface>) {
        self.typeface_array = typefaces;
    }

    /// `setFactoryArray(array, count)`: the factories that a flattenable's index refers to, by
    /// their names, in order (index 1 is the first). An empty table means the flattenables are
    /// read by name.
    // Port of: src/core/SkReadBuffer.h#L180 (chrome/m156), setFactoryArray
    #[doc(alias = "setFactoryArray")]
    pub(crate) fn set_factory_names(&mut self, names: Vec<String>) {
        self.factory_names = names;
    }

    /// `readPoint`: two scalars (`SkReadBuffer::readPoint`).
    // Port of: src/core/SkReadBuffer.cpp#L175-L178 (chrome/m156)
    #[doc(alias = "readPoint")]
    pub fn read_point(&mut self) -> Point {
        let x = self.read_scalar();
        let y = self.read_scalar();
        Point::new(x, y)
    }

    /// `readRect`: four scalars. A short read gives the empty rectangle.
    // Port of: src/core/SkReadBuffer.cpp#L213-L217 (chrome/m156)
    #[doc(alias = "readRect")]
    pub fn read_rect(&mut self) -> Rect {
        let left = self.read_scalar();
        let top = self.read_scalar();
        let right = self.read_scalar();
        let bottom = self.read_scalar();
        if self.is_valid() {
            Rect {
                left,
                top,
                right,
                bottom,
            }
        } else {
            // `rect->setEmpty()`
            Rect {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            }
        }
    }

    /// Makes the buffer read `data` (`setMemory`).
    // Port of: src/core/SkReadBuffer.cpp#L53-L59 (chrome/m156)
    #[doc(alias = "setMemory")]
    pub fn set_memory(&mut self, data: &'a [u8]) {
        self.validate(align4(data.len()) == data.len());
        if !self.error {
            self.data = data;
            self.curr = 0;
        }
    }

    /// True if the version is older than `target_version` (`isVersionLT`).
    // Port of: src/core/SkReadBuffer.h#L64-L69 (chrome/m156)
    #[doc(alias = "isVersionLT")]
    #[must_use]
    pub fn is_version_lt(&self, target_version: u32) -> bool {
        debug_assert!(target_version > 0);
        self.version > 0 && self.version < target_version
    }

    /// The version (`getVersion`).
    #[doc(alias = "getVersion")]
    #[must_use]
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Sets the version; at most once (`setVersion`).
    // Port of: src/core/SkReadBuffer.h#L74-L77 (chrome/m156)
    #[doc(alias = "setVersion")]
    pub fn set_version(&mut self, version: u32) {
        debug_assert!(self.version == 0 || version == self.version);
        self.version = version;
    }

    /// The number of bytes left to read (`available`).
    // Port of: src/core/SkReadBuffer.h#L85 (chrome/m156)
    #[must_use]
    pub fn available(&self) -> usize {
        self.data.len() - self.curr
    }

    /// `isAvailable(size)`.
    // Port of: src/core/SkReadBuffer.h#L258 (chrome/m156)
    fn is_available(&self, size: usize) -> bool {
        size <= self.available()
    }

    /// Marks the buffer invalid and sends the read cursor to the end (`setInvalid`).
    // Port of: src/core/SkReadBuffer.cpp#L61-L67 (chrome/m156)
    fn set_invalid(&mut self) {
        if !self.error {
            // When an error is found, send the read cursor to the end of the stream
            self.curr = self.data.len();
            self.error = true;
        }
    }

    /// If `is_valid` is false, makes the buffer invalid. Returns whether the buffer is still
    /// valid (`validate`).
    // Port of: src/core/SkReadBuffer.h#L201-L206 (chrome/m156)
    pub fn validate(&mut self, is_valid: bool) -> bool {
        if !is_valid {
            self.set_invalid();
        }
        !self.error
    }

    /// Whether nothing has gone wrong (`isValid`).
    #[doc(alias = "isValid")]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.error
    }

    /// Skips `size` bytes (rounded up to a multiple of 4) and returns them, or `None` (and an
    /// invalid buffer) if there are not as many (`skip`).
    // Port of: src/core/SkReadBuffer.cpp#L69-L82 (chrome/m156)
    pub fn skip(&mut self, size: usize) -> Option<&'a [u8]> {
        let inc = align4(size);
        self.validate(inc >= size);
        let addr = self.curr;
        self.validate(self.curr.is_multiple_of(4) && self.is_available(inc));
        if self.error {
            return None;
        }

        self.curr += inc;
        Some(&self.data[addr..addr + inc])
    }

    /// Reads an `i32` (`readInt`).
    // Port of: src/core/SkReadBuffer.cpp#L102-L110 (chrome/m156)
    #[doc(alias = "readInt")]
    pub fn read_int(&mut self) -> i32 {
        const INC: usize = size_of::<i32>();
        if !self.validate(self.curr.is_multiple_of(4) && self.is_available(INC)) {
            return 0;
        }
        let bytes = &self.data[self.curr..self.curr + INC];
        let value = i32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        self.curr += INC;
        value
    }

    /// Reads a `u32` (`readUInt`).
    // Port of: src/core/SkReadBuffer.cpp#L122-L124 (chrome/m156)
    #[doc(alias = "readUInt")]
    pub fn read_uint(&mut self) -> u32 {
        u32::from_ne_bytes(self.read_int().to_ne_bytes())
    }

    /// Reads a boolean, which must be stored as 0 or 1: any other value makes the buffer invalid
    /// (`readBool`).
    // Port of: src/core/SkReadBuffer.cpp#L91-L96 (chrome/m156)
    #[doc(alias = "readBool")]
    pub fn read_bool(&mut self) -> bool {
        let value = self.read_uint();
        // Boolean value should be either 0 or 1
        self.validate(value & !1 == 0);
        value != 0
    }

    /// Reads an `i32` (`read32`).
    // Port of: src/core/SkReadBuffer.cpp#L126-L128 (chrome/m156)
    pub fn read32(&mut self) -> i32 {
        self.read_int()
    }

    /// Reads a scalar stored as its bit pattern (`readScalar`). Returns 0 if the buffer is
    /// invalid.
    // Port of: src/core/SkReadBuffer.cpp#L112-L120 (chrome/m156)
    #[doc(alias = "readScalar")]
    pub fn read_scalar(&mut self) -> f32 {
        const INC: usize = size_of::<f32>();
        if !self.validate(self.curr.is_multiple_of(4) && self.is_available(INC)) {
            return 0.0;
        }
        let bytes = &self.data[self.curr..self.curr + INC];
        let value = f32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        self.curr += INC;
        value
    }

    /// Reads a typeface reference (`readTypeface`). The index arm is invalid: the buffer has no
    /// typeface table yet (it arrives with picture serialization, T16). The custom arm reads the
    /// bytes and hands them to the deserial proc; without bytes or a proc the buffer is invalid.
    // Port of: src/core/SkReadBuffer.cpp#L443-L466 (chrome/m156), the `fTFCount == 0` index arm
    // and the custom arm
    #[doc(alias = "readTypeface")]
    pub fn read_typeface(&mut self) -> Option<Typeface> {
        // 0 -- return null (empty font); >0 -- index; <0 -- custom: negative size in bytes.
        let index = self.read_int();
        if index == 0 {
            return None;
        }
        if index > 0 {
            // The index names a typeface of the array, 1-based.
            let typeface = usize::try_from(index)
                .ok()
                .and_then(|i| self.typeface_array.get(i - 1).cloned());
            if typeface.is_none() {
                self.validate(false);
            }
            return typeface;
        }
        let size = usize::try_from(index.unsigned_abs()).unwrap_or(usize::MAX);
        let bytes = self.skip(size);
        let read = self.deserial_procs.typeface.clone();
        let (Some(bytes), Some(read)) = (bytes, read) else {
            self.validate(false);
            return None;
        };
        // C++ reads the bytes through an `SkMemoryStream` over them; the stream owns a copy here.
        let mut stream = MemoryStream::make_copy(bytes);
        read(&mut *stream)
    }

    /// Reads `buffer.len()` bytes, skipping the padding up to a multiple of 4 (`readPad32`).
    // Port of: src/core/SkReadBuffer.cpp#L138-L146 (chrome/m156)
    #[doc(alias = "readPad32")]
    pub fn read_pad32(&mut self, buffer: &mut [u8]) -> bool {
        if let Some(src) = self.skip(buffer.len()) {
            buffer.copy_from_slice(&src[..buffer.len()]);
            return true;
        }
        false
    }

    /// `readArray`: reads the count of the elements, which must be `size`, then the bytes.
    // Port of: src/core/SkReadBuffer.cpp#L286-L290 (chrome/m156)
    fn read_array(&mut self, value: &mut [u8], size: usize) -> bool {
        let count = self.read_uint();
        self.validate(usize::try_from(count).ok() == Some(size)) && self.read_pad32(value)
    }

    /// Reads `value.len()` bytes written by `writeByteArray` (`readByteArray`).
    // Port of: src/core/SkReadBuffer.cpp#L292-L294 (chrome/m156)
    #[doc(alias = "readByteArray")]
    pub fn read_byte_array(&mut self, value: &mut [u8]) -> bool {
        let size = value.len();
        self.read_array(value, size)
    }

    /// Skips a byte array written by `writeByteArray` and returns its bytes (with the padding)
    /// and its size, which is 0 if the buffer is invalid (`skipByteArray`).
    // Port of: src/core/SkReadBuffer.cpp#L316-L323 (chrome/m156)
    #[doc(alias = "skipByteArray")]
    pub fn skip_byte_array(&mut self) -> (Option<&'a [u8]>, usize) {
        let count = self.read_uint();
        let buf = usize::try_from(count).ok().and_then(|c| self.skip(c));
        let size = if self.is_valid() {
            usize::try_from(count).unwrap_or(0)
        } else {
            0
        };
        (buf, size)
    }
}

impl ReadBuffer<'_> {
    /// `readByteArrayAsData`: the bytes written by `writeByteArray`, as data. `None`, and the
    /// buffer is invalid, if the bytes are not all there.
    // Port of: src/core/SkReadBuffer.cpp#L325-L336 (chrome/m156)
    #[doc(alias = "readByteArrayAsData")]
    pub fn read_byte_array_as_data(&mut self) -> Option<Data> {
        let num_bytes = usize::try_from(self.get_array_count()).unwrap_or(usize::MAX);
        if !self.validate(self.is_available(num_bytes)) {
            return None;
        }
        let mut bytes = vec![0; num_bytes];
        if !self.read_byte_array(&mut bytes) {
            return None;
        }
        Some(Data::new_from_vec(bytes))
    }

    /// `readIRect`: the left, top, right and bottom words.
    // Port of: src/core/SkReadBuffer.cpp#L220-L225 (chrome/m156), readIRect
    #[doc(alias = "readIRect")]
    pub fn read_irect(&mut self) -> IRect {
        let left = self.read_int();
        let top = self.read_int();
        let right = self.read_int();
        let bottom = self.read_int();
        IRect {
            left,
            top,
            right,
            bottom,
        }
    }

    /// `readSampling`: the sampling that `write_sampling` wrote. A zero anisotropy is followed by
    /// the cubic flag and its coefficients, or by the filter and mipmap modes.
    // Port of: src/core/SkReadBuffer.cpp#L227-L243 (chrome/m156)
    #[doc(alias = "readSampling")]
    pub fn read_sampling(&mut self) -> SamplingOptions {
        if !self.is_version_lt(VERSION_ANISOTROPIC_FILTER) {
            let max_aniso = self.read_int();
            if max_aniso != 0 {
                return SamplingOptions::from_aniso(max_aniso);
            }
        }
        if self.read_bool() {
            let b = self.read_scalar();
            let c = self.read_scalar();
            // `SkSamplingOptions({B, C})`: the cubic filter, with the default filter and mipmap.
            SamplingOptions {
                use_cubic: true,
                cubic: CubicResampler { b, c },
                ..SamplingOptions::default()
            }
        } else {
            let filter = match self.read32_le(FilterMode::Linear as u32) {
                1 => FilterMode::Linear,
                _ => FilterMode::Nearest,
            };
            let mipmap = match self.read32_le(MipmapMode::Linear as u32) {
                1 => MipmapMode::Nearest,
                2 => MipmapMode::Linear,
                _ => MipmapMode::None,
            };
            SamplingOptions::new(filter, mipmap)
        }
    }

    /// `deserialize_image`: the image that the data makes, by the image data procedure, else by
    /// the image procedure. `None` if the buffer has neither, or the procedure fails.
    // Port of: src/core/SkReadBuffer.cpp#L346-L355 (chrome/m156)
    fn deserialize_image(&self, data: Data, alpha: Option<AlphaType>) -> Option<Image> {
        if let Some(read) = &self.deserial_procs.image_data {
            return read(data, alpha);
        }
        let read = self.deserial_procs.image.as_ref()?;
        read(data.as_bytes(), alpha)
    }

    /// `readImage`: the flags, the image's bytes (made into an image by the procedures), and the
    /// subset rect when the flags have one. An image that the procedures cannot make is the empty
    /// image of [`make_empty_image`], and the picture still loads. `None` (and the buffer is
    /// invalid) for a corrupt stream. Mipmap levels are not read yet, so an image that has them is
    /// treated as corrupt.
    // Port of: src/core/SkReadBuffer.cpp#L404-L435 (chrome/m156), readImage, without the mipmaps
    // (`add_mipmaps`)
    #[doc(alias = "readImage")]
    pub fn read_image(&mut self) -> Option<Image> {
        let flags = self.read_uint();
        let alpha = if flags & IMAGE_FLAG_UNPREMUL != 0 {
            Some(AlphaType::Unpremul)
        } else {
            None
        };
        let Some(data) = self.read_byte_array_as_data() else {
            self.validate(false);
            return None;
        };
        let mut image = self.deserialize_image(data, alpha);

        // This flag is not written by new pictures anymore.
        if flags & IMAGE_FLAG_HAS_SUBSET != 0 {
            let subset = self.read_irect();
            if let Some(img) = image.take() {
                image = img.make_subset(subset, RequiredProperties::default());
            }
        }

        if flags & IMAGE_FLAG_HAS_MIPMAP != 0 {
            // The mipmap levels are not read yet (`add_mipmaps`).
            let _ = self.read_byte_array_as_data();
            self.validate(false);
            return None;
        }
        Some(image.unwrap_or_else(make_empty_image))
    }

    /// `getArrayCount`: the count at the current position, which is not consumed. Returns 0 (and
    /// makes the buffer invalid) if there is no word left.
    // Port of: src/core/SkReadBuffer.cpp#L338-L344 (chrome/m156)
    #[doc(alias = "getArrayCount")]
    pub fn get_array_count(&mut self) -> u32 {
        const INC: usize = size_of::<u32>();
        if !self.validate(self.curr.is_multiple_of(4) && self.is_available(INC)) {
            return 0;
        }
        u32::from_ne_bytes([
            self.data[self.curr],
            self.data[self.curr + 1],
            self.data[self.curr + 2],
            self.data[self.curr + 3],
        ])
    }

    /// `validateCanReadN<T>`: whether `n` elements of `element_size` bytes are left to read.
    // Port of: src/core/SkReadBuffer.h#L214-L216 (chrome/m156)
    #[doc(alias = "validateCanReadN")]
    pub fn validate_can_read_n(&mut self, n: usize, element_size: usize) -> bool {
        self.validate(n <= self.available() / element_size)
    }

    /// Reads a 32-bit value that must not exceed `max`, or 0 (`read32LE`).
    // Port of: src/core/SkReadBuffer.h#L102-L108 (chrome/m156)
    #[doc(alias = "read32LE")]
    pub fn read32_le(&mut self, max: u32) -> u32 {
        let value = self.read_uint();
        if !self.validate(value <= max) {
            return 0;
        }
        value
    }

    /// Reads the scalars of an array whose count must be `values.len()` (`readScalarArray`).
    // Port of: src/core/SkReadBuffer.cpp#L312-L314 (chrome/m156)
    #[doc(alias = "readScalarArray")]
    pub fn read_scalar_array(&mut self, values: &mut [f32]) -> bool {
        let count = self.read_uint();
        if !self.validate(usize::try_from(count).ok() == Some(values.len())) {
            return false;
        }
        let Some(src) = self.skip(size_of_val(values)) else {
            return false;
        };
        let (chunks, _) = src.as_chunks::<{ size_of::<f32>() }>();
        for (value, chunk) in values.iter_mut().zip(chunks) {
            *value = f32::from_ne_bytes(*chunk);
        }
        true
    }

    /// Reads a string written by `writeString`: its length, its bytes and a terminating zero
    /// (`readString`). Returns `None` (and makes the buffer invalid) if it is malformed.
    // Port of: src/core/SkReadBuffer.cpp#L148-L158 (chrome/m156)
    #[doc(alias = "readString")]
    pub fn read_string(&mut self) -> Option<String> {
        let len = usize::try_from(self.read_uint()).ok()?;
        let bytes = self.skip(len.checked_add(1)?)?;
        if !self.validate(bytes[len] == 0) {
            return None;
        }
        let Ok(value) = std::str::from_utf8(&bytes[..len]) else {
            self.validate(false);
            return None;
        };
        Some(value.to_owned())
    }

    /// Reads a matrix written by `writeMatrix`: 9 scalars. The matrix is the identity if the
    /// buffer is invalid (`readMatrix`).
    // Port of: src/core/SkReadBuffer.cpp#L195-L205 (chrome/m156)
    #[doc(alias = "readMatrix")]
    pub fn read_matrix(&mut self) -> Matrix {
        const SIZE: usize = 9 * size_of::<f32>();
        let mut matrix = Matrix::new_identity();
        let mut size = 0;
        if self.is_valid() {
            // `SkMatrix::readFromMemory` reads nothing if fewer than 36 bytes are left.
            if self.is_available(SIZE) {
                let mut values = [0.0; 9];
                for (i, value) in values.iter_mut().enumerate() {
                    let at = self.curr + i * size_of::<f32>();
                    *value = f32::from_ne_bytes([
                        self.data[at],
                        self.data[at + 1],
                        self.data[at + 2],
                        self.data[at + 3],
                    ]);
                }
                matrix.set_9(&values);
                size = SIZE;
            }
            self.validate(size != 0);
        }
        if !self.is_valid() {
            matrix = Matrix::new_identity();
        }
        let _ = self.skip(size);
        matrix
    }

    /// Reads a path written by `writePath`, and moves past it whether or not it is valid
    /// (`readPath`).
    // Port of: src/core/SkReadBuffer.cpp#L267-L283 (chrome/m156)
    #[doc(alias = "readPath")]
    pub fn read_path(&mut self) -> Option<Path> {
        if !self.is_valid() {
            return None;
        }
        let (path, size) = Path::read_from_memory(&self.data[self.curr..]);
        // The path format is 4-byte aligned, and the path must have been read.
        self.validate(align4(size) == size && path.is_some());
        let _ = self.skip(size);
        path
    }

    /// Reads a path effect written by `writeFlattenable` (`readPathEffect`). `registry` maps the
    /// names to their factories.
    // Port of: src/core/SkReadBuffer.cpp#L538-L546 (chrome/m156), with the factory table passed
    // in (see `flattenable`)
    #[doc(alias = "readPathEffect")]
    pub fn read_path_effect(&mut self, registry: &FlattenableRegistry) -> Option<PathEffect> {
        let name = self.read_flattenable_name()?;
        let Some(factory) = registry.path_effect_factory(&name) else {
            self.validate(false);
            return None;
        };
        self.read_flattenable_body(|buffer| factory(buffer, registry))
    }

    /// Reads a mask filter written by `writeFlattenable` (`readMaskFilter`).
    // Port of: src/core/SkReadBuffer.cpp#L538-L546 (chrome/m156), with the factory table passed
    // in (see `flattenable`)
    #[doc(alias = "readMaskFilter")]
    pub fn read_mask_filter(&mut self, registry: &FlattenableRegistry) -> Option<MaskFilter> {
        let name = self.read_flattenable_name()?;
        let Some(factory) = registry.mask_filter_factory(&name) else {
            self.validate(false);
            return None;
        };
        self.read_flattenable_body(|buffer| factory(buffer, registry))
    }

    /// The current position in bytes (`SkReadBuffer::offset`).
    #[must_use]
    pub fn offset(&self) -> usize {
        self.curr
    }

    /// Reads a color (`readColor4f`): four scalars, or all zeros if the buffer cannot give them.
    // Port of: src/core/SkReadBuffer.cpp#L169-L173 (chrome/m156)
    #[doc(alias = "readColor4f")]
    pub fn read_color4f(&mut self) -> Color4f {
        let r = self.read_scalar();
        let g = self.read_scalar();
        let b = self.read_scalar();
        let a = self.read_scalar();
        if self.is_valid() {
            Color4f { r, g, b, a }
        } else {
            Color4f {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }
        }
    }

    /// Reads a round rectangle (`readRRect`): its bounds and radii, [`RRect::SIZE_IN_MEMORY`]
    /// bytes. Invalidates the buffer, and returns the default, if there are not that many bytes.
    // Port of: src/core/SkReadBuffer.cpp#L245-L254 (chrome/m156)
    #[doc(alias = "readRRect")]
    pub fn read_rrect(&mut self) -> RRect {
        let Some(bytes) = self.skip(RRect::SIZE_IN_MEMORY) else {
            return RRect::default();
        };
        let mut rrect = RRect::default();
        rrect.read_from_memory(bytes);
        rrect
    }

    /// Reads a flattenable that is not ported yet, which can only be null here: the writer
    /// wrote a zero word. Otherwise the buffer is invalidated. Returns whether it is still valid.
    fn read_null_flattenable(&mut self) -> bool {
        let word = self.read32();
        self.validate(word == 0)
    }

    /// Reads a paint (`readPaint`, `SkPaintPriv::Unflatten`). The paint is reset if the buffer is
    /// invalid afterwards. The path effect and mask filter are read with `registry`; a shader,
    /// color filter, image filter or custom blender must be null, as they are not read yet.
    // Port of: src/core/SkPaintPriv.cpp#L289-L331 (chrome/m156), with the arms of the
    // flattenables that are ported (path effect, mask filter); the others must be null
    #[doc(alias = "readPaint")]
    pub fn read_paint(&mut self, registry: &FlattenableRegistry) -> Paint {
        let mut paint = Paint::default();

        let stroke_width = self.read_scalar();
        paint.set_stroke_width(stroke_width);
        let stroke_miter = self.read_scalar();
        paint.set_stroke_miter(stroke_miter);
        let color = self.read_color4f();
        paint.set_color4f(color, srgb_singleton());

        let mut safe = true;
        let packed = self.read_uint();
        let flat_flags = unpack_v68(&mut paint, packed, &mut safe);

        if flat_flags & FLAT_HAS_EFFECTS != 0 {
            let path_effect;
            let mask_filter;
            if self.is_version_lt(VERSION_SK_BLENDER_IN_SK_PAINT) {
                // This paint predates the introduction of user blend functions (via SkBlender).
                path_effect = self.read_path_effect(registry);
                self.read_null_flattenable(); // shader
                mask_filter = self.read_mask_filter(registry);
                self.read_null_flattenable(); // color filter
                self.read32(); // drawLooper, now deprecated
                self.read_null_flattenable(); // image filter
            } else {
                path_effect = self.read_path_effect(registry);
                self.read_null_flattenable(); // shader
                mask_filter = self.read_mask_filter(registry);
                self.read_null_flattenable(); // color filter
                self.read_null_flattenable(); // image filter
                self.read_null_flattenable(); // blender
            }
            paint.set_path_effect(path_effect);
            paint.set_mask_filter(mask_filter);
        }

        if !self.validate(safe) {
            paint.reset();
        }
        paint
    }

    /// The name part of `readRawFlattenable`: the factory index (when there is a factory table), a
    /// string (which is added to the dictionary) or the dictionary index of an earlier one. `None`
    /// if the writer wrote nothing, or on an error.
    // Port of: src/core/SkReadBuffer.cpp#L470-L512 (chrome/m156)
    fn read_flattenable_name(&mut self) -> Option<String> {
        if !self.is_valid() {
            return None;
        }
        if !self.factory_names.is_empty() {
            // The index of a factory, which is the position in the table plus one.
            let index = self.read32();
            if index == 0 || !self.is_valid() {
                return None; // writer failed to give us the flattenable
            }
            if index < 0 {
                self.validate(false);
                return None;
            }
            let name = usize::try_from(index - 1)
                .ok()
                .and_then(|position| self.factory_names.get(position).cloned());
            if !self.validate(name.is_some()) {
                return None;
            }
            return name;
        }
        // If the first byte is non-zero, the flattenable is specified by a string. Otherwise it
        // is the index, shifted left by 8.
        if self.peek_byte() != 0 {
            let name = self.read_string()?;
            self.flattenable_names.push(name.clone());
            Some(name)
        } else {
            let index = self.read_uint() >> 8;
            if index == 0 {
                return None; // writer failed to give us the flattenable
            }
            let name = usize::try_from(index)
                .ok()
                .and_then(|index| self.flattenable_names.get(index - 1).cloned());
            if !self.validate(name.is_some()) {
                return None;
            }
            name
        }
    }

    /// The body part of `readRawFlattenable`: the recorded size, then what the factory reads,
    /// which must be exactly that size.
    // Port of: src/core/SkReadBuffer.cpp#L515-L536 (chrome/m156)
    fn read_flattenable_body<T>(
        &mut self,
        factory: impl FnOnce(&mut Self) -> Option<T>,
    ) -> Option<T> {
        let size_recorded = self.read_uint();
        let offset = self.curr;
        let obj = factory(self);
        let size_read = self.curr - offset;
        if usize::try_from(size_recorded).ok() != Some(size_read) {
            self.validate(false);
            return None;
        }
        if !self.is_valid() {
            return None;
        }
        obj
    }

    /// The first byte at the current position, without consuming it (`peekByte`). Returns 0,
    /// and makes the buffer invalid, if there is no byte left.
    // Port of: src/core/SkReadBuffer.cpp#L130-L136 (chrome/m156)
    #[doc(alias = "peekByte")]
    fn peek_byte(&mut self) -> u8 {
        if self.available() == 0 {
            self.validate(false);
            return 0;
        }
        self.data[self.curr]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write_buffer::BinaryWriteBuffer;

    fn bytes(b: &BinaryWriteBuffer) -> Vec<u8> {
        let mut out = vec![0; b.bytes_written()];
        b.write_to_memory(&mut out);
        out
    }

    #[test]
    fn reads_what_was_written() {
        let mut w = BinaryWriteBuffer::new();
        w.write_uint(0xDEAD_BEEF);
        w.write_int(-5);
        w.write_byte_array(&[1, 2, 3, 4, 5]);
        w.write_byte_array(&[6, 7]);
        let data = bytes(&w);

        let mut r = ReadBuffer::new(&data);
        assert!(r.is_valid());
        assert_eq!(r.read_uint(), 0xDEAD_BEEF);
        assert_eq!(r.read_int(), -5);
        let mut a = [0; 5];
        assert!(r.read_byte_array(&mut a));
        assert_eq!(a, [1, 2, 3, 4, 5]);
        let (skipped, size) = r.skip_byte_array();
        assert_eq!((skipped.map(<[u8]>::len), size), (Some(4), 2));
        assert_eq!(r.available(), 0);
        assert!(r.is_valid());
    }

    #[test]
    fn errors_are_sticky_and_end_the_buffer() {
        let data = [0u8; 8];
        let mut r = ReadBuffer::new(&data);
        assert_eq!(r.read_int(), 0);
        assert_eq!(r.read_int(), 0);
        assert!(r.is_valid());
        // Past the end.
        assert_eq!(r.read_int(), 0);
        assert!(!r.is_valid());
        assert_eq!(r.available(), 0);
        assert!(!r.validate(true));

        // The wrong array size.
        let mut w = BinaryWriteBuffer::new();
        w.write_byte_array(&[1, 2, 3, 4]);
        let data = bytes(&w);
        let mut r = ReadBuffer::new(&data);
        let mut a = [0; 3];
        assert!(!r.read_byte_array(&mut a));
        assert!(!r.is_valid());

        // A size that is not a multiple of 4.
        assert!(!ReadBuffer::new(&[0, 0, 0]).is_valid());
    }

    #[test]
    fn scalars_and_empty_typefaces_round_trip() {
        let mut w = BinaryWriteBuffer::new();
        w.write_scalar(1.5);
        w.write_typeface(None);
        w.write_scalar(-0.25);
        let data = bytes(&w);
        assert_eq!(data.len(), 12);

        let mut r = ReadBuffer::new(&data);
        assert_eq!(r.read_scalar(), 1.5);
        assert!(r.read_typeface().is_none());
        assert_eq!(r.read_scalar(), -0.25);
        assert!(r.is_valid());

        // A non-zero typeface index has no table to refer to.
        let mut w = BinaryWriteBuffer::new();
        w.write_int(3);
        let data = bytes(&w);
        let mut r = ReadBuffer::new(&data);
        assert!(r.read_typeface().is_none());
        assert!(!r.is_valid());
    }

    #[test]
    fn versions() {
        let mut r = ReadBuffer::new(&[]);
        assert!(!r.is_version_lt(86)); // no version: the latest
        r.set_version(85);
        assert!(r.is_version_lt(86));
        assert!(!r.is_version_lt(85));
        assert_eq!(r.version(), 85);
    }
}
