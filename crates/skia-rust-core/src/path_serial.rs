// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkPath_serial.cpp

//! Byte serialization of paths (`SkPath_serial.cpp`).
//!
//! The format matches Skia's byte for byte: native-endian 32-bit header words, then points,
//! conic weights and verbs, padded to a multiple of 4 bytes. Ovals and round rects are written
//! as a round rect record.

use crate::path::Path;
use crate::path_types::{PathDirection, PathFillType};
use crate::point::Point;
use crate::rrect::RRect;

// Port of: src/core/SkPath_serial.cpp#L31-L37 (chrome/m156)
const TYPE_SERIALIZATION_SHIFT: u32 = 28; // requires 4 bits
const DIRECTION_SERIALIZATION_SHIFT: u32 = 26; // requires 2 bits
const FILL_TYPE_SERIALIZATION_SHIFT: u32 = 8; // requires 8 bits
// low-8-bits are version
const VERSION_SERIALIZATION_MASK: u32 = 0xFF;

// Port of: src/core/SkPath_serial.cpp#L39-L47 (chrome/m156)
const JUST_PUBLIC_DATA_VERSION: u32 = 4; // introduced Feb/2018
const VERBS_ARE_STORED_FORWARD_VERSION: u32 = 5; // introduced Sept/2019
const CURRENT_VERSION: u32 = VERBS_ARE_STORED_FORWARD_VERSION;

// Port of: src/core/SkPath_serial.cpp#L49-L52 (chrome/m156)
const SERIALIZATION_TYPE_GENERAL: u32 = 0;
const SERIALIZATION_TYPE_RRECT: u32 = 1;

// Port of: src/core/SkPath_serial.cpp#L54-L56 (chrome/m156)
fn extract_version(packed: u32) -> u32 {
    packed & VERSION_SERIALIZATION_MASK
}

// Port of: src/core/SkPath_serial.cpp#L58-L60 (chrome/m156)
fn extract_filltype(packed: u32) -> PathFillType {
    PathFillType::from_bits((packed >> FILL_TYPE_SERIALIZATION_SHIFT) & 0x3)
}

// Port of: src/core/SkPath_serial.cpp#L62-L64 (chrome/m156)
fn extract_serializationtype(packed: u32) -> u32 {
    (packed >> TYPE_SERIALIZATION_SHIFT) & 0xF
}

/// `SkWBuffer` over an optional byte buffer (`None` only measures).
struct WBuffer<'a> {
    data: Option<&'a mut [u8]>,
    pos: usize,
}

impl WBuffer<'_> {
    fn write(&mut self, bytes: &[u8]) {
        if let Some(data) = self.data.as_deref_mut() {
            data[self.pos..self.pos + bytes.len()].copy_from_slice(bytes);
        }
        self.pos += bytes.len();
    }

    fn write32(&mut self, x: u32) {
        self.write(&x.to_ne_bytes());
    }

    fn pad_to_align4(&mut self) {
        let n = self.pos.next_multiple_of(4) - self.pos;
        self.write(&[0u8; 3][..n]);
    }
}

/// `SkRBuffer` over a byte slice (offsets from the start are treated as aligned).
struct RBuffer<'a> {
    data: &'a [u8],
    pos: usize,
    valid: bool,
}

impl<'a> RBuffer<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            valid: true,
        }
    }

    fn available(&self) -> usize {
        self.data.len() - self.pos
    }

    // Port of: src/core/SkBuffer.cpp#L14-L22 (chrome/m156)
    fn skip(&mut self, size: Option<usize>) -> Option<&'a [u8]> {
        if let Some(size) = size
            && self.valid
            && size <= self.available()
        {
            let s = &self.data[self.pos..self.pos + size];
            self.pos += size;
            return Some(s);
        }
        self.valid = false;
        None
    }

    fn read_u32(&mut self) -> Option<u32> {
        let b = self.skip(Some(4))?;
        Some(u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_s32(&mut self) -> Option<i32> {
        let b = self.skip(Some(4))?;
        Some(i32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
    }

    // Port of: src/core/SkBuffer.cpp#L31-L42 (chrome/m156)
    fn skip_to_align4(&mut self) -> bool {
        let n = self.pos.next_multiple_of(4) - self.pos;
        if self.valid && n <= self.available() {
            self.pos += n;
            true
        } else {
            self.valid = false;
            false
        }
    }
}

impl Path {
    // Port of: src/core/SkPath_serial.cpp#L68-L106 (chrome/m156)
    fn write_to_memory_as_rrect(&self, storage: Option<&mut [u8]>) -> usize {
        let rrect;
        let first_dir;
        let start: u32;
        if let Some(oinfo) = self.get_oval_info() {
            rrect = RRect::new_oval(oinfo.bounds);
            first_dir = oinfo.direction;
            // Convert to rrect start indices.
            start = u32::from(oinfo.start_index) * 2;
        } else if let Some(rinfo) = self.get_rrect_info() {
            rrect = rinfo.rrect;
            first_dir = rinfo.direction;
            start = u32::from(rinfo.start_index);
        } else {
            return 0;
        }

        // packed header, rrect, start index.
        let size_needed = 4 + RRect::SIZE_IN_MEMORY + 4;
        let Some(storage) = storage else {
            return size_needed;
        };

        let packed = ((self.fill_type() as u32) << FILL_TYPE_SERIALIZATION_SHIFT)
            | ((first_dir as u32) << DIRECTION_SERIALIZATION_SHIFT)
            | (SERIALIZATION_TYPE_RRECT << TYPE_SERIALIZATION_SHIFT)
            | CURRENT_VERSION;

        let mut buffer = WBuffer {
            data: Some(storage),
            pos: 0,
        };
        buffer.write32(packed);
        let mut rr_bytes = Vec::new();
        rrect.write_to_memory(&mut rr_bytes);
        buffer.write(&rr_bytes);
        buffer.write32(start);
        buffer.pad_to_align4();
        debug_assert_eq!(size_needed, buffer.pos);
        buffer.pos
    }

    /// Writes the path to `buffer`, or only measures it if `buffer` is `None`. Returns the
    /// number of bytes (a multiple of 4), or 0 if the size overflows.
    ///
    /// # Panics
    /// If `buffer` is shorter than the returned size.
    // Port of: src/core/SkPath_serial.cpp#L108-L151 (chrome/m156)
    #[doc(alias = "writeToMemory")]
    #[must_use]
    pub fn write_to_memory(&self, buffer: Option<&mut [u8]>) -> usize {
        let mut buffer = buffer;
        let bytes = self.write_to_memory_as_rrect(buffer.as_deref_mut());
        if bytes != 0 {
            return bytes;
        }

        let packed = ((self.fill_type() as u32) << FILL_TYPE_SERIALIZATION_SHIFT)
            | (SERIALIZATION_TYPE_GENERAL << TYPE_SERIALIZATION_SHIFT)
            | CURRENT_VERSION;

        let points = self.points();
        let verbs = self.verbs();
        let conics = self.conic_weights();

        let size = (|| {
            let mut size: usize = 4 * 4;
            size = size.checked_add(points.len().checked_mul(8)?)?;
            size = size.checked_add(conics.len().checked_mul(4)?)?;
            size = size.checked_add(verbs.len())?;
            size.checked_next_multiple_of(4)
        })();
        let Some(size) = size else {
            return 0;
        };
        let Some(storage) = buffer else {
            return size;
        };

        let mut buffer = WBuffer {
            data: Some(storage),
            pos: 0,
        };
        #[allow(clippy::cast_possible_truncation)] // SkToS32
        {
            buffer.write32(packed);
            buffer.write32(points.len() as u32);
            buffer.write32(conics.len() as u32);
            buffer.write32(verbs.len() as u32);
        }
        for p in points {
            buffer.write(&p.x.to_ne_bytes());
            buffer.write(&p.y.to_ne_bytes());
        }
        for w in conics {
            buffer.write(&w.to_ne_bytes());
        }
        for v in verbs {
            buffer.write(&[*v as u8]);
        }
        buffer.pad_to_align4();
        debug_assert_eq!(buffer.pos, size);
        size
    }

    /// The serialized bytes of the path (`SkPath::serialize`).
    // Port of: src/core/SkPath_serial.cpp#L153-L158 (chrome/m156)
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let size = self.write_to_memory(None);
        let mut data = vec![0u8; size];
        let _ = self.write_to_memory(Some(&mut data));
        data
    }

    /// Reads a path written by [`write_to_memory`](Self::write_to_memory). Returns the path (or
    /// `None` if the buffer is too short or invalid) and the number of bytes read (0 on failure).
    // Port of: src/core/SkPath_serial.cpp#L218-L285 (chrome/m156)
    #[doc(alias = "ReadFromMemory")]
    #[must_use]
    #[allow(clippy::chunks_exact_to_as_chunks)] // plain per-element decoding
    pub fn read_from_memory(storage: &[u8]) -> (Option<Path>, usize) {
        let mut buffer = RBuffer::new(storage);
        let Some(packed) = buffer.read_u32() else {
            return (None, 0);
        };

        let version = extract_version(packed);
        let verbs_are_forward = version == VERBS_ARE_STORED_FORWARD_VERSION;
        if !verbs_are_forward && version != JUST_PUBLIC_DATA_VERSION {
            // Old/unsupported version.
            return (None, 0);
        }

        match extract_serializationtype(packed) {
            SERIALIZATION_TYPE_RRECT => return read_rrect_path(storage),
            SERIALIZATION_TYPE_GENERAL => {} // fall out
            _ => return (None, 0),
        }

        // To minimize the number of reads done a structure with the counts is used.
        let (Some(pts), Some(cnx), Some(vbs)) =
            (buffer.read_u32(), buffer.read_u32(), buffer.read_u32())
        else {
            return (None, 0);
        };
        let (pts, cnx, vbs) = (pts as usize, cnx as usize, vbs as usize);

        let points = buffer.skip(pts.checked_mul(8));
        let conics = buffer.skip(cnx.checked_mul(4));
        let verbs = buffer.skip(Some(vbs));
        buffer.skip_to_align4();
        let (Some(points), Some(conics), Some(verbs)) = (points, conics, verbs) else {
            return (None, 0);
        };
        if !buffer.valid {
            return (None, 0);
        }
        debug_assert!(buffer.pos <= storage.len());

        if vbs == 0 {
            if pts == 0 && cnx == 0 {
                let path = Path::new_with_fill_type(extract_filltype(packed));
                return (Some(path), buffer.pos);
            }
            // No verbs but points and/or conic weights is a not a valid path.
            return (None, 0);
        }

        let points: Vec<Point> = points
            .chunks_exact(8)
            .map(|b| {
                Point::new(
                    f32::from_ne_bytes([b[0], b[1], b[2], b[3]]),
                    f32::from_ne_bytes([b[4], b[5], b[6], b[7]]),
                )
            })
            .collect();
        let conics: Vec<f32> = conics
            .chunks_exact(4)
            .map(|b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        let mut verb_bytes: Vec<u8> = verbs.to_vec();
        if !verbs_are_forward {
            verb_bytes.reverse();
        }

        let bytes_read = buffer.pos;
        let path = Path::new_from(
            &points,
            &verb_bytes,
            &conics,
            extract_filltype(packed),
            false,
        );
        (Some(path), bytes_read)
    }

    /// Reads a path from `data` (`SkPath::ReadFromMemory` without the byte count).
    #[must_use]
    pub fn deserialize(data: &[u8]) -> Option<Path> {
        Self::read_from_memory(data).0
    }
}

// Port of: src/core/SkPath_serial.cpp#L163-L216 (chrome/m156)
fn read_rrect_path(storage: &[u8]) -> (Option<Path>, usize) {
    let mut buffer = RBuffer::new(storage);
    let Some(packed) = buffer.read_u32() else {
        return (None, 0);
    };

    debug_assert_eq!(extract_serializationtype(packed), SERIALIZATION_TYPE_RRECT);
    let dir = (packed >> DIRECTION_SERIALIZATION_SHIFT) & 0x3;
    let fill_type = extract_filltype(packed);

    let rrect_dir = match dir {
        0 => PathDirection::CW,  // (int)SkPathFirstDirection::kCW
        1 => PathDirection::CCW, // (int)SkPathFirstDirection::kCCW
        _ => return (None, 0),
    };

    // SkRRectPriv::ReadFromBuffer
    if buffer.available() < RRect::SIZE_IN_MEMORY {
        return (None, 0);
    }
    let Some(bytes) = buffer.skip(Some(RRect::SIZE_IN_MEMORY)) else {
        return (None, 0);
    };
    let mut rrect = RRect::default();
    if rrect.read_from_memory(bytes) != RRect::SIZE_IN_MEMORY {
        return (None, 0);
    }

    let Some(start) = buffer.read_s32() else {
        return (None, 0);
    };
    if start != start.clamp(0, 7) {
        return (None, 0);
    }

    #[allow(clippy::cast_sign_loss)] // 0..=7
    let mut path = Path::rrect_with_start_index(rrect, rrect_dir, start as usize);
    path.set_fill_type(fill_type);
    buffer.skip_to_align4();
    (Some(path), buffer.pos)
}
