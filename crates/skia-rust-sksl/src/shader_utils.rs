// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/utils/SkShaderUtils.{h,cpp} (chrome/m156). Only the pretty-printer is
// ported (`PrettyPrint`, `GLSLPrettyPrint`); the SPIR-V and shader-error helpers belong to the
// GPU back ends that are not ported.

//! `SkShaderUtils::PrettyPrint`: indents generated shader text, one statement per line.
//!
//! The rules are Skia's: braces get their own lines and tabs, `;` ends a line, `,` and `;` after a
//! `}` stay with it, `(` and `)` keep a `for` header on one line, and `#`, `//` and `/* */`
//! comments are copied through. The text is bytes, as the C++ is.

// Port of: src/utils/SkShaderUtils.cpp#L24-L241 (GLSLPrettyPrint) and #L244-L247 (PrettyPrint),
// chrome/m156.

/// `GLSLPrettyPrint`: the state of one `prettify` run.
struct GlslPrettyPrint<'a> {
    freshline: bool,
    tabs: i32,
    index: usize,
    length: usize,
    input: &'a [u8],
    pretty: Vec<u8>,
    // Some helpers for parseUntil when we go over a string length.
    in_parse_until_newline: bool,
    in_parse_until: bool,
    in_parse_until_token: &'static [u8],
}

/// `PrettyPrint(const std::string&)`: the prettified form of `text`.
// Port of: src/utils/SkShaderUtils.cpp#L244-L247 (chrome/m156)
#[doc(alias = "SkShaderUtils::PrettyPrint")]
#[must_use]
pub fn pretty_print(text: &[u8]) -> Vec<u8> {
    let mut pp = GlslPrettyPrint {
        freshline: true,
        tabs: 0,
        index: 0,
        length: text.len(),
        input: text,
        pretty: Vec::new(),
        in_parse_until_newline: false,
        in_parse_until: false,
        in_parse_until_token: b"",
    };
    pp.prettify()
}

impl GlslPrettyPrint<'_> {
    // Port of: src/utils/SkShaderUtils.cpp#L30-L140 (GLSLPrettyPrint::prettify, chrome/m156)
    fn prettify(&mut self) -> Vec<u8> {
        let mut parens_depth: i32 = 0;

        while self.length > self.index {
            // The heart and soul of our prettification algorithm. The rules are described in the
            // module documentation, and follow Skia's comments in `prettify`.
            if self.in_parse_until_newline {
                self.parse_until_newline();
                continue;
            }
            if self.in_parse_until {
                let token = self.in_parse_until_token;
                self.parse_until(token);
                continue;
            }
            if self.has_token(b"#") || self.has_token(b"//") {
                self.parse_until_newline();
                continue;
            }
            if self.has_token(b"/*") {
                self.parse_until(b"*/");
                continue;
            }
            if self.current() == b'{' {
                self.newline();
                self.append_char();
                self.tabs += 1;
                self.newline();
                continue;
            }
            if self.current() == b'}' {
                self.tabs -= 1;
                self.newline();
                self.append_char();
                self.newline();
                continue;
            }
            if self.freshline && self.current() == b';' {
                self.undo_newline_after(b'}');
                self.append_char();
                self.newline();
                continue;
            }
            if self.freshline && self.current() == b',' {
                self.undo_newline_after(b'}');
                self.append_char();
                continue;
            }
            if self.has_token(b")") {
                parens_depth -= 1;
                continue;
            }
            if self.has_token(b"(") {
                parens_depth += 1;
                continue;
            }
            if self.has_token(b")") {
                parens_depth -= 1;
                continue;
            }
            if parens_depth == 0 && self.has_token(b";") {
                self.newline();
                continue;
            }
            if self.current() == b'\t'
                || self.current() == b'\n'
                || (self.freshline && self.current() == b' ')
            {
                self.index += 1;
                continue;
            }
            self.append_char();
        }
        std::mem::take(&mut self.pretty)
    }

    /// The byte at the cursor. The loops only call it while `index < length`.
    fn current(&self) -> u8 {
        self.input[self.index]
    }

    /// `appendChar`: copies the byte at the cursor, indented if it starts a line.
    fn append_char(&mut self) {
        self.tab_string();
        self.pretty.push(self.input[self.index]);
        self.index += 1;
        self.freshline = false;
    }

    /// `hasToken`: when the input continues with `token` (or ends while it is still matching),
    /// consumes it and appends the whole token, after tabbing if needed.
    fn has_token(&mut self, token: &[u8]) -> bool {
        let mut i = self.index;
        let mut j = 0;
        while j < token.len() && self.length > i {
            if token[j] != self.input[i] {
                return false;
            }
            i += 1;
            j += 1;
        }
        self.tab_string();
        self.index = i;
        self.pretty.extend_from_slice(token);
        self.freshline = false;
        true
    }

    /// `parseUntilNewline`: copies the rest of the line, verbatim.
    fn parse_until_newline(&mut self) {
        while self.length > self.index {
            if self.input[self.index] == b'\n' {
                self.index += 1;
                self.newline();
                self.in_parse_until_newline = false;
                break;
            }
            self.pretty.push(self.input[self.index]);
            self.index += 1;
            self.in_parse_until_newline = true;
        }
    }

    /// `parseUntil`: copies text up to and including `token`. Newlines inside are kept, with the
    /// next line tabbed out. Only long-style comments reach here.
    fn parse_until(&mut self, token: &'static [u8]) {
        while self.length > self.index {
            // For embedded newlines, this code will make sure to embed the newline in the pretty
            // string, increase the linecount, and tab out the next line to the appropriate place.
            if self.input[self.index] == b'\n' {
                self.newline();
                self.tab_string();
                self.index += 1;
            }
            if self.has_token(token) {
                self.in_parse_until = false;
                break;
            }
            self.freshline = false;
            self.pretty.push(self.input[self.index]);
            self.index += 1;
            self.in_parse_until = true;
            self.in_parse_until_token = token;
        }
    }

    /// `tabString`: only tabs when on a fresh line; otherwise the line is considered tabbed.
    fn tab_string(&mut self) {
        if self.freshline {
            for _ in 0..self.tabs {
                self.pretty.push(b'\t');
            }
        }
    }

    /// `newline`: a request for a line break. On a fresh line there is no reason to add another.
    fn newline(&mut self) {
        if !self.freshline {
            self.freshline = true;
            self.pretty.push(b'\n');
        }
    }

    /// `undoNewlineAfter`: undoes the effect of `newline`, if the last byte before the newline is
    /// `c`.
    fn undo_newline_after(&mut self, c: u8) {
        if self.freshline {
            let n = self.pretty.len();
            if n >= 2 && self.pretty[n - 1] == b'\n' && self.pretty[n - 2] == c {
                self.freshline = false;
                self.pretty.pop();
            }
        }
    }
}
