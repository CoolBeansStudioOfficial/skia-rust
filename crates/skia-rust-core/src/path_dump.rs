// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkPathDump.cpp

//! Text dumps of paths and path builders (`SkPathDump.cpp`).

use crate::path::Path;
use crate::path_builder::{DumpFormat, PathBuilder};
use crate::path_iter::PathIter;
use crate::path_types::{PathFillType, PathVerb};
use crate::point::Point;
use crate::string_utils::{ScalarAsStringType, append_scalar, append_scalar_dec};

const FILL_TYPE_STRS: [&str; 4] = ["Winding", "EvenOdd", "InverseWinding", "InverseEvenOdd"];

const SENTINEL_CONIC_WEIGHT: f32 = -12345.0; // not a valid weight

fn fill_type_str(ft: PathFillType) -> &'static str {
    FILL_TYPE_STRS[ft as usize]
}

// Port of: src/core/SkPathDump.cpp#L27-L60 (chrome/m156)
#[allow(clippy::float_cmp)] // sentinel comparison, as in C++
fn append_params(
    str: &mut String,
    label: &str,
    pts: &[Point],
    str_type: ScalarAsStringType,
    use_semicolon: bool,
    conic_weight: f32,
) {
    str.push_str(label);
    str.push('(');

    let values: Vec<f32> = pts.iter().flat_map(|p| [p.x, p.y]).collect();
    let count = values.len();

    for (i, &v) in values.iter().enumerate() {
        append_scalar(str, v, str_type);
        if i < count - 1 {
            str.push_str(", ");
        }
    }
    if conic_weight != SENTINEL_CONIC_WEIGHT {
        str.push_str(", ");
        append_scalar(str, conic_weight, str_type);
    }
    str.push_str(if use_semicolon { ");" } else { ")" });
    if str_type == ScalarAsStringType::Hex {
        str.push_str("  // ");
        for (i, &v) in values.iter().enumerate() {
            append_scalar_dec(str, v);
            if i < count - 1 {
                str.push_str(", ");
            }
        }
        if conic_weight >= 0.0 {
            str.push_str(", ");
            append_scalar_dec(str, conic_weight);
        }
    }
    str.push('\n');
}

// Port of: src/core/SkPathDump.cpp#L62-L109 (chrome/m156)
fn dump_iter(
    iter: PathIter<'_>,
    builder: &mut String,
    cmd_prefix: &str,
    str_type: ScalarAsStringType,
    use_semicolon: bool,
    mut post_verb_proc: impl FnMut(&mut String),
) {
    for rec in iter {
        let mut cmd = String::from(cmd_prefix);
        let all = rec.points();
        let mut pts: &[Point] = &[];
        let mut cw = SENTINEL_CONIC_WEIGHT;

        match rec.verb() {
            PathVerb::Move => {
                cmd.push_str(".moveTo");
                pts = &all[0..1];
            }
            PathVerb::Line => {
                cmd.push_str(".lineTo");
                pts = &all[1..2];
            }
            PathVerb::Quad => {
                cmd.push_str(".quadTo");
                pts = &all[1..3];
            }
            PathVerb::Conic => {
                cmd.push_str(".conicTo");
                pts = &all[1..3];
                cw = rec.conic_weight();
            }
            PathVerb::Cubic => {
                cmd.push_str(".cubicTo");
                pts = &all[1..4];
            }
            PathVerb::Close => {
                cmd.push_str(".close()");
                if use_semicolon {
                    cmd.push(';');
                }
                cmd.push('\n');
                builder.push_str(&cmd);
            }
        }
        // we don't do this for kClose
        if !pts.is_empty() {
            append_params(builder, &cmd, pts, str_type, use_semicolon, cw);
        }
        post_verb_proc(builder);
    }
}

impl Path {
    /// The path as C++ source (`path.setFillType(...); path.moveTo(...); ...`), with exact hex
    /// floats if `dump_as_hex`.
    // Port of: src/core/SkPathDump.cpp#L111-L127 (chrome/m156)
    #[doc(alias = "dump")]
    #[must_use]
    pub fn dump_to_string(&self, dump_as_hex: bool) -> String {
        let as_type = if dump_as_hex {
            ScalarAsStringType::Hex
        } else {
            ScalarAsStringType::Dec
        };

        let mut builder = format!(
            "path.setFillType(SkPathFillType::k{});\n",
            fill_type_str(self.fill_type())
        );

        dump_iter(self.iter(), &mut builder, "path", as_type, true, |_| {});
        builder
    }

    /// Prints [`dump_to_string(false)`](Self::dump_to_string) to stderr (`SkDebugf`).
    pub fn dump(&self) {
        eprint!("{}", self.dump_to_string(false));
    }

    /// Prints [`dump_to_string(true)`](Self::dump_to_string) to stderr (`SkDebugf`).
    #[doc(alias = "dumpHex")]
    pub fn dump_hex(&self) {
        eprint!("{}", self.dump_to_string(true));
    }
}

impl PathBuilder {
    /// The builder as C++ source (`SkPathBuilder(...)\n.moveTo(...)\n...`).
    // Port of: src/core/SkPathDump.cpp#L129-L141 (chrome/m156)
    #[doc(alias = "dumpToString")]
    #[must_use]
    pub fn dump_to_string(&self, format: DumpFormat) -> String {
        let as_type = if format == DumpFormat::Hex {
            ScalarAsStringType::Hex
        } else {
            ScalarAsStringType::Dec
        };

        let mut builder = format!(
            "SkPathBuilder(SkPathFillType::k{})\n",
            fill_type_str(self.fill_type())
        );

        dump_iter(self.iter(), &mut builder, "", as_type, false, |_| {});

        builder
    }

    /// Prints [`dump_to_string`](Self::dump_to_string) to stderr (`SkDebugf`).
    // Port of: src/core/SkPathDump.cpp#L143-L145 (chrome/m156)
    pub fn dump(&self, format: DumpFormat) {
        // C++ ignores `format` here and dumps in decimal.
        let _ = format;
        eprint!("{}", self.dump_to_string(DumpFormat::Decimal));
    }
}
