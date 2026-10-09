// Copyright (C) 2004, 2006-2025 Glenn Randers-Pehrson and the libpng contributors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
// Port of: pngerror.c#L1-L850 (libpng 1.6.56, skia.googlesource.com/third_party/libpng@d5515b5b),
// the functions the read path reaches.
//
// libpng reports an error with `png_error`, which calls the application's error function. Skia's
// error function (`sk_error_fn`) longjmps back to `setjmp` in the decoder. Here every function that
// can reach `png_error` returns [`PngResult`], and the longjmp becomes the `Err` that propagates to
// the caller, which is what `setjmp` returning non-zero did.

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
use crate::structs::PngStruct;

/// Why a libpng call stopped early. This replaces libpng's longjmp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PngError {
    /// Port of `png_error`: the operation failed with this message. Skia's `kPngError`.
    Error(String),
    /// A row or info callback asked to stop decoding (Skia's `kStopDecoding` longjmp). Not a
    /// failure: `processData` returns `true`.
    Stop,
}

impl std::fmt::Display for PngError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error(msg) => f.write_str(msg),
            Self::Stop => f.write_str("stop decoding"),
        }
    }
}

impl std::error::Error for PngError {}

/// Result of a libpng call that can longjmp.
pub type PngResult<T> = Result<T, PngError>;

/// Port of `PNG_MAX_ERROR_TEXT` (pngerror.c#L366).
const PNG_MAX_ERROR_TEXT: usize = 196;

/// Port of the `PNG_FLAG_BENIGN_ERRORS_WARN` and `mode` tests in pngerror.c. Holds the chunk
/// name bytes that `png_format_buffer` prints.
fn format_chunk_message(chunk_name: u32, error_message: &str) -> String {
    // Port of: png_format_buffer (pngerror.c#L380-L420). Non-alphabetic bytes are printed as
    // `[XX]` in hex.
    let mut out = String::new();
    let mut ishift: i32 = 24;
    while ishift >= 0 {
        let c = ((chunk_name >> ishift) & 0xff) as u8;
        ishift -= 8;
        // Port of isnonalpha (pngerror.c#L372).
        let nonalpha = !(65..=122).contains(&c) || (c > 90 && c < 97);
        if nonalpha {
            out.push('[');
            out.push(char::from(b"0123456789ABCDEF"[usize::from(c >> 4)]));
            out.push(char::from(b"0123456789ABCDEF"[usize::from(c & 0x0f)]));
            out.push(']');
        } else {
            out.push(char::from(c));
        }
    }
    out.push_str(": ");
    let text: String = error_message.chars().take(PNG_MAX_ERROR_TEXT - 1).collect();
    out.push_str(&text);
    out
}

impl PngStruct {
    /// Port of `png_error` (pngerror.c#L27-L35). Never returns normally: always an `Err`.
    #[doc(alias = "png_error")]
    #[must_use]
    pub fn error(&mut self, message: &str) -> PngError {
        self.warnings.push(format!("error: {message}"));
        PngError::Error(message.to_owned())
    }

    /// Port of `png_warning` (pngerror.c#L178-L186). Warnings do not change control flow.
    #[doc(alias = "png_warning")]
    pub fn warning(&mut self, message: &str) {
        self.warnings.push(message.to_owned());
    }

    /// Port of `png_benign_error` (pngerror.c#L263-L280): a warning if the application asked for
    /// benign errors to warn, otherwise an error. Read structs with a current chunk report the
    /// error with the chunk name.
    #[doc(alias = "png_benign_error")]
    pub fn benign_error(&mut self, message: &str) -> PngResult<()> {
        if self.flags & PNG_FLAG_BENIGN_ERRORS_WARN != 0 {
            if self.mode & PNG_IS_READ_STRUCT != 0 && self.chunk_name != 0 {
                self.chunk_warning(message);
            } else {
                self.warning(message);
            }
            Ok(())
        } else if self.mode & PNG_IS_READ_STRUCT != 0 && self.chunk_name != 0 {
            Err(self.chunk_error(message))
        } else {
            Err(self.error(message))
        }
    }

    /// Port of `png_chunk_error` (pngerror.c#L333-L345). The message is prefixed by the chunk
    /// name.
    #[doc(alias = "png_chunk_error")]
    #[must_use]
    pub fn chunk_error(&mut self, message: &str) -> PngError {
        let msg = format_chunk_message(self.chunk_name, message);
        self.error(&msg)
    }

    /// Port of `png_chunk_warning` (pngerror.c#L348-L357).
    #[doc(alias = "png_chunk_warning")]
    pub fn chunk_warning(&mut self, message: &str) {
        let msg = format_chunk_message(self.chunk_name, message);
        self.warning(&msg);
    }

    /// Port of `png_chunk_benign_error` (pngerror.c#L359-L366).
    #[doc(alias = "png_chunk_benign_error")]
    pub fn chunk_benign_error(&mut self, message: &str) -> PngResult<()> {
        if self.flags & PNG_FLAG_BENIGN_ERRORS_WARN != 0 {
            self.chunk_warning(message);
            Ok(())
        } else {
            Err(self.chunk_error(message))
        }
    }

    /// Port of `png_chunk_report` (pngerror.c#L371-L389) for a read struct.
    /// `error` is one of the `PNG_CHUNK_WARNING`/`ERROR` levels.
    #[doc(alias = "png_chunk_report")]
    pub fn chunk_report(&mut self, message: &str, error: i32) -> PngResult<()> {
        if error < PNG_CHUNK_ERROR {
            self.chunk_warning(message);
            Ok(())
        } else {
            self.chunk_benign_error(message)
        }
    }

    /// Port of `png_fixed_error` (pngerror.c#L431-L442).
    #[doc(alias = "png_fixed_error")]
    #[must_use]
    pub fn fixed_error(&mut self, name: &str) -> PngError {
        let text: String = name.chars().take(PNG_MAX_ERROR_TEXT - 1).collect();
        self.error(&format!("fixed point overflow in {text}"))
    }
}

// Flags and modes used by the error functions. Values from pngpriv.h and pngstruct.h.
pub(crate) const PNG_FLAG_BENIGN_ERRORS_WARN: u32 = 0x0010_0000;
pub(crate) const PNG_IS_READ_STRUCT: u32 = 0x8000;
/// Port of `PNG_CHUNK_ERROR` (pngpriv.h).
pub(crate) const PNG_CHUNK_ERROR: i32 = 2;
