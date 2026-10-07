// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/utils/SkParsePath.h, src/utils/SkParsePath.cpp

//! SVG path data strings (`SkParsePath`).

use super::parse::{self, at};
use crate::geometry::AutoConicToQuads;
use crate::path::{Iter, Path};
use crate::path_builder::{ArcSize, PathBuilder};
use crate::path_types::{PathDirection, PathVerb};
use crate::point::Point;
use crate::scalar::{SCALAR_1, scalar};
use crate::string_utils::str_append_scalar;

/// Absolute (`M L Q C`) or relative (`m l q c`) commands in [`to_svg_with_encoding`].
// Port of: include/utils/SkParsePath.h#L29 (chrome/m156)
#[doc(alias = "SkParsePath::PathEncoding")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum PathEncoding {
    #[default]
    Absolute,
    Relative,
}

// Port of: src/utils/SkParsePath.cpp#L21-L23 (chrome/m156)
fn is_between(c: i32, min: i32, max: i32) -> bool {
    #[allow(clippy::cast_sign_loss)] // the C++ unsigned-compare trick
    let r = (c - min) as u32 <= (max - min) as u32;
    r
}

// Port of: src/utils/SkParsePath.cpp#L25-L27 (chrome/m156)
fn is_ws(c: u8) -> bool {
    is_between(i32::from(c), 1, 32)
}

// Port of: src/utils/SkParsePath.cpp#L29-L31 (chrome/m156)
fn is_digit(c: u8) -> bool {
    is_between(i32::from(c), i32::from(b'0'), i32::from(b'9'))
}

// Port of: src/utils/SkParsePath.cpp#L33-L35 (chrome/m156)
fn is_sep(c: u8) -> bool {
    is_ws(c) || c == b','
}

// Port of: src/utils/SkParsePath.cpp#L37-L39 (chrome/m156)
fn is_lower(c: u8) -> bool {
    is_between(i32::from(c), i32::from(b'a'), i32::from(b'z'))
}

// Port of: src/utils/SkParsePath.cpp#L41-L43 (chrome/m156)
fn to_upper(c: u8) -> u8 {
    c - b'a' + b'A'
}

// Port of: src/utils/SkParsePath.cpp#L45-L50 (chrome/m156)
fn skip_ws(s: &[u8], mut i: usize) -> usize {
    while is_ws(at(s, i)) {
        i += 1;
    }
    i
}

// Port of: src/utils/SkParsePath.cpp#L52-L60 (chrome/m156)
fn skip_sep(s: &[u8], i: Option<usize>) -> Option<usize> {
    let mut i = i?;
    while is_sep(at(s, i)) {
        i += 1;
    }
    Some(i)
}

// If unable to read count points from str into value, this will return nullptr
// to signal the failure. Otherwise, it will return the next offset to read from.
// Port of: src/utils/SkParsePath.cpp#L64-L75 (chrome/m156)
#[allow(clippy::chunks_exact_to_as_chunks)] // plain per-element decoding
fn find_points(
    s: &[u8],
    i: Option<usize>,
    value: &mut [Point],
    is_relative: bool,
    relative: Point,
) -> Option<usize> {
    let mut scalars: Vec<scalar> = value.iter().flat_map(|p| [p.x, p.y]).collect();
    let result = i.and_then(|i| parse::find_scalars(s, i, &mut scalars));
    for (p, xy) in value.iter_mut().zip(scalars.chunks_exact(2)) {
        p.x = xy[0];
        p.y = xy[1];
    }
    if is_relative {
        for p in value.iter_mut() {
            p.x += relative.x;
            p.y += relative.y;
        }
    }
    result
}

// If unable to read a scalar from str into value, this will return nullptr
// to signal the failure. Otherwise, it will return the next offset to read from.
// Port of: src/utils/SkParsePath.cpp#L79-L91 (chrome/m156)
fn find_scalar(
    s: &[u8],
    i: Option<usize>,
    value: &mut scalar,
    is_relative: bool,
    relative: scalar,
) -> Option<usize> {
    let (i, v) = parse::find_scalar(s, i?)?;
    *value = v;
    if is_relative {
        *value += relative;
    }
    skip_sep(s, Some(i))
}

// https://www.w3.org/TR/SVG11/paths.html#PathDataBNF
//
// flag:
//    "0" | "1"
// Port of: src/utils/SkParsePath.cpp#L97-L107 (chrome/m156)
fn find_flag(s: &[u8], i: Option<usize>, value: &mut bool) -> Option<usize> {
    let i = i?;
    if at(s, i) != b'1' && at(s, i) != b'0' {
        return None;
    }
    *value = at(s, i) != b'0';
    skip_sep(s, Some(i + 1))
}

/// Parses SVG path data; `None` if it is malformed or truncated.
// Port of: src/utils/SkParsePath.cpp#L109-L247 (chrome/m156)
#[doc(alias = "FromSVGString")]
#[must_use]
#[allow(clippy::too_many_lines)] // mirrors the C++ function
pub fn from_svg(svg: impl AsRef<str>) -> Option<Path> {
    let s = svg.as_ref().as_bytes();
    // We will write all data to this local path and only write it
    // to result if the whole parsing succeeds.
    let mut builder = PathBuilder::new();
    let mut first = Point::new(0.0, 0.0);
    let mut c = Point::new(0.0, 0.0);
    let mut lastc = Point::new(0.0, 0.0);
    // We will use find_points and find_scalar to read into these.
    // There might not be enough data to fill them, so to avoid
    // MSAN warnings about using uninitialized bytes, we initialize
    // them there.
    let mut points = [Point::default(); 3];
    let mut scratch: scalar = 0.0;
    let mut op: u8 = 0;
    let mut previous_op: u8 = 0;
    let mut relative = false;
    let mut data: Option<usize> = Some(0);
    loop {
        let Some(mut d) = data else {
            // Truncated data
            return None;
        };
        d = skip_ws(s, d);
        data = Some(d);
        if at(s, d) == 0 {
            break;
        }
        let ch = at(s, d);
        if is_digit(ch) || ch == b'-' || ch == b'+' || ch == b'.' {
            if op == 0 || op == b'Z' {
                return None;
            }
        } else if is_sep(ch) {
            data = skip_sep(s, data);
        } else {
            op = ch;
            relative = false;
            if is_lower(op) {
                op = to_upper(op);
                relative = true;
            }
            data = skip_sep(s, Some(d + 1));
        }
        match op {
            b'M' => {
                // Move
                data = find_points(s, data, &mut points[..1], relative, c);
                // find_points might have failed, so this might be the
                // previous point. However, data will be set to nullptr
                // if it failed, so we will check this at the top of the loop.
                builder.move_to(points[0]);
                previous_op = 0;
                op = b'L';
                c = points[0];
            }
            b'L' => {
                // Line
                data = find_points(s, data, &mut points[..1], relative, c);
                builder.line_to(points[0]);
                c = points[0];
            }
            b'H' => {
                // Horizontal Line
                data = find_scalar(s, data, &mut scratch, relative, c.x);
                // Similarly, if there wasn't a scalar to read, data will
                // be set to nullptr and this lineTo is bogus but will
                // be ultimately ignored when the next time through the loop
                // detects that and bails out.
                builder.line_to((scratch, c.y));
                c.x = scratch;
            }
            b'V' => {
                // Vertical Line
                data = find_scalar(s, data, &mut scratch, relative, c.y);
                builder.line_to((c.x, scratch));
                c.y = scratch;
            }
            b'C' | b'S' => {
                if op == b'C' {
                    // Cubic Bezier Curve
                    data = find_points(s, data, &mut points[..3], relative, c);
                } else {
                    // Continued "Smooth" Cubic Bezier Curve
                    data = find_points(s, data, &mut points[1..3], relative, c);
                    points[0] = c;
                    if previous_op == b'C' || previous_op == b'S' {
                        points[0].x -= lastc.x - c.x;
                        points[0].y -= lastc.y - c.y;
                    }
                }
                // cubicCommon:
                builder.cubic_to(points[0], points[1], points[2]);
                lastc = points[1];
                c = points[2];
            }
            b'Q' | b'T' => {
                if op == b'Q' {
                    // Quadratic Bezier Curve
                    data = find_points(s, data, &mut points[..2], relative, c);
                } else {
                    // Continued Quadratic Bezier Curve
                    data = find_points(s, data, &mut points[1..2], relative, c);
                    points[0] = c;
                    if previous_op == b'Q' || previous_op == b'T' {
                        points[0].x -= lastc.x - c.x;
                        points[0].y -= lastc.y - c.y;
                    }
                }
                // quadraticCommon:
                builder.quad_to(points[0], points[1]);
                lastc = points[0];
                c = points[1];
            }
            b'A' => {
                // Arc (Elliptical)
                let mut radii = [Point::default(); 1];
                let mut angle: scalar = 0.0;
                let mut large_arc = false;
                let mut sweep = false;
                data = find_points(s, data, &mut radii, false, Point::default());
                if data.is_some() {
                    data = skip_sep(s, data);
                }
                if data.is_some() {
                    data = find_scalar(s, data, &mut angle, false, 0.0);
                }
                if data.is_some() {
                    data = skip_sep(s, data);
                }
                if data.is_some() {
                    data = find_flag(s, data, &mut large_arc);
                }
                if data.is_some() {
                    data = skip_sep(s, data);
                }
                if data.is_some() {
                    data = find_flag(s, data, &mut sweep);
                }
                if data.is_some() {
                    data = skip_sep(s, data);
                }
                if data.is_some() {
                    data = find_points(s, data, &mut points[..1], relative, c);
                }
                if data.is_some() {
                    builder.arc_to_radius(
                        radii[0],
                        angle,
                        if large_arc {
                            ArcSize::Large
                        } else {
                            ArcSize::Small
                        },
                        if sweep {
                            PathDirection::CW
                        } else {
                            PathDirection::CCW
                        },
                        points[0],
                    );
                    c = builder.points()[builder.points().len() - 1];
                }
            }
            b'Z' => {
                // Close Path
                builder.close();
                c = first;
            }
            _ => return None,
        }
        if previous_op == 0 {
            first = c;
        }
        previous_op = op;
    }

    Some(builder.detach())
}

/// The path as SVG path data, with absolute commands.
#[doc(alias = "ToSVGString")]
#[must_use]
pub fn to_svg(path: &Path) -> String {
    to_svg_with_encoding(path, PathEncoding::Absolute)
}

/// The path as SVG path data. Conics are converted to quads.
// Port of: src/utils/SkParsePath.cpp#L251-L306 (chrome/m156)
#[doc(alias = "ToSVGString")]
#[must_use]
pub fn to_svg_with_encoding(path: &Path, encoding: PathEncoding) -> String {
    let mut stream = String::new();

    let mut current_point = Point::new(0.0, 0.0);
    let rel_selector = encoding == PathEncoding::Relative;

    let mut append_command = |stream: &mut String, cmd: u8, pts: &[Point]| {
        // Use lower case cmds for relative encoding.
        let cmd = cmd + 32 * u8::from(rel_selector);
        stream.push(char::from(cmd));

        for (i, &p) in pts.iter().enumerate() {
            let pt = p - current_point;
            if i > 0 {
                stream.push(' ');
            }
            str_append_scalar(stream, pt.x);
            stream.push(' ');
            str_append_scalar(stream, pt.y);
        }

        debug_assert_ne!(pts, []);
        // For relative encoding, track the current point (otherwise == origin).
        current_point = pts[pts.len() - 1] * if rel_selector { 1.0 } else { 0.0 };
    };

    let mut iter = Iter::new(path, false);

    while let Some(rec) = iter.next_rec() {
        let pts = rec.points();
        match rec.verb() {
            PathVerb::Conic => {
                let tol = SCALAR_1 / 1024.0; // how close to a quad
                let mut quadder = AutoConicToQuads::new();
                let quad_pts = quadder
                    .compute_quads_with_weight(pts, rec.conic_weight(), tol)
                    .to_vec();
                for i in 0..quadder.count_quads() {
                    append_command(&mut stream, b'Q', &quad_pts[i * 2 + 1..i * 2 + 3]);
                }
            }
            PathVerb::Move => append_command(&mut stream, b'M', &pts[0..1]),
            PathVerb::Line => append_command(&mut stream, b'L', &pts[1..2]),
            PathVerb::Quad => append_command(&mut stream, b'Q', &pts[1..3]),
            PathVerb::Cubic => append_command(&mut stream, b'C', &pts[1..4]),
            PathVerb::Close => stream.push('Z'),
        }
    }

    stream
}

impl Path {
    /// Parses SVG path data (`SkParsePath::FromSVGString`).
    #[must_use]
    pub fn from_svg(svg: impl AsRef<str>) -> Option<Path> {
        from_svg(svg)
    }

    /// The path as SVG path data (`SkParsePath::ToSVGString`).
    #[must_use]
    pub fn to_svg(&self) -> String {
        to_svg(self)
    }

    /// The path as SVG path data with the given encoding.
    #[must_use]
    pub fn to_svg_with_encoding(&self, encoding: PathEncoding) -> String {
        to_svg_with_encoding(self, encoding)
    }
}
