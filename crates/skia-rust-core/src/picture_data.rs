// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkPictureData.h, src/core/SkPictureData.cpp (the subset below)

//! `SkPictureData`: the body of a serialized picture, in tagged sections.
//!
//! A serialized picture is its header ([`PictInfo`]), a byte that says what follows, and then
//! the data: the op stream (`READER`), the factories (`FACTORY`), the typefaces (`TYPEFACE`), a
//! buffer of paints, paths and the other tables (`BUFFER_SIZE`), and the end (`EOF`). Each
//! section but the end starts with its tag and, for most, a size.
//!
//! skia-rust: the factories are written by index, as in Skia, with their names in the factory
//! section. The sections for slugs, vertices, images, drawables and nested pictures are not
//! written or read yet: a picture that has them is not serialized, and a stream that has them
//! does not load.

use crate::flattenable::FlattenableRegistry;
use crate::paint::Paint;
use crate::path::Path;
use crate::read_buffer::ReadBuffer;
use crate::rect::Rect;
use crate::serial_procs::{DeserialProcs, SerialProcs};
use crate::stream::{Stream, WStream, size_of_packed_uint};
use crate::text_blob::{TextBlob, make_from_buffer};
use crate::typeface::{SerializeBehavior, Typeface};
use crate::write_buffer::BinaryWriteBuffer;

/// A four byte tag, with the first character in the top byte (`SkSetFourByteTag`).
// Port of: include/core/SkFourByteTag.h#L15-L17 (chrome/m156)
const fn four_byte_tag(a: u8, b: u8, c: u8, d: u8) -> u32 {
    ((a as u32) << 24) | ((b as u32) << 16) | ((c as u32) << 8) | (d as u32)
}

/// The op stream (`SK_PICT_READER_TAG`).
// Port of: src/core/SkPictureData.h#L62 (chrome/m156)
const READER_TAG: u32 = four_byte_tag(b'r', b'e', b'a', b'd');
/// The factories (`SK_PICT_FACTORY_TAG`).
// Port of: src/core/SkPictureData.h#L63 (chrome/m156)
const FACTORY_TAG: u32 = four_byte_tag(b'f', b'a', b'c', b't');
/// The typefaces (`SK_PICT_TYPEFACE_TAG`).
// Port of: src/core/SkPictureData.h#L64 (chrome/m156)
const TYPEFACE_TAG: u32 = four_byte_tag(b't', b'p', b'f', b'c');
/// The nested pictures (`SK_PICT_PICTURE_TAG`).
// Port of: src/core/SkPictureData.h#L65 (chrome/m156)
const PICTURE_TAG: u32 = four_byte_tag(b'p', b'c', b't', b'r');
/// The size of the buffer that holds the tables (`SK_PICT_BUFFER_SIZE_TAG`).
// Port of: src/core/SkPictureData.h#L69 (chrome/m156)
const BUFFER_SIZE_TAG: u32 = four_byte_tag(b'a', b'r', b'a', b'y');
/// The paints in the buffer (`SK_PICT_PAINT_BUFFER_TAG`).
// Port of: src/core/SkPictureData.h#L71 (chrome/m156)
const PAINT_BUFFER_TAG: u32 = four_byte_tag(b'p', b'n', b't', b' ');
/// The paths in the buffer (`SK_PICT_PATH_BUFFER_TAG`).
// Port of: src/core/SkPictureData.h#L72 (chrome/m156)
const PATH_BUFFER_TAG: u32 = four_byte_tag(b'p', b't', b'h', b' ');
/// The text blobs in the buffer (`SK_PICT_TEXTBLOB_BUFFER_TAG`).
// Port of: src/core/SkPictureData.h#L73 (chrome/m156)
const TEXTBLOB_BUFFER_TAG: u32 = four_byte_tag(b'b', b'l', b'o', b'b');
/// The slugs in the buffer (`SK_PICT_SLUG_BUFFER_TAG`).
// Port of: src/core/SkPictureData.h#L74 (chrome/m156)
const SLUG_BUFFER_TAG: u32 = four_byte_tag(b's', b'l', b'u', b'g');
/// The vertices in the buffer (`SK_PICT_VERTICES_BUFFER_TAG`).
// Port of: src/core/SkPictureData.h#L75 (chrome/m156)
const VERTICES_BUFFER_TAG: u32 = four_byte_tag(b'v', b'e', b'r', b't');
/// The images in the buffer (`SK_PICT_IMAGE_BUFFER_TAG`).
// Port of: src/core/SkPictureData.h#L76 (chrome/m156)
const IMAGE_BUFFER_TAG: u32 = four_byte_tag(b'i', b'm', b'a', b'g');
/// The end of the picture (`SK_PICT_EOF_TAG`); it has no size after it.
// Port of: src/core/SkPictureData.h#L79 (chrome/m156)
const EOF_TAG: u32 = four_byte_tag(b'e', b'o', b'f', b' ');

/// The header of a serialized picture (`SkPictInfo`): the magic, the version and the cull rect.
// Port of: src/core/SkPictureData.h#L41-L60 (chrome/m156)
#[doc(alias = "SkPictInfo")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PictInfo {
    /// `fMagic`: `skiapict`.
    pub(crate) magic: [u8; 8],
    /// `getVersion`.
    pub(crate) version: u32,
    /// `fCullRect`.
    pub(crate) cull_rect: Rect,
}

/// The magic that starts every picture (`kMagic`).
// Port of: src/core/SkPicture.cpp#L75 (chrome/m156)
pub(crate) const MAGIC: [u8; 8] = *b"skiapict";

/// Writes `value` as a tag or size word (`write_tag_size`, the stream form).
fn write_tag_size(stream: &mut dyn WStream, tag: u32, size: usize) -> bool {
    stream.write32(tag) && stream.write32(u32::try_from(size).expect("SkToU32"))
}

/// Writes `value` as a tag or size word into a buffer (`write_tag_size`, the buffer form).
fn write_tag_size_to_buffer(buffer: &mut BinaryWriteBuffer, tag: u32, size: usize) {
    buffer.write_uint(tag);
    buffer.write_uint(u32::try_from(size).expect("SkToU32"));
}

/// Reads exactly `size` bytes. The vector grows as the bytes come, so a size larger than the
/// stream is not allocated up front.
fn read_exact(stream: &mut dyn Stream, size: usize) -> Option<Vec<u8>> {
    const CHUNK: usize = 64 * 1024;
    let mut bytes = Vec::new();
    while bytes.len() < size {
        let start = bytes.len();
        let take = CHUNK.min(size - start);
        bytes.resize(start + take, 0);
        if stream.read(&mut bytes[start..]) != take {
            return None;
        }
    }
    Some(bytes)
}

/// Writes the factory section: its size, the count, and the name of each factory, with the
/// length packed (`WriteFactories`).
// Port of: src/core/SkPictureData.cpp#L84-L122 (chrome/m156), compute_chunk_size and WriteFactories
fn write_factories(stream: &mut dyn WStream, names: &[String]) -> bool {
    let size = 4 + names
        .iter()
        .map(|name| size_of_packed_uint(name.len()) + name.len())
        .sum::<usize>();
    if !write_tag_size(stream, FACTORY_TAG, size) {
        return false;
    }
    if !stream.write32(u32::try_from(names.len()).expect("SkToU32")) {
        return false;
    }
    names
        .iter()
        .all(|name| stream.write_packed_uint(name.len()) && stream.write(name.as_bytes()))
}

/// The data of a picture, as read from or written to a serialized picture (`SkPictureData`).
// Port of: src/core/SkPictureData.h#L88-L185 (chrome/m156), the parts that are ported
#[doc(alias = "SkPictureData")]
#[derive(Debug, Default)]
pub(crate) struct PictureData {
    /// `fOpData`: the op stream. `None` until it is read (a picture without one is invalid).
    op_data: Option<Vec<u8>>,
    /// `fPaints`: the paints that the ops index, from 1.
    paints: Vec<Paint>,
    /// `fPaths`: the paths that the ops index, from 1.
    paths: Vec<Path>,
    /// `fTFPlayback`: the typefaces that the buffer refers to by index, from 1.
    typefaces: Vec<Typeface>,
    /// `fFactoryPlayback`: the names of the factories that the buffer refers to by index, from 1.
    factories: Vec<String>,
    /// `fTextBlobs`: the text blobs that the ops refer to by index, from 1.
    text_blobs: Vec<TextBlob>,
}

impl PictureData {
    /// Data of an op stream and its tables.
    // Port of: src/core/SkPictureData.cpp#L44-L64 (chrome/m156), the constructor from a record
    pub(crate) fn new(
        op_data: Vec<u8>,
        paints: Vec<Paint>,
        paths: Vec<Path>,
        text_blobs: Vec<TextBlob>,
    ) -> PictureData {
        PictureData {
            op_data: Some(op_data),
            paints,
            paths,
            typefaces: Vec::new(),
            factories: Vec::new(),
            text_blobs,
        }
    }

    /// The op stream (`opData`).
    pub(crate) fn op_data(&self) -> Option<&[u8]> {
        self.op_data.as_deref()
    }

    /// The paints the ops index (`fPaints`).
    pub(crate) fn paints(&self) -> &[Paint] {
        &self.paints
    }

    /// The paths the ops index (`fPaths`).
    pub(crate) fn paths(&self) -> &[Path] {
        &self.paths
    }

    /// The text blobs the ops index (`fTextBlobs`).
    pub(crate) fn text_blobs(&self) -> &[TextBlob] {
        &self.text_blobs
    }

    /// Writes the buffer of the tables: paints, then paths, then text blobs, then the (empty)
    /// slugs (`flattenToBuffer`, with no vertices or images). The buffer records the factories
    /// and typefaces that it writes by index. `None` if a paint has an effect that is not written
    /// yet.
    // Port of: src/core/SkPictureData.cpp#L151-L200 (chrome/m156), flattenToBuffer
    fn flatten_to_buffer(&self) -> Option<BinaryWriteBuffer> {
        let mut buffer = BinaryWriteBuffer::with_serial_procs(SerialProcs::default());
        buffer.set_typeface_recorder();
        buffer.set_factory_recorder();

        if !self.paints.is_empty() {
            write_tag_size_to_buffer(&mut buffer, PAINT_BUFFER_TAG, self.paints.len());
            for paint in &self.paints {
                if !buffer.write_paint(paint) {
                    return None;
                }
            }
        }
        if !self.paths.is_empty() {
            write_tag_size_to_buffer(&mut buffer, PATH_BUFFER_TAG, self.paths.len());
            buffer.write_int(i32::try_from(self.paths.len()).expect("SkToInt"));
            for path in &self.paths {
                buffer.write_path(path);
            }
        }
        if !self.text_blobs.is_empty() {
            write_tag_size_to_buffer(&mut buffer, TEXTBLOB_BUFFER_TAG, self.text_blobs.len());
            for blob in &self.text_blobs {
                blob.flatten(&mut buffer);
            }
        }
        // The slugs are always written, even when there are none.
        write_tag_size_to_buffer(&mut buffer, SLUG_BUFFER_TAG, 0);
        Some(buffer)
    }

    /// Writes the data of a picture to `stream` (`SkPictureData::serialize`, for the pictures
    /// without nested pictures or text). `procs` decides how the typefaces are written. Returns
    /// false if the data cannot be written (see [`flatten_to_buffer`](Self::flatten_to_buffer)),
    /// or if the stream fails.
    // Port of: src/core/SkPictureData.cpp#L219-L271 (chrome/m156), serialize
    pub(crate) fn serialize(&self, stream: &mut dyn WStream, procs: &SerialProcs) -> bool {
        let Some(op_data) = self.op_data() else {
            return false;
        };
        // The buffer is made first: the factories and typefaces it indexes are written before it.
        let Some(buffer) = self.flatten_to_buffer() else {
            return false;
        };

        // The op stream comes first.
        if !write_tag_size(stream, READER_TAG, op_data.len()) || !stream.write(op_data) {
            return false;
        }

        // The factories, by name, in the order that the buffer indexes them.
        if !write_factories(stream, buffer.factory_recorder().unwrap_or(&[])) {
            return false;
        }

        // The typefaces that the buffer indexes: each as its custom bytes or as it serializes.
        let typefaces = buffer.typeface_recorder().unwrap_or(&[]);
        if !write_tag_size(stream, TYPEFACE_TAG, typefaces.len()) {
            return false;
        }
        for typeface in typefaces {
            let custom = procs
                .typeface
                .as_ref()
                .and_then(|serialize| serialize(typeface));
            let written = match custom {
                Some(data) => stream.write(data.as_bytes()),
                None => typeface.serialize_to(stream, SerializeBehavior::DoIncludeData),
            };
            if !written {
                return false;
            }
        }

        // The buffer of tables, which the size is written before.
        let mut bytes = vec![0; buffer.bytes_written()];
        buffer.write_to_memory(&mut bytes);
        if !write_tag_size(stream, BUFFER_SIZE_TAG, bytes.len()) || !stream.write(&bytes) {
            return false;
        }

        // No nested pictures, and the end.
        stream.write32(EOF_TAG)
    }

    /// Reads the data of a picture from `stream`, which holds the version `version`
    /// (`SkPictureData::CreateFromStream`, with `parseStream`). `None` if the stream is invalid
    /// or has a section that is not ported.
    // Port of: src/core/SkPictureData.cpp#L537-L554 (chrome/m156), CreateFromStream, and
    // src/core/SkPictureData.cpp#L570-L588 (chrome/m156), parseStream
    pub(crate) fn parse_stream(
        stream: &mut dyn Stream,
        version: u32,
        procs: &DeserialProcs,
        registry: &FlattenableRegistry,
    ) -> Option<PictureData> {
        let mut data = PictureData::default();
        loop {
            let tag = stream.read_u32()?;
            if tag == EOF_TAG {
                break;
            }
            let size = stream.read_u32()?;
            data.parse_stream_tag(stream, tag, size, version, procs, registry)?;
        }
        Some(data)
    }

    /// Reads one section of the stream (`parseStreamTag`). An unknown tag is ignored, as it is in
    /// Skia.
    // Port of: src/core/SkPictureData.cpp#L298-L417 (chrome/m156), parseStreamTag
    fn parse_stream_tag(
        &mut self,
        stream: &mut dyn Stream,
        tag: u32,
        size: u32,
        version: u32,
        procs: &DeserialProcs,
        registry: &FlattenableRegistry,
    ) -> Option<()> {
        let size = usize::try_from(size).ok()?;
        match tag {
            READER_TAG => {
                if self.op_data.is_some() {
                    return None;
                }
                self.op_data = Some(read_exact(stream, size)?);
            }
            FACTORY_TAG => {
                // The names of the factories, which the buffer's flattenables index.
                let count = stream.read_u32()?;
                let mut names = Vec::new();
                for _ in 0..count {
                    let len = stream.read_packed_uint()?;
                    let bytes = read_exact(stream, len)?;
                    names.push(String::from_utf8_lossy(&bytes).into_owned());
                }
                self.factories = names;
            }
            TYPEFACE_TAG => {
                for _ in 0..size {
                    if stream.is_at_end() {
                        return None;
                    }
                    let typeface = match &procs.typeface {
                        Some(deserialize) => deserialize(stream),
                        None => Typeface::make_deserialize(stream, None, None),
                    };
                    // A typeface that fails to read is the empty one, as in Skia.
                    self.typefaces
                        .push(typeface.unwrap_or_else(Typeface::empty));
                }
            }
            PICTURE_TAG => {
                // Nested pictures are not ported.
                if size != 0 {
                    return None;
                }
            }
            BUFFER_SIZE_TAG => {
                let bytes = read_exact(stream, size)?;
                let mut buffer = ReadBuffer::with_deserial_procs(&bytes, procs.clone());
                buffer.set_version(version);
                buffer.set_typeface_array(self.typefaces.clone());
                buffer.set_factory_names(self.factories.clone());
                while buffer.available() > 0 && buffer.is_valid() {
                    let tag = buffer.read_uint();
                    let size = buffer.read_uint();
                    self.parse_buffer_tag(&mut buffer, tag, size, registry);
                }
                if !buffer.is_valid() {
                    return None;
                }
            }
            _ => {}
        }
        Some(())
    }

    /// Reads one section of the buffer (`parseBufferTag`): paints and paths are read, the empty
    /// sections of the other kinds are accepted, and anything else invalidates the buffer.
    // Port of: src/core/SkPictureData.cpp#L455-L535 (chrome/m156), parseBufferTag
    fn parse_buffer_tag(
        &mut self,
        buffer: &mut ReadBuffer<'_>,
        tag: u32,
        size: u32,
        registry: &FlattenableRegistry,
    ) {
        match tag {
            PAINT_BUFFER_TAG => {
                let Ok(count) = i32::try_from(size) else {
                    buffer.validate(false);
                    return;
                };
                for _ in 0..count {
                    let paint = buffer.read_paint(registry);
                    if !buffer.is_valid() {
                        return;
                    }
                    self.paints.push(paint);
                }
            }
            PATH_BUFFER_TAG => {
                if size > 0 {
                    let count = buffer.read_int();
                    if !buffer.validate(count >= 0) {
                        return;
                    }
                    for _ in 0..count {
                        match buffer.read_path() {
                            Some(path) => self.paths.push(path),
                            None => return,
                        }
                    }
                }
            }
            TEXTBLOB_BUFFER_TAG => {
                // `new_array_from_buffer`: the array must be empty, and each blob must read.
                let Ok(count) = usize::try_from(size) else {
                    buffer.validate(false);
                    return;
                };
                if !buffer.validate(self.text_blobs.is_empty()) {
                    return;
                }
                for _ in 0..count {
                    let Some(blob) = make_from_buffer(buffer) else {
                        buffer.validate(false);
                        self.text_blobs.clear();
                        return;
                    };
                    self.text_blobs.push(blob);
                }
            }
            // The sections that are not ported: an empty one is fine, a full one is not.
            SLUG_BUFFER_TAG | VERTICES_BUFFER_TAG | IMAGE_BUFFER_TAG => {
                buffer.validate(size == 0);
            }
            _ => {
                buffer.validate(false);
            }
        }
    }
}

impl PictInfo {
    /// The header of a picture with `cull_rect` and the current version (`createHeader`, the
    /// fields of it).
    // Port of: src/core/SkPicture.cpp#L77-L88 (chrome/m156), createHeader
    pub(crate) fn new(cull_rect: Rect, version: u32) -> PictInfo {
        PictInfo {
            magic: MAGIC,
            version,
            cull_rect,
        }
    }

    /// Writes the header: the magic, the version and the four cull scalars, which is the
    /// `SkPictInfo` struct in memory (`SkPicture::serialize`, the first write).
    // Port of: src/core/SkPicture.cpp#L301-L330 (chrome/m156), the write of the header
    pub(crate) fn write_to(&self, stream: &mut dyn WStream) -> bool {
        stream.write(&self.magic)
            && stream.write32(self.version)
            && stream.write32(self.cull_rect.left.to_bits())
            && stream.write32(self.cull_rect.top.to_bits())
            && stream.write32(self.cull_rect.right.to_bits())
            && stream.write32(self.cull_rect.bottom.to_bits())
    }

    /// Reads the header (`StreamIsSKP`, without the validity check): the magic, the version and
    /// the cull scalars. `None` if the stream is too short.
    // Port of: src/core/SkPicture.cpp#L101-L124 (chrome/m156), StreamIsSKP
    pub(crate) fn read_from(stream: &mut dyn Stream) -> Option<PictInfo> {
        let mut magic = [0u8; 8];
        if stream.read(&mut magic) != magic.len() {
            return None;
        }
        let version = stream.read_u32()?;
        let left = stream.read_scalar()?;
        let top = stream.read_scalar()?;
        let right = stream.read_scalar()?;
        let bottom = stream.read_scalar()?;
        Some(PictInfo {
            magic,
            version,
            cull_rect: Rect {
                left,
                top,
                right,
                bottom,
            },
        })
    }

    /// Whether the header is a picture this reads: the magic, and a version in the readable
    /// range (`IsValidPictInfo`).
    // Port of: src/core/SkPicture.cpp#L90-L99 (chrome/m156), IsValidPictInfo
    pub(crate) fn is_valid(&self) -> bool {
        self.magic == MAGIC
            && (crate::picture_priv::MIN_VERSION..=crate::picture_priv::CURRENT_VERSION)
                .contains(&self.version)
    }
}
