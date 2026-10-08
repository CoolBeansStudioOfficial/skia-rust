// Copyright (C) 1998-2025 Glenn Randers-Pehrson and the libpng contributors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
// Port of: png.c (libpng 1.6.56, skia.googlesource.com/third_party/libpng@d5515b5b), the parts the
// read path uses: struct creation, CRC, unknown-chunk rules, IHDR validation, option setting and
// the ICC profile checks.

// Clippy: each module is a line-by-line port of libpng's C, whose integer casts, long
// functions, argument lists and error returns are kept as written so they can be compared
// with the C. The Port of links name the C source for each item.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::missing_errors_doc,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::cognitive_complexity
)]
use std::fmt::Write as _;

use crate::error::PngResult;
use crate::rutil::{PNG_UINT_31_MAX, get_uint_32};
use crate::structs::{PngInfo, PngStruct, flag, mode};

/// Port of `PNG_MAXIMUM_INFLATE_WINDOW` (png.h): the option number.
pub const PNG_MAXIMUM_INFLATE_WINDOW: u32 = 2;
/// Port of `PNG_OPTION_ON` (png.h).
pub const PNG_OPTION_ON: u32 = 3;
/// Port of `PNG_OPTION_INVALID` (png.h).
pub const PNG_OPTION_INVALID: i32 = 1;
/// Port of `PNG_OPTION_NEXT` (png.h).
const PNG_OPTION_NEXT: u32 = 16;

/// Port of `PNG_HANDLE_CHUNK_*` (png.h): how an unknown chunk is kept.
pub const PNG_HANDLE_CHUNK_AS_DEFAULT: u8 = 0;
pub const PNG_HANDLE_CHUNK_NEVER: u8 = 1;
pub const PNG_HANDLE_CHUNK_IF_SAFE: u8 = 2;
pub const PNG_HANDLE_CHUNK_ALWAYS: u8 = 3;

/// Port of `PNG_USER_CHUNK_MALLOC_MAX` (pnglibconf.h, Skia's configuration).
pub(crate) const PNG_USER_CHUNK_MALLOC_MAX: usize = 8_000_000;
/// Port of `PNG_USER_WIDTH_MAX` and `PNG_USER_HEIGHT_MAX` (pnglibconf.h, Skia's configuration).
pub(crate) const PNG_USER_WIDTH_MAX: u32 = 1_000_000;
pub(crate) const PNG_USER_HEIGHT_MAX: u32 = 1_000_000;
/// Port of `PNG_USER_CHUNK_CACHE_MAX` (pnglibconf.h, Skia's configuration).
pub(crate) const PNG_USER_CHUNK_CACHE_MAX: u32 = 1000;

/// Port of `D50_nCIEXYZ` (png.c#L816): the PCS illuminant an ICC profile must carry.
const D50_NCIEXYZ: [u8; 12] = [
    0x00, 0x00, 0xf6, 0xd6, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0xd3, 0x2d,
];

/// Port of `PNG_sRGB_INTENT_LAST`: the first value after the defined rendering intents.
const PNG_SRGB_INTENT_LAST: u32 = 4;

/// Port of `PNG_COMPRESSION_TYPE_BASE` and `PNG_FILTER_TYPE_BASE` (png.h).
const PNG_FILTER_TYPE_BASE: u8 = 0;
const PNG_COMPRESSION_TYPE_BASE: u8 = 0;

/// Port of `PNG_INTERLACE_LAST` (png.h): the first invalid interlace type.
const PNG_INTERLACE_LAST: u8 = 2;

/// Receives the progressive reader's callbacks: `png_set_progressive_read_fn`'s `info_fn`,
/// `row_fn` and `end_fn`, with the user data owned by the implementor.
///
/// `row` is `None` for the empty rows of an interlaced pass (`png_push_have_row(NULL)`). An error
/// returned from a callback stops the read and is returned to the caller, which is the longjmp.
pub trait ProgressiveHandler: Send {
    /// Port of `info_fn`: called once the IDAT chunk is reached.
    fn info(&mut self, png: &mut PngStruct, info: &mut PngInfo) -> PngResult<()>;
    /// Port of `row_fn`: one decoded row, after the transforms.
    fn row(
        &mut self,
        png: &mut PngStruct,
        row: Option<&[u8]>,
        row_num: u32,
        pass: i32,
    ) -> PngResult<()>;
    /// Port of `end_fn`: called after the IEND chunk.
    fn end(&mut self, png: &mut PngStruct, info: &mut PngInfo) -> PngResult<()>;
}

/// Port of the `png_set_read_user_chunk_fn` callback. Returns `1` if the chunk was handled, `-1`
/// on an error, and `0` to let libpng handle it as an unknown chunk.
pub trait UserChunkReader: Send {
    /// Port of `user_chunk_fn(png_ptr, chunk)`. `name` is the four-byte chunk type.
    fn read_chunk(&mut self, name: &[u8; 4], data: &[u8]) -> i32;
}

/// Port of `crc32` from zlib: the reflected CRC-32 (polynomial `0xEDB88320`). `crc` is the running
/// value (`0` to start), as `crc32(crc, buf, len)` takes it. zlib's SIMD CRC computes the same
/// function.
#[must_use]
pub(crate) fn crc32(crc: u32, buf: &[u8]) -> u32 {
    let mut c = !crc;
    for &b in buf {
        c = CRC_TABLE[((c ^ u32::from(b)) & 0xff) as usize] ^ (c >> 8);
    }
    !c
}

/// Table for [`crc32`]: `CRC_TABLE[n]` is the CRC of the byte `n`.
const CRC_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
};

impl PngStruct {
    /// Port of `png_create_read_struct` (png.c#L124-L160) with libpng's defaults.
    #[doc(alias = "png_create_read_struct")]
    #[must_use]
    pub fn new_read() -> Self {
        PngStruct {
            mode: mode::IS_READ_STRUCT,
            // Port of png_read_init_3 (pngread.c#L60-L70): benign errors are warnings for read
            // structs, and in a release build application warnings are too.
            flags: flag::BENIGN_ERRORS_WARN | flag::APP_WARNINGS_WARN,
            user_width_max: PNG_USER_WIDTH_MAX,
            user_height_max: PNG_USER_HEIGHT_MAX,
            user_chunk_cache_max: PNG_USER_CHUNK_CACHE_MAX,
            user_chunk_malloc_max: PNG_USER_CHUNK_MALLOC_MAX,
            ..PngStruct::default()
        }
    }

    /// Port of `png_reset_crc` (png.c#L55-L58).
    #[doc(alias = "png_reset_crc")]
    pub(crate) fn reset_crc(&mut self) {
        self.crc = crc32(0, &[]);
    }

    /// Port of `png_calculate_crc` (png.c#L60-L88).
    #[doc(alias = "png_calculate_crc")]
    pub(crate) fn calculate_crc(&mut self, ptr: &[u8]) {
        let mut need_crc = true;
        if crate::structs::chunk_ancillary(self.chunk_name) != 0 {
            if self.flags & flag::CRC_ANCILLARY_MASK
                == (flag::CRC_ANCILLARY_USE | flag::CRC_ANCILLARY_NOWARN)
            {
                need_crc = false;
            }
        } else if self.flags & flag::CRC_CRITICAL_IGNORE != 0 {
            need_crc = false;
        }
        if need_crc && !ptr.is_empty() {
            self.crc = crc32(self.crc, ptr);
        }
    }

    /// Port of `png_chunk_unknown_handling` and `png_handle_as_unknown` (png.c#L509-L533): the
    /// handling for a chunk type, from the `png_set_keep_unknown_chunks` list or the default.
    #[doc(alias = "png_handle_as_unknown")]
    pub(crate) fn chunk_unknown_handling_for_name(&self, chunk_name: u32) -> u8 {
        let name = chunk_name.to_be_bytes();
        for (entry, keep) in self.chunk_list.iter().rev() {
            if entry.as_slice() == name {
                return *keep;
            }
        }
        // `png_handle_as_unknown` returns PNG_HANDLE_CHUNK_AS_DEFAULT, and the caller then uses
        // `png_ptr->unknown_default` (see `handle_unknown`).
        PNG_HANDLE_CHUNK_AS_DEFAULT
    }

    /// Port of `png_set_option` (png.c#L1738-L1753). Returns the old setting of the option.
    #[doc(alias = "png_set_option")]
    pub fn set_option(&mut self, option: u32, onoff: bool) -> i32 {
        if option < PNG_OPTION_NEXT && option & 1 == 0 {
            let mask = 3u32 << option;
            let setting = (2u32 + u32::from(onoff)) << option;
            let current = self.options;
            self.options = (current & !mask) | setting;
            return ((current & mask) >> option) as i32;
        }
        PNG_OPTION_INVALID
    }

    /// Port of `png_check_IHDR` (png.c#L1000-L1100). Each failed check is a warning, and the
    /// header is then rejected with an error.
    #[doc(alias = "png_check_IHDR")]
    pub(crate) fn check_ihdr(
        &mut self,
        width: u32,
        height: u32,
        bit_depth: u8,
        color_type: u8,
        interlace_type: u8,
        compression_type: u8,
        filter_type: u8,
    ) -> PngResult<()> {
        let mut error = false;
        if width == 0 {
            self.warning("Image width is zero in IHDR");
            error = true;
        }
        if width > PNG_UINT_31_MAX {
            self.warning("Invalid image width in IHDR");
            error = true;
        }
        // Port of the architecture check: `((width + 7) & ~7) > (SIZE_MAX - 48 - 1) / 8 - 1`.
        let width_round = (u64::from(width) + 7) & !7u64;
        if width_round > ((u64::MAX - 48 - 1) / 8) - 1 {
            self.warning("Image width is too large for this architecture");
            error = true;
        }
        if width > self.user_width_max {
            self.warning("Image width exceeds user limit in IHDR");
            error = true;
        }
        if height == 0 {
            self.warning("Image height is zero in IHDR");
            error = true;
        }
        if height > PNG_UINT_31_MAX {
            self.warning("Invalid image height in IHDR");
            error = true;
        }
        if height > self.user_height_max {
            self.warning("Image height exceeds user limit in IHDR");
            error = true;
        }
        if bit_depth != 1 && bit_depth != 2 && bit_depth != 4 && bit_depth != 8 && bit_depth != 16 {
            self.warning("Invalid bit depth in IHDR");
            error = true;
        }
        if color_type == 1 || color_type == 5 || color_type > 6 {
            self.warning("Invalid color type in IHDR");
            error = true;
        }
        if (color_type == 3 && bit_depth > 8)
            || ((color_type == 2 || color_type == 4 || color_type == 6) && bit_depth < 8)
        {
            self.warning("Invalid color type/bit depth combination in IHDR");
            error = true;
        }
        if interlace_type >= PNG_INTERLACE_LAST {
            self.warning("Unknown interlace method in IHDR");
            error = true;
        }
        if compression_type != PNG_COMPRESSION_TYPE_BASE {
            self.warning("Unknown compression method in IHDR");
            error = true;
        }
        // MNG features are never permitted in Skia's configuration, so a non-base filter method is
        // always unknown. A signature-less stream may not use the MNG filter either.
        if filter_type != PNG_FILTER_TYPE_BASE {
            self.warning("Unknown filter method in IHDR");
            error = true;
            if self.mode & mode::HAVE_PNG_SIGNATURE != 0 {
                self.warning("Invalid filter method in IHDR");
            }
        }
        if error {
            return Err(self.error("Invalid IHDR data"));
        }
        Ok(())
    }

    /// Port of `png_icc_profile_error` (png.c#L778-L806). A benign error: an error unless the
    /// application asked for benign errors to warn, in which case the result is `false`.
    fn icc_profile_error(&mut self, name: &[u8], value: u32, reason: &str) -> PngResult<bool> {
        let mut message = format!("profile '{}': ", String::from_utf8_lossy(name));
        if is_icc_signature(value) {
            let tag: String = value
                .to_be_bytes()
                .iter()
                .map(|&c| {
                    if (32..=126).contains(&c) {
                        char::from(c)
                    } else {
                        '?'
                    }
                })
                .collect();
            // The writes to a String cannot fail.
            let _ = write!(message, "'{tag}': ");
        } else {
            let _ = write!(message, "{value:x}h: ");
        }
        message.push_str(reason);
        self.chunk_benign_error(&message)?;
        Ok(false)
    }

    /// Port of `png_icc_check_length` (png.c#L817-L826): the length must be at least 132 bytes and
    /// no longer than `png_chunk_max`.
    #[doc(alias = "png_icc_check_length")]
    pub(crate) fn icc_check_length(&mut self, name: &[u8], profile_length: u32) -> PngResult<bool> {
        if profile_length < 132 {
            return self.icc_profile_error(name, profile_length, "too short");
        }
        if profile_length as usize > self.user_chunk_malloc_max {
            return self.icc_profile_error(name, profile_length, "profile too long");
        }
        Ok(true)
    }

    /// Port of `png_icc_check_header` (png.c#L828-L911).
    #[doc(alias = "png_icc_check_header")]
    pub(crate) fn icc_check_header(
        &mut self,
        name: &[u8],
        profile_length: u32,
        profile: &[u8],
        color_type: u8,
    ) -> PngResult<bool> {
        let temp = get_uint_32(&profile[0..4]);
        if temp != profile_length {
            return self.icc_profile_error(name, temp, "length does not match profile");
        }
        let temp = u32::from(profile[8]);
        if temp > 3 && profile_length & 3 != 0 {
            return self.icc_profile_error(name, profile_length, "invalid length");
        }
        let temp = get_uint_32(&profile[128..132]);
        if temp > 357_913_930 || profile_length < 132 + 12 * temp {
            return self.icc_profile_error(name, temp, "tag count too large");
        }
        let temp = get_uint_32(&profile[64..68]);
        if temp >= 0xffff {
            return self.icc_profile_error(name, temp, "invalid rendering intent");
        }
        if temp >= PNG_SRGB_INTENT_LAST {
            self.icc_profile_error(name, temp, "intent outside defined range")?;
        }
        let temp = get_uint_32(&profile[36..40]);
        if temp != 0x6163_7370 {
            return self.icc_profile_error(name, temp, "invalid signature");
        }
        if profile[68..80] != D50_NCIEXYZ {
            self.icc_profile_error(name, 0, "PCS illuminant is not D50")?;
        }
        let temp = get_uint_32(&profile[16..20]);
        match temp {
            0x5247_4220 => {
                if color_type & 2 == 0 {
                    return self.icc_profile_error(
                        name,
                        temp,
                        "RGB color space not permitted on grayscale PNG",
                    );
                }
            }
            0x4752_4159 => {
                if color_type & 2 != 0 {
                    return self.icc_profile_error(
                        name,
                        temp,
                        "Gray color space not permitted on RGB PNG",
                    );
                }
            }
            _ => return self.icc_profile_error(name, temp, "invalid ICC profile color space"),
        }
        let temp = get_uint_32(&profile[12..16]);
        match temp {
            0x7363_6e72 | 0x6d6e_7472 | 0x7072_7472 | 0x7370_6163 => {}
            0x6162_7374 => {
                return self.icc_profile_error(name, temp, "invalid embedded Abstract ICC profile");
            }
            0x6c69_6e6b => {
                return self.icc_profile_error(
                    name,
                    temp,
                    "unexpected DeviceLink ICC profile class",
                );
            }
            0x6e6d_636c => {
                self.icc_profile_error(name, temp, "unexpected NamedColor ICC profile class")?;
            }
            _ => {
                self.icc_profile_error(name, temp, "unrecognized ICC profile class")?;
            }
        }
        let temp = get_uint_32(&profile[20..24]);
        match temp {
            0x5859_5a20 | 0x4c61_6220 => {}
            _ => return self.icc_profile_error(name, temp, "unexpected ICC PCS encoding"),
        }
        Ok(true)
    }

    /// Port of `png_icc_check_tag_table` (png.c#L913-L935).
    #[doc(alias = "png_icc_check_tag_table")]
    pub(crate) fn icc_check_tag_table(
        &mut self,
        name: &[u8],
        profile_length: u32,
        profile: &[u8],
    ) -> PngResult<bool> {
        let tag_count = get_uint_32(&profile[128..132]);
        for itag in 0..tag_count as usize {
            let tag = &profile[132 + 12 * itag..];
            let tag_id = get_uint_32(&tag[0..4]);
            let tag_start = get_uint_32(&tag[4..8]);
            let tag_length = get_uint_32(&tag[8..12]);
            if tag_start > profile_length || tag_length > profile_length - tag_start {
                return self.icc_profile_error(name, tag_id, "ICC profile tag outside profile");
            }
            if tag_start & 3 != 0 {
                self.icc_profile_error(name, tag_id, "ICC profile tag start not a multiple of 4")?;
            }
        }
        Ok(true)
    }

    /// Port of `png_read_buffer` (pngrutil.c#L378-L400): a zeroed buffer of `new_size` bytes, or
    /// `None` when it exceeds `png_chunk_max` (the `user_chunk_malloc_max` limit).
    #[doc(alias = "png_read_buffer")]
    pub(crate) fn read_buffer_alloc(&mut self, new_size: u32) -> Option<Vec<u8>> {
        if new_size as usize > self.user_chunk_malloc_max {
            return None;
        }
        Some(vec![0u8; new_size as usize])
    }
}

/// Port of `png_is_ICC_signature` (png.c#L764-L775): the four bytes are printable signature
/// characters.
fn is_icc_signature(it: u32) -> bool {
    fn ok(c: u32) -> bool {
        c == 32 || (48..=57).contains(&c) || (65..=90).contains(&c) || (97..=122).contains(&c)
    }
    ok(it >> 24) && ok((it >> 16) & 0xff) && ok((it >> 8) & 0xff) && ok(it & 0xff)
}
