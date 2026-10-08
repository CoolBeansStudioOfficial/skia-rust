// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file.
// Port of: src/core/SkFontStream.cpp#L1-L215 and include/core/SkFontStream.h (chrome/m156)

//! Reads the table directory of an sfnt font, or of a TrueType collection, straight from a
//! stream (`SkFontStream`). Nothing here needs a typeface.

use crate::font_types::FourByteTag;
use crate::stream::StreamRewindable;

/// `sizeof(SkSFNTHeader)`: the version, the table count and the three search fields.
const SFNT_HEADER_SIZE: usize = 12;
/// `sizeof(SkSharedTTHeader)`: the larger of `SkSFNTHeader` and `SkTTCFHeader` (16 bytes).
const SHARED_TT_HEADER_SIZE: usize = 16;
/// `sizeof(SkSFNTDirEntry)`: tag, checksum, offset and length.
const SFNT_DIR_ENTRY_SIZE: usize = 16;
/// `SkSetFourByteTag('t', 't', 'c', 'f')`: the tag of a TrueType collection header.
const TTCF_TAG: u32 = u32::from_be_bytes(*b"ttcf");

/// `SkEndian_SwapBE32` of the field at `at` (big-endian in the file).
fn be32(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// `SkEndian_SwapBE16` of the field at `at` (big-endian in the file).
fn be16(bytes: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([bytes[at], bytes[at + 1]])
}

// Port of: src/core/SkFontStream.cpp#L46-L48 (chrome/m156)
fn read(stream: &mut dyn StreamRewindable, buffer: &mut [u8]) -> bool {
    stream.read(buffer) == buffer.len()
}

// Port of: src/core/SkFontStream.cpp#L50-L52 (chrome/m156)
fn skip(stream: &mut dyn StreamRewindable, amount: usize) -> bool {
    stream.skip(amount) == amount
}

/// Port of `count_tables`: the number of tables, or for a collection the number in the entry
/// `tt_index` of it, with the offset of the table headers (`SkSFNTDirEntry`) from the start of
/// the stream. `None` is C++'s error, where the count is 0 and the offset is ignored.
// Port of: src/core/SkFontStream.cpp#L61-L109 (chrome/m156)
fn count_tables(stream: &mut dyn StreamRewindable, tt_index: i32) -> Option<(usize, usize)> {
    let mut header = vec![0u8; SHARED_TT_HEADER_SIZE];
    if !read(stream, &mut header) {
        return None;
    }
    // by default, SkSFNTHeader is at the start of the stream
    let mut offset = 0;
    // if we're really a collection, the first 4-bytes will be 'ttcf'
    if be32(&header, 0) == TTCF_TAG {
        let count = be32(&header, 8);
        // `(unsigned)ttcIndex >= count`: a negative index is out of range too.
        let index = usize::try_from(tt_index).ok()?;
        if usize::try_from(count).map_or(true, |count| index >= count) {
            return None;
        }
        if index > 0 {
            // need to read more of the shared header
            stream.rewind();
            let amount = SHARED_TT_HEADER_SIZE + index * 4;
            header = vec![0u8; amount];
            if !read(stream, &mut header) {
                return None;
            }
        }
        // this is the offset to the local SkSFNTHeader: (&fCollection.fOffset0)[ttcIndex]
        offset = be32(&header, 12 + index * 4) as usize;
        stream.rewind();
        if !skip(stream, offset) {
            return None;
        }
        if !read(stream, &mut header[..SFNT_HEADER_SIZE]) {
            return None;
        }
    }
    // add the size of the header, so we will point to the DirEntries
    let offset_to_dir = offset + SFNT_HEADER_SIZE;
    Some((usize::from(be16(&header, 4)), offset_to_dir))
}

/// Port of `SfntHeader::init`: the directory entries, still big-endian as in the file. `None`
/// (or no entries) is C++'s failure.
// Port of: src/core/SkFontStream.cpp#L111-L143 (chrome/m156)
fn sfnt_dir(stream: &mut dyn StreamRewindable, tt_index: i32) -> Option<Vec<u8>> {
    stream.rewind();
    let (count, offset_to_dir) = count_tables(stream, tt_index)?;
    if count == 0 {
        return None;
    }
    stream.rewind();
    if !skip(stream, offset_to_dir) {
        return None;
    }
    let mut dir = vec![0u8; count * SFNT_DIR_ENTRY_SIZE];
    if !read(stream, &mut dir) {
        return None;
    }
    Some(dir)
}

/// `SkFontStream::CountTTCEntries`: the number of fonts in a collection, or 1 for a single font.
// Port of: src/core/SkFontStream.cpp#L145-L160 (chrome/m156)
#[doc(alias = "CountTTCEntries")]
#[must_use]
pub fn count_ttc_entries(stream: &mut dyn StreamRewindable) -> i32 {
    stream.rewind();
    let mut shared = vec![0u8; SHARED_TT_HEADER_SIZE];
    if !read(stream, &mut shared) {
        return 0;
    }
    // if we're really a collection, the first 4-bytes will be 'ttcf'
    if be32(&shared, 0) == TTCF_TAG {
        be32(&shared, 8).cast_signed()
    } else {
        1 // normal 'sfnt' has 1 dir entry
    }
}

/// `SkFontStream::GetTableTags`: writes as many tags as `tags` holds, and returns the number of
/// tables (0 on failure). An empty `tags` asks for the count.
// Port of: src/core/SkFontStream.cpp#L162-L173 (chrome/m156)
#[doc(alias = "GetTableTags")]
#[must_use]
pub fn get_table_tags(
    stream: &mut dyn StreamRewindable,
    tt_index: i32,
    tags: &mut [FourByteTag],
) -> i32 {
    let Some(dir) = sfnt_dir(stream, tt_index) else {
        return 0;
    };
    let count = dir.len() / SFNT_DIR_ENTRY_SIZE;
    let n = count.min(tags.len());
    for (i, tag) in tags.iter_mut().enumerate().take(n) {
        *tag = be32(&dir, i * SFNT_DIR_ENTRY_SIZE);
    }
    // C++ `int`: a table count is at most 65535.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let count = count as i32;
    count
}

/// `SkFontStream::GetTableData`: copies up to `length` bytes of the table `tag`, from `offset`,
/// into `data` (when there is one). Returns the number of bytes of the table that are, or would
/// be, copied, and 0 when the table is missing or the range is invalid.
// Port of: src/core/SkFontStream.cpp#L175-L215 (chrome/m156)
#[doc(alias = "GetTableData")]
#[must_use]
pub fn get_table_data(
    stream: &mut dyn StreamRewindable,
    tt_index: i32,
    tag: FourByteTag,
    offset: usize,
    length: usize,
    data: Option<&mut [u8]>,
) -> usize {
    let Some(dir) = sfnt_dir(stream, tt_index) else {
        return 0;
    };
    let (entries, _) = dir.as_chunks::<SFNT_DIR_ENTRY_SIZE>();
    for entry in entries {
        if be32(entry, 0) != tag {
            continue;
        }
        let real_offset = be32(entry, 8) as usize;
        let real_length = be32(entry, 12) as usize;
        if offset >= real_length {
            // invalid
            return 0;
        }
        // if the caller is trusting the length from the file, then a hostile file might choose a
        // value which would overflow offset + length.
        if offset.checked_add(length).is_none() {
            return 0;
        }
        let length = length.min(real_length - offset);
        if let Some(data) = data {
            // skip the stream to the part of the table we want to copy from
            stream.rewind();
            let bytes_to_skip = real_offset + offset;
            if !skip(stream, bytes_to_skip) {
                return 0;
            }
            if !read(stream, &mut data[..length]) {
                return 0;
            }
        }
        return length;
    }
    0
}

/// `SkFontStream::GetTableSize`: the size of the table `tag`, or 0 when it is missing.
// Port of: include/core/SkFontStream.h#L47-L49 (chrome/m156)
#[doc(alias = "GetTableSize")]
#[must_use]
pub fn get_table_size(stream: &mut dyn StreamRewindable, tt_index: i32, tag: FourByteTag) -> usize {
    // `~0U`: every byte of the table.
    get_table_data(stream, tt_index, tag, 0, u32::MAX as usize, None)
}
