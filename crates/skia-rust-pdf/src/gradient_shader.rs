// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFGradientShader.{h,cpp} (chrome/m156)

//! `SkPDFGradientShader`: a gradient as a PDF pattern. Clamp and decal gradients without
//! perspective are an axial or radial shading with a stitching function; the others are a
//! shading of type function with the gradient written as a PostScript calculator function. A
//! gradient with transparent stops is a tiling pattern that draws the colors with a soft mask of
//! the alphas.

use std::hash::{Hash, Hasher};

use skia_rust_core::color::Color4f;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Style as PaintStyle;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::scalar::{Scalar, scalar_invert};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::shader_base::{GradientInfo, GradientType};
use skia_rust_core::stream::{DynamicMemoryWStream, WStream};
use skia_rust_core::tile_mode::TileMode;

use crate::document::DocHandle;
use crate::form_xobject::make_form_x_object;
use crate::graphic_state::{SMaskMode, get_smask_graphic_state};
use crate::resource_dict::make_resource_dict;
use crate::types::{PdfArray, PdfDict, PdfIndirectReference, PdfParentTreeKey};
use crate::utils::{
    append_color_component_f, append_rectangle, append_scalar, apply_graphic_state, apply_pattern,
    get_shader_local_matrix, inverse_transform_bbox, make_int_array, make_scalar_array,
    matrix_to_array, paint_path, populate_tiling_pattern_dict, rect_to_array,
};

/// `SkShaderBase::GradientInfo` with owned colors and offsets.
#[derive(Debug, Clone)]
pub struct GradientData {
    /// `fColors`.
    pub colors: Vec<Color4f>,
    /// `fColorOffsets`.
    pub color_offsets: Vec<f32>,
    /// `fPoint`.
    pub point: [Point; 2],
    /// `fRadius`.
    pub radius: [f32; 2],
    /// `fTileMode`.
    pub tile_mode: TileMode,
    /// `fPremulInterp`.
    pub premul_interp: bool,
}

/// `SkPDFGradientShader::Key`. Two keys are equal when the bits of their parts are.
// Port of: src/pdf/SkPDFGradientShader.h#L26-L35 (chrome/m156)
#[doc(alias = "SkPDFGradientShader::Key")]
#[derive(Debug, Clone)]
pub struct GradientKey {
    /// `fType`.
    pub gradient_type: GradientType,
    /// `fInfo`.
    pub info: GradientData,
    /// `fCanvasTransform`.
    pub canvas_transform: Matrix,
    /// `fShaderTransform`.
    pub shader_transform: Matrix,
    /// `fBBox`.
    pub bbox: IRect,
    /// The bits of all of the above, for equality and the hash.
    signature: Vec<u32>,
}

fn matrix_bits(m: &Matrix, out: &mut Vec<u32>) {
    for i in 0..9usize {
        out.push(m.get(i).to_bits());
    }
}

impl GradientKey {
    // Port of: src/pdf/SkPDFGradientShader.h#L37-L61 (operator==, chrome/m156)
    fn new(
        gradient_type: GradientType,
        info: GradientData,
        canvas_transform: Matrix,
        shader_transform: Matrix,
        bbox: IRect,
    ) -> Self {
        let mut signature = vec![
            gradient_type as u32,
            u32::try_from(info.colors.len()).expect("fits"),
        ];
        for c in &info.colors {
            signature.extend([c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()]);
        }
        for o in &info.color_offsets {
            signature.push(o.to_bits());
        }
        for p in &info.point {
            signature.extend([p.x.to_bits(), p.y.to_bits()]);
        }
        for r in &info.radius {
            signature.push(r.to_bits());
        }
        signature.push(info.tile_mode as u32);
        signature.push(u32::from(info.premul_interp));
        matrix_bits(&canvas_transform, &mut signature);
        matrix_bits(&shader_transform, &mut signature);
        signature.extend([
            bbox.left.cast_unsigned(),
            bbox.top.cast_unsigned(),
            bbox.right.cast_unsigned(),
            bbox.bottom.cast_unsigned(),
        ]);
        Self {
            gradient_type,
            info,
            canvas_transform,
            shader_transform,
            bbox,
            signature,
        }
    }

    /// The key with `info` replaced (`clone_key` and then changing the colors).
    fn with_info(&self, info: GradientData) -> Self {
        Self::new(
            self.gradient_type,
            info,
            self.canvas_transform.clone(),
            self.shader_transform.clone(),
            self.bbox,
        )
    }
}

impl PartialEq for GradientKey {
    fn eq(&self, other: &Self) -> bool {
        self.signature == other.signature
    }
}

impl Eq for GradientKey {}

impl Hash for GradientKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.signature.hash(state);
    }
}

// Port of: src/pdf/SkPDFGradientShader.cpp#L44-L53 (unit_to_points_matrix, chrome/m156)
fn unit_to_points_matrix(pts: &[Point; 2]) -> Matrix {
    let mut vec = Point::new(pts[1].x - pts[0].x, pts[1].y - pts[0].y);
    let mag = vec.length();
    let inv = if mag != 0.0 { scalar_invert(mag) } else { 0.0 };

    vec.scale(inv);
    let mut matrix = Matrix::new_identity();
    matrix.set_sin_cos((vec.y, vec.x), None);
    matrix.pre_scale((mag, mag), None);
    matrix.post_translate((pts[0].x, pts[0].y));
    matrix
}

/// Assumes t - startOffset is on the stack and does a linear interpolation on t between
/// startOffset and endOffset from prevColor to curColor (for each color component), leaving the
/// result in component order on the stack.
///
/// `range` is endOffset - startOffset, `num_components` the number of components (3, or 4 if
/// alpha is needed).
// Port of: src/pdf/SkPDFGradientShader.cpp#L61-L125 (interpolate_color_code, chrome/m156)
#[allow(clippy::float_cmp)] // exact comparisons, as in Skia
fn interpolate_color_code(
    range: f32,
    num_components: usize,
    prev_color: &Color4f,
    cur_color: &Color4f,
    result: &mut dyn WStream,
) {
    debug_assert!(range != 0.0);

    // Linearly interpolate from the previous color to the current.
    // Take the components 0..1 and determine the multipliers for interpolation.
    // C{r,g,b}(t, section) = t - offset_(section-1) + t * Multiplier{r,g,b}.

    // Figure out how to scale each color component.
    let mut multiplier = [0.0f32; 4];
    for i in 0..num_components {
        multiplier[i] = (cur_color[i] - prev_color[i]) / range;
    }

    // Calculate when we no longer need to keep a copy of the input parameter t.
    // If the last component to use t is i, then dupInput[0..i - 1] = true
    // and dupInput[i .. components] = false.
    let mut dup_input = [false; 4];
    dup_input[num_components - 1] = false;
    for i in (0..num_components - 1).rev() {
        dup_input[i] = dup_input[i + 1] || multiplier[i + 1] != 0.0;
    }

    if !dup_input[0] && multiplier[0] == 0.0 {
        result.write_text("pop ");
    }

    for i in 0..num_components {
        // If the next components needs t and this component will consume a
        // copy, make another copy.
        if dup_input[i] && multiplier[i] != 0.0 {
            result.write_text("dup ");
        }

        if multiplier[i] == 0.0 {
            append_color_component_f(prev_color[i], result);
            result.write_text(" ");
        } else {
            if multiplier[i] != 1.0 {
                append_scalar(multiplier[i], result);
                result.write_text(" mul ");
            }
            if prev_color[i] != 0.0 {
                append_color_component_f(prev_color[i], result);
                result.write_text(" add ");
            }
        }

        if dup_input[i] {
            result.write_text("exch ");
        }
    }
}

// Convert { r, g, b, a } to a == 0 ? {0 0 0} : { r/a, g/a, b/a }
// Port of: src/pdf/SkPDFGradientShader.cpp#L127-L143 (unpremul, chrome/m156)
fn unpremul(function: &mut dyn WStream) {
    // Preview Version 11.0 (1069.7.1) aborts the function if the predicate is like
    // "dup 0 eq" or any other use of "eq" with "a".
    function.write_text(
        "dup abs 0.00001 lt\
         { pop pop pop pop 0 0 0 }\
         {\
          dup\
          3 1 roll\
          div\
          4 1 roll\
          dup\
          3 1 roll\
          div\
          4 1 roll\
          div\
          3 1 roll\
         } ifelse\n",
    );
}

// Port of: src/pdf/SkPDFGradientShader.cpp#L145-L195 (write_gradient_ranges, chrome/m156)
fn write_gradient_ranges(
    info: &GradientData,
    range_ends: &[usize],
    num_components: usize,
    top: bool,
    first: bool,
    result: &mut dyn WStream,
) {
    debug_assert!(!range_ends.is_empty());

    let range_end_index = range_ends[range_ends.len() - 1];
    let range_end = info.color_offsets[range_end_index];

    // Each range check tests 0 < t <= end.
    if top {
        debug_assert!(first);
        // t may have been set to 0 to signal that the answer has already been found.
        result.write_text("dup dup 0 gt exch "); // In Preview 11.0 (1033.3) `0. 0 ne` is true.
        append_scalar(range_end, result);
        result.write_text(" le and {\n");
    } else if first {
        // After the top level check, only t <= end needs to be tested on if (lo) side.
        result.write_text("dup ");
        append_scalar(range_end, result);
        result.write_text(" le {\n");
    } else {
        // The else (hi) side.
        result.write_text("{\n");
    }

    if range_ends.len() == 1 {
        // Set the stack to [r g b].
        let range_begin_index = range_end_index - 1;
        let range_begin = info.color_offsets[range_begin_index];
        append_scalar(range_begin, result);
        result.write_text(" sub "); // consume t, put t - startOffset on the stack.
        interpolate_color_code(
            range_end - range_begin,
            num_components,
            &info.colors[range_begin_index],
            &info.colors[range_end_index],
            result,
        );
        result.write_text("\n");
    } else {
        let lo_count = range_ends.len() / 2;
        let lo_span = &range_ends[..lo_count];
        write_gradient_ranges(info, lo_span, num_components, false, true, result);

        let hi_span = &range_ends[lo_count..];
        write_gradient_ranges(info, hi_span, num_components, false, false, result);
    }

    if top {
        // Put 0 on the stack for t once here instead of after every call to
        // interpolate_color_code.
        result.write_text("0} if\n");
    } else if first {
        result.write_text("}"); // The else (hi) side will come next.
    } else {
        result.write_text("} ifelse\n");
    }
}

/// Generate Type 4 function code to map t to the passed gradient, clamping at the ends. The types
/// integer, real, and boolean are available. There are no string, array, procedure, variable, or
/// name types available.
///
/// The generated code will be of the following form with all values hard coded.
///
/// ```text
///   if (t <= 0) {
///     ret = color[0];
///     t = 0;
///   }
///   if (t > 0 && t <= stop[4]) {
///     if (t <= stop[2]) {
///       if (t <= stop[1]) {
///         ret = interp(t - stop[0], stop[1] - stop[0], color[0], color[1]);
///       } else {
///         ret = interp(t - stop[1], stop[2] - stop[1], color[1], color[2]);
///       }
///     } else {
///       if (t <= stop[3] {
///         ret = interp(t - stop[2], stop[3] - stop[2], color[2], color[3]);
///       } else {
///         ret = interp(t - stop[3], stop[4] - stop[3], color[3], color[4]);
///       }
///     }
///     t = 0;
///   }
///   if (t > 0) {
///     ret = color[4];
///   }
/// ```
///
/// which in PDF will be represented like
///
/// ```text
///   dup 0 le {pop 0 0 0 0} if
///   dup dup 0 gt exch 1 le and {
///     dup .5 le {
///       dup .25 le {
///         0 sub 2 mul 0 0
///       }{
///         .25 sub .5 exch 2 mul 0
///       } ifelse
///     }{
///       dup .75 le {
///         .5 sub .5 exch .5 exch 2 mul
///       }{
///         .75 sub dup 2 mul .5 add exch dup 2 mul .5 add exch 2 mul .5 add
///       } ifelse
///     } ifelse
///   0} if
///   0 gt {1 1 1} if
/// ```
// Port of: src/pdf/SkPDFGradientShader.cpp#L197-L305 (gradient_function_code, chrome/m156)
fn gradient_function_code(info: &GradientData, result: &mut dyn WStream) {
    // While looking for a hit the stack is [t].
    // After finding a hit the stack is [r g b 0].
    // The 0 is consumed just before returning.

    let premul = info.premul_interp;
    let num_components = if premul { 4 } else { 3 };

    // The initial range has no previous and contains a solid color.
    // Any t <= 0 will be handled by this initial range, so later t == 0 indicates a hit was found.
    result.write_text("dup 0 le {pop ");
    append_color_component_f(info.colors[0].r, result);
    result.write_text(" ");
    append_color_component_f(info.colors[0].g, result);
    result.write_text(" ");
    append_color_component_f(info.colors[0].b, result);
    if num_components == 4 {
        result.write_text(" ");
        append_color_component_f(info.colors[0].a, result);
    }
    result.write_text(" 0} if\n");

    // Optimize out ranges which don't make any visual difference.
    let color_count = info.colors.len();
    let mut range_ends: Vec<usize> = Vec::with_capacity(color_count);
    for i in 1..color_count {
        // Ignoring the alpha, is this range the same solid color as the next range?
        // This optimizes gradients where sometimes only the color or only the alpha is changing.
        let eq_ignoring_alpha = |a: &Color4f, b: &Color4f| {
            if premul {
                a == b
            } else {
                a.to_opaque() == b.to_opaque()
            }
        };
        let constant_color_both_sides =
            eq_ignoring_alpha(&info.colors[i - 1], &info.colors[i]) && // This range is a solid color.
            i != color_count - 1 &&                                    // This is not the last range.
            eq_ignoring_alpha(&info.colors[i], &info.colors[i + 1]); // Next range is same solid color.

        // Does this range have zero size?
        #[allow(clippy::float_cmp)] // exact comparison, as in Skia
        let degenerate_range = info.color_offsets[i - 1] == info.color_offsets[i];

        if !degenerate_range && !constant_color_both_sides {
            range_ends.push(i);
        }
    }

    // If a cap on depth is needed, loop here.
    // All ranges might be optimized out (e.g. all degenerate or constant color), in which case
    // the color writes below handle everything.
    if !range_ends.is_empty() {
        write_gradient_ranges(info, &range_ends, num_components, true, true, result);
    }

    // Clamp the final color.
    let last = &info.colors[color_count - 1];
    result.write_text("0 gt {");
    append_color_component_f(last.r, result);
    result.write_text(" ");
    append_color_component_f(last.g, result);
    result.write_text(" ");
    append_color_component_f(last.b, result);
    if num_components == 4 {
        result.write_text(" ");
        append_color_component_f(last.a, result);
    }
    result.write_text("} if\n");

    if premul {
        unpremul(result);
    }
}

// Port of: src/pdf/SkPDFGradientShader.cpp#L307-L328 (createInterpolationFunction, chrome/m156)
fn create_interpolation_function(color1: &Color4f, color2: &Color4f) -> PdfDict {
    let mut retval = PdfDict::new(None);

    let mut c0 = PdfArray::new();
    c0.append_color_component_f(color1.r);
    c0.append_color_component_f(color1.g);
    c0.append_color_component_f(color1.b);
    retval.insert_object("C0", Box::new(c0));

    let mut c1 = PdfArray::new();
    c1.append_color_component_f(color2.r);
    c1.append_color_component_f(color2.g);
    c1.append_color_component_f(color2.b);
    retval.insert_object("C1", Box::new(c1));

    retval.insert_object("Domain", Box::new(make_int_array(&[0, 1])));

    retval.insert_int("FunctionType", 2);
    retval.insert_scalar("N", 1.0);

    retval
}

// Port of: src/pdf/SkPDFGradientShader.cpp#L330-L398 (gradientStitchCode, chrome/m156)
#[allow(clippy::float_cmp)] // exact comparisons, as in Skia
fn gradient_stitch_code(info: &GradientData) -> PdfDict {
    let mut retval = PdfDict::new(None);

    // normalize color stops
    let mut color_count = info.colors.len();
    let mut colors: Vec<Color4f> = info.colors.clone();
    let mut color_offsets: Vec<f32> = info.color_offsets.clone();

    let mut i = 1;
    while i + 1 < color_count {
        // ensure stops are in order
        if color_offsets[i - 1] > color_offsets[i] {
            color_offsets[i] = color_offsets[i - 1];
        }

        // remove points that are between 2 coincident points
        if (color_offsets[i - 1] == color_offsets[i]) && (color_offsets[i] == color_offsets[i + 1])
        {
            color_count -= 1;
            colors.remove(i);
            color_offsets.remove(i);
        } else {
            i += 1;
        }
    }
    // find coincident points and slightly move them over
    i = 1;
    while i + 1 < color_count {
        if color_offsets[i - 1] == color_offsets[i] {
            color_offsets[i] += 0.00001;
        }
        i += 1;
    }
    // check if last 2 stops coincide
    if color_offsets[i - 1] == color_offsets[i] {
        color_offsets[i - 1] -= 0.00001;
    }

    // no need for a stitch function if there are only 2 stops.
    if color_count == 2 {
        return create_interpolation_function(&colors[0], &colors[1]);
    }

    let mut encode = PdfArray::new();
    let mut bounds = PdfArray::new();
    let mut functions = PdfArray::new();

    retval.insert_object("Domain", Box::new(make_int_array(&[0, 1])));
    retval.insert_int("FunctionType", 3);

    for idx in 1..color_count {
        if idx > 1 {
            bounds.append_scalar(color_offsets[idx - 1]);
        }

        encode.append_scalar(0.0);
        encode.append_scalar(1.0);

        functions.append_object(Box::new(create_interpolation_function(
            &colors[idx - 1],
            &colors[idx],
        )));
    }

    retval.insert_object("Encode", Box::new(encode));
    retval.insert_object("Bounds", Box::new(bounds));
    retval.insert_object("Functions", Box::new(functions));

    retval
}

/// Map a value of t on the stack into [0, 1) for Repeat or Mirror tile mode.
// Port of: src/pdf/SkPDFGradientShader.cpp#L400-L431 (tileModeCode, chrome/m156)
fn tile_mode_code(mode: TileMode, result: &mut dyn WStream) {
    if mode == TileMode::Repeat {
        result.write_text("dup truncate sub\n"); // Get the fractional part.
        result.write_text("dup 0 le {1 add} if\n"); // Map (-1,0) => (0,1)
        return;
    }

    if mode == TileMode::Mirror {
        // In Preview 11.0 (1033.3) `a n mod r eq` (with a and n both integers, r integer or real)
        // early aborts the function when false would be put on the stack.
        // Work around this by re-writing `t 2 mod 1 eq` as `t 2 mod 0 gt`.

        // Map t mod 2 into [0, 1, 1, 0].
        //                Code                 Stack t
        result.write_text(
            "abs \
             dup \
             truncate \
             dup \
             cvi \
             2 mod \
             0 gt \
             3 1 roll \
             sub \
             exch \
             {1 exch sub} if\n",
        );
    }
}

/// Returns PS function code that applies inverse perspective to a x, y point. The function
/// assumes that the stack has at least two elements, and that the top 2 elements are numeric
/// values. After executing this code on a PS stack, the last 2 elements are updated while the
/// rest of the stack is preserved intact. `inverse_perspective_matrix` is the inverse
/// perspective matrix.
// Port of: src/pdf/SkPDFGradientShader.cpp#L433-L476 (apply_perspective_to_coordinates,
// chrome/m156)
fn apply_perspective_to_coordinates(inverse_perspective_matrix: &Matrix, code: &mut dyn WStream) {
    if !inverse_perspective_matrix.has_perspective() {
        return;
    }

    // Perspective matrix should be:
    // 1   0  0
    // 0   1  0
    // p0 p1 p2

    let p0 = inverse_perspective_matrix.get(6usize); // kMPersp0
    let p1 = inverse_perspective_matrix.get(7usize); // kMPersp1
    let p2 = inverse_perspective_matrix.get(8usize); // kMPersp2

    // y = y / (p2 + p0 x + p1 y)
    // x = x / (p2 + p0 x + p1 y)

    // Input on stack: x y
    code.write_text(" dup "); // x y y
    append_scalar(p1, code); // x y y p1
    code.write_text(
        " mul \
          2 index ",
    ); // x y y*p1 x
    append_scalar(p0, code); // x y y p1 x p0
    code.write_text(" mul "); // x y y*p1 x*p0
    append_scalar(p2, code); // x y y p1 x*p0 p2
    code.write_text(
        " add \
         add \
         3 1 roll \
         2 index \
         div \
         3 1 roll \
         exch \
         div \
         exch\n",
    );
}

// Port of: src/pdf/SkPDFGradientShader.cpp#L478-L491 (linearCode, chrome/m156)
fn linear_code(info: &GradientData, perspective_remover: &Matrix, function: &mut dyn WStream) {
    function.write_text("{");

    apply_perspective_to_coordinates(perspective_remover, function);

    function.write_text("pop\n"); // Just ditch the y value.
    tile_mode_code(info.tile_mode, function);
    gradient_function_code(info, function);
    function.write_text("}");
}

// Port of: src/pdf/SkPDFGradientShader.cpp#L493-L514 (radialCode, chrome/m156)
fn radial_code(info: &GradientData, perspective_remover: &Matrix, function: &mut dyn WStream) {
    function.write_text("{");

    apply_perspective_to_coordinates(perspective_remover, function);

    // Find the distance from the origin.
    function.write_text(
        "dup \
         mul \
         exch \
         dup \
         mul \
         add \
         sqrt\n",
    );

    tile_mode_code(info.tile_mode, function);
    gradient_function_code(info, function);
    function.write_text("}");
}

/// Conical gradient shader, based on the Canvas spec for radial gradients. See:
/// <http://www.w3.org/TR/2dcontext/#dom-context-2d-createradialgradient>
// Port of: src/pdf/SkPDFGradientShader.cpp#L516-L624 (twoPointConicalCode, chrome/m156)
#[allow(clippy::float_cmp)] // exact comparison, as in Skia
fn two_point_conical_code(
    info: &GradientData,
    perspective_remover: &Matrix,
    function: &mut dyn WStream,
) {
    let dx = info.point[1].x - info.point[0].x;
    let dy = info.point[1].y - info.point[0].y;
    let r0 = info.radius[0];
    let dr = info.radius[1] - info.radius[0];
    let a = dx * dx + dy * dy - dr * dr;

    // First compute t, if the pixel falls outside the cone, then we'll end
    // with 'false' on the stack, otherwise we'll push 'true' with t below it

    // We start with a stack of (x y), copy it and then consume one copy in
    // order to calculate b and the other to calculate c.
    function.write_text("{");

    apply_perspective_to_coordinates(perspective_remover, function);

    function.write_text("2 copy ");

    // Calculate b and b^2; b = -2 * (y * dy + x * dx + r0 * dr).
    append_scalar(dy, function);
    function.write_text(" mul exch ");
    append_scalar(dx, function);
    function.write_text(" mul add ");
    append_scalar(r0 * dr, function);
    function.write_text(" add -2 mul dup dup mul\n");

    // c = x^2 + y^2 + radius0^2
    function.write_text("4 2 roll dup mul exch dup mul add ");
    append_scalar(r0 * r0, function);
    function.write_text(" sub dup 4 1 roll\n");

    // Contents of the stack at this point: c, b, b^2, c

    // if a = 0, then we collapse to a simpler linear case
    if a == 0.0 {
        // t = -c/b
        function.write_text("pop pop div neg dup ");

        // compute radius(t)
        append_scalar(dr, function);
        function.write_text(" mul ");
        append_scalar(r0, function);
        function.write_text(" add\n");

        // if r(t) < 0, then it's outside the cone
        function.write_text("0 lt {pop false} {true} ifelse\n");
    } else {
        // quadratic case: the Canvas spec wants the largest
        // root t for which radius(t) > 0

        // compute the discriminant (b^2 - 4ac)
        append_scalar(a * 4.0, function);
        function.write_text(" mul sub dup\n");

        // if d >= 0, proceed
        function.write_text("0 ge {\n");

        // an intermediate value we'll use to compute the roots:
        // q = -0.5 * (b +/- sqrt(d))
        function.write_text("sqrt exch dup 0 lt {exch -1 mul} if");
        function.write_text(" add -0.5 mul dup\n");

        // first root = q / a
        append_scalar(a, function);
        function.write_text(" div\n");

        // second root = c / q
        function.write_text("3 1 roll div\n");

        // put the larger root on top of the stack
        function.write_text("2 copy gt {exch} if\n");

        // compute radius(t) for larger root
        function.write_text("dup ");
        append_scalar(dr, function);
        function.write_text(" mul ");
        append_scalar(r0, function);
        function.write_text(" add\n");

        // if r(t) > 0, we have our t, pop off the smaller root and we're done
        function.write_text(" 0 gt {exch pop true}\n");

        // otherwise, throw out the larger one and try the smaller root
        function.write_text("{pop dup\n");
        append_scalar(dr, function);
        function.write_text(" mul ");
        append_scalar(r0, function);
        function.write_text(" add\n");

        // if r(t) < 0, push false, otherwise the smaller root is our t
        function.write_text("0 le {pop false} {true} ifelse\n");
        function.write_text("} ifelse\n");

        // d < 0, clear the stack and push false
        function.write_text("} {pop pop pop false} ifelse\n");
    }

    // if the pixel is in the cone, proceed to compute a color
    function.write_text("{");
    tile_mode_code(info.tile_mode, function);
    gradient_function_code(info, function);

    // otherwise, just write black
    // TODO: Correctly draw gradients_local_persepective, need to mask out this black
    // The "gradients" gm works as falls into the 8.7.4.5.4 "Type 3 (Radial) Shadings" case.
    function.write_text("} {0 0 0} ifelse }");
}

// Port of: src/pdf/SkPDFGradientShader.cpp#L626-L644 (sweepCode, chrome/m156)
#[allow(clippy::float_cmp)] // exact comparisons, as in Skia
fn sweep_code(info: &GradientData, _perspective_remover: &Matrix, function: &mut dyn WStream) {
    function.write_text("{exch atan 360 div\n");
    let bias = info.point[1].y;
    if bias != 0.0 {
        append_scalar(bias, function);
        function.write_text(" add\n");
    }
    let scale = info.point[1].x;
    if scale != 1.0 {
        append_scalar(scale, function);
        function.write_text(" mul\n");
    }
    tile_mode_code(info.tile_mode, function);
    gradient_function_code(info, function);
    function.write_text("}");
}

// catch cases where the inner just touches the outer circle
// and make the inner circle just inside the outer one to match raster
// Port of: src/pdf/SkPDFGradientShader.cpp#L647-L659 (FixUpRadius, chrome/m156)
fn fix_up_radius(p1: Point, r1: &mut f32, p2: Point, r2: &mut f32) {
    // detect touching circles
    let distance = Point::distance(p1, p2);
    let subtract_radii = (*r1 - *r2).abs();
    if (distance - subtract_radii).abs() < 0.002 {
        if *r1 > *r2 {
            *r1 += 0.002;
        } else {
            *r2 += 0.002;
        }
    }
}

// Finds affine and persp such that in = affine * persp.
// but it returns the inverse of perspective matrix.
// Port of: src/pdf/SkPDFGradientShader.cpp#L661-L694 (split_perspective, chrome/m156)
fn split_perspective(input: &Matrix) -> Option<(Matrix, Matrix)> {
    let p2 = input.get(8usize);

    if p2.nearly_zero(None) {
        return None;
    }

    let zero = 0.0f32;
    let one = 1.0f32;

    let sx = input.scale_x();
    let kx = input.skew_x();
    let tx = input.translate_x();
    let ky = input.skew_y();
    let sy = input.scale_y();
    let ty = input.translate_y();
    let p0 = input.get(6usize);
    let p1 = input.get(7usize);

    // Perspective matrix would be:
    // 1  0  0
    // 0  1  0
    // p0 p1 p2
    // But we need the inverse of persp.
    let mut perspective_inverse = Matrix::new_identity();
    perspective_inverse.set_all(one, zero, zero, zero, one, zero, -p0 / p2, -p1 / p2, 1.0 / p2);

    let mut affine = Matrix::new_identity();
    affine.set_all(
        sx - p0 * tx / p2,
        kx - p1 * tx / p2,
        tx / p2,
        ky - p0 * ty / p2,
        sy - p1 * ty / p2,
        ty / p2,
        zero,
        zero,
        one,
    );

    Some((affine, perspective_inverse))
}

// Port of: src/pdf/SkPDFGradientShader.cpp#L696-L706 (make_ps_function, chrome/m156)
fn make_ps_function(
    ps_code: &[u8],
    domain: PdfArray,
    range: PdfArray,
    doc: &DocHandle,
) -> PdfIndirectReference {
    let mut dict = PdfDict::new(None);
    dict.insert_int("FunctionType", 4);
    dict.insert_object("Domain", Box::new(domain));
    dict.insert_object("Range", Box::new(range));
    doc.stream_out(Some(dict), ps_code, true)
}

// ShadingType values of PDF32000 8.7.4.5.
const SHADING_TYPE_FUNCTION: i32 = 1;
const SHADING_TYPE_AXIAL: i32 = 2;
const SHADING_TYPE_RADIAL: i32 = 3;

// Port of: src/pdf/SkPDFGradientShader.cpp#L708-L873 (make_function_shader, chrome/m156)
#[allow(clippy::too_many_lines)] // one function in Skia
fn make_function_shader(doc: &DocHandle, state: &GradientKey) -> PdfIndirectReference {
    let info = &state.info;
    let mut final_matrix = state.canvas_transform.clone();
    final_matrix.pre_concat(&state.shader_transform);

    let do_stitch_functions = (state.gradient_type == GradientType::Linear
        || state.gradient_type == GradientType::Radial
        || state.gradient_type == GradientType::Conical)
        && (info.tile_mode == TileMode::Clamp || info.tile_mode == TileMode::Decal)
        && !final_matrix.has_perspective()
        && !info.premul_interp;

    let shading_type;

    let mut pdf_shader = PdfDict::new(None);
    if do_stitch_functions {
        pdf_shader.insert_object("Function", Box::new(gradient_stitch_code(info)));

        if info.tile_mode == TileMode::Clamp {
            let mut extend = PdfArray::new();
            extend.reserve(2);
            extend.append_bool(true);
            extend.append_bool(true);
            pdf_shader.insert_object("Extend", Box::new(extend));
        }

        let coords = match state.gradient_type {
            GradientType::Linear => {
                shading_type = SHADING_TYPE_AXIAL;
                let pt1 = info.point[0];
                let pt2 = info.point[1];
                make_scalar_array(&[pt1.x, pt1.y, pt2.x, pt2.y])
            }
            GradientType::Radial => {
                shading_type = SHADING_TYPE_RADIAL;
                let pt1 = info.point[0];
                let mut coords = PdfArray::new();
                coords.reserve(6);
                coords.append_scalar(pt1.x);
                coords.append_scalar(pt1.y);
                coords.append_int(0);
                coords.append_scalar(pt1.x);
                coords.append_scalar(pt1.y);
                coords.append_scalar(info.radius[0]);
                coords
            }
            GradientType::Conical => {
                shading_type = SHADING_TYPE_RADIAL;
                let mut r1 = info.radius[0];
                let mut r2 = info.radius[1];
                let pt1 = info.point[0];
                let pt2 = info.point[1];
                fix_up_radius(pt1, &mut r1, pt2, &mut r2);

                make_scalar_array(&[pt1.x, pt1.y, r1, pt2.x, pt2.y, r2])
            }
            GradientType::Sweep | GradientType::None => {
                debug_assert!(false);
                return PdfIndirectReference::default();
            }
        };
        pdf_shader.insert_object("Coords", Box::new(coords));
    } else {
        shading_type = SHADING_TYPE_FUNCTION;

        // Transform the coordinate space for the type of gradient.
        let mut transform_points = [info.point[0], info.point[1]];
        match state.gradient_type {
            GradientType::Linear => {}
            GradientType::Radial => {
                transform_points[1] = transform_points[0];
                transform_points[1].x += info.radius[0];
            }
            GradientType::Conical | GradientType::Sweep => {
                transform_points[1] = transform_points[0];
                transform_points[1].x += 1.0;
            }
            GradientType::None => return PdfIndirectReference::default(),
        }

        // Move any scaling (assuming a unit gradient) or translation
        // (and rotation for linear gradient), of the final gradient from
        // info.fPoints to the matrix (updating bbox appropriately).  Now
        // the gradient can be drawn on on the unit segment.
        let mapper_matrix = unit_to_points_matrix(&transform_points);

        final_matrix.pre_concat(&mapper_matrix);

        // Preserves as much as possible in the final matrix, and only removes
        // the perspective. The inverse of the perspective is stored in
        // perspectiveInverseOnly matrix and has 3 useful numbers
        // (p0, p1, p2), while everything else is either 0 or 1.
        // In this way the shader will handle it eficiently, with minimal code.
        let mut perspective_inverse_only = Matrix::new_identity();
        if final_matrix.has_perspective() {
            let Some((affine, perspective_inverse)) = split_perspective(&final_matrix) else {
                return PdfIndirectReference::default();
            };
            final_matrix = affine;
            perspective_inverse_only = perspective_inverse;
        }

        let mut bbox = Rect::from_irect(state.bbox);
        if !inverse_transform_bbox(&final_matrix, &mut bbox) {
            return PdfIndirectReference::default();
        }

        let mut function_code = DynamicMemoryWStream::new();
        match state.gradient_type {
            GradientType::Linear => {
                linear_code(info, &perspective_inverse_only, &mut function_code);
            }
            GradientType::Radial => {
                radial_code(info, &perspective_inverse_only, &mut function_code);
            }
            GradientType::Conical => {
                // The two point radial gradient further references state.fInfo
                // in translating from x, y coordinates to the t parameter. So, we have
                // to transform the points and radii according to the calculated matrix.
                let Some(inverse_mapper_matrix) = mapper_matrix.invert() else {
                    return PdfIndirectReference::default();
                };
                let mut info_copy = info.clone();
                inverse_mapper_matrix.map_points_inplace(&mut info_copy.point);
                info_copy.radius[0] = inverse_mapper_matrix.map_radius(info.radius[0]);
                info_copy.radius[1] = inverse_mapper_matrix.map_radius(info.radius[1]);
                two_point_conical_code(&info_copy, &perspective_inverse_only, &mut function_code);
            }
            GradientType::Sweep => {
                sweep_code(info, &perspective_inverse_only, &mut function_code);
            }
            GradientType::None => debug_assert!(false),
        }
        pdf_shader.insert_object(
            "Domain",
            Box::new(make_scalar_array(&[
                bbox.left, bbox.right, bbox.top, bbox.bottom,
            ])),
        );

        let domain = make_scalar_array(&[bbox.left, bbox.right, bbox.top, bbox.bottom]);
        let range_object = make_int_array(&[0, 1, 0, 1, 0, 1]);
        let function_ref = make_ps_function(
            &function_code.detach_as_vector(),
            domain,
            range_object,
            doc,
        );
        pdf_shader.insert_ref("Function", function_ref);
    }

    pdf_shader.insert_int("ShadingType", shading_type);
    pdf_shader.insert_name("ColorSpace", "DeviceRGB");

    let mut pdf_function_shader = PdfDict::new(Some("Pattern"));
    pdf_function_shader.insert_int("PatternType", 2);
    pdf_function_shader.insert_object("Matrix", Box::new(matrix_to_array(&final_matrix)));
    pdf_function_shader.insert_object("Shading", Box::new(pdf_shader));
    doc.emit_new(&pdf_function_shader)
}

// Port of: src/pdf/SkPDFGradientShader.cpp#L877-L891 (get_gradient_resource_dict, chrome/m156)
fn get_gradient_resource_dict(
    function_shader: PdfIndirectReference,
    g_state: PdfIndirectReference,
) -> PdfDict {
    let mut pattern_shaders = Vec::new();
    if function_shader != PdfIndirectReference::default() {
        pattern_shaders.push(function_shader);
    }
    let mut graphic_states = Vec::new();
    if g_state != PdfIndirectReference::default() {
        graphic_states.push(g_state);
    }
    make_resource_dict(&graphic_states, &pattern_shaders, &[], &[])
}

/// Creates a content stream which fills the pattern P0 across bounds. `gs_index` is a graphics
/// state resource index to apply, or <0 if no graphics state to apply.
// Port of: src/pdf/SkPDFGradientShader.cpp#L893-L907 (create_pattern_fill_content, chrome/m156)
fn create_pattern_fill_content(gs_index: i32, pattern_index: i32, bounds: &Rect) -> Vec<u8> {
    let mut content = DynamicMemoryWStream::new();
    if gs_index >= 0 {
        apply_graphic_state(gs_index, &mut content);
    }
    apply_pattern(pattern_index, &mut content);
    append_rectangle(bounds, &mut content);
    paint_path(PaintStyle::Fill, PathFillType::EvenOdd, &mut content);
    content.detach_as_vector()
}

// Port of: src/pdf/SkPDFGradientShader.cpp#L909-L917 (has_alpha, chrome/m156)
fn key_has_alpha(key: &GradientKey) -> bool {
    debug_assert!(key.gradient_type != GradientType::None);
    key.info.colors.iter().any(|c| !c.is_opaque())
}

// Port of: src/pdf/SkPDFGradientShader.cpp#L919-L927 (has_alpha, chrome/m156)
fn shader_has_alpha(shader: &Shader, key: &GradientKey) -> bool {
    debug_assert!(key.gradient_type != GradientType::None);
    if shader.is_opaque() {
        return false;
    }
    key_has_alpha(key)
}

// Port of: src/pdf/SkPDFGradientShader.cpp#L953-L977 (create_smask_graphic_state, chrome/m156)
fn create_smask_graphic_state(doc: &DocHandle, state: &GradientKey) -> PdfIndirectReference {
    debug_assert!(state.gradient_type != GradientType::None);
    let mut info = state.info.clone();
    for color in &mut info.colors {
        let alpha = color.a;
        *color = Color4f::new(alpha, alpha, alpha, 1.0);
    }
    info.premul_interp = false;
    let luminosity_state = state.with_info(info);

    debug_assert!(!key_has_alpha(&luminosity_state));
    let luminosity_shader = find_pdf_shader(doc, luminosity_state, false);
    let resources = get_gradient_resource_dict(luminosity_shader, PdfIndirectReference::default());
    let bbox = Rect::from_irect(state.bbox);
    let alpha_mask = make_form_x_object(
        doc,
        &create_pattern_fill_content(-1, luminosity_shader.value, &bbox),
        PdfParentTreeKey::default(),
        rect_to_array(&bbox),
        resources,
        &Matrix::new_identity(),
        Some("DeviceRGB"),
    );
    get_smask_graphic_state(alpha_mask, false, SMaskMode::Luminosity, doc)
}

// Port of: src/pdf/SkPDFGradientShader.cpp#L979-L1012 (make_alpha_function_shader, chrome/m156)
fn make_alpha_function_shader(doc: &DocHandle, state: &GradientKey) -> PdfIndirectReference {
    debug_assert!(state.gradient_type != GradientType::None);
    let mut opaque_info = state.info.clone();
    let keep_alpha = opaque_info.premul_interp;
    if !keep_alpha {
        for color in &mut opaque_info.colors {
            color.a = 1.0;
        }
    }
    let opaque_state = state.with_info(opaque_info);
    if !keep_alpha {
        debug_assert!(!key_has_alpha(&opaque_state));
    }
    let mut bbox = Rect::from_irect(state.bbox);
    let color_shader = find_pdf_shader(doc, opaque_state, false);
    if !color_shader.is_valid() {
        return PdfIndirectReference::default();
    }
    // Create resource dict with alpha graphics state as G0 and
    // pattern shader as P0, then write content stream.
    let alpha_gs_ref = create_smask_graphic_state(doc, state);

    let resource_dict = get_gradient_resource_dict(color_shader, alpha_gs_ref);

    let color_stream = create_pattern_fill_content(alpha_gs_ref.value, color_shader.value, &bbox);
    let mut alpha_function_shader = PdfDict::new(None);
    populate_tiling_pattern_dict(
        &mut alpha_function_shader,
        &bbox,
        false,
        false,
        resource_dict,
        &Matrix::new_identity(),
    );
    let _ = &mut bbox;
    doc.stream_out(Some(alpha_function_shader), &color_stream, true)
}

// Port of: src/pdf/SkPDFGradientShader.cpp#L1014-L1050 (make_key, chrome/m156)
fn make_key(shader: &Shader, canvas_transform: &Matrix, bbox: &IRect) -> GradientKey {
    let base = shader.as_base();
    let mut probe = GradientInfo::default();
    let gradient_type = base.as_gradient(Some(&mut probe), None);
    debug_assert!(GradientType::None != gradient_type);
    let color_count = probe.color_count;
    debug_assert!(color_count > 0);
    let mut colors = vec![Color4f::default(); color_count];
    let mut offsets = vec![0.0f32; color_count];
    let (point, radius, tile_mode, mut premul_interp);
    {
        let mut info = GradientInfo {
            color_count,
            colors: Some(&mut colors),
            color_offsets: Some(&mut offsets),
            ..GradientInfo::default()
        };
        base.as_gradient(Some(&mut info), None);
        point = info.point;
        radius = info.radius;
        tile_mode = info.tile_mode;
        premul_interp = info.premul_interp;
    }
    if premul_interp {
        let mut changed_by_premul = false;
        for c in &mut colors {
            if c.a != 1.0 {
                changed_by_premul = true;
            }
            let pm = c.premul();
            *c = Color4f::new(pm.r, pm.g, pm.b, pm.a);
        }
        if !changed_by_premul {
            premul_interp = false;
        }
    }
    GradientKey::new(
        gradient_type,
        GradientData {
            colors,
            color_offsets: offsets,
            point,
            radius,
            tile_mode,
            premul_interp,
        },
        canvas_transform.clone(),
        get_shader_local_matrix(shader),
        *bbox,
    )
}

// Port of: src/pdf/SkPDFGradientShader.cpp#L1052-L1071 (find_pdf_shader, chrome/m156)
fn find_pdf_shader(
    doc: &DocHandle,
    key: GradientKey,
    make_alpha_shader: bool,
) -> PdfIndirectReference {
    if let Some(reference) = doc.with(|d| d.gradient_pattern_map.get(&key).copied()) {
        return reference;
    }
    let pdf_shader = if make_alpha_shader {
        make_alpha_function_shader(doc, &key)
    } else {
        make_function_shader(doc, &key)
    };
    doc.with(|d| d.gradient_pattern_map.insert(key, pdf_shader));
    pdf_shader
}

/// `SkPDFGradientShader::Make`: the pattern for a gradient shader, or none if it can't be made
/// (or the alpha gradients are to be rasterized for printing).
// Port of: src/pdf/SkPDFGradientShader.cpp#L1073-L1089 (chrome/m156)
#[doc(alias = "SkPDFGradientShader::Make")]
#[must_use]
pub fn make_gradient_shader(
    doc: &DocHandle,
    shader: &Shader,
    canvas_transform: &Matrix,
    bbox: &IRect,
) -> PdfIndirectReference {
    let key = make_key(shader, canvas_transform, bbox);
    let make_alpha_shader = shader_has_alpha(shader, &key);
    if doc.metadata().rasterize_alpha_gradients_for_printing && make_alpha_shader {
        return PdfIndirectReference::default();
    }
    find_pdf_shader(doc, key, make_alpha_shader)
}
