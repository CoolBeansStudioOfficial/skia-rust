// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/utils/SkPatchUtils.h, src/utils/SkPatchUtils.cpp

//! `SkPatchUtils`: helpers to turn a Coons patch (four cubics with shared corners, optionally
//! with a color and a texture coordinate at each corner) into [`Vertices`]; used by
//! `drawPatch`.

use skia_rust_simd::vx::{Float2, Float4};

use crate::alpha_type::AlphaType;
use crate::color::{Color, PMColor4f};
use crate::color_space::ColorSpace;
use crate::color_space_priv::srgb_singleton;
use crate::color_type::ColorType;
use crate::convert_pixels::convert_pixels;
use crate::floating_point::{float_floor2int_no_saturate, is_finite};
use crate::geometry::{CubicCoeff, times_2, to_point};
use crate::image_info::ImageInfo;
use crate::matrix::Matrix;
use crate::point::Point;
use crate::scalar::scalar;
use crate::size::ISize;
use crate::t_pin::t_pin;
use crate::vertices::{Builder, BuilderFlags, VertexMode, Vertices};

/// The number of control points of a patch (`SkPatchUtils::kNumCtrlPts`).
// Port of: src/utils/SkPatchUtils.h#L24-L28 (chrome/m156)
#[doc(alias = "kNumCtrlPts")]
pub const NUM_CTRL_PTS: usize = 12;
/// The number of corners of a patch (`SkPatchUtils::kNumCorners`).
#[doc(alias = "kNumCorners")]
pub const NUM_CORNERS: usize = 4;
/// The number of points of a cubic (`SkPatchUtils::kNumPtsCubic`).
#[doc(alias = "kNumPtsCubic")]
pub const NUM_PTS_CUBIC: usize = 4;

// Port of: src/utils/SkPatchUtils.cpp#L36-L56 (chrome/m156)
mod ctrl_pts {
    pub(super) const TOP_P0: usize = 0;
    pub(super) const TOP_P1: usize = 1;
    pub(super) const TOP_P2: usize = 2;
    pub(super) const TOP_P3: usize = 3;

    pub(super) const RIGHT_P0: usize = 3;
    pub(super) const RIGHT_P1: usize = 4;
    pub(super) const RIGHT_P2: usize = 5;
    pub(super) const RIGHT_P3: usize = 6;

    pub(super) const BOTTOM_P0: usize = 9;
    pub(super) const BOTTOM_P1: usize = 8;
    pub(super) const BOTTOM_P2: usize = 7;
    pub(super) const BOTTOM_P3: usize = 6;

    pub(super) const LEFT_P0: usize = 0;
    pub(super) const LEFT_P1: usize = 11;
    pub(super) const LEFT_P2: usize = 10;
    pub(super) const LEFT_P3: usize = 9;
}

// Port of: src/utils/SkPatchUtils.cpp#L58-L65 (chrome/m156)
// The corners are also clockwise.
const TOP_LEFT_CORNER: usize = 0;
const TOP_RIGHT_CORNER: usize = 1;
const BOTTOM_RIGHT_CORNER: usize = 2;
const BOTTOM_LEFT_CORNER: usize = 3;

/// Evaluator to sample the values of a cubic bezier using forward differences.
///
/// Forward differences is a method for evaluating a nth degree polynomial at a uniform step by
/// only adding precalculated values. For a linear example we have the function f(t) = m*t+b,
/// then the value of that function at t+h would be f(t+h) = m*(t+h)+b. If we want to know the
/// uniform step that we must add to the first evaluation f(t) then we need to substract
/// f(t+h) - f(t) = m*t + m*h + b - m*t + b = mh. After obtaining this value (mh) we could just
/// add this constant step to our first sampled point to compute the next one.
///
/// For the cubic case the first difference gives as a result a quadratic polynomial to which we
/// can apply again forward differences and get linear function to which we can apply again
/// forward differences to get a constant difference. This is why we keep an array of size 4, the
/// 0th position keeps the sampled value while the next ones keep the quadratic, linear and
/// constant difference values.
// Port of: src/utils/SkPatchUtils.cpp#L67-L143 (chrome/m156)
struct FwDCubicEvaluator {
    coefs: CubicCoeff,
    max: i32,
    current: i32,
    divisions: i32,
    fw_diff: [Point; 4],
    points: [Point; 4],
}

impl FwDCubicEvaluator {
    /// Receives the 4 control points of the cubic bezier.
    fn new(points: &[Point; 4]) -> FwDCubicEvaluator {
        let mut e = FwDCubicEvaluator {
            coefs: CubicCoeff::new(points),
            max: 0,
            current: 0,
            divisions: 0,
            fw_diff: [Point::default(); 4],
            points: *points,
        };
        e.restart(1);
        e
    }

    /// Restarts the forward differences evaluator to the first value of t = 0.
    #[allow(clippy::cast_precision_loss)] // mirrors `1.f / fDivisions`
    fn restart(&mut self, divisions: i32) {
        self.divisions = divisions;
        self.current = 0;
        self.max = self.divisions + 1;
        let h = Float2::splat(1.0 / self.divisions as scalar);
        let h2 = h * h;
        let h3 = h2 * h;
        let fw_diff3 = Float2::splat(6.0) * self.coefs.a * h3;
        self.fw_diff[3] = to_point(fw_diff3);
        self.fw_diff[2] = to_point(fw_diff3 + times_2(self.coefs.b) * h2);
        self.fw_diff[1] = to_point(self.coefs.a * h3 + self.coefs.b * h2 + self.coefs.c * h);
        self.fw_diff[0] = to_point(self.coefs.d);
    }

    /// Calls next to obtain the point sampled and move to the next one.
    fn next(&mut self) -> Point {
        let point = self.fw_diff[0];
        self.fw_diff[0] = add(self.fw_diff[0], self.fw_diff[1]);
        self.fw_diff[1] = add(self.fw_diff[1], self.fw_diff[2]);
        self.fw_diff[2] = add(self.fw_diff[2], self.fw_diff[3]);
        self.current += 1;
        point
    }

    fn ctrl_points(&self) -> &[Point; 4] {
        &self.points
    }
}

/// `SkPoint::operator+=`'s sum.
fn add(a: Point, b: Point) -> Point {
    Point::new(a.x + b.x, a.y + b.y)
}

/// Size in pixels of each partition per axis, adjust this knob.
// Port of: src/utils/SkPatchUtils.cpp#L148 (chrome/m156)
const PARTITION_SIZE: i32 = 10;

/// Calculates the approximate arc length given a bezier curve's control points. Returns -1 if
/// bad calc (i.e. non-finite).
// Port of: src/utils/SkPatchUtils.cpp#L150-L163 (chrome/m156)
fn approx_arc_length(points: &[Point]) -> scalar {
    if points.len() < 2 {
        return 0.0;
    }
    let mut arc_length = 0.0;
    for i in 0..points.len() - 1 {
        arc_length += Point::distance(points[i], points[i + 1]);
    }
    if is_finite(arc_length) {
        arc_length
    } else {
        -1.0
    }
}

// Port of: src/utils/SkPatchUtils.cpp#L165-L170 (chrome/m156)
fn bilerp(tx: scalar, ty: scalar, c00: scalar, c10: scalar, c01: scalar, c11: scalar) -> scalar {
    let a = c00 * (1.0 - tx) + c10 * tx;
    let b = c01 * (1.0 - tx) + c11 * tx;
    a * (1.0 - ty) + b * ty
}

// Port of: src/utils/SkPatchUtils.cpp#L172-L180 (chrome/m156)
fn bilerp4(tx: scalar, ty: scalar, c00: Float4, c10: Float4, c01: Float4, c11: Float4) -> Float4 {
    let a = c00 * (1.0 - tx) + c10 * tx;
    let b = c01 * (1.0 - tx) + c11 * tx;
    a * (1.0 - ty) + b * ty
}

/// The number of subdivisions (the level of detail) for a patch in both axes, based on the
/// lengths of its sides mapped by `matrix` (`GetLevelOfDetail`).
// Port of: src/utils/SkPatchUtils.cpp#L182-L210 (chrome/m156)
#[doc(alias = "GetLevelOfDetail")]
#[must_use]
#[allow(clippy::cast_possible_truncation)] // mirrors static_cast<int>(float): UB out of range
pub fn get_level_of_detail(cubics: &[Point; NUM_CTRL_PTS], matrix: &Matrix) -> ISize {
    // Approximate length of each cubic.
    let mut pts = get_top_cubic(cubics);
    matrix.map_points_inplace(&mut pts);
    let top_length = approx_arc_length(&pts);

    let mut pts = get_bottom_cubic(cubics);
    matrix.map_points_inplace(&mut pts);
    let bottom_length = approx_arc_length(&pts);

    let mut pts = get_left_cubic(cubics);
    matrix.map_points_inplace(&mut pts);
    let left_length = approx_arc_length(&pts);

    let mut pts = get_right_cubic(cubics);
    matrix.map_points_inplace(&mut pts);
    let right_length = approx_arc_length(&pts);

    if top_length < 0.0 || bottom_length < 0.0 || left_length < 0.0 || right_length < 0.0 {
        return ISize::new(0, 0); // negative length is a sentinel for bad length (i.e. non-finite)
    }

    // Level of detail per axis, based on the larger side between top and bottom or left and right
    // (`std::max(a, b)` is `a < b ? b : a`).
    let max_tb = if top_length < bottom_length {
        bottom_length
    } else {
        top_length
    };
    let max_lr = if left_length < right_length {
        right_length
    } else {
        left_length
    };
    #[allow(clippy::cast_precision_loss)] // 10 is exact in f32
    let partition_size = PARTITION_SIZE as scalar;
    let lod_x = (max_tb / partition_size) as i32;
    let lod_y = (max_lr / partition_size) as i32;

    ISize::new(lod_x.max(8), lod_y.max(8))
}

/// The points of the top cubic of `cubics` (`GetTopCubic`).
// Port of: src/utils/SkPatchUtils.cpp#L212-L217 (chrome/m156)
#[doc(alias = "GetTopCubic")]
#[must_use]
pub fn get_top_cubic(cubics: &[Point; NUM_CTRL_PTS]) -> [Point; NUM_PTS_CUBIC] {
    [
        cubics[ctrl_pts::TOP_P0],
        cubics[ctrl_pts::TOP_P1],
        cubics[ctrl_pts::TOP_P2],
        cubics[ctrl_pts::TOP_P3],
    ]
}

/// The points of the bottom cubic of `cubics` (`GetBottomCubic`).
// Port of: src/utils/SkPatchUtils.cpp#L219-L224 (chrome/m156)
#[doc(alias = "GetBottomCubic")]
#[must_use]
pub fn get_bottom_cubic(cubics: &[Point; NUM_CTRL_PTS]) -> [Point; NUM_PTS_CUBIC] {
    [
        cubics[ctrl_pts::BOTTOM_P0],
        cubics[ctrl_pts::BOTTOM_P1],
        cubics[ctrl_pts::BOTTOM_P2],
        cubics[ctrl_pts::BOTTOM_P3],
    ]
}

/// The points of the left cubic of `cubics` (`GetLeftCubic`).
// Port of: src/utils/SkPatchUtils.cpp#L226-L231 (chrome/m156)
#[doc(alias = "GetLeftCubic")]
#[must_use]
pub fn get_left_cubic(cubics: &[Point; NUM_CTRL_PTS]) -> [Point; NUM_PTS_CUBIC] {
    [
        cubics[ctrl_pts::LEFT_P0],
        cubics[ctrl_pts::LEFT_P1],
        cubics[ctrl_pts::LEFT_P2],
        cubics[ctrl_pts::LEFT_P3],
    ]
}

/// The points of the right cubic of `cubics` (`GetRightCubic`).
// Port of: src/utils/SkPatchUtils.cpp#L233-L238 (chrome/m156)
#[doc(alias = "GetRightCubic")]
#[must_use]
pub fn get_right_cubic(cubics: &[Point; NUM_CTRL_PTS]) -> [Point; NUM_PTS_CUBIC] {
    [
        cubics[ctrl_pts::RIGHT_P0],
        cubics[ctrl_pts::RIGHT_P1],
        cubics[ctrl_pts::RIGHT_P2],
        cubics[ctrl_pts::RIGHT_P3],
    ]
}

/// `[B, G, R, A]` bytes of `colors` (little-endian `kBGRA_8888`).
fn skcolors_to_bytes(colors: &[Color]) -> Vec<u8> {
    colors
        .iter()
        .flat_map(|&c| u32::from(c).to_le_bytes())
        .collect()
}

// Port of: src/utils/SkPatchUtils.cpp#L240-L246 (chrome/m156)
fn skcolor_to_float(src: &[Color], dst_cs: &ColorSpace) -> Vec<PMColor4f> {
    let count = src.len();
    let src_info = ImageInfo::new(
        (
            i32::try_from(count).expect("a patch has at most a few hundred thousand vertices"),
            1,
        ),
        ColorType::BGRA8888,
        AlphaType::Unpremul,
        Some(ColorSpace::new_srgb()),
    );
    let dst_info = ImageInfo::new(
        src_info.dimensions(),
        ColorType::RGBAF32,
        AlphaType::Premul,
        Some(dst_cs.clone()),
    );
    let src_bytes = skcolors_to_bytes(src);
    let mut dst_bytes = vec![0u8; count * 16];
    // SkAssertResult
    assert!(convert_pixels(
        &dst_info,
        &mut dst_bytes,
        0,
        &src_info,
        &src_bytes,
        0
    ));
    dst_bytes
        .as_chunks::<16>()
        .0
        .iter()
        .map(|px| {
            let c = |i: usize| f32::from_ne_bytes([px[i], px[i + 1], px[i + 2], px[i + 3]]);
            PMColor4f {
                r: c(0),
                g: c(4),
                b: c(8),
                a: c(12),
            }
        })
        .collect()
}

// Port of: src/utils/SkPatchUtils.cpp#L248-L254 (chrome/m156)
fn float_to_skcolor(dst: &mut [Color], src: &[PMColor4f], src_cs: &ColorSpace) {
    let count = src.len();
    let src_info = ImageInfo::new(
        (
            i32::try_from(count).expect("a patch has at most a few hundred thousand vertices"),
            1,
        ),
        ColorType::RGBAF32,
        AlphaType::Premul,
        Some(src_cs.clone()),
    );
    let dst_info = ImageInfo::new(
        src_info.dimensions(),
        ColorType::BGRA8888,
        AlphaType::Unpremul,
        Some(ColorSpace::new_srgb()),
    );
    let src_bytes: Vec<u8> = src
        .iter()
        .flat_map(|c| [c.r, c.g, c.b, c.a])
        .flat_map(f32::to_ne_bytes)
        .collect();
    let mut dst_bytes = vec![0u8; count * 4];
    // SkAssertResult
    assert!(convert_pixels(
        &dst_info,
        &mut dst_bytes,
        0,
        &src_info,
        &src_bytes,
        0
    ));
    for (color, px) in dst.iter_mut().zip(dst_bytes.as_chunks::<4>().0) {
        *color = Color::from(u32::from_le_bytes(*px));
    }
}

fn load4(c: &PMColor4f) -> Float4 {
    Float4::load(&[c.r, c.g, c.b, c.a])
}

/// Makes the vertices of a patch with `lod_x` by `lod_y` subdivisions: the interpolation of the
/// four `cubics` (clockwise from the top left, sharing every fourth point), with the colors
/// interpolated in `color_space` (sRGB if `None`) and the texture coordinates, if any,
/// associated with the corners (`MakeVertices`). `None` if the level of detail is smaller than
/// one or the vertex count overflows.
///
/// # Panics
/// Never: the counts are checked to fit before they are converted.
// Port of: src/utils/SkPatchUtils.cpp#L256-L390 (chrome/m156)
#[doc(alias = "MakeVertices")]
#[must_use]
#[allow(clippy::too_many_lines)] // mirrors SkPatchUtils::MakeVertices
#[allow(clippy::similar_names)] // mirrors lodX / lodY, s0 / s1 / s2
pub fn make_vertices(
    cubics: &[Point; NUM_CTRL_PTS],
    src_colors: Option<&[Color; NUM_CORNERS]>,
    src_tex_coords: Option<&[Point; NUM_CORNERS]>,
    mut lod_x: i32,
    mut lod_y: i32,
    color_space: Option<&ColorSpace>,
) -> Option<Vertices> {
    if lod_x < 1 || lod_y < 1 {
        return None;
    }

    // check for overflow in multiplication
    let lod_x64 = i64::from(lod_x) + 1;
    let lod_y64 = i64::from(lod_y) + 1;
    let mult64 = lod_x64 * lod_y64;
    if mult64 > i64::from(i32::MAX) {
        return None;
    }

    // Treat null interpolation space as sRGB.
    let color_space = color_space.unwrap_or_else(|| srgb_singleton());

    let mut vertex_count = i32::try_from(mult64).expect("SkToS32(mult64)");
    // it is recommended to generate draw calls of no more than 65536 indices, so we never
    // generate more than 60000 indices. To accomplish that we resize the LOD and vertex count
    if vertex_count > 10000 || lod_x > 200 || lod_y > 200 {
        #[allow(clippy::cast_precision_loss)] // mirrors static_cast<float>(lodX)
        let weight_x = lod_x as f32 / (lod_x + lod_y) as f32;
        #[allow(clippy::cast_precision_loss)] // mirrors static_cast<float>(lodY)
        let weight_y = lod_y as f32 / (lod_x + lod_y) as f32;

        // 200 comes from the 100 * 2 which is the max value of vertices because of the limit of
        // 60000 indices ( sqrt(60000 / 6) that comes from data->fIndexCount = lodX * lodY * 6)
        // Need a min of 1 since we later divide by lod
        lod_x = 1.max(float_floor2int_no_saturate(weight_x * 200.0));
        lod_y = 1.max(float_floor2int_no_saturate(weight_y * 200.0));
        vertex_count = (lod_x + 1) * (lod_y + 1);
    }
    let index_count = lod_x * lod_y * 6;
    let mut flags = BuilderFlags::empty();
    if src_tex_coords.is_some() {
        flags |= BuilderFlags::HAS_TEX_COORDS;
    }
    if src_colors.is_some() {
        flags |= BuilderFlags::HAS_COLORS;
    }

    let vertex_count = usize::try_from(vertex_count).expect("a positive vertex count");
    let mut builder = Builder::new(
        VertexMode::Triangles,
        vertex_count,
        usize::try_from(index_count).expect("a positive index count"),
        flags,
    );

    let corner_colors = src_colors.map(|colors| skcolor_to_float(colors, color_space));
    let mut tmp_colors = src_colors.map(|_| vec![PMColor4f::default(); vertex_count]);

    let mut pos = vec![Point::default(); vertex_count];
    let mut texs = src_tex_coords.map(|_| vec![Point::default(); vertex_count]);
    let mut indices = vec![0u16; usize::try_from(index_count).expect("a positive index count")];

    let mut f_bottom = FwDCubicEvaluator::new(&get_bottom_cubic(cubics));
    let mut f_top = FwDCubicEvaluator::new(&get_top_cubic(cubics));
    let mut f_left = FwDCubicEvaluator::new(&get_left_cubic(cubics));
    let mut f_right = FwDCubicEvaluator::new(&get_right_cubic(cubics));

    f_bottom.restart(lod_x);
    f_top.restart(lod_x);

    let mut u: scalar = 0.0;
    let stride = lod_y + 1;
    for x in 0..=lod_x {
        let bottom = f_bottom.next();
        let top = f_top.next();
        f_left.restart(lod_y);
        f_right.restart(lod_y);
        let mut v: scalar = 0.0;
        for y in 0..=lod_y {
            let data_index = usize::try_from(x * (lod_y + 1) + y).expect("a vertex index");

            let left = f_left.next();
            let right = f_right.next();

            let s0 = Point::new(
                (1.0 - v) * top.x + v * bottom.x,
                (1.0 - v) * top.y + v * bottom.y,
            );
            let s1 = Point::new(
                (1.0 - u) * left.x + u * right.x,
                (1.0 - u) * left.y + u * right.y,
            );
            let s2 = Point::new(
                (1.0 - v) * ((1.0 - u) * f_top.ctrl_points()[0].x + u * f_top.ctrl_points()[3].x)
                    + v * ((1.0 - u) * f_bottom.ctrl_points()[0].x
                        + u * f_bottom.ctrl_points()[3].x),
                (1.0 - v) * ((1.0 - u) * f_top.ctrl_points()[0].y + u * f_top.ctrl_points()[3].y)
                    + v * ((1.0 - u) * f_bottom.ctrl_points()[0].y
                        + u * f_bottom.ctrl_points()[3].y),
            );
            // `s0 + s1 - s2`
            let s01 = add(s0, s1);
            pos[data_index] = Point::new(s01.x - s2.x, s01.y - s2.y);

            if let (Some(corner_colors), Some(tmp_colors)) = (&corner_colors, &mut tmp_colors) {
                let out = bilerp4(
                    u,
                    v,
                    load4(&corner_colors[TOP_LEFT_CORNER]),
                    load4(&corner_colors[TOP_RIGHT_CORNER]),
                    load4(&corner_colors[BOTTOM_LEFT_CORNER]),
                    load4(&corner_colors[BOTTOM_RIGHT_CORNER]),
                );
                tmp_colors[data_index] = PMColor4f {
                    r: out[0],
                    g: out[1],
                    b: out[2],
                    a: out[3],
                };
            }

            if let (Some(texs), Some(src_tex_coords)) = (&mut texs, src_tex_coords) {
                texs[data_index] = Point::new(
                    bilerp(
                        u,
                        v,
                        src_tex_coords[TOP_LEFT_CORNER].x,
                        src_tex_coords[TOP_RIGHT_CORNER].x,
                        src_tex_coords[BOTTOM_LEFT_CORNER].x,
                        src_tex_coords[BOTTOM_RIGHT_CORNER].x,
                    ),
                    bilerp(
                        u,
                        v,
                        src_tex_coords[TOP_LEFT_CORNER].y,
                        src_tex_coords[TOP_RIGHT_CORNER].y,
                        src_tex_coords[BOTTOM_LEFT_CORNER].y,
                        src_tex_coords[BOTTOM_RIGHT_CORNER].y,
                    ),
                );
            }

            if x < lod_x && y < lod_y {
                let i = usize::try_from(6 * (x * lod_y + y)).expect("an index offset");
                // The indices fit in 16 bits: `lodX`, `lodY` <= 200 when there are many.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                // mirrors the implicit int -> uint16_t conversions
                let idx = |n: i32| n as u16;
                indices[i] = idx(x * stride + y);
                indices[i + 1] = idx(x * stride + 1 + y);
                indices[i + 2] = idx((x + 1) * stride + 1 + y);
                indices[i + 3] = indices[i];
                indices[i + 4] = indices[i + 2];
                indices[i + 5] = idx((x + 1) * stride + y);
            }
            #[allow(clippy::cast_precision_loss)] // mirrors `1.f / lodY`
            {
                v = t_pin(v + 1.0 / lod_y as scalar, 0.0, 1.0);
            }
        }
        #[allow(clippy::cast_precision_loss)] // mirrors `1.f / lodX`
        {
            u = t_pin(u + 1.0 / lod_x as scalar, 0.0, 1.0);
        }
    }

    // Fill in the builder (Skia writes into its arrays as it goes).
    builder.positions().copy_from_slice(&pos);
    if let (Some(dst), Some(src)) = (builder.tex_coords(), &texs) {
        dst.copy_from_slice(src);
    }
    if let Some(dst) = builder.indices() {
        dst.copy_from_slice(&indices);
    }
    if let (Some(tmp_colors), Some(dst)) = (&tmp_colors, builder.colors()) {
        float_to_skcolor(dst, tmp_colors, color_space);
    }
    builder.detach()
}

#[cfg(test)]
#[allow(clippy::cast_precision_loss)] // small loop counters
#[allow(clippy::many_single_char_names)] // x, y, u, w, i of the geometry
mod tests {
    use super::*;

    /// A straight-edged square patch from (0, 0) to (30, 30), clockwise from the top left.
    fn square() -> [Point; NUM_CTRL_PTS] {
        [
            // top
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(20.0, 0.0),
            Point::new(30.0, 0.0),
            // right
            Point::new(30.0, 10.0),
            Point::new(30.0, 20.0),
            // bottom
            Point::new(30.0, 30.0),
            Point::new(20.0, 30.0),
            Point::new(10.0, 30.0),
            Point::new(0.0, 30.0),
            // left
            Point::new(0.0, 20.0),
            Point::new(0.0, 10.0),
        ]
    }

    #[test]
    fn cubics_of_the_sides() {
        let c = square();
        assert_eq!(get_top_cubic(&c)[3], Point::new(30.0, 0.0));
        assert_eq!(get_right_cubic(&c)[0], Point::new(30.0, 0.0));
        assert_eq!(get_right_cubic(&c)[3], Point::new(30.0, 30.0));
        // The bottom cubic runs from the bottom-left corner to the bottom-right one.
        assert_eq!(get_bottom_cubic(&c)[0], Point::new(0.0, 30.0));
        assert_eq!(get_bottom_cubic(&c)[3], Point::new(30.0, 30.0));
        // The left cubic runs from the top-left corner to the bottom-left one.
        assert_eq!(get_left_cubic(&c)[0], Point::new(0.0, 0.0));
        assert_eq!(get_left_cubic(&c)[3], Point::new(0.0, 30.0));
    }

    #[test]
    fn level_of_detail() {
        let c = square();
        // 30 pixels per side at 10 pixels per partition is 3, but there are at least 8.
        assert_eq!(
            get_level_of_detail(&c, &Matrix::new_identity()),
            ISize::new(8, 8)
        );
        // 300 pixels per side.
        assert_eq!(
            get_level_of_detail(&c, &Matrix::scale((10.0, 10.0))),
            ISize::new(30, 30)
        );
        assert_eq!(
            get_level_of_detail(&c, &Matrix::scale((10.0, 20.0))),
            ISize::new(30, 60)
        );
        // Non-finite lengths have no level of detail.
        let mut bad = c;
        bad[1] = Point::new(f32::INFINITY, 0.0);
        assert_eq!(
            get_level_of_detail(&bad, &Matrix::new_identity()),
            ISize::new(0, 0)
        );
    }

    #[test]
    fn vertices_of_a_square_patch() {
        let c = square();
        let colors = [Color::RED, Color::GREEN, Color::BLUE, Color::WHITE];
        let tex = [
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 50.0),
            Point::new(0.0, 50.0),
        ];
        let v = make_vertices(&c, Some(&colors), Some(&tex), 3, 2, None).unwrap();
        assert_eq!(v.mode(), VertexMode::Triangles);
        assert_eq!(v.vertex_count(), 4 * 3);
        assert_eq!(v.index_count(), 3 * 2 * 6);

        // Vertex (x, y) is at u = x / 3 along the top and bottom, v = y / 2 down the sides.
        let near = |a: Point, b: Point| (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4;
        for x in 0..=3_usize {
            for y in 0..=2_usize {
                let i = x * 3 + y;
                let (u, w) = (x as f32 / 3.0, y as f32 / 2.0);
                assert!(
                    near(v.positions()[i], Point::new(30.0 * u, 30.0 * w)),
                    "{i}: {:?}",
                    v.positions()[i]
                );
                assert!(near(
                    v.tex_coords().unwrap()[i],
                    Point::new(100.0 * u, 50.0 * w)
                ));
            }
        }
        // The corners keep their colors.
        let vc = v.colors().unwrap();
        assert_eq!(vc[0], Color::RED);
        assert_eq!(vc[3 * 3], Color::GREEN);
        assert_eq!(vc[3 * 3 + 2], Color::BLUE);
        assert_eq!(vc[2], Color::WHITE);

        // The first quad's triangles.
        let i = v.indices().unwrap();
        assert_eq!(&i[..6], &[0, 1, 4, 0, 4, 3]);
    }

    #[test]
    fn vertices_of_a_patch_are_limited() {
        let c = square();
        assert!(make_vertices(&c, None, None, 0, 5, None).is_none());
        assert!(make_vertices(&c, None, None, 5, 0, None).is_none());
        assert!(make_vertices(&c, None, None, i32::MAX, i32::MAX, None).is_none());
        // A large level of detail is reduced (here to 100 x 100: the sum is 200) so that there
        // are at most 60000 indices.
        let v = make_vertices(&c, None, None, 1000, 1000, None).unwrap();
        assert_eq!(v.vertex_count(), 101 * 101);
        assert_eq!(v.index_count(), 100 * 100 * 6);
        assert!(v.colors().is_none() && v.tex_coords().is_none());
    }
}
