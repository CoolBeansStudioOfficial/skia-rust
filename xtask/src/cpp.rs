//! A tiny C++ lexer: just enough to find test-registration macros, split their
//! arguments, and find the extent of the body that follows them, while ignoring
//! brackets inside comments and string/char literals.

/// A registration-macro use site found in a source file.
#[derive(Debug)]
pub struct MacroUse {
    pub name: String,
    /// 1-based line of the macro name.
    pub line: usize,
    /// Top-level arguments, whitespace-collapsed.
    pub args: Vec<String>,
    /// Source text from the macro name through the end of its body (or its
    /// closing parenthesis when no `{ ... }` body follows).
    pub span: String,
    /// 1-based line where the span ends.
    pub end_line: usize,
}

/// Whether `ident` looks like one of Skia's registration macros by name alone.
pub fn is_registration_macro(ident: &str) -> bool {
    ident == "SKSL_TEST"
        || (ident.starts_with("DEF_")
            && ["TEST", "GM", "BENCH", "FUZZ"]
                .iter()
                .any(|k| ident[4..].contains(k)))
}

/// Collect `#define NAME body` pairs (continuation lines joined) from `src`.
pub fn collect_defines(src: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut lines = src.lines();
    while let Some(line) = lines.next() {
        let t = line.trim_start();
        let Some(rest) = t.strip_prefix('#') else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix("define") else {
            continue;
        };
        let rest = rest.trim_start();
        let name_len = rest
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
            .count();
        if name_len == 0 {
            continue;
        }
        let name = rest[..name_len].to_owned();
        let mut body = rest[name_len..].to_owned();
        // Drop a function-like macro's parameter list.
        if body.starts_with('(') {
            body = body
                .find(')')
                .map_or(String::new(), |i| body[i + 1..].to_owned());
        }
        let mut cur = line;
        while cur.trim_end().ends_with('\\') {
            body = body.trim_end().trim_end_matches('\\').to_owned();
            match lines.next() {
                Some(next) => {
                    body.push(' ');
                    body.push_str(next);
                    cur = next;
                }
                None => break,
            }
        }
        out.push((name, collapse(body.trim_end_matches('\\').as_bytes())));
    }
    out
}

/// Find every use of a macro accepted by `is_registration` in `src`, skipping
/// `#define` lines and macro-body continuation lines.
pub fn find_macro_uses(src: &str, is_registration: &dyn Fn(&str) -> bool) -> Vec<MacroUse> {
    let bytes = src.as_bytes();
    let mut uses = Vec::new();
    let mut offset = 0;
    let mut prev_continues = false;
    for (line_idx, line) in src.split_inclusive('\n').enumerate() {
        let start = offset;
        offset += line.len();
        let continues = line.trim_end().ends_with('\\');
        let in_define = prev_continues;
        prev_continues = continues;
        if in_define {
            continue;
        }
        let trimmed = line.trim_start();
        let ident_len = trimmed
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
            .count();
        let ident = &trimmed[..ident_len];
        if ident.is_empty() || !is_registration(ident) {
            continue;
        }
        let ident_start = start + (line.len() - trimmed.len());
        let mut pos = skip_trivia(bytes, ident_start + ident_len);
        if bytes.get(pos) != Some(&b'(') {
            continue;
        }
        let Some((args, after_args)) = split_args(bytes, pos) else {
            continue;
        };
        pos = after_args;
        let body_start = skip_trivia(bytes, pos);
        let end = if bytes.get(body_start) == Some(&b'{') {
            match_close(bytes, body_start, b'{', b'}').unwrap_or(pos)
        } else {
            pos
        };
        let span = &src[ident_start..end];
        uses.push(MacroUse {
            name: ident.to_owned(),
            line: line_idx + 1,
            args,
            span: span.to_owned(),
            end_line: line_idx + 1 + span.matches('\n').count(),
        });
    }
    uses
}

/// Skip whitespace and comments starting at `pos`.
fn skip_trivia(b: &[u8], mut pos: usize) -> usize {
    loop {
        while pos < b.len() && b[pos].is_ascii_whitespace() {
            pos += 1;
        }
        match skip_comment(b, pos) {
            Some(next) => pos = next,
            None => return pos,
        }
    }
}

/// If a comment starts at `pos`, return the position after it.
fn skip_comment(b: &[u8], pos: usize) -> Option<usize> {
    match (b.get(pos), b.get(pos + 1)) {
        (Some(b'/'), Some(b'/')) => {
            let mut p = pos + 2;
            while p < b.len() && b[p] != b'\n' {
                p += 1;
            }
            Some(p)
        }
        (Some(b'/'), Some(b'*')) => {
            let mut p = pos + 2;
            while p + 1 < b.len() && !(b[p] == b'*' && b[p + 1] == b'/') {
                p += 1;
            }
            Some((p + 2).min(b.len()))
        }
        _ => None,
    }
}

/// If a string or char literal starts at `pos`, return the position after it.
/// Raw strings (`R"(...)"`) are handled for the default delimiter only.
fn skip_literal(b: &[u8], pos: usize) -> Option<usize> {
    if b.get(pos) == Some(&b'R') && b.get(pos + 1) == Some(&b'"') {
        let mut p = pos + 2;
        let delim_start = p;
        while p < b.len() && b[p] != b'(' {
            p += 1;
        }
        let close: Vec<u8> = [b")".as_slice(), &b[delim_start..p], b"\""].concat();
        let rest = &b[p..];
        let found = rest
            .windows(close.len())
            .position(|w| w == close.as_slice())?;
        return Some(p + found + close.len());
    }
    let quote = *b.get(pos)?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    // A `'` between digits is a C++14 digit separator, not a char literal.
    if quote == b'\'' && pos > 0 && b[pos - 1].is_ascii_alphanumeric() {
        return None;
    }
    let mut p = pos + 1;
    while p < b.len() && b[p] != quote {
        match b[p] {
            b'\\' => p += 2,
            b'\n' => return Some(p), // unterminated; don't run away
            _ => p += 1,
        }
    }
    Some((p + 1).min(b.len()))
}

/// Given `b[open] == open_ch`, return the position after the matching `close_ch`.
fn match_close(b: &[u8], open: usize, open_ch: u8, close_ch: u8) -> Option<usize> {
    let mut depth = 0usize;
    let mut p = open;
    while p < b.len() {
        if let Some(next) = skip_comment(b, p).or_else(|| skip_literal(b, p)) {
            p = next;
            continue;
        }
        if b[p] == open_ch {
            depth += 1;
        } else if b[p] == close_ch {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(p + 1);
            }
        }
        p += 1;
    }
    None
}

/// Split the parenthesized argument list starting at `b[open] == '('` into
/// top-level arguments. Returns the arguments and the position after `)`.
fn split_args(b: &[u8], open: usize) -> Option<(Vec<String>, usize)> {
    let mut args = Vec::new();
    let mut depth = 0usize;
    let mut arg_start = open + 1;
    let mut p = open;
    while p < b.len() {
        if let Some(next) = skip_comment(b, p).or_else(|| skip_literal(b, p)) {
            p = next;
            continue;
        }
        match b[p] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    args.push(collapse(&b[arg_start..p]));
                    if args.len() == 1 && args[0].is_empty() {
                        args.clear();
                    }
                    return Some((args, p + 1));
                }
            }
            b',' if depth == 1 => {
                args.push(collapse(&b[arg_start..p]));
                arg_start = p + 1;
            }
            _ => {}
        }
        p += 1;
    }
    None
}

fn collapse(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_def_test_with_body() {
        let src = "// x\nDEF_TEST(Foo, r) {\n  REPORTER_ASSERT(r, s == \"}\");\n}\nint y;\n";
        let uses = find_macro_uses(src, &is_registration_macro);
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].name, "DEF_TEST");
        assert_eq!(uses[0].args, ["Foo", "r"]);
        assert_eq!(uses[0].line, 2);
        assert_eq!(uses[0].end_line, 4);
        assert!(uses[0].span.ends_with('}'));
    }

    #[test]
    fn skips_defines_and_continuations() {
        let src =
            "#define DEF_X_TEST(n) \\\n    DEF_TEST(n, r) {}\nDEF_GM(return new FooGM(1, 2);)\n";
        let uses = find_macro_uses(src, &is_registration_macro);
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].name, "DEF_GM");
        assert_eq!(uses[0].args, ["return new FooGM(1, 2);"]);
    }

    #[test]
    fn defines() {
        let d = collect_defines("#define A(x) DEF_TEST(x, r) \\\n  { f(); }\n#  define B\n");
        assert_eq!(d[0].0, "A");
        assert_eq!(d[0].1, "DEF_TEST(x, r) { f(); }");
        assert_eq!(d[1], ("B".to_owned(), String::new()));
    }

    #[test]
    fn multi_line_args_and_digit_separators() {
        let src = "DEF_SIMPLE_GM(big, canvas,\n              1'000, 2'000) { canvas->clear(0); }\n";
        let uses = find_macro_uses(src, &is_registration_macro);
        assert_eq!(uses[0].args, ["big", "canvas", "1'000", "2'000"]);
        assert_eq!(uses[0].end_line, 2);
    }
}
