//! Errors and warnings of the decompressor.
//!
//! libjpeg reports fatal errors with `ERREXIT*`, which calls `error_exit` and, in Skia, longjmps
//! back to the codec. Here every such call is an `Err(Error)` returned up the call chain. The
//! variants are the `JERR_*` codes that the decoder path can raise (`jerror.h`, libjpeg-turbo
//! 3.1.0). Warnings (`WARNMS*`) do not stop decoding; they are counted in
//! [`crate::Decompress::num_warnings`] and their codes kept in `last_warning`.

use std::fmt;

/// A fatal libjpeg error (`ERREXIT*`). Each variant names the `JERR_*` code it stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// `JERR_BAD_STATE`: an API function was called in the wrong global state.
    BadState(i32),
    /// `JERR_BAD_LIB_VERSION`.
    BadLibVersion,
    /// `JERR_BAD_STRUCT_SIZE`.
    BadStructSize,
    /// `JERR_NO_IMAGE`: the stream had no frame when one was required.
    NoImage,
    /// `JERR_NO_SOI`: the first two bytes were not `FF D8`.
    NoSoi(i32, i32),
    /// `JERR_SOI_DUPLICATE`.
    SoiDuplicate,
    /// `JERR_SOF_DUPLICATE`.
    SofDuplicate,
    /// `JERR_SOF_UNSUPPORTED`: a SOF type that is not supported (SOF5..SOF7, JPG, SOF13..15).
    SofUnsupported(i32),
    /// `JERR_EMPTY_IMAGE`: zero width, height or components.
    EmptyImage,
    /// `JERR_BAD_LENGTH`: a marker segment length disagrees with its content.
    BadLength,
    /// `JERR_SOS_NO_SOF`: SOS before SOF.
    SosNoSof,
    /// `JERR_BAD_COMPONENT_ID`.
    BadComponentId(i32),
    /// `JERR_DAC_INDEX`.
    DacIndex(i32),
    /// `JERR_DAC_VALUE`.
    DacValue(i32),
    /// `JERR_DHT_INDEX`.
    DhtIndex(i32),
    /// `JERR_BAD_HUFF_TABLE`.
    BadHuffTable,
    /// `JERR_DQT_INDEX`.
    DqtIndex(i32),
    /// `JERR_UNKNOWN_MARKER`.
    UnknownMarker(i32),
    /// `JERR_TOO_LITTLE_DATA`: `finish_decompress` before every scanline was read.
    TooLittleData,
    /// `JERR_BAD_SAMPLING`: unsupported sampling factors.
    BadSampling,
    /// `JERR_BAD_DCTSIZE`.
    BadDctSize,
    /// `JERR_NOT_COMPILED` or `JERR_NOTIMPL`: a feature this port does not provide.
    NotImplemented,
    /// `JERR_BAD_PROGRESSION`: an invalid progressive scan script.
    BadProgression,
    /// `JERR_BAD_PROG_SCRIPT`.
    BadProgScript,
    /// `JERR_BAD_BUFFER_MODE`.
    BadBufferMode,
    /// `JERR_BAD_CROP_SPEC`: `jpeg_crop_scanline` with an invalid range.
    BadCropSpec,
    /// `JERR_BAD_IN_COLORSPACE` / `JERR_BAD_J_COLORSPACE` / `JERR_BAD_OUT_COLORSPACE`.
    BadColorspace,
    /// `JERR_CANT_SUSPEND`.
    CantSuspend,
    /// `JERR_WIDTH_OVERFLOW`.
    WidthOverflow,
    /// `JERR_IMAGE_TOO_BIG`.
    ImageTooBig,
    /// `JERR_BAD_PRECISION`.
    BadPrecision,
    /// `JERR_ARITH_NOTIMPL`: arithmetic coding was requested but is not compiled in.
    ArithNotImplemented,
    /// `JERR_FRACT_SAMPLE_NOTIMPL`: the output scale is not one libjpeg supports.
    FractSampleNotImplemented,
    /// `JERR_BAD_DROP_SAMPLING`.
    BadDropSampling,
    /// `JERR_HUFF_CLEN_OVERFLOW` / `JERR_HUFF_MISSING_CODE`: a Huffman code that the table does
    /// not contain (raised by `jpeg_huff_decode`).
    HuffMissingCode,
    /// A source manager failure that Skia turns into `error_exit` (`skipInputBytes` past the end).
    SourceSkipFailed,
    /// `JERR_BAD_ALIGN_TYPE` and similar internal checks that cannot be reached with valid API use.
    Internal(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadState(s) => write!(f, "Improper call to JPEG library in state {s}"),
            Error::BadLibVersion => f.write_str("JPEG library version mismatch"),
            Error::BadStructSize => f.write_str("JPEG parameter struct mismatch"),
            Error::NoImage => f.write_str("Empty JPEG image (DNL not supported)"),
            Error::NoSoi(a, b) => write!(f, "Not a JPEG file: starts with 0x{a:02x} 0x{b:02x}"),
            Error::SoiDuplicate => f.write_str("Duplicate SOI marker"),
            Error::SofDuplicate => f.write_str("Duplicate SOF marker"),
            Error::SofUnsupported(m) => write!(f, "Unsupported JPEG process: SOF type 0x{m:02x}"),
            Error::EmptyImage => f.write_str("Empty JPEG image"),
            Error::BadLength => f.write_str("Bogus marker length"),
            Error::SosNoSof => f.write_str("Invalid SOS parameters for sequential JPEG"),
            Error::BadComponentId(c) => write!(f, "Invalid component ID {c} in SOS"),
            Error::DacIndex(i) => write!(f, "Bogus DAC index {i}"),
            Error::DacValue(v) => write!(f, "Bogus DAC value 0x{v:x}"),
            Error::DhtIndex(i) => write!(f, "Bogus DHT index {i}"),
            Error::BadHuffTable => f.write_str("Bogus Huffman table definition"),
            Error::DqtIndex(i) => write!(f, "Bogus DQT index {i}"),
            Error::UnknownMarker(m) => write!(f, "Unknown marker type 0x{m:02x}"),
            Error::TooLittleData => f.write_str("Premature end of JPEG file"),
            Error::BadSampling => f.write_str("Unsupported JPEG data precision or sampling"),
            Error::BadDctSize => f.write_str("Invalid DCT block size"),
            Error::NotImplemented => f.write_str("Requested feature was omitted at compile time"),
            Error::BadProgression => f.write_str("Invalid progressive parameters"),
            Error::BadProgScript => f.write_str("Invalid progressive parameters"),
            Error::BadBufferMode => f.write_str("Bogus buffered-image mode"),
            Error::BadCropSpec => f.write_str("Bogus crop request"),
            Error::BadColorspace => f.write_str("Unsupported color conversion request"),
            Error::CantSuspend => f.write_str("Suspension not allowed here"),
            Error::WidthOverflow => f.write_str("Image too wide for this implementation"),
            Error::ImageTooBig => f.write_str("Image too big"),
            Error::BadPrecision => f.write_str("Unsupported color conversion request"),
            Error::ArithNotImplemented => f.write_str("Arithmetic coding not supported"),
            Error::FractSampleNotImplemented => f.write_str("Fractional sampling not implemented yet"),
            Error::BadDropSampling => f.write_str("Bogus drop-sampling request"),
            Error::HuffMissingCode => f.write_str("Corrupt JPEG data: bad Huffman code"),
            Error::SourceSkipFailed => f.write_str("Failure to skip input data"),
            Error::Internal(what) => write!(f, "internal error: {what}"),
        }
    }
}

impl std::error::Error for Error {}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;
