// Port of: libjpeg-turbo src/jutils.c#L59-L72 (libjpeg_turbo@e14cbfaa, 3.1.0 source)
//
//! Shared tables and small types: the colour spaces (`J_COLOR_SPACE`), the natural-order table,
//! the component and table structs (`jpeg_component_info`, `JQUANT_TBL`, `JHUFF_TBL`).

/// `DCTSIZE`: the basic DCT block is 8x8.
pub const DCTSIZE: usize = 8;
/// `DCTSIZE2`: samples per block.
pub const DCTSIZE2: usize = 64;
/// `MAX_COMPONENTS`.
pub const MAX_COMPONENTS: usize = 10;
/// `MAX_COMPS_IN_SCAN`.
pub const MAX_COMPS_IN_SCAN: usize = 4;
/// `MAX_SAMP_FACTOR`.
pub const MAX_SAMP_FACTOR: i32 = 4;
/// `NUM_QUANT_TBLS`.
pub const NUM_QUANT_TBLS: usize = 4;
/// `NUM_HUFF_TBLS`.
pub const NUM_HUFF_TBLS: usize = 4;
/// `NUM_ARITH_TBLS`.
pub const NUM_ARITH_TBLS: usize = 16;
/// `D_MAX_BLOCKS_IN_MCU`.
pub const D_MAX_BLOCKS_IN_MCU: usize = 10;
/// `JPEG_MAX_DIMENSION`.
pub const JPEG_MAX_DIMENSION: u32 = 65500;

/// `J_COLOR_SPACE`: the colour spaces of the JPEG data and of the decoder output.
///
/// The `Ext*` variants are libjpeg-turbo's extended output layouts (`jdcolext.c`); the
/// discriminants match `jpeglib.h`, so they can be compared with the C enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ColorSpace {
    /// `JCS_UNKNOWN`.
    #[default]
    Unknown = 0,
    /// `JCS_GRAYSCALE`.
    Grayscale = 1,
    /// `JCS_RGB`.
    Rgb = 2,
    /// `JCS_YCbCr`.
    YCbCr = 3,
    /// `JCS_CMYK`.
    Cmyk = 4,
    /// `JCS_YCCK`.
    Ycck = 5,
    /// `JCS_EXT_RGB`.
    ExtRgb = 6,
    /// `JCS_EXT_RGBX`.
    ExtRgbx = 7,
    /// `JCS_EXT_BGR`.
    ExtBgr = 8,
    /// `JCS_EXT_BGRX`.
    ExtBgrx = 9,
    /// `JCS_EXT_XBGR`.
    ExtXbgr = 10,
    /// `JCS_EXT_XRGB`.
    ExtXrgb = 11,
    /// `JCS_EXT_RGBA`.
    ExtRgba = 12,
    /// `JCS_EXT_BGRA`.
    ExtBgra = 13,
    /// `JCS_EXT_ABGR`.
    ExtAbgr = 14,
    /// `JCS_EXT_ARGB`.
    ExtArgb = 15,
    /// `JCS_RGB565`.
    Rgb565 = 16,
}

/// `rgb_pixelsize[]` (jdcolext.c, jdcolor.c): bytes per output pixel for each RGB-type space.
pub(crate) fn rgb_pixelsize(cs: ColorSpace) -> i32 {
    match cs {
        ColorSpace::Rgb | ColorSpace::ExtRgb | ColorSpace::ExtBgr => 3,
        ColorSpace::ExtRgbx
        | ColorSpace::ExtBgrx
        | ColorSpace::ExtXbgr
        | ColorSpace::ExtXrgb
        | ColorSpace::ExtRgba
        | ColorSpace::ExtBgra
        | ColorSpace::ExtAbgr
        | ColorSpace::ExtArgb => 4,
        _ => 1,
    }
}

/// `jpeg_natural_order` (`jutils.c`): zigzag index to natural (row-major) index, with 16
/// trailing entries of 63 for corrupt-data overrun.
pub(crate) const NATURAL_ORDER: [usize; DCTSIZE2 + 16] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63,
    63, 63, 63, 63, 63, 63,
];

/// `JQUANT_TBL`: a quantization table in natural order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JQuantTbl {
    /// The 64 quantizers, in natural (row-major) order.
    pub quantval: [u16; DCTSIZE2],
    /// Whether the table has been sent (libjpeg bookkeeping; unused by the decoder).
    pub sent_table: bool,
}

/// `JHUFF_TBL`: a Huffman table as stored in a DHT segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JHuffTbl {
    /// `bits[1..=16]`: number of codes of each length. `bits[0]` is unused.
    pub bits: [u8; 17],
    /// The symbol values, in code order.
    pub huffval: [u8; 256],
    /// Whether the table has been sent (libjpeg bookkeeping; unused by the decoder).
    pub sent_table: bool,
}

/// `jpeg_component_info`: one colour component of the frame, plus its scan state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompInfo {
    /// Identifier of the component (`Ci`).
    pub component_id: i32,
    /// Index in the SOF list.
    pub component_index: i32,
    /// Horizontal sampling factor (`Hi`).
    pub h_samp_factor: i32,
    /// Vertical sampling factor (`Vi`).
    pub v_samp_factor: i32,
    /// Quantization table number.
    pub quant_tbl_no: i32,
    /// DC Huffman table number, set by SOS.
    pub dc_tbl_no: i32,
    /// AC Huffman table number, set by SOS.
    pub ac_tbl_no: i32,
    /// Horizontal scaled DCT size (1..=16).
    pub dct_h_scaled_size: i32,
    /// Vertical scaled DCT size (1..=16).
    pub dct_v_scaled_size: i32,
    /// Width in blocks, at the padded size.
    pub width_in_blocks: u32,
    /// Height in blocks, at the padded size.
    pub height_in_blocks: u32,
    /// Real (unpadded) downsampled width.
    pub downsampled_width: u32,
    /// Real (unpadded) downsampled height.
    pub downsampled_height: u32,
    /// Whether the component is needed for output.
    pub component_needed: bool,
    /// MCU width in blocks for the current scan.
    pub mcu_width: i32,
    /// MCU height in blocks for the current scan.
    pub mcu_height: i32,
    /// Blocks per MCU for this component.
    pub mcu_blocks: i32,
    /// Sample width of one MCU at the scaled DCT size.
    pub mcu_sample_width: i32,
    /// Width of the last MCU column, in blocks.
    pub last_col_width: i32,
    /// Height of the last MCU row, in blocks.
    pub last_row_height: i32,
    /// The quantization table latched for this component at its first scan.
    pub quant_table: Option<[u16; DCTSIZE2]>,
    /// Whether `quant_table` has been latched (libjpeg's `quant_table != NULL`).
    pub quant_table_latched: bool,
}
