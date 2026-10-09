// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/core/SkMasks.cpp#L1-L140 and src/core/SkMasks.h (chrome/m156)
// Ported from: src/core/SkMasks.cpp, src/core/SkMasks.h

//! Bit masks that extract colour components from packed pixels (BMP's "bitfields" format).

/// Lookup from an n-bit component (1 to 7 bits) to its 8-bit value. Entries for n bits start at
/// `(1 << n) - 2`, as in Skia.
// Port of: src/core/SkMasks.cpp#L15-L37 (n_bit_to_8_bit_lookup_table)
#[rustfmt::skip]
static N_BIT_TO_8_BIT_LOOKUP_TABLE: [u8; 254] = [
    // 1 bit
    0, 255,
    // 2 bits
    0, 85, 170, 255,
    // 3 bits
    0, 36, 73, 109, 146, 182, 219, 255,
    // 4 bits
    0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255,
    // 5 bits
    0, 8, 16, 25, 33, 41, 49, 58, 66, 74, 82, 90, 99, 107, 115, 123, 132, 140,
    148, 156, 165, 173, 181, 189, 197, 206, 214, 222, 230, 239, 247, 255,
    // 6 bits
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 45, 49, 53, 57, 61, 65, 69, 73,
    77, 81, 85, 89, 93, 97, 101, 105, 109, 113, 117, 121, 125, 130, 134, 138,
    142, 146, 150, 154, 158, 162, 166, 170, 174, 178, 182, 186, 190, 194, 198,
    202, 206, 210, 215, 219, 223, 227, 231, 235, 239, 243, 247, 251, 255,
    // 7 bits
    0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36, 38,
    40, 42, 44, 46, 48, 50, 52, 54, 56, 58, 60, 62, 64, 66, 68, 70, 72, 74, 76,
    78, 80, 82, 84, 86, 88, 90, 92, 94, 96, 98, 100, 102, 104, 106, 108, 110,
    112, 114, 116, 118, 120, 122, 124, 126, 129, 131, 133, 135, 137, 139, 141,
    143, 145, 147, 149, 151, 153, 155, 157, 159, 161, 163, 165, 167, 169, 171,
    173, 175, 177, 179, 181, 183, 185, 187, 189, 191, 193, 195, 197, 199, 201,
    203, 205, 207, 209, 211, 213, 215, 217, 219, 221, 223, 225, 227, 229, 231,
    233, 235, 237, 239, 241, 243, 245, 247, 249, 251, 253, 255,
];

/// Port of `SkMasks::MaskInfo`: one mask, with its shift and width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[doc(alias = "SkMasks::MaskInfo")]
pub struct MaskInfo {
    /// The bits of the mask.
    pub mask: u32,
    /// Bits to shift right to reach the component.
    pub shift: u32,
    /// Width of the component in bits (at most 8 after processing).
    pub size: u32,
}

/// Port of `SkMasks::InputMasks`: the masks as stored in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[doc(alias = "SkMasks::InputMasks")]
pub struct InputMasks {
    /// Red mask.
    pub red: u32,
    /// Green mask.
    pub green: u32,
    /// Blue mask.
    pub blue: u32,
    /// Alpha mask (zero when the file has none).
    pub alpha: u32,
}

/// Port of `SkMasks`: the processed red, green, blue and alpha masks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "SkMasks")]
pub struct Masks {
    red: MaskInfo,
    green: MaskInfo,
    blue: MaskInfo,
    alpha: MaskInfo,
}

// Port of: src/core/SkMasks.cpp#L40-L52 (convert_to_8). The component is at most 8 bits wide, so
// the narrowing from `u32` cannot lose bits.
#[allow(clippy::cast_possible_truncation)]
fn convert_to_8(component: u8, n: u32) -> u8 {
    if n == 0 {
        0
    } else if n < 8 {
        N_BIT_TO_8_BIT_LOOKUP_TABLE[((1u32 << n) - 2 + u32::from(component)) as usize]
    } else {
        // Port of the `SkASSERT(8 == n)` branch.
        component
    }
}

// Port of: src/core/SkMasks.cpp#L54-L57 (get_comp). `(pixel & mask) >> shift` has at most `size`
// (at most 8) significant bits, so the narrowing to u8 is exact.
#[allow(clippy::cast_possible_truncation)]
fn get_comp(pixel: u32, info: MaskInfo) -> u8 {
    convert_to_8(((pixel & info.mask) >> info.shift) as u8, info.size)
}

// Port of: src/core/SkMasks.cpp#L104-L131 (process_mask)
fn process_mask(mut mask: u32) -> MaskInfo {
    let mut temp_mask = mask;
    let mut shift = 0u32;
    let mut size = 0u32;
    if temp_mask != 0 {
        // Count trailing zeros on masks.
        while temp_mask & 1 == 0 {
            shift += 1;
            temp_mask >>= 1;
        }
        // Count the size of the mask.
        while temp_mask & 1 != 0 {
            size += 1;
            temp_mask >>= 1;
        }
        // Verify that the mask is continuous. Skia only prints a warning here; the size is
        // extended over the remaining bits either way.
        if temp_mask != 0 {
            while temp_mask != 0 {
                size += 1;
                temp_mask >>= 1;
            }
        }
        // Truncate masks greater than 8 bits. The shift stays below 32 because the mask's bits
        // all lie within 32 bits.
        if size > 8 {
            shift += size - 8;
            size = 8;
            mask &= 0xFF << shift;
        }
    }
    MaskInfo { mask, shift, size }
}

impl Masks {
    /// Port of `SkMasks::CreateMasks`: trims the masks to `bytes_per_pixel` and processes them.
    /// Returns `None` when two masks overlap.
    // Port of: src/core/SkMasks.cpp#L133-L160 (chrome/m156)
    #[doc(alias = "SkMasks::CreateMasks")]
    #[must_use]
    pub fn create(masks: InputMasks, bytes_per_pixel: u32) -> Option<Self> {
        let mut masks = masks;
        // Trim the input masks to match bytesPerPixel.
        if bytes_per_pixel < 4 {
            let bits_per_pixel = 8 * bytes_per_pixel;
            let keep = (1u32 << bits_per_pixel) - 1;
            masks.red &= keep;
            masks.green &= keep;
            masks.blue &= keep;
            masks.alpha &= keep;
        }

        // Check that masks do not overlap.
        let overlap = (masks.red & masks.green)
            | (masks.red & masks.blue)
            | (masks.red & masks.alpha)
            | (masks.green & masks.blue)
            | (masks.green & masks.alpha)
            | (masks.blue & masks.alpha);
        if overlap != 0 {
            return None;
        }

        Some(Self {
            red: process_mask(masks.red),
            green: process_mask(masks.green),
            blue: process_mask(masks.blue),
            alpha: process_mask(masks.alpha),
        })
    }

    /// Port of `SkMasks::getRed`.
    #[must_use]
    pub fn get_red(&self, pixel: u32) -> u8 {
        get_comp(pixel, self.red)
    }

    /// Port of `SkMasks::getGreen`.
    #[must_use]
    pub fn get_green(&self, pixel: u32) -> u8 {
        get_comp(pixel, self.green)
    }

    /// Port of `SkMasks::getBlue`.
    #[must_use]
    pub fn get_blue(&self, pixel: u32) -> u8 {
        get_comp(pixel, self.blue)
    }

    /// Port of `SkMasks::getAlpha`.
    #[must_use]
    pub fn get_alpha(&self, pixel: u32) -> u8 {
        get_comp(pixel, self.alpha)
    }

    /// Port of `SkMasks::getAlphaMask`: zero when there is no alpha mask.
    #[must_use]
    pub fn alpha_mask(&self) -> u32 {
        self.alpha.mask
    }
}
