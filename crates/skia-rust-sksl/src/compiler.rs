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

    /// `FRAGCOLOR_NAME`: the fragment output that `out location=0` may only declare.
    pub const FRAGCOLOR_NAME: &'static str = "sk_FragColor";
    /// `RTADJUST_NAME`: the uniform that makes the IR generator emit position-fixup expressions.
    pub const RTADJUST_NAME: &'static str = "sk_RTAdjust";

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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Compiler;
    use crate::position::Position;

    /// The byte range of the `nth` (0-based) occurrence of `needle` in `src`.
    fn range_of(src: &str, needle: &str, nth: usize) -> Position {
        let start = src
            .match_indices(needle)
            .nth(nth)
            .expect("needle in source")
            .0;
        let start = i32::try_from(start).unwrap();
        let len = i32::try_from(needle.len()).unwrap();
        Position::range(start, start + len)
    }

    fn compiler_for(src: &str) -> Compiler {
        let mut compiler = Compiler::new();
        compiler.error_reporter().set_source(Arc::from(src));
        compiler
    }

    /// The text `skslc` writes after its `### Compilation failed:\n\n` header.
    fn golden_body(golden: &str) -> &str {
        golden
            .strip_prefix("### Compilation failed:\n\n")
            .expect("an error golden")
    }

    #[test]
    fn error_text_matches_ossfuzz38140_golden() {
        // resources/sksl/errors/Ossfuzz38140.sksl and tests/sksl/errors/Ossfuzz38140.glsl.
        let src = concat!(
            "half4 blend_src_over(half4 src, half4 dst) {\n",
            "    return src + (1 - src.a)*dst;\n",
            "}\n",
            "\n",
            "half4 main(half4 src, half4 dst) {\n",
            "    return blend_src_over(src, half4(1) - dst);\n",
            "}\n",
            "\n",
            "/*%%*\n",
            "differ only in modifiers\n",
            "*%%*/\n",
        );
        let golden = concat!(
            "### Compilation failed:\n",
            "\n",
            "error: 1: functions 'half4 blend_src_over(half4 src, half4 dst)' and '$pure half4 ",
            "blend_src_over(half4 src, half4 dst)' differ only in modifiers\n",
            "half4 blend_src_over(half4 src, half4 dst) {\n",
            "^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n",
            "error: 2: unknown identifier 'src'\n",
            "    return src + (1 - src.a)*dst;\n",
            "           ^^^\n",
            "error: 2: unknown identifier 'src'\n",
            "    return src + (1 - src.a)*dst;\n",
            "                      ^^^\n",
            "error: 2: unknown identifier 'dst'\n",
            "    return src + (1 - src.a)*dst;\n",
            "                             ^^^\n",
            "error: 5: shader 'main' must be main() or main(float2)\n",
            "half4 main(half4 src, half4 dst) {\n",
            "^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n",
            "error: 6: unknown identifier 'src'\n",
            "    return blend_src_over(src, half4(1) - dst);\n",
            "                          ^^^\n",
            "error: 6: unknown identifier 'dst'\n",
            "    return blend_src_over(src, half4(1) - dst);\n",
            "                                          ^^^\n",
            "7 errors\n",
        );
        let mut compiler = compiler_for(src);
        let errors = [
            (
                range_of(src, "half4 blend_src_over(half4 src, half4 dst)", 0),
                "functions 'half4 blend_src_over(half4 src, half4 dst)' and '$pure half4 \
                 blend_src_over(half4 src, half4 dst)' differ only in modifiers",
            ),
            (range_of(src, "src", 2), "unknown identifier 'src'"),
            (range_of(src, "src", 3), "unknown identifier 'src'"),
            (range_of(src, "dst", 1), "unknown identifier 'dst'"),
            (
                range_of(src, "half4 main(half4 src, half4 dst)", 0),
                "shader 'main' must be main() or main(float2)",
            ),
            (range_of(src, "src", 6), "unknown identifier 'src'"),
            (range_of(src, "dst", 3), "unknown identifier 'dst'"),
        ];
        for (pos, msg) in errors {
            compiler.context_mut().errors.error(pos, msg);
        }
        assert_eq!(compiler.error_count(), 7);
        assert_eq!(compiler.error_text(true), golden_body(golden));
        // errorText resets the errors.
        assert_eq!(compiler.error_count(), 0);
        assert_eq!(compiler.error_text(true), "");
    }

    #[test]
    fn error_text_marks_ranges_that_run_past_the_line() {
        // resources/sksl/errors/ForLoopOverflow.rts and tests/sksl/errors/ForLoopOverflow.glsl.
        let src = concat!(
            "half4 main(float2 coords) {\n",
            "    half arr[4];\n",
            "    for (int i = 2147483640; i < 2147483647; i += 100) {\n",
            "        arr[i - 2147483640] = half(1);\n",
            "    }\n",
            "    return half4(0);\n",
            "}\n",
            "\n",
            "/*%%*\n",
            "loop must guarantee termination in fewer iterations\n",
            "*%%*/\n",
        );
        let golden = concat!(
            "### Compilation failed:\n",
            "\n",
            "error: 3: loop must guarantee termination in fewer iterations\n",
            "    for (int i = 2147483640; i < 2147483647; i += 100) {\n",
            "    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^...\n",
            "1 error\n",
        );
        let mut compiler = compiler_for(src);
        let start = range_of(src, "for (", 0).start_offset();
        let end = range_of(src, "    }\n", 0).end_offset() - 1;
        compiler.context_mut().errors.error(
            Position::range(start, end),
            "loop must guarantee termination in fewer iterations",
        );
        assert_eq!(compiler.error_text(true), golden_body(golden));
    }

    #[test]
    fn error_text_echoes_multi_line_messages() {
        // resources/sksl/errors/IllegalRecursionSimple.rts and its .glsl golden.
        let src = concat!(
            "// Expect 1 error\n",
            "\n",
            "// Simple recursion is not allowed, even with branching:\n",
            "int fibonacci(int n) { return n <= 1 ? n : fibonacci(n - 1) + fibonacci(n - 2); }\n",
            "\n",
            "/*%%*\n",
            "potential recursion (function call cycle) not allowed:\n",
            "\tint fibonacci(int n)\n",
            "\tint fibonacci(int n)\n",
            "*%%*/\n",
        );
        let golden = concat!(
            "### Compilation failed:\n",
            "\n",
            "error: 4: potential recursion (function call cycle) not allowed:\n",
            "\tint fibonacci(int n)\n",
            "\tint fibonacci(int n)\n",
            "int fibonacci(int n) { return n <= 1 ? n : fibonacci(n - 1) + fibonacci(n - 2); }\n",
            "                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n",
            "1 error\n",
        );
        let mut compiler = compiler_for(src);
        let body = range_of(
            src,
            "{ return n <= 1 ? n : fibonacci(n - 1) + fibonacci(n - 2); }",
            0,
        );
        compiler.context_mut().errors.error(
            body,
            "potential recursion (function call cycle) not allowed:\n\tint fibonacci(int n)\n\
             \tint fibonacci(int n)",
        );
        assert_eq!(compiler.error_text(true), golden_body(golden));
    }

    #[test]
    fn error_text_cuts_long_lines_with_ellipses() {
        // The first two errors of tests/sksl/errors/Ossfuzz44561.glsl, from the first two lines
        // of resources/sksl/errors/Ossfuzz44561.sksl (which hold a DEL and a CR byte).
        let line1 = concat!(
            "void m(){ix;void[(0).r1(((5).ss0s.ss0s.sss0.ss0s+(5).ss0s.sss.00ss.ss0s.ss .ss0.",
            "ss00.ss0s+(5).ss0s.ss0.s0s.ss00.sssch (int) {case 0:{{{{{{{{{{{{{{{{{{{{{{{{\x7fe;",
            "void n(){;; int \rm;;half x;",
        );
        let src = format!("{line1}\nx*x++.ss1.ss;0;\n");
        let expected = format!(
            "error: 1: unknown identifier 'ix'\n{}...\n{}^^\n\
             error: 1: too many components in swizzle mask\n...{}\n{}^\n",
            &line1[..111],
            " ".repeat(9),
            &line1[16..],
            " ".repeat(103),
        );
        // The echoed texts above, as the golden spells them.
        assert!(expected.contains(
            "void m(){ix;void[(0).r1(((5).ss0s.ss0s.sss0.ss0s+(5).ss0s.sss.00ss.ss0s.ss .ss0.\
             ss00.ss0s+(5).ss0s.ss0.s0s.ss00...\n"
        ));
        let mut compiler = compiler_for(&src);
        compiler
            .context_mut()
            .errors
            .error(Position::range(9, 11), "unknown identifier 'ix'");
        compiler.context_mut().errors.error(
            Position::range(116, 117),
            "too many components in swizzle mask",
        );
        assert_eq!(compiler.error_text(false), expected);
    }

    #[test]
    fn poison_errors_and_invalid_positions() {
        let mut compiler = compiler_for("x");
        compiler
            .context_mut()
            .errors
            .error(Position::range(0, 1), "'<POISON>' is not a type");
        assert_eq!(compiler.error_count(), 0, "errors about poison are dropped");
        compiler
            .context_mut()
            .errors
            .error(Position::default(), "no position");
        // A position at the end of the text prints its line number but no source line.
        compiler
            .context_mut()
            .errors
            .error(Position::range(1, 1), "at end");
        assert_eq!(
            compiler.error_text(true),
            "error: no position\nerror: 1: at end\n2 errors\n"
        );
    }
}
