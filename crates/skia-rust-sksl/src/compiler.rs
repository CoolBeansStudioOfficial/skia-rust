// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLCompiler.{h,cpp}: the error text (`handleError`, `errorText`,
// `writeErrorCount`, `resetErrors`) and `POISON_TAG`. Module loading, `convertProgram`,
// `optimize` and `finalize` come with tasks S11 and S13.

//! [`Compiler`]: owns the compilation [`Context`] and formats its errors.

use crate::context::Context;
use crate::error_reporter::{ErrorReporter, ErrorSink};
use crate::position::Position;

/// `SkSL::Compiler`.
// Port of: src/sksl/SkSLCompiler.h#L64-L225 (chrome/m156)
#[doc(alias = "SkSL::Compiler")]
#[derive(Debug)]
pub struct Compiler {
    context: Context,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    /// `POISON_TAG`: the name of the poison type and the text of a poison expression. Errors
    /// whose message contains it are not reported.
    pub const POISON_TAG: &'static str = "<POISON>";

    /// A compiler whose context reports errors into the compiler's error text.
    #[must_use]
    pub fn new() -> Self {
        Self {
            context: Context::new(ErrorReporter::compiler()),
        }
    }

    /// `context()`.
    #[must_use]
    pub fn context(&self) -> &Context {
        &self.context
    }

    /// `context()`, mutable.
    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }

    /// `errorReporter()`: the context's current reporter.
    pub fn error_reporter(&mut self) -> &mut ErrorReporter {
        &mut self.context.errors
    }

    /// `errorCount()`.
    #[must_use]
    pub fn error_count(&self) -> i32 {
        self.context.errors.error_count()
    }

    /// `handleError(msg, pos)`: appends one formatted error to the error text (without counting
    /// it; [`ErrorReporter::error`] counts).
    pub fn handle_error(&mut self, msg: &str, pos: Position) {
        let reporter = &mut self.context.errors;
        let source = reporter.source_arc().clone();
        handle_error(error_text_mut(reporter), source.as_bytes(), msg, pos);
    }

    /// `errorText(showCount)`: the accumulated error text (with the `N error(s)` line when
    /// `show_count`), then resets the errors. Source bytes that are not UTF-8 (a line window cut
    /// inside a character) are replaced; [`Compiler::error_text_bytes`] returns them verbatim.
    // Port of: src/sksl/SkSLCompiler.cpp#L521-L528 (chrome/m156)
    pub fn error_text(&mut self, show_count: bool) -> String {
        String::from_utf8_lossy(&self.error_text_bytes(show_count)).into_owned()
    }

    /// [`Compiler::error_text`] as the exact bytes Skia produces.
    pub fn error_text_bytes(&mut self, show_count: bool) -> Vec<u8> {
        if show_count {
            self.write_error_count();
        }
        let result = std::mem::take(error_text_mut(&mut self.context.errors));
        self.reset_errors();
        result
    }

    /// `writeErrorCount()`: appends `N error` or `N errors` when there are errors.
    // Port of: src/sksl/SkSLCompiler.cpp#L530-L536 (chrome/m156)
    pub fn write_error_count(&mut self) {
        let count = self.error_count();
        if count != 0 {
            let line = format!(
                "{count}{}",
                if count == 1 { " error\n" } else { " errors\n" }
            );
            error_text_mut(&mut self.context.errors).extend_from_slice(line.as_bytes());
        }
    }

    /// `resetErrors()`: clears the error text and the error count.
    pub fn reset_errors(&mut self) {
        error_text_mut(&mut self.context.errors).clear();
        self.context.errors.reset_error_count();
    }
}

/// The compiler's error text inside its reporter.
///
/// # Panics
///
/// If the context's reporter is not the compiler's (a parser checkpoint has swapped it out).
fn error_text_mut(reporter: &mut ErrorReporter) -> &mut Vec<u8> {
    match reporter.sink_mut() {
        ErrorSink::Compiler { error_text } => error_text,
        _ => panic!("Compiler: the context's error reporter is not the compiler's"),
    }
}

/// `Compiler::handleError`: appends `error: <line>: <msg>`, then (when the position is in the
/// source) the source line and a caret run under the error's range.
///
/// The echoed line shows at most 100 characters before the error (`...` marks the cut) and at
/// most 100 after its end (`...` again, unless the line or the text ends first). Tabs print as
/// four spaces (four carets inside the range), NULs as one space. A range that runs past the
/// end of its line ends its carets with `...`.
///
/// # Panics
///
/// If `pos` lies outside `src` (positions come from the lexer, so they never do).
// Port of: src/sksl/SkSLCompiler.cpp#L441-L519 (chrome/m156)
#[doc(alias = "Compiler::handleError")]
pub fn handle_error(error_text: &mut Vec<u8>, src: &[u8], msg: &str, pos: Position) {
    error_text.extend_from_slice(b"error: ");
    let mut print_location = false;
    let src_len = i32::try_from(src.len()).unwrap_or(i32::MAX);
    if pos.valid() {
        let line = pos.line(src);
        print_location = pos.start_offset() < src_len;
        error_text.extend_from_slice(format!("{line}: ").as_bytes());
    }
    error_text.extend_from_slice(msg.as_bytes());
    error_text.push(b'\n');
    if print_location {
        const MAX_SURROUNDING_CHARS: i32 = 100;
        let byte = |i: i32| src[usize::try_from(i).expect("offset within the source")];

        // Find the beginning of the line.
        let mut line_start = pos.start_offset();
        while line_start > 0 {
            if byte(line_start - 1) == b'\n' {
                break;
            }
            line_start -= 1;
        }

        // We don't want to show more than 100 characters surrounding the error, so push the
        // line start forward and add a leading ellipsis if there would be more than this.
        let mut line_text: Vec<u8> = Vec::new();
        let mut caret_text: Vec<u8> = Vec::new();
        if pos.start_offset() - line_start > MAX_SURROUNDING_CHARS {
            line_start = pos.start_offset() - MAX_SURROUNDING_CHARS;
            line_text.extend_from_slice(b"...");
            caret_text.extend_from_slice(b"   ");
        }

        // Echo the line. Again, we don't want to show more than 100 characters after the end of
        // the error, so truncate with a trailing ellipsis if needed.
        let mut line_suffix: &[u8] = b"...\n";
        let mut line_stop = pos.end_offset() + MAX_SURROUNDING_CHARS;
        if line_stop >= src_len {
            line_stop = src_len - 1;
            line_suffix = b"\n"; // no ellipsis if we reach end-of-file
        }
        for i in line_start..line_stop {
            let c = byte(i);
            if c == b'\n' {
                line_suffix = b"\n"; // no ellipsis if we reach end-of-line
                break;
            }
            match c {
                b'\t' => line_text.extend_from_slice(b"    "),
                b'\0' => line_text.push(b' '),
                _ => line_text.push(c),
            }
        }
        error_text.extend_from_slice(&line_text);
        error_text.extend_from_slice(line_suffix);

        // Print the carets underneath it, pointing to the range in question.
        let mut i = line_start;
        while i < src_len {
            if i >= pos.end_offset() {
                break;
            }
            match byte(i) {
                b'\t' => caret_text.extend_from_slice(if i >= pos.start_offset() {
                    b"^^^^"
                } else {
                    b"    "
                }),
                b'\n' => {
                    debug_assert!(i >= pos.start_offset());
                    // Use an ellipsis if the error continues past the end of the line.
                    caret_text.extend_from_slice(if pos.end_offset() > i + 1 {
                        b"..."
                    } else {
                        b"^"
                    });
                    i = src_len;
                }
                _ => caret_text.push(if i >= pos.start_offset() { b'^' } else { b' ' }),
            }
            i += 1;
        }
        error_text.extend_from_slice(&caret_text);
        error_text.push(b'\n');
    }
}
