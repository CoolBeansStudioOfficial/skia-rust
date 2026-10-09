// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/utils/SkShadowTessellator.{h,cpp} (chrome/m156)

//! Shadow mesh generation (`SkShadowTessellator`): builds ambient and spot shadow triangles for
//! a path. Convex paths are fully supported. Concave paths need `SkOffsetSimplePolygon`,
//! `SkIsSimplePolygon` and `SkTriangulateSimplePolygon`, which are not ported yet, so for them
//! the tessellator reports failure and the caller falls back to a blur.

// The index and count casts here mirror the C++ int indices of the Skia code this file ports;
// every value stays inside the polygon or vertex count, which is bounded by u16::MAX for meshes.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::similar_names, // sNumer/tNumer etc. keep the C++ names
    clippy::many_single_char_names, // p0/p1/v0/v1 keep the C++ names for auditing the port
)]
use crate::color::Color;
use crate::draw_shadow_info::{
    ambient_blur_radius, ambient_recip_alpha, get_spot_shadow_transform,
};
use crate::floating_point::{ieee_float_divide, is_finite_all};
use crate::matrix::Matrix;
use crate::path::{Iter as PathIterSk, Path, Verb};
use crate::point::{Point, Vector, point_priv};
use crate::point3::Point3;
use crate::poly_utils::{compute_radial_steps, inset_convex_polygon};
use crate::rect::Rect;
use crate::scalar::{
    SCALAR_NEARLY_ZERO, scalar, scalar_invert, scalar_round_to_scalar, scalar_sqrt,
};
use crate::vertices::{VertexMode, Vertices};

// Keep the Ganesh tessellation tolerances in the same place as the C++ file.
// Port of: src/utils/SkShadowTessellator.cpp#L36-L37 (chrome/m156)
const QUAD_TOLERANCE: scalar = 0.2;
const CUBIC_TOLERANCE: scalar = 0.2;
// Port of: src/utils/SkShadowTessellator.cpp#L40-L41 (chrome/m156)
const QUAD_TOLERANCE_SQD: scalar = QUAD_TOLERANCE * QUAD_TOLERANCE;
const CUBIC_TOLERANCE_SQD: scalar = CUBIC_TOLERANCE * CUBIC_TOLERANCE;
// Port of: src/utils/SkShadowTessellator.cpp#L43 (chrome/m156)
const CONIC_TOLERANCE: scalar = 0.25;

/// Color of the inner (umbra) vertices (`kUmbraColor`).
const UMBRA_COLOR: Color = Color::BLACK;
/// Color of the outer (penumbra) vertices (`kPenumbraColor`).
const PENUMBRA_COLOR: Color = Color::TRANSPARENT;

/// `SkBaseShadowTessellator`: builds the triangle mesh of a shadow from a path walked in device
/// space. The ambient and spot tessellators are its subclasses in C++; here the path walking is
/// done by [`ambient_tessellate`] and [`spot_tessellate`] on the same struct.
// Port of: src/utils/SkShadowTessellator.cpp#L41-L90 (chrome/m156)
#[allow(clippy::struct_excessive_bools)] // the C++ class keeps these flags as separate bools
struct BaseShadowTessellator {
    z_plane_params: Point3,
    path_bounds: Rect,
    point_buffer: Vec<Point>,
    positions: Vec<Point>,
    colors: Vec<Color>,
    indices: Vec<u16>,
    path_polygon: Vec<Point>,
    clip_polygon: Vec<Point>,
    clip_vectors: Vec<Vector>,
    centroid: Point,
    area: scalar,
    last_area: scalar,
    last_cross: scalar,
    first_vertex_index: i32,
    first_outset: Vector,
    first_point: Point,
    succeeded: bool,
    transparent: bool,
    is_convex: bool,
    valid_umbra: bool,
    direction: scalar,
    prev_umbra_index: i32,
    curr_umbra_index: i32,
    curr_clip_index: i32,
    prev_umbra_outside: bool,
    first_umbra_outside: bool,
    prev_outset: Vector,
    prev_point: Point,
}

// Port of: src/utils/SkShadowTessellator.cpp#L93-L115 (chrome/m156)
fn compute_normal(p0: Point, p1: Point, dir: scalar) -> Option<Vector> {
    // compute perpendicular
    let mut normal = Vector::new(p0.y - p1.y, p1.x - p0.x);
    normal *= dir;
    if !normal.normalize() {
        return None;
    }
    Some(normal)
}

// Port of: src/utils/SkShadowTessellator.cpp#L117-L121 (chrome/m156)
fn duplicate_pt(p0: Point, p1: Point) -> bool {
    const CLOSE: scalar = 1.0 / 16.0;
    const CLOSE_SQD: scalar = CLOSE * CLOSE;
    let dist_sq = point_priv::distance_to_sqd(p0, p1);
    dist_sq < CLOSE_SQD
}

// Port of: src/utils/SkShadowTessellator.cpp#L123-L127 (chrome/m156)
fn perp_dot(p0: Point, p1: Point, p2: Point) -> scalar {
    let v0 = p1 - p0;
    let v1 = p2 - p1;
    v0.cross(v1)
}

impl BaseShadowTessellator {
    // Port of: src/utils/SkShadowTessellator.cpp#L129-L146 (chrome/m156)
    fn new(z_plane_params: Point3, bounds: Rect, transparent: bool) -> Self {
        BaseShadowTessellator {
            z_plane_params,
            path_bounds: bounds,
            point_buffer: Vec::new(),
            positions: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
            path_polygon: Vec::new(),
            clip_polygon: Vec::new(),
            clip_vectors: Vec::new(),
            centroid: Point::new(0.0, 0.0),
            area: 0.0,
            last_area: 0.0,
            last_cross: 0.0,
            first_vertex_index: -1,
            first_outset: Vector::default(),
            first_point: Point::default(),
            succeeded: false,
            transparent,
            is_convex: true,
            valid_umbra: true,
            direction: 1.0,
            prev_umbra_index: -1,
            curr_umbra_index: 0,
            curr_clip_index: 0,
            prev_umbra_outside: false,
            first_umbra_outside: false,
            prev_outset: Vector::default(),
            prev_point: Point::default(),
        }
    }

    /// `releaseVertices`: the triangle mesh, or `None` if the tessellation failed.
    // Port of: src/utils/SkShadowTessellator.cpp#L50-L58 (chrome/m156)
    fn release_vertices(&self) -> Option<Vertices> {
        if !self.succeeded {
            return None;
        }
        Vertices::new_copy(
            VertexMode::Triangles,
            &self.positions,
            None,
            Some(&self.colors),
            Some(&self.indices),
        )
    }

    /// `heightFunc`.
    // Port of: src/utils/SkShadowTessellator.cpp#L80-L82 (chrome/m156)
    fn height_func(&self, x: scalar, y: scalar) -> scalar {
        self.z_plane_params.x * x + self.z_plane_params.y * y + self.z_plane_params.z
    }

    // Port of: src/utils/SkShadowTessellator.cpp#L153-L173 (chrome/m156)
    fn accumulate_centroid(&mut self, curr: Point, next: Point) -> bool {
        if duplicate_pt(curr, next) {
            return false;
        }
        let v0 = curr - self.path_polygon[0];
        let v1 = next - self.path_polygon[0];
        let quad_area = v0.cross(v1);
        self.centroid.x += (v0.x + v1.x) * quad_area;
        self.centroid.y += (v0.y + v1.y) * quad_area;
        self.area += quad_area;

        // convexity check
        if quad_area * self.last_area < 0.0 {
            self.is_convex = false;
        }
        if 0.0 != quad_area {
            self.last_area = quad_area;
        }
        true
    }

    // Port of: src/utils/SkShadowTessellator.cpp#L175-L191 (chrome/m156)
    fn check_convexity(&mut self, p0: Point, p1: Point, p2: Point) -> bool {
        let cross = perp_dot(p0, p1, p2);
        // skip collinear point
        if cross.abs() <= SCALAR_NEARLY_ZERO {
            return false;
        }
        // check for convexity
        if self.last_cross * cross < 0.0 {
            self.is_convex = false;
        }
        if 0.0 != cross {
            self.last_cross = cross;
        }
        true
    }

    // Port of: src/utils/SkShadowTessellator.cpp#L193-L218 (chrome/m156)
    fn finish_path_polygon(&mut self) {
        if self.path_polygon.len() > 1 {
            let last = self.path_polygon[self.path_polygon.len() - 1];
            let first = self.path_polygon[0];
            if !self.accumulate_centroid(last, first) {
                // remove coincident point
                self.path_polygon.pop();
            }
        }
        if self.path_polygon.len() > 2 {
            // do this before the final convexity check, so we use the correct fPathPolygon[0]
            self.centroid *= ieee_float_divide(1.0, 3.0 * self.area);
            self.centroid += self.path_polygon[0];
            let n = self.path_polygon.len();
            if !self.check_convexity(
                self.path_polygon[n - 2],
                self.path_polygon[n - 1],
                self.path_polygon[0],
            ) {
                // remove collinear point
                self.path_polygon[0] = self.path_polygon[n - 1];
                self.path_polygon.pop();
            }
        }
        // if area is positive, winding is ccw
        self.direction = if self.area > 0.0 { -1.0 } else { 1.0 };
    }

    // Port of: src/utils/SkShadowTessellator.cpp#L220-L294 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the length of computeConvexShadow
    fn compute_convex_shadow(&mut self, mut inset: scalar, outset: scalar, do_clip: bool) -> bool {
        if do_clip {
            self.compute_clip_vectors_and_test_centroid();
        }

        // adjust inset distance and umbra color if necessary
        let mut umbra_color = UMBRA_COLOR;
        let n = self.path_polygon.len();
        let mut min_dist_sq = point_priv::distance_to_line_segment_between_sqd(
            self.centroid,
            self.path_polygon[0],
            self.path_polygon[1],
        );
        for i in 1..n {
            let mut j = i + 1;
            if i == n - 1 {
                j = 0;
            }
            let curr_point = self.path_polygon[i];
            let next_point = self.path_polygon[j];
            let dist_sq = point_priv::distance_to_line_segment_between_sqd(
                self.centroid,
                curr_point,
                next_point,
            );
            if dist_sq < min_dist_sq {
                min_dist_sq = dist_sq;
            }
        }

        let mut inset_polygon: Vec<Point> = Vec::new();
        if inset > SCALAR_NEARLY_ZERO {
            const TOLERANCE: scalar = 1.0e-2;
            if min_dist_sq < (inset + TOLERANCE) * (inset + TOLERANCE) {
                // if the umbra would collapse, we back off a bit on inner blur and adjust the alpha
                let new_inset = scalar_sqrt(min_dist_sq) - TOLERANCE;
                let ratio = 128.0 * (new_inset / inset + 1.0);
                // they aren't PMColors, but the interpolation algorithm is the same
                umbra_color = Color::new(crate::color_data::pm_lerp(
                    u32::from(UMBRA_COLOR),
                    u32::from(UMBRA_COLOR),
                    ratio as u32,
                ));
                inset = new_inset;
            }
            // generate inner ring
            match inset_convex_polygon(&self.path_polygon, inset) {
                Some(poly) => inset_polygon = poly,
                None => {
                    // not ideal, but in this case we'll inset using the centroid
                    self.valid_umbra = false;
                }
            }
        }
        let umbra_polygon: Vec<Point> = if inset > SCALAR_NEARLY_ZERO {
            inset_polygon
        } else {
            self.path_polygon.clone()
        };

        // walk around the path polygon, generate outer ring and connect to inner ring
        if self.transparent {
            self.positions.push(self.centroid);
            self.colors.push(umbra_color);
        }

        self.curr_umbra_index = 0;

        // initial setup
        // add first quad
        let poly_count = self.path_polygon.len();
        let Some(first_outset) = compute_normal(
            self.path_polygon[poly_count - 1],
            self.path_polygon[0],
            self.direction,
        ) else {
            // polygon should be sanitized by this point, so this is unrecoverable
            return false;
        };
        self.first_outset = first_outset;
        self.first_outset *= outset;
        self.first_point = self.path_polygon[poly_count - 1];
        self.first_vertex_index = self.positions.len() as i32;
        self.prev_outset = self.first_outset;
        self.prev_point = self.first_point;
        self.prev_umbra_index = -1;
        let first_point = self.first_point;
        let (_, prev_umbra_index) = self.add_inner_point(first_point, umbra_color, &umbra_polygon);
        self.prev_umbra_index = prev_umbra_index;

        if !self.transparent && do_clip {
            let first_vertex = self.positions[self.first_vertex_index as usize];
            let clip_point = self.clip_umbra_point(first_vertex, self.centroid);
            let is_outside = clip_point.is_some();
            if let Some(clip_point) = clip_point {
                self.positions.push(clip_point);
                self.colors.push(umbra_color);
            }
            self.prev_umbra_outside = is_outside;
            self.first_umbra_outside = is_outside;
        }

        let new_point = self.first_point + self.first_outset;
        self.positions.push(new_point);
        self.colors.push(PENUMBRA_COLOR);

        let first_path_point = self.path_polygon[0];
        let first_outset = self.first_outset;
        self.add_edge(
            first_path_point,
            first_outset,
            umbra_color,
            &umbra_polygon,
            false,
            do_clip,
        );

        for i in 1..poly_count {
            let Some(mut normal) =
                compute_normal(self.prev_point, self.path_polygon[i], self.direction)
            else {
                return false;
            };
            normal *= outset;
            self.add_arc(normal, outset, true);
            let path_point = self.path_polygon[i];
            self.add_edge(
                path_point,
                normal,
                umbra_color,
                &umbra_polygon,
                i == poly_count - 1,
                do_clip,
            );
        }

        // final fan
        let last = self.positions.len() as i32 - 1;
        if self.add_arc(self.first_outset, outset, false) {
            if self.first_umbra_outside {
                self.append_triangle(
                    self.first_vertex_index,
                    self.positions.len() as i32 - 1,
                    self.first_vertex_index + 2,
                );
            } else {
                self.append_triangle(
                    self.first_vertex_index,
                    self.positions.len() as i32 - 1,
                    self.first_vertex_index + 1,
                );
            }
        } else {
            // no arc added, fix up by setting first penumbra point position to last one
            let last_pos = self.positions[last as usize];
            if self.first_umbra_outside {
                self.positions[(self.first_vertex_index + 2) as usize] = last_pos;
            } else {
                self.positions[(self.first_vertex_index + 1) as usize] = last_pos;
            }
        }
        true
    }

    // Port of: src/utils/SkShadowTessellator.cpp#L296-L317 (chrome/m156)
    fn compute_clip_vectors_and_test_centroid(&mut self) {
        let n = self.clip_polygon.len();
        self.curr_clip_index = n as i32 - 1;

        // init clip vectors
        let mut v0 = self.clip_polygon[1] - self.clip_polygon[0];
        self.clip_vectors.push(v0);

        // init centroid check
        let mut hidden_centroid = true;
        let v1 = self.centroid - self.clip_polygon[0];
        let init_cross = v0.cross(v1);
        for p in 1..n {
            // add to clip vectors
            v0 = self.clip_polygon[(p + 1) % n] - self.clip_polygon[p];
            self.clip_vectors.push(v0);

            // Determine if transformed centroid is inside clipPolygon.
            let v1 = self.centroid - self.clip_polygon[p];
            if init_cross * v0.cross(v1) <= 0.0 {
                hidden_centroid = false;
            }
        }
        self.transparent = self.transparent || !hidden_centroid;
    }

    // Port of: src/utils/SkShadowTessellator.cpp#L319-L373 (chrome/m156)
    fn add_edge(
        &mut self,
        next_point: Point,
        next_normal: Vector,
        umbra_color: Color,
        umbra_polygon: &[Point],
        last_edge: bool,
        do_clip: bool,
    ) {
        // add next umbra point
        let curr_umbra_index: i32;
        let duplicate: bool;
        if last_edge {
            duplicate = false;
            curr_umbra_index = self.first_vertex_index;
            self.prev_point = next_point;
        } else {
            let (dup, idx) = self.add_inner_point(next_point, umbra_color, umbra_polygon);
            duplicate = dup;
            curr_umbra_index = idx;
        }

        let prev_penumbra_index = if duplicate || curr_umbra_index == self.first_vertex_index {
            self.positions.len() as i32 - 1
        } else {
            self.positions.len() as i32 - 2
        };

        if !duplicate {
            // add to center fan if transparent or centroid showing
            if self.transparent {
                self.append_triangle(0, self.prev_umbra_index, curr_umbra_index);
                // otherwise add to clip ring
            } else if do_clip {
                let (is_outside, clip_point) = if last_edge {
                    (self.first_umbra_outside, None)
                } else {
                    let umbra = self.positions[curr_umbra_index as usize];
                    let clip = self.clip_umbra_point(umbra, self.centroid);
                    (clip.is_some(), clip)
                };
                if is_outside {
                    // `clip_point` is only `Some` when this is not the last edge.
                    if let Some(clip_point) = clip_point {
                        self.positions.push(clip_point);
                        self.colors.push(umbra_color);
                    }
                    self.append_triangle(
                        self.prev_umbra_index,
                        curr_umbra_index,
                        curr_umbra_index + 1,
                    );
                    if self.prev_umbra_outside {
                        // fill out quad
                        self.append_triangle(
                            self.prev_umbra_index,
                            curr_umbra_index + 1,
                            self.prev_umbra_index + 1,
                        );
                    }
                } else if self.prev_umbra_outside {
                    // add tri
                    self.append_triangle(
                        self.prev_umbra_index,
                        curr_umbra_index,
                        self.prev_umbra_index + 1,
                    );
                }
                self.prev_umbra_outside = is_outside;
            }
        }

        // add next penumbra point and quad
        let new_point = next_point + next_normal;
        self.positions.push(new_point);
        self.colors.push(PENUMBRA_COLOR);

        if !duplicate {
            self.append_triangle(self.prev_umbra_index, prev_penumbra_index, curr_umbra_index);
        }
        self.append_triangle(
            prev_penumbra_index,
            self.positions.len() as i32 - 1,
            curr_umbra_index,
        );

        self.prev_umbra_index = curr_umbra_index;
        self.prev_outset = next_normal;
    }

    /// `clipUmbraPoint`: where the segment from `umbra_point` to `centroid` leaves the clip
    /// polygon, or `None` if it does not (or the segment is collinear with an edge).
    // Port of: src/utils/SkShadowTessellator.cpp#L375-L405 (chrome/m156)
    fn clip_umbra_point(&mut self, umbra_point: Point, centroid: Point) -> Option<Point> {
        let mut segment_vector = centroid - umbra_point;
        let n = self.clip_polygon.len() as i32;
        let start_clip_point = self.curr_clip_index;
        loop {
            let cur = self.curr_clip_index as usize;
            let dp = umbra_point - self.clip_polygon[cur];
            let denom = self.clip_vectors[cur].cross(segment_vector);
            let t_num = dp.cross(segment_vector);

            // if line segments are nearly parallel
            if denom.abs() <= SCALAR_NEARLY_ZERO {
                // and collinear
                if t_num.abs() <= SCALAR_NEARLY_ZERO {
                    return None;
                }
                // otherwise are separate, will try the next poly segment
                // else if crossing lies within poly segment
            } else if t_num >= 0.0 && t_num <= denom {
                let s_num = dp.cross(self.clip_vectors[cur]);
                // if umbra point is inside the clip polygon
                if s_num >= 0.0 && s_num <= denom {
                    segment_vector *= s_num / denom;
                    return Some(umbra_point + segment_vector);
                }
            }
            self.curr_clip_index = (self.curr_clip_index + 1) % n;
            if self.curr_clip_index == start_clip_point {
                break;
            }
        }
        None
    }

    /// `addInnerPoint`: adds the umbra vertex nearest `path_point`, merging close points. Returns
    /// whether the point duplicates the previous one, and the index of the umbra vertex.
    // Port of: src/utils/SkShadowTessellator.cpp#L407-L438 (chrome/m156)
    fn add_inner_point(
        &mut self,
        path_point: Point,
        umbra_color: Color,
        umbra_polygon: &[Point],
    ) -> (bool, i32) {
        let umbra_point = if self.valid_umbra {
            let idx = self.get_closest_umbra_index(path_point, umbra_polygon);
            umbra_polygon[idx as usize]
        } else {
            let mut v = self.centroid - path_point;
            v *= 0.95;
            path_point + v
        };

        self.prev_point = path_point;

        // merge "close" points
        if self.prev_umbra_index == -1
            || !duplicate_pt(umbra_point, self.positions[self.prev_umbra_index as usize])
        {
            // if we've wrapped around, don't add a new point
            let curr_umbra_index;
            if self.prev_umbra_index >= 0
                && duplicate_pt(
                    umbra_point,
                    self.positions[self.first_vertex_index as usize],
                )
            {
                curr_umbra_index = self.first_vertex_index;
            } else {
                curr_umbra_index = self.positions.len() as i32;
                self.positions.push(umbra_point);
                self.colors.push(umbra_color);
            }
            (false, curr_umbra_index)
        } else {
            (true, self.prev_umbra_index)
        }
    }

    /// `getClosestUmbraIndex`: walks the umbra polygon from the current index to the vertex
    /// nearest `p`.
    // Port of: src/utils/SkShadowTessellator.cpp#L440-L467 (chrome/m156)
    fn get_closest_umbra_index(&mut self, p: Point, umbra_polygon: &[Point]) -> i32 {
        let size = umbra_polygon.len() as i32;
        let mut min_distance =
            point_priv::distance_to_sqd(p, umbra_polygon[self.curr_umbra_index as usize]);
        let mut index = self.curr_umbra_index;
        let mut dir = 1;
        let mut next = (index + dir) % size;

        // init travel direction
        let mut distance = point_priv::distance_to_sqd(p, umbra_polygon[next as usize]);
        if distance < min_distance {
            index = next;
            min_distance = distance;
        } else {
            dir = size - 1;
        }

        // iterate until we find a point that increases the distance
        next = (index + dir) % size;
        distance = point_priv::distance_to_sqd(p, umbra_polygon[next as usize]);
        while distance < min_distance {
            index = next;
            min_distance = distance;
            next = (index + dir) % size;
            distance = point_priv::distance_to_sqd(p, umbra_polygon[next as usize]);
        }

        self.curr_umbra_index = index;
        index
    }

    /// `addArc`: fills in the fan from the previous quad to `next_normal`.
    // Port of: src/utils/SkShadowTessellator.cpp#L860-L886 (chrome/m156)
    fn add_arc(&mut self, next_normal: Vector, offset: scalar, finish_arc: bool) -> bool {
        // fill in fan from previous quad
        let (rot_sin, rot_cos, num_steps) =
            match compute_radial_steps(self.prev_outset, next_normal, offset) {
                Some((s, c, n)) => (s, c, n),
                // recover as best we can
                None => (0.0, 0.0, 0),
            };

        let mut prev_normal = self.prev_outset;
        let mut i = 0;
        while i < num_steps - 1 {
            let curr_normal = Vector::new(
                prev_normal.x * rot_cos - prev_normal.y * rot_sin,
                prev_normal.y * rot_cos + prev_normal.x * rot_sin,
            );
            self.positions.push(self.prev_point + curr_normal);
            self.colors.push(PENUMBRA_COLOR);
            let n = self.positions.len() as i32;
            self.append_triangle(self.prev_umbra_index, n - 1, n - 2);
            prev_normal = curr_normal;
            i += 1;
        }

        if finish_arc && num_steps != 0 {
            self.positions.push(self.prev_point + next_normal);
            self.colors.push(PENUMBRA_COLOR);
            let n = self.positions.len() as i32;
            self.append_triangle(self.prev_umbra_index, n - 1, n - 2);
        }
        self.prev_outset = next_normal;
        num_steps > 0
    }

    // Port of: src/utils/SkShadowTessellator.cpp#L888-L894 (chrome/m156)
    fn append_triangle(&mut self, index0: i32, index1: i32, index2: i32) {
        self.indices.push(index0 as u16);
        self.indices.push(index1 as u16);
        self.indices.push(index2 as u16);
    }

    /// `handleLine(p)`: sanitizes a point and adds it to the path polygon, removing collinear and
    /// coincident points.
    // Port of: src/utils/SkShadowTessellator.cpp#L658-L684 (chrome/m156)
    fn handle_line(&mut self, p: Point) {
        let p_sanitized = sanitize_point(p);
        if !self.path_polygon.is_empty() {
            let last = self.path_polygon[self.path_polygon.len() - 1];
            if !self.accumulate_centroid(last, p_sanitized) {
                // skip coincident point
                return;
            }
        }

        if self.path_polygon.len() > 1 {
            let n = self.path_polygon.len();
            if !self.check_convexity(
                self.path_polygon[n - 2],
                self.path_polygon[n - 1],
                p_sanitized,
            ) {
                // remove collinear point
                self.path_polygon.pop();
                // it's possible that the previous point is coincident with the new one now
                let last = self.path_polygon[self.path_polygon.len() - 1];
                if duplicate_pt(last, p_sanitized) {
                    self.path_polygon.pop();
                }
            }
        }
        self.path_polygon.push(p_sanitized);
    }

    /// `handleLine(m, p)`: maps `p` by `m` and handles it.
    // Port of: src/utils/SkShadowTessellator.cpp#L686-L689 (chrome/m156)
    fn handle_line_mapped(&mut self, m: &Matrix, p: &mut Point) {
        *p = m.map_point(*p);
        self.handle_line(*p);
    }

    /// `handleQuad(pts)`: subdivides a quad into line segments (Ganesh tessellation).
    // Port of: src/utils/SkShadowTessellator.cpp#L691-L715 (chrome/m156)
    fn handle_quad(&mut self, pts: &[Point; 3]) {
        // check for degeneracy
        let v0 = pts[1] - pts[0];
        let v1 = pts[2] - pts[0];
        if v0.cross(v1).abs() <= SCALAR_NEARLY_ZERO {
            return;
        }
        let max_count = gr_path_utils::quadratic_point_count(pts, QUAD_TOLERANCE);
        self.point_buffer.clear();
        gr_path_utils::generate_quadratic_points(
            pts[0],
            pts[1],
            pts[2],
            QUAD_TOLERANCE_SQD,
            &mut self.point_buffer,
            max_count,
        );
        let buffer = std::mem::take(&mut self.point_buffer);
        for &p in &buffer {
            self.handle_line(p);
        }
        self.point_buffer = buffer;
    }

    // Port of: src/utils/SkShadowTessellator.cpp#L717-L720 (chrome/m156)
    fn handle_quad_mapped(&mut self, m: &Matrix, pts: &mut [Point; 3]) {
        m.map_points_inplace(pts);
        self.handle_quad(&*pts);
    }

    /// `handleCubic(m, pts)`: maps and subdivides a cubic into line segments (Ganesh).
    // Port of: src/utils/SkShadowTessellator.cpp#L722-L742 (chrome/m156)
    fn handle_cubic_mapped(&mut self, m: &Matrix, pts: &mut [Point; 4]) {
        m.map_points_inplace(pts);
        let max_count = gr_path_utils::cubic_point_count(pts, CUBIC_TOLERANCE);
        self.point_buffer.clear();
        gr_path_utils::generate_cubic_points(
            pts[0],
            pts[1],
            pts[2],
            pts[3],
            CUBIC_TOLERANCE_SQD,
            &mut self.point_buffer,
            max_count,
        );
        let buffer = std::mem::take(&mut self.point_buffer);
        for &p in &buffer {
            self.handle_line(p);
        }
        self.point_buffer = buffer;
    }

    /// `handleConic(m, pts, w)`: maps a conic, converts it to quads and handles each quad.
    // Port of: src/utils/SkShadowTessellator.cpp#L744-L765 (chrome/m156)
    fn handle_conic_mapped(&mut self, m: &Matrix, pts: &mut [Point; 3], mut w: scalar) {
        if m.has_perspective() {
            w = crate::geometry::Conic::transform_w(pts, w, m);
        }
        m.map_points_inplace(pts);
        let mut quadder = crate::geometry::AutoConicToQuads::new();
        let quads: Vec<Point> = quadder
            .compute_quads_with_weight(pts, w, CONIC_TOLERANCE)
            .to_vec();
        let count = quadder.count_quads();
        let mut quads_idx = 1usize;
        let mut last_point = quads[0];
        for i in 0..count {
            let mut quad_pts = [Point::default(); 3];
            quad_pts[0] = last_point;
            quad_pts[1] = quads[quads_idx];
            quad_pts[2] = if i == count - 1 {
                pts[2]
            } else {
                quads[quads_idx + 1]
            };
            self.handle_quad(&quad_pts);
            last_point = quad_pts[2];
            quads_idx += 2;
        }
    }

    /// `computeConcaveShadow`. Not ported yet (needs `SkOffsetSimplePolygon`): reports failure so
    /// the caller falls back to a blur.
    // Port of: src/utils/SkShadowTessellator.cpp#L461-L494 (chrome/m156) (not yet ported)
    #[allow(clippy::unused_self)] // uses `self` once the concave path is ported
    fn compute_concave_shadow(&mut self, _inset: scalar, _outset: scalar) -> bool {
        false
    }
}

/// `sanitize_point`: clamps the point to the nearest 16th of a pixel.
// Port of: src/utils/SkShadowTessellator.cpp#L637-L641 (chrome/m156)
fn sanitize_point(p: Point) -> Point {
    Point::new(
        scalar_round_to_scalar(16.0 * p.x) * 0.0625,
        scalar_round_to_scalar(16.0 * p.y) * 0.0625,
    )
}

/// Walks a path with `SkPath::Iter(path, true)` (force-closing open contours) and collects each
/// verb with its points and conic weight (`0` when not a conic).
// Port of: src/utils/SkShadowTessellator.cpp#L836-L858 (chrome/m156) (the iter loop)
fn collect_records(path: &Path) -> Vec<(Verb, Vec<Point>, scalar)> {
    let mut iter = PathIterSk::new(path, true);
    let mut out = Vec::new();
    while let Some((verb, pts)) = iter.next() {
        let weight = iter.conic_weight().unwrap_or(0.0);
        out.push((verb, pts, weight));
    }
    out
}

/// The ambient shadow tessellator's path walk (`SkAmbientShadowTessellator`).
// Port of: src/utils/SkShadowTessellator.cpp#L769-L828 (chrome/m156)
fn ambient_tessellate(t: &mut BaseShadowTessellator, path: &Path, ctm: &Matrix) {
    // Set base colors
    let base_z = t.height_func(t.path_bounds.center_x(), t.path_bounds.center_y());

    // umbraColor is the interior value, penumbraColor the exterior value.
    let outset = ambient_blur_radius(base_z);
    let inset = outset * ambient_recip_alpha(base_z) - outset;

    if !compute_path_polygon_ambient(t, path, ctm) {
        return;
    }

    if t.path_polygon.len() < 3 || !is_finite_all(t.area, &[]) {
        // We don't want to try to blur these cases, so we will return an empty SkVertices
        // instead.
        t.succeeded = true;
        return;
    }

    t.succeeded = if t.is_convex {
        t.compute_convex_shadow(inset, outset, false)
    } else {
        t.compute_concave_shadow(inset, outset)
    };
}

/// `computePathPolygon` for the ambient shadow.
// Port of: src/utils/SkShadowTessellator.cpp#L836-L858 (chrome/m156)
fn compute_path_polygon_ambient(t: &mut BaseShadowTessellator, path: &Path, ctm: &Matrix) -> bool {
    let mut verb_seen = false;
    let mut close_seen = false;
    for (verb, src, weight) in collect_records(path) {
        if close_seen {
            return false;
        }
        let mut pts = [Point::default(); 4];
        pts[..src.len()].copy_from_slice(&src); // need a writable copy
        match verb {
            Verb::Line => {
                t.handle_line_mapped(ctm, &mut pts[1]);
            }
            Verb::Quad => {
                let mut q = [pts[0], pts[1], pts[2]];
                t.handle_quad_mapped(ctm, &mut q);
            }
            Verb::Cubic => {
                t.handle_cubic_mapped(ctm, &mut pts);
            }
            Verb::Conic => {
                let mut c = [pts[0], pts[1], pts[2]];
                t.handle_conic_mapped(ctm, &mut c, weight);
            }
            Verb::Move => {
                if verb_seen {
                    return false;
                }
            }
            Verb::Close => {
                close_seen = true;
            }
            Verb::Done => {}
        }
        verb_seen = true;
    }
    t.finish_path_polygon();
    true
}

/// The spot shadow tessellator (`SkSpotShadowTessellator`).
// Port of: src/utils/SkShadowTessellator.cpp#L835-L1019 (chrome/m156)
fn spot_tessellate(
    t: &mut BaseShadowTessellator,
    path: &Path,
    ctm: &Matrix,
    light_pos: Point3,
    light_radius: scalar,
    directional: bool,
) {
    // Compute the blur radius, scale and translation for the spot shadow.
    let Some((shadow_transform, outset)) = get_spot_shadow_transform(
        light_pos,
        light_radius,
        ctm,
        t.z_plane_params,
        path.bounds(),
        directional,
    ) else {
        return;
    };

    let inset = outset;

    // compute rough clip bounds for umbra, plus offset polygon, plus centroid
    if !compute_clip_and_path_polygons(t, path, ctm, &shadow_transform) {
        return;
    }

    if t.clip_polygon.len() < 3 || t.path_polygon.len() < 3 || !is_finite_all(t.area, &[]) {
        // We don't want to try to blur these cases, so we will return an empty SkVertices
        // instead.
        t.succeeded = true;
        return;
    }

    if t.is_convex {
        t.succeeded = t.compute_convex_shadow(inset, outset, true);
    } else {
        t.succeeded = t.compute_concave_shadow(inset, outset);
    }
    if !t.succeeded {
        return;
    }
    t.succeeded = true;
}

/// `computeClipAndPathPolygons` for the spot shadow.
// Port of: src/utils/SkShadowTessellator.cpp#L935-L1031 (chrome/m156)
fn compute_clip_and_path_polygons(
    t: &mut BaseShadowTessellator,
    path: &Path,
    ctm: &Matrix,
    shadow_transform: &Matrix,
) -> bool {
    // Walk around the path and compute clip polygon and path polygon.
    // Will also accumulate sum of areas for centroid.
    // For Bezier curves, we compute additional interior points on curve.
    // coefficients to compute cubic Bezier at t = 5/16
    const A: scalar = 0.324_951_17;
    const B: scalar = 0.443_115_23;
    const C: scalar = 0.201_416_02;
    const D: scalar = 0.030_517_578;

    let mut close_seen = false;
    let mut verb_seen = false;
    for (verb, src, weight) in collect_records(path) {
        if close_seen {
            return false;
        }
        let mut pts = [Point::default(); 4];
        pts[..src.len()].copy_from_slice(&src); // need a writable copy
        match verb {
            Verb::Line => {
                let clip0 = ctm.map_point(pts[1]);
                add_to_clip(t, clip0);
                t.handle_line_mapped(shadow_transform, &mut pts[1]);
            }
            Verb::Quad => {
                let mut clip = [Point::default(); 3];
                ctm.map_points(&mut clip, &pts[..3]);
                // point at t = 1/2
                let curve_point = Point::new(
                    0.25 * clip[0].x + 0.5 * clip[1].x + 0.25 * clip[2].x,
                    0.25 * clip[0].y + 0.5 * clip[1].y + 0.25 * clip[2].y,
                );
                add_to_clip(t, curve_point);
                add_to_clip(t, clip[2]);
                let mut q = [pts[0], pts[1], pts[2]];
                t.handle_quad_mapped(shadow_transform, &mut q);
            }
            Verb::Conic => {
                let mut clip = [Point::default(); 3];
                ctm.map_points(&mut clip, &pts[..3]);
                let w = weight;
                // point at t = 1/2
                let mut curve_point = Point::new(
                    0.25 * clip[0].x + w * 0.5 * clip[1].x + 0.25 * clip[2].x,
                    0.25 * clip[0].y + w * 0.5 * clip[1].y + 0.25 * clip[2].y,
                );
                curve_point *= scalar_invert(0.5 + 0.5 * w);
                add_to_clip(t, curve_point);
                add_to_clip(t, clip[2]);
                let mut c = [pts[0], pts[1], pts[2]];
                t.handle_conic_mapped(shadow_transform, &mut c, w);
            }
            Verb::Cubic => {
                let mut clip = [Point::default(); 4];
                ctm.map_points(&mut clip, &pts[..4]);
                // point at t = 5/16
                let curve_point = Point::new(
                    A * clip[0].x + B * clip[1].x + C * clip[2].x + D * clip[3].x,
                    A * clip[0].y + B * clip[1].y + C * clip[2].y + D * clip[3].y,
                );
                add_to_clip(t, curve_point);
                // point at t = 11/16
                let curve_point = Point::new(
                    D * clip[0].x + C * clip[1].x + B * clip[2].x + A * clip[3].x,
                    D * clip[0].y + C * clip[1].y + B * clip[2].y + A * clip[3].y,
                );
                add_to_clip(t, curve_point);
                add_to_clip(t, clip[3]);
                t.handle_cubic_mapped(shadow_transform, &mut pts);
            }
            Verb::Move => {
                if verb_seen {
                    return false;
                }
            }
            Verb::Close => {
                close_seen = true;
            }
            Verb::Done => {}
        }
        verb_seen = true;
    }
    t.finish_path_polygon();
    true
}

/// `addToClip`: appends `point` unless it duplicates the last clip point.
// Port of: src/utils/SkShadowTessellator.cpp#L1033-L1038 (chrome/m156)
fn add_to_clip(t: &mut BaseShadowTessellator, point: Point) {
    if t.clip_polygon.is_empty() || !duplicate_pt(point, t.clip_polygon[t.clip_polygon.len() - 1]) {
        t.clip_polygon.push(point);
    }
}

/// `SkShadowTessellator::MakeAmbient`: the ambient shadow mesh for `path`.
// Port of: src/utils/SkShadowTessellator.cpp#L1041-L1048 (chrome/m156)
#[must_use]
pub fn make_ambient(
    path: &Path,
    ctm: &Matrix,
    z_plane: Point3,
    transparent: bool,
) -> Option<Vertices> {
    if !ctm.map_rect(path.bounds()).0.is_finite() || !z_plane.is_finite() {
        return None;
    }
    let mut t = BaseShadowTessellator::new(z_plane, *path.bounds(), transparent);
    ambient_tessellate(&mut t, path, ctm);
    t.release_vertices()
}

/// `SkShadowTessellator::MakeSpot`: the spot shadow mesh for `path`.
// Port of: src/utils/SkShadowTessellator.cpp#L1050-L1066 (chrome/m156)
#[must_use]
#[allow(clippy::too_many_arguments)] // mirrors SkShadowTessellator::MakeSpot's signature
#[allow(clippy::neg_cmp_op_on_partial_ord)] // NaN-aware negations mirror the C++ checks
pub fn make_spot(
    path: &Path,
    ctm: &Matrix,
    z_plane: Point3,
    light_pos: Point3,
    light_radius: scalar,
    transparent: bool,
    directional: bool,
) -> Option<Vertices> {
    if !ctm.map_rect(path.bounds()).0.is_finite()
        || !z_plane.is_finite()
        || !light_pos.is_finite()
        || !(light_pos.z >= SCALAR_NEARLY_ZERO)
        || !is_finite_all(light_radius, &[])
        || !(light_radius >= SCALAR_NEARLY_ZERO)
    {
        return None;
    }
    let mut t = BaseShadowTessellator::new(z_plane, *path.bounds(), transparent);
    spot_tessellate(&mut t, path, ctm, light_pos, light_radius, directional);
    t.release_vertices()
}

/// The Ganesh curve subdivision (`GrPathUtils`) and the scalar Wang's formula (`skgpu::
/// wangs_formula`) with the identity vector transform, which the tessellator uses to pick how
/// many line segments to emit per curve.
mod gr_path_utils {
    use crate::floating_point::float_midpoint;
    use crate::point::{Point, point_priv};
    use crate::scalar::scalar;

    // Port of: src/gpu/tessellate/WangsFormula.h#L27-L30 (chrome/m156)
    fn length_term_p2(degree: i32, precision: scalar) -> scalar {
        ((degree * degree) * ((degree - 1) * (degree - 1))) as scalar / 64.0
            * (precision * precision)
    }

    // Port of: src/gpu/tessellate/WangsFormula.h#L70-L83 (chrome/m156) (nextlog2)
    fn nextlog2(x: scalar) -> i32 {
        const DIGITS_AFTER_BINARY_POINT: u32 = 23;
        if x <= 1.0 {
            return 0;
        }
        let mut bits = x.to_bits();
        bits = bits.wrapping_add((1u32 << DIGITS_AFTER_BINARY_POINT) - 1);
        let exp = ((bits >> DIGITS_AFTER_BINARY_POINT) & 0xFF) as i32 - 127;
        if exp > 0 { exp } else { 0 }
    }

    // Port of: src/gpu/tessellate/WangsFormula.h#L86-L88 (chrome/m156)
    fn nextlog16(x: scalar) -> i32 {
        (nextlog2(x) + 3) >> 2
    }

    // Port of: src/gpu/tessellate/WangsFormula.h#L184-L197 (chrome/m156)
    fn quadratic_p4(precision: scalar, p0: Point, p1: Point, p2: Point) -> scalar {
        let vx = ((-2.0 * p1.x) + p0.x) + p2.x;
        let vy = ((-2.0 * p1.y) + p0.y) + p2.y;
        let vv0 = vx * vx;
        let vv1 = vy * vy;
        (vv0 + vv1) * length_term_p2(2, precision)
    }

    // Port of: src/gpu/tessellate/WangsFormula.h#L199-L209 (chrome/m156)
    fn cubic_p4(precision: scalar, pts: &[Point]) -> scalar {
        let (p0, p1, p2, p3) = (pts[0], pts[1], pts[2], pts[3]);
        // lanes: v = -2*p12 + p01 + p23 with p01 = (p0,p1), p12 = (p1,p2), p23 = (p2,p3)
        let v0x = ((-2.0 * p1.x) + p0.x) + p2.x;
        let v0y = ((-2.0 * p1.y) + p0.y) + p2.y;
        let v1x = ((-2.0 * p2.x) + p1.x) + p3.x;
        let v1y = ((-2.0 * p2.y) + p1.y) + p3.y;
        let vv0 = v0x * v0x;
        let vv1 = v0y * v0y;
        let vv2 = v1x * v1x;
        let vv3 = v1y * v1y;
        let a = vv0 + vv1;
        let b = vv2 + vv3;
        let m = if a < b { b } else { a };
        m * length_term_p2(3, precision)
    }

    // Port of: src/gpu/ganesh/geometry/GrPathUtils.cpp#L26-L40 (chrome/m156)
    fn max_bezier_vertices(chop_count: u32) -> u32 {
        const MAX_CHOPS_PER_CURVE: u32 = 10;
        1 << chop_count.min(MAX_CHOPS_PER_CURVE)
    }

    // Port of: src/gpu/ganesh/geometry/GrPathUtils.cpp#L74-L77 (chrome/m156)
    pub(super) fn quadratic_point_count(points: &[Point], tol: scalar) -> u32 {
        let precision = 1.0 / tol;
        let log2 = nextlog16(quadratic_p4(precision, points[0], points[1], points[2]));
        max_bezier_vertices(log2 as u32)
    }

    // Port of: src/gpu/ganesh/geometry/GrPathUtils.cpp#L79-L102 (chrome/m156)
    pub(super) fn generate_quadratic_points(
        p0: Point,
        p1: Point,
        p2: Point,
        tol_sqd: scalar,
        out: &mut Vec<Point>,
        points_left: u32,
    ) -> u32 {
        if points_left < 2 || point_priv::distance_to_line_segment_between_sqd(p1, p0, p2) < tol_sqd
        {
            out.push(p2);
            return 1;
        }
        let q = [
            Point::new(float_midpoint(p0.x, p1.x), float_midpoint(p0.y, p1.y)),
            Point::new(float_midpoint(p1.x, p2.x), float_midpoint(p1.y, p2.y)),
        ];
        let r = Point::new(
            float_midpoint(q[0].x, q[1].x),
            float_midpoint(q[0].y, q[1].y),
        );
        let points_left = points_left >> 1;
        let a = generate_quadratic_points(p0, q[0], r, tol_sqd, out, points_left);
        let b = generate_quadratic_points(r, q[1], p2, tol_sqd, out, points_left);
        a + b
    }

    // Port of: src/gpu/ganesh/geometry/GrPathUtils.cpp#L104-L107 (chrome/m156)
    pub(super) fn cubic_point_count(points: &[Point], tol: scalar) -> u32 {
        let precision = 1.0 / tol;
        let log2 = nextlog16(cubic_p4(precision, points));
        max_bezier_vertices(log2 as u32)
    }

    // Port of: src/gpu/ganesh/geometry/GrPathUtils.cpp#L109-L137 (chrome/m156)
    pub(super) fn generate_cubic_points(
        p0: Point,
        p1: Point,
        p2: Point,
        p3: Point,
        tol_sqd: scalar,
        out: &mut Vec<Point>,
        points_left: u32,
    ) -> u32 {
        if points_left < 2
            || (point_priv::distance_to_line_segment_between_sqd(p1, p0, p3) < tol_sqd
                && point_priv::distance_to_line_segment_between_sqd(p2, p0, p3) < tol_sqd)
        {
            out.push(p3);
            return 1;
        }
        let q = [
            Point::new(float_midpoint(p0.x, p1.x), float_midpoint(p0.y, p1.y)),
            Point::new(float_midpoint(p1.x, p2.x), float_midpoint(p1.y, p2.y)),
            Point::new(float_midpoint(p2.x, p3.x), float_midpoint(p2.y, p3.y)),
        ];
        let r = [
            Point::new(
                float_midpoint(q[0].x, q[1].x),
                float_midpoint(q[0].y, q[1].y),
            ),
            Point::new(
                float_midpoint(q[1].x, q[2].x),
                float_midpoint(q[1].y, q[2].y),
            ),
        ];
        let s = Point::new(
            float_midpoint(r[0].x, r[1].x),
            float_midpoint(r[0].y, r[1].y),
        );
        let points_left = points_left >> 1;
        let a = generate_cubic_points(p0, q[0], r[0], s, tol_sqd, out, points_left);
        let b = generate_cubic_points(s, r[1], q[2], p3, tol_sqd, out, points_left);
        a + b
    }
}
