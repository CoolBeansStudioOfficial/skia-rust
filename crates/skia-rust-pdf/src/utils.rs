// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFUtils.{h,cpp} (chrome/m156)

//! `SkPDFUtils`: the content-stream operators, the number and colour formatters, `EmitPath` and
//! `GetDateTime`.
//!
//! The functions that need a live device, shader, image or resource dictionary (`ApplyPattern`,
//! `PopulateTilingPatternDict`, `ToBitmap`, `GetShaderLocalMatrix`, `MatrixToArray`,
//! `RectToArray`) come with the PDF device (`skia-rust-pdf` M25).

use std::time::{SystemTime, UNIX_EPOCH};

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::image::Image;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::shader::Shader;
use skia_rust_core::fixed::{FIXED_1, fixed_round_to_int};
use skia_rust_core::floating_point::float_round2int;
use skia_rust_core::geometry::AutoConicToQuads;
use skia_rust_core::paint::Style as PaintStyle;
use skia_rust_core::path::Path;
use skia_rust_core::path_priv::all_points_eq;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::path_types::{PathDirection, PathVerb};
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::stream::{DynamicMemoryWStream, WStream};
use skia_rust_core::utf::to_utf16;

use crate::date_time::DateTime;
use crate::resource_dict::{ResourceType, write_resource_name};
use crate::types::{PdfArray, PdfDict};
use crate::float_to_decimal::{MAXIMUM_SK_FLOAT_TO_DECIMAL_LENGTH, float_to_decimal};
use crate::types::HEX_DIGITS_UPPER;

/// `SkScalarNearlyZero` tolerance for collinearity: `1 / 2^24`. `SkScalarNearlyZero`'s default
/// epsilon is too coarse for some of the GMs that stress large scales.
const COLLINEAR_EPS: f32 = 1.0 / 16_777_216.0;

/// `SkPDFUtils::EmptyPath`: whether an empty path is drawn as an empty rectangle.
// Port of: src/pdf/SkPDFUtils.h#L99 (chrome/m156)
#[doc(alias = "SkPDFUtils::EmptyPath")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmptyPath {
    /// Emit nothing.
    Discard,
    /// Emit an empty rectangle at the origin.
    Preserve,
}

/// `SkPDFUtils::EmptyVerb`: whether verbs that add no geometry are kept.
// Port of: src/pdf/SkPDFUtils.h#L100 (chrome/m156)
#[doc(alias = "SkPDFUtils::EmptyVerb")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmptyVerb {
    /// Drop verbs whose points are all equal.
    Discard,
    /// Keep them.
    Preserve,
}

/// `SkPDFUtils::EmptyArea`: whether contours with no area are kept.
// Port of: src/pdf/SkPDFUtils.h#L101 (chrome/m156)
#[doc(alias = "SkPDFUtils::EmptyArea")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmptyArea {
    /// Drop contours whose points are all collinear.
    Discard,
    /// Keep them.
    Preserve,
}

/// `SkPDFUtils::BlendModeName`: the PDF name of a blend mode, or `None` for the modes that the
/// device sets up itself.
// Port of: src/pdf/SkPDFUtils.cpp#L39-L63 (chrome/m156)
#[doc(alias = "SkPDFUtils::BlendModeName")]
#[must_use]
pub fn blend_mode_name(mode: BlendMode) -> Option<&'static str> {
    // PDF32000.book section 11.3.5 "Blend Mode"
    match mode {
        // `Xor` and `Plus` are unsupported by PDF and fall back to Normal.
        BlendMode::SrcOver | BlendMode::Xor | BlendMode::Plus => Some("Normal"),
        BlendMode::Screen => Some("Screen"),
        BlendMode::Overlay => Some("Overlay"),
        BlendMode::Darken => Some("Darken"),
        BlendMode::Lighten => Some("Lighten"),
        BlendMode::ColorDodge => Some("ColorDodge"),
        BlendMode::ColorBurn => Some("ColorBurn"),
        BlendMode::HardLight => Some("HardLight"),
        BlendMode::SoftLight => Some("SoftLight"),
        BlendMode::Difference => Some("Difference"),
        BlendMode::Exclusion => Some("Exclusion"),
        BlendMode::Multiply => Some("Multiply"),
        BlendMode::Hue => Some("Hue"),
        BlendMode::Saturation => Some("Saturation"),
        BlendMode::Color => Some("Color"),
        BlendMode::Luminosity => Some("Luminosity"),
        // Other blend modes are handled in SkPDFDevice::setUpContentEntry.
        _ => None,
    }
}

/// `SkPDFUtils::AppendScalar`: a scalar as a PDF number.
// Port of: src/pdf/SkPDFUtils.h#L162-L168 (chrome/m156)
#[doc(alias = "SkPDFUtils::AppendScalar")]
pub fn append_scalar(value: f32, stream: &mut dyn WStream) {
    let mut result = [0u8; MAXIMUM_SK_FLOAT_TO_DECIMAL_LENGTH];
    let len = float_to_decimal(value, &mut result);
    debug_assert!(len < MAXIMUM_SK_FLOAT_TO_DECIMAL_LENGTH);
    stream.write(&result[..len]);
}

/// `SkPDFUtils::WriteUInt16BE`: four uppercase hex digits.
// Port of: src/pdf/SkPDFUtils.h#L170-L177 (chrome/m156)
pub fn write_uint16_be(stream: &mut dyn WStream, value: u16) {
    let result = [
        HEX_DIGITS_UPPER[usize::from(value >> 12)],
        HEX_DIGITS_UPPER[usize::from((value >> 8) & 0xF)],
        HEX_DIGITS_UPPER[usize::from((value >> 4) & 0xF)],
        HEX_DIGITS_UPPER[usize::from(value & 0xF)],
    ];
    stream.write(&result);
}

/// `SkPDFUtils::WriteUInt8`: two uppercase hex digits.
// Port of: src/pdf/SkPDFUtils.h#L179-L184 (chrome/m156)
pub fn write_uint8(stream: &mut dyn WStream, value: u8) {
    let result = [
        HEX_DIGITS_UPPER[usize::from(value >> 4)],
        HEX_DIGITS_UPPER[usize::from(value & 0xF)],
    ];
    stream.write(&result);
}

/// `SkPDFUtils::WriteUTF16beHex`: a code point as one or two UTF-16BE code units.
// Port of: src/pdf/SkPDFUtils.h#L186-L194 (chrome/m156)
pub fn write_utf16be_hex(stream: &mut dyn WStream, utf32: i32) {
    let mut utf16 = [0u16; 2];
    let len = to_utf16(utf32, Some(&mut utf16));
    debug_assert!(len == 1 || len == 2);
    write_uint16_be(stream, utf16[0]);
    if len == 2 {
        write_uint16_be(stream, utf16[1]);
    }
}

/// `SkPDFUtils::MoveTo`.
// Port of: src/pdf/SkPDFUtils.cpp#L77-L82 (chrome/m156)
pub fn move_to(x: f32, y: f32, content: &mut dyn WStream) {
    append_scalar(x, content);
    content.write_text(" ");
    append_scalar(y, content);
    content.write_text(" m\n");
}

/// `SkPDFUtils::AppendLine`.
// Port of: src/pdf/SkPDFUtils.cpp#L84-L89 (chrome/m156)
pub fn append_line(x: f32, y: f32, content: &mut dyn WStream) {
    append_scalar(x, content);
    content.write_text(" ");
    append_scalar(y, content);
    content.write_text(" l\n");
}

/// The `c` (or `y`, when the second control point is the end point) operator.
// Port of: src/pdf/SkPDFUtils.cpp#L91-L111 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparison, as in Skia
fn append_cubic(ctl1: Point, ctl2: Point, dst: Point, content: &mut dyn WStream) {
    let mut cmd = "y\n";
    append_scalar(ctl1.x, content);
    content.write_text(" ");
    append_scalar(ctl1.y, content);
    content.write_text(" ");
    // Exact comparison, as in Skia: the second control point is dropped only when it equals the
    // end point.
    if ctl2.x != dst.x || ctl2.y != dst.y {
        cmd = "c\n";
        append_scalar(ctl2.x, content);
        content.write_text(" ");
        append_scalar(ctl2.y, content);
        content.write_text(" ");
    }
    append_scalar(dst.x, content);
    content.write_text(" ");
    append_scalar(dst.y, content);
    content.write_text(" ");
    content.write_text(cmd);
}

/// `append_quad`: a quadratic as a cubic.
// Port of: src/pdf/SkPDFUtils.cpp#L113-L118 (chrome/m156)
fn append_quad(quad: &[Point], content: &mut dyn WStream) {
    let mut cubic = [Point::default(); 4];
    skia_rust_core::geometry::convert_quad_to_cubic(quad, &mut cubic);
    append_cubic(cubic[1], cubic[2], cubic[3], content);
}

/// `SkPDFUtils::AppendRectangle`. Skia has (0, 0) at the top left, PDF at the bottom left.
// Port of: src/pdf/SkPDFUtils.cpp#L120-L135 (chrome/m156)
pub fn append_rectangle(rect: &Rect, content: &mut dyn WStream) {
    let bottom = rect.bottom.min(rect.top);
    append_scalar(rect.left, content);
    content.write_text(" ");
    append_scalar(bottom, content);
    content.write_text(" ");
    append_scalar(rect.right - rect.left, content);
    content.write_text(" ");
    append_scalar(rect.bottom - rect.top, content);
    content.write_text(" re\n");
}

/// Collects one contour, and drops it when it has no area and that is requested.
///
/// The check is per contour, so a zero-area extrusion inside a larger contour is kept.
// Port of: src/pdf/SkPDFUtils.cpp#L136-L266 (chrome/m156)
struct ContourBuffer<'a> {
    content_stream: &'a mut dyn WStream,
    empty_area: EmptyArea,
    buffer: DynamicMemoryWStream,
    current_line: Option<Line>,
    all_collinear: bool,
    wrote_content: bool,
}

#[derive(Clone, Copy)]
struct Line {
    orig: Point,
    vec: Vector,
}

impl<'a> ContourBuffer<'a> {
    fn new(content_stream: &'a mut dyn WStream, empty_area: EmptyArea) -> Self {
        Self {
            content_stream,
            empty_area,
            buffer: DynamicMemoryWStream::new(),
            current_line: None,
            all_collinear: true,
            wrote_content: false,
        }
    }

    fn append_move(&mut self, pts: &[Point]) {
        debug_assert_eq!(pts.len(), 1);
        self.flush_contour();
        move_to(pts[0].x, pts[0].y, &mut self.buffer);
    }

    fn append_line(&mut self, pts: &[Point]) {
        debug_assert_eq!(pts.len(), 2);
        self.update_state(pts);
        append_line(pts[1].x, pts[1].y, &mut self.buffer);
    }

    fn append_quad(&mut self, pts: &[Point]) {
        debug_assert_eq!(pts.len(), 3);
        self.update_state(pts);
        append_quad(pts, &mut self.buffer);
    }

    fn append_cubic(&mut self, pts: &[Point]) {
        debug_assert_eq!(pts.len(), 4);
        self.update_state(pts);
        append_cubic(pts[1], pts[2], pts[3], &mut self.buffer);
    }

    fn append_close(&mut self) {
        close_path(&mut self.buffer);
        self.flush_contour();
    }

    fn flush_contour(&mut self) {
        let discard = self.empty_area == EmptyArea::Discard && self.all_collinear;
        if !discard {
            self.buffer.write_to_stream(&mut *self.content_stream);
            self.wrote_content |= self.buffer.bytes_written() > 0;
        }
        self.buffer.reset();
        self.current_line = None;
        self.all_collinear = true;
    }

    fn wrote_content(&self) -> bool {
        self.wrote_content
    }

    /// Picks a line determined by two distinct points, or `None` if all points coincide.
    /// Only collinearity matters, so any two distinct points will do.
    // Port of: src/pdf/SkPDFUtils.cpp#L163-L173 (chrome/m156)
    fn select_line(pts: &[Point]) -> Option<Line> {
        let p0 = pts[0];
        pts[1..].iter().rev().find(|&&p| p != p0).map(|&p| Line {
            orig: p0,
            vec: p - p0,
        })
    }

    // Port of: src/pdf/SkPDFUtils.cpp#L175-L200 (chrome/m156)
    fn update_state(&mut self, pts: &[Point]) {
        debug_assert!(pts.len() >= 2);
        if self.empty_area == EmptyArea::Preserve || !self.all_collinear {
            return;
        }
        if self.current_line.is_none() {
            self.current_line = Self::select_line(pts);
            if self.current_line.is_none() {
                // All points are coincident.
                return;
            }
        }
        let Some(line) = self.current_line else {
            return;
        };
        // We receive a starting point for all verbs, which coincides with the last point of the
        // previous verb. It can be ignored for collinearity tests.
        for &pt in &pts[1..] {
            let d = pt - line.orig;
            let cross = line.vec.x * d.y - line.vec.y * d.x;
            // `!SkScalarNearlyZero(cross, eps)`: written so that NaN counts as not collinear.
            #[allow(clippy::neg_cmp_op_on_partial_ord)] // mirrors the C++ `!(x <= tol)`
            if !(cross.abs() <= COLLINEAR_EPS) {
                self.all_collinear = false;
                break;
            }
        }
    }
}

/// `SkPDFUtils::ClosePath`.
// Port of: src/pdf/SkPDFUtils.cpp#L339-L341 (chrome/m156)
pub fn close_path(content: &mut dyn WStream) {
    content.write_text("h\n");
}

/// `SkPDFUtils::EmitPath`: writes `path` as content-stream operators. Returns whether anything
/// was written.
// Port of: src/pdf/SkPDFUtils.cpp#L268-L337 (chrome/m156)
#[doc(alias = "SkPDFUtils::EmitPath")]
#[must_use]
pub fn emit_path(
    path: &Path,
    empty_path: EmptyPath,
    empty_verb: EmptyVerb,
    empty_area: EmptyArea,
    content: &mut dyn WStream,
    tolerance: f32,
) -> bool {
    if path.is_empty() {
        if empty_path == EmptyPath::Preserve {
            append_rectangle(&Rect::new(0.0, 0.0, 0.0, 0.0), content);
            return true;
        }
        return false;
    }

    // Both closure and direction need to be checked.
    if let Some((rect, is_closed, direction)) = path.is_rect()
        && is_closed
        && (direction == PathDirection::CW || path.fill_type() == PathFillType::EvenOdd)
    {
        append_rectangle(&rect, content);
        return true;
    }

    // Filling a path with no area results in a drawing in PDF renderers, but Chrome expects to be
    // able to draw some such entities with no visible result, so those drawings are discarded.
    let mut cbuffer = ContourBuffer::new(content, empty_area);
    let preserve_empty_verbs = empty_verb == EmptyVerb::Preserve;
    // `SkPath::Iter iter(path, false)`: the legacy iterator, which turns the close of an open
    // contour into a line back to its start, followed by the close.
    let mut iter = skia_rust_core::path::Iter::new(path, false);
    while let Some(rec) = iter.next_rec() {
        // `args` gets all the points, even the implicit first point.
        let args = rec.points();
        match rec.verb() {
            PathVerb::Move => cbuffer.append_move(args),
            PathVerb::Line => {
                if preserve_empty_verbs || !all_points_eq(args) {
                    cbuffer.append_line(args);
                }
            }
            PathVerb::Quad => {
                if preserve_empty_verbs || !all_points_eq(args) {
                    cbuffer.append_quad(args);
                }
            }
            PathVerb::Conic => {
                if preserve_empty_verbs || !all_points_eq(args) {
                    let mut converter = AutoConicToQuads::new();
                    let quads = converter
                        .compute_quads_with_weight(args, rec.conic_weight(), tolerance)
                        .to_vec();
                    let count = converter.count_quads();
                    for i in 0..count {
                        cbuffer.append_quad(&quads[i * 2..i * 2 + 3]);
                    }
                }
            }
            PathVerb::Cubic => {
                if preserve_empty_verbs || !all_points_eq(args) {
                    cbuffer.append_cubic(args);
                }
            }
            PathVerb::Close => cbuffer.append_close(),
        }
    }
    cbuffer.flush_contour();
    cbuffer.wrote_content()
}

/// `SkPDFUtils::StrokePath` and friends: the paint operator for `style` and `fill`.
// Port of: src/pdf/SkPDFUtils.cpp#L343-L360 (chrome/m156)
pub fn paint_path(style: PaintStyle, fill: PathFillType, content: &mut dyn WStream) {
    if style == PaintStyle::Fill {
        content.write_text("f");
    } else if style == PaintStyle::StrokeAndFill {
        content.write_text("B");
    } else if style == PaintStyle::Stroke {
        content.write_text("S");
    }
    if style != PaintStyle::Stroke && fill == PathFillType::EvenOdd {
        content.write_text("*");
    }
    content.write_text("\n");
}

/// `SkPDFUtils::StrokePath`.
// Port of: src/pdf/SkPDFUtils.cpp#L362-L364 (chrome/m156)
pub fn stroke_path(content: &mut dyn WStream) {
    paint_path(PaintStyle::Stroke, PathFillType::Winding, content);
}

/// `print_permil_as_decimal`: `x / 10^places`, given `0 < x < 10^places`. `result` holds
/// `places + 2` bytes. Returns the length, with a NUL after it.
// Port of: src/pdf/SkPDFUtils.cpp#L383-L398 (chrome/m156)
fn print_permil_as_decimal(mut x: i32, result: &mut [u8], places: usize) -> usize {
    result[0] = b'.';
    for i in (1..=places).rev() {
        result[i] = b'0' + u8::try_from(x % 10).unwrap_or(0);
        x /= 10;
    }
    let mut j = places;
    while j > 1 {
        if result[j] != b'0' {
            break;
        }
        j -= 1;
    }
    result[j + 1] = 0;
    j + 1
}

/// `SkPDFUtils::ColorToDecimalF`: a colour in `0..=1` with four decimal places.
// Port of: src/pdf/SkPDFUtils.cpp#L408-L417 (chrome/m156)
pub fn color_to_decimal_f(value: f32, result: &mut [u8; 6]) -> usize {
    const K_FACTOR: i32 = 10_000; // int_pow(10, kFloatColorDecimalCount)
    const K_FACTOR_F: f32 = 10_000.0; // the same, as the float that `value * kFactor` multiplies by
    let x = float_round2int(value * K_FACTOR_F);
    if x >= K_FACTOR || x <= 0 {
        // clamp to 0-1
        result[0] = if x > 0 { b'1' } else { b'0' };
        result[1] = 0;
        return 1;
    }
    print_permil_as_decimal(x, result, 4)
}

/// `SkPDFUtils::ColorToDecimal`: a colour byte as a decimal in `0..=1`, with three decimal
/// places.
// Port of: src/pdf/SkPDFUtils.cpp#L419-L428 (chrome/m156)
pub fn color_to_decimal(value: u8, result: &mut [u8; 5]) -> usize {
    if value == 255 || value == 0 {
        result[0] = if value != 0 { b'1' } else { b'0' };
        result[1] = 0;
        return 1;
    }
    // int x = 0.5 + (1000.0 / 255.0) * value;
    let x = fixed_round_to_int((FIXED_1 * 1000 / 255) * i32::from(value));
    print_permil_as_decimal(x, result, 3)
}

/// `SkPDFUtils::AppendColorComponent`.
// Port of: src/pdf/SkPDFUtils.h#L131-L136 (chrome/m156)
pub fn append_color_component(value: u8, stream: &mut dyn WStream) {
    let mut buffer = [0u8; 5];
    let len = color_to_decimal(value, &mut buffer);
    stream.write(&buffer[..len]);
}

/// `SkPDFUtils::AppendColorComponentF`.
// Port of: src/pdf/SkPDFUtils.h#L138-L143 (chrome/m156)
pub fn append_color_component_f(value: f32, stream: &mut dyn WStream) {
    let mut buffer = [0u8; 6];
    let len = color_to_decimal_f(value, &mut buffer);
    stream.write(&buffer[..len]);
}

/// `SkPDFUtils::GetDateTime`: the current UTC time.
///
/// Skia uses `gmtime_r` on Unix and `GetSystemTime` on Windows. Both give UTC, so the civil
/// date is computed from the Unix time, and `time_zone_minutes` is 0.
// Port of: src/pdf/SkPDFUtils.cpp#L491-L506 (chrome/m156)
#[doc(alias = "SkPDFUtils::GetDateTime")]
#[must_use]
pub fn get_date_time() -> DateTime {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    date_time_from_unix_seconds(secs)
}

/// `gmtime_r` for a Unix time in seconds, as the `SkPDF::DateTime` fields.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // every field is in range
fn date_time_from_unix_seconds(secs: i64) -> DateTime {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    // 1970-01-01 was a Thursday (4).
    let day_of_week = (days + 4).rem_euclid(7);
    DateTime {
        time_zone_minutes: 0,
        year: year as u16,
        month: month as u8,
        day_of_week: day_of_week as u8,
        day: day as u8,
        hour: (rem / 3600) as u8,
        minute: (rem % 3600 / 60) as u8,
        second: (rem % 60) as u8,
    }
}

/// Days since 1970-01-01 to (year, month, day), in the proleptic Gregorian calendar.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // ranges are small
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `SkPDFMakeArray` for scalars: an array of the given scalars.
// Port of: src/pdf/SkPDFTypes.h#L194-L209 (SkPDFMakeArray, chrome/m156)
#[must_use]
pub fn make_scalar_array(values: &[f32]) -> PdfArray {
    let mut array = PdfArray::new();
    array.reserve(values.len());
    for &v in values {
        array.append_scalar(v);
    }
    array
}

/// `SkPDFMakeArray` for ints: an array of the given integers.
// Port of: src/pdf/SkPDFTypes.h#L194-L209 (SkPDFMakeArray, chrome/m156)
#[must_use]
pub fn make_int_array(values: &[i32]) -> PdfArray {
    let mut array = PdfArray::new();
    array.reserve(values.len());
    for &v in values {
        array.append_int(v);
    }
    array
}

/// `SkPDFUtils::RectToArray`.
// Port of: src/pdf/SkPDFUtils.cpp#L65-L67 (chrome/m156)
#[doc(alias = "SkPDFUtils::RectToArray")]
#[must_use]
pub fn rect_to_array(r: &Rect) -> PdfArray {
    make_scalar_array(&[r.left, r.top, r.right, r.bottom])
}

/// `SkPDFUtils::MatrixToArray`.
// Port of: src/pdf/SkPDFUtils.cpp#L69-L75 (chrome/m156)
#[doc(alias = "SkPDFUtils::MatrixToArray")]
#[must_use]
pub fn matrix_to_array(matrix: &Matrix) -> PdfArray {
    let a = matrix.to_affine().unwrap_or_else(|| {
        let mut a = [0.0; 6];
        Matrix::set_affine_identity(&mut a);
        a
    });
    make_scalar_array(&a)
}

/// `SkPDFUtils::ApplyGraphicState`.
// Port of: src/pdf/SkPDFUtils.cpp#L366-L369 (chrome/m156)
#[doc(alias = "SkPDFUtils::ApplyGraphicState")]
pub fn apply_graphic_state(object_index: i32, content: &mut dyn WStream) {
    write_resource_name(content, ResourceType::ExtGState, object_index);
    content.write_text(" gs\n");
}

/// `SkPDFUtils::ApplyPattern`.
// Port of: src/pdf/SkPDFUtils.cpp#L371-L381 (chrome/m156)
#[doc(alias = "SkPDFUtils::ApplyPattern")]
pub fn apply_pattern(object_index: i32, content: &mut dyn WStream) {
    // Select Pattern color space (CS, cs) and set pattern object as current
    // color (SCN, scn)
    content.write_text("/Pattern CS/Pattern cs");
    write_resource_name(content, ResourceType::Pattern, object_index);
    content.write_text(" SCN");
    write_resource_name(content, ResourceType::Pattern, object_index);
    content.write_text(" scn\n");
}

/// `SkPDFUtils::GetShaderLocalMatrix`: the local matrix of a local-matrix shader, else identity.
// Port of: src/pdf/SkPDFUtils.h#L111-L117 (chrome/m156)
#[doc(alias = "SkPDFUtils::GetShaderLocalMatrix")]
#[must_use]
pub fn get_shader_local_matrix(shader: &Shader) -> Matrix {
    if let Some((_, local_matrix)) = shader.as_base().make_as_a_local_matrix_shader() {
        return local_matrix;
    }
    Matrix::new_identity()
}

/// `SkPDFUtils::InverseTransformBBox`: maps `bbox` through the inverse of `matrix`.
// Port of: src/pdf/SkPDFUtils.cpp#L430-L436 (chrome/m156)
#[doc(alias = "SkPDFUtils::InverseTransformBBox")]
pub fn inverse_transform_bbox(matrix: &Matrix, bbox: &mut Rect) -> bool {
    if let Some(inverse) = matrix.invert() {
        *bbox = inverse.map_rect(*bbox).0;
        return true;
    }
    false
}

/// `SkPDFUtils::PopulateTilingPatternDict`.
// Port of: src/pdf/SkPDFUtils.cpp#L438-L460 (chrome/m156)
#[doc(alias = "SkPDFUtils::PopulateTilingPatternDict")]
pub fn populate_tiling_pattern_dict(
    pattern: &mut PdfDict,
    bbox: &Rect,
    tile_x: bool,
    tile_y: bool,
    resources: PdfDict,
    matrix: &Matrix,
) {
    const TILING_PATTERN_TYPE: i32 = 1;
    const COLORED_TILING_PATTERN_PAINT_TYPE: i32 = 1;
    const CONSTANT_SPACING_TILING_TYPE: i32 = 1;

    pattern.insert_name("Type", "Pattern");
    pattern.insert_int("PatternType", TILING_PATTERN_TYPE);
    pattern.insert_int("PaintType", COLORED_TILING_PATTERN_PAINT_TYPE);
    pattern.insert_int("TilingType", CONSTANT_SPACING_TILING_TYPE);
    pattern.insert_object("BBox", Box::new(rect_to_array(bbox)));
    // PDF tiling is a raster operation which may involve pixel snapping XStep and YStep values.
    // Add space between "tiles" if not tiling in the given direction. https://crbug.com/41496385
    pattern.insert_scalar("XStep", bbox.width() + if tile_x { 0.0 } else { 2.0 });
    pattern.insert_scalar("YStep", bbox.height() + if tile_y { 0.0 } else { 2.0 });
    pattern.insert_object("Resources", Box::new(resources));
    if !matrix.is_identity() {
        pattern.insert_object("Matrix", Box::new(matrix_to_array(matrix)));
    }
}

/// `SkPDFUtils::ToBitmap`: the read-only pixels of an image.
// Port of: src/pdf/SkPDFUtils.cpp#L462-L474 (chrome/m156)
#[doc(alias = "SkPDFUtils::ToBitmap")]
#[must_use]
pub fn to_bitmap(img: &Image) -> Option<Bitmap> {
    // TODO: support GPU images
    let bitmap = img.get_ro_pixels()?;
    debug_assert_eq!(bitmap.dimensions(), img.dimensions());
    debug_assert!(!bitmap.draws_nothing());
    Some(bitmap)
}

/// `SkPDFUtils::AppendTransform`: the `cm` operator.
// Port of: src/pdf/SkPDFUtils.cpp#L476-L489 (chrome/m156)
#[doc(alias = "SkPDFUtils::AppendTransform")]
pub fn append_transform(matrix: &Matrix, content: &mut dyn WStream) {
    let values = matrix.to_affine().unwrap_or_else(|| {
        let mut a = [0.0; 6];
        Matrix::set_affine_identity(&mut a);
        a
    });
    for v in values {
        append_scalar(v, content);
        content.write_text(" ");
    }
    content.write_text("cm\n");
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(civil_from_days(10_957), (2000, 1, 1));
    }
}
