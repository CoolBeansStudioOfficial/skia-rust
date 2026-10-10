// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/layers/shapelayer/Gradient.cpp (chrome/m156)

use std::rc::{Rc, Weak};

use skia_rust_core::color::Color4f;
use skia_rust_core::floating_point::{ieee_float_divide, is_nan};
use skia_rust_core::m44::V2;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::safe_math::SafeMath;
use skia_rust_core::scalar::SCALAR_NEARLY_ZERO;
use skia_rust_core::t_pin::t_pin;
use skia_rust_sksg::gradient::ColorStop;
use skia_rust_sksg::{LinearGradient, PaintNode, RadialGradient, ShaderNode, ShaderPaint};

use crate::internal::animator::{AnimatablePropertyContainer, Prop, PropertyContainer};
use crate::internal::skottie_priv::AnimationBuilder;
use crate::json::ObjectValue;
use crate::skottie_json::{ValueExt, parse_default};
use crate::skottie_value::VectorValue;

use super::geometry::ShapeBuilder;

/// The kinds of gradients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GradientType {
    Linear,
    Radial,
}

/// The gradient node: a linear or a radial one (`sksg::Gradient`).
enum GradientNode {
    Linear(Rc<LinearGradient>),
    Radial(Rc<RadialGradient>),
}

/// A color stop record of the stops vector: position and RGB.
// Port of: modules/skottie/src/layers/shapelayer/Gradient.cpp#L140 (chrome/m156) (`ColorRec`)
#[derive(Clone, Copy)]
struct ColorRec {
    t: f32,
    r: f32,
    g: f32,
    b: f32,
}

/// An opacity stop record of the stops vector: position and alpha.
// Port of: modules/skottie/src/layers/shapelayer/Gradient.cpp#L141 (chrome/m156) (`OpacityRec`)
#[derive(Clone, Copy)]
struct OpacityRec {
    t: f32,
    a: f32,
}

/// Drives a gradient node.
// Port of: modules/skottie/src/layers/shapelayer/Gradient.cpp#L28-L255 (chrome/m156) (`class GradientAdapter`)
pub(super) struct GradientAdapter {
    container: PropertyContainer,
    gradient: GradientNode,
    gradient_type: GradientType,
    stop_count: usize,

    stops: Prop<VectorValue>,
    start_point: Prop<V2>,
    end_point: Prop<V2>,
    highlight_length: Prop<f32>,
    highlight_angle: Prop<f32>,
}

impl GradientAdapter {
    // Port of: modules/skottie/src/layers/shapelayer/Gradient.cpp#L31-L53 (chrome/m156) (`Make`)
    pub(super) fn make(jgrad: &ObjectValue, abuilder: &AnimationBuilder<'_>) -> Option<Rc<Self>> {
        let jstops = jgrad.get("g").as_object()?;

        let stop_count = parse_default::<i32>(jstops.get("p"), -1);
        if stop_count < 0 {
            return None;
        }

        let gradient_type = if parse_default::<i32>(jgrad.get("t"), 1) == 1 {
            GradientType::Linear
        } else {
            GradientType::Radial
        };
        let gradient = match gradient_type {
            GradientType::Linear => GradientNode::Linear(LinearGradient::make()),
            GradientType::Radial => GradientNode::Radial(RadialGradient::make()),
        };

        let stop_count = usize::try_from(stop_count).expect("checked to be non-negative");
        let adapter = Rc::new_cyclic(|weak: &Weak<Self>| {
            let container = PropertyContainer::new(weak.clone());
            let stops = Prop::new(VectorValue::new());
            let start_point = Prop::new(V2::new(0.0, 0.0));
            let end_point = Prop::new(V2::new(0.0, 0.0));
            let highlight_length = Prop::new(0.0);
            let highlight_angle = Prop::new(0.0);

            container.bind(abuilder, jgrad.get("s"), &start_point);
            container.bind(abuilder, jgrad.get("e"), &end_point);
            container.bind(abuilder, jgrad.get("h"), &highlight_length);
            container.bind(abuilder, jgrad.get("a"), &highlight_angle);
            container.bind(abuilder, jstops.get("k"), &stops);

            Self {
                container,
                gradient,
                gradient_type,
                stop_count,
                stops,
                start_point,
                end_point,
                highlight_length,
                highlight_angle,
            }
        });
        adapter.container.shrink_to_fit();
        Some(adapter)
    }

    /// The gradient as a shader node.
    pub(super) fn node(&self) -> Rc<dyn ShaderNode> {
        match &self.gradient {
            GradientNode::Linear(grad) => Rc::clone(grad) as Rc<dyn ShaderNode>,
            GradientNode::Radial(grad) => Rc::clone(grad) as Rc<dyn ShaderNode>,
        }
    }

    fn set_color_stops(&self, stops: Vec<ColorStop>) {
        match &self.gradient {
            GradientNode::Linear(grad) => grad.set_color_stops(stops),
            GradientNode::Radial(grad) => grad.set_color_stops(stops),
        }
    }

    // Port of: modules/skottie/src/layers/shapelayer/Gradient.cpp#L66-L247 (chrome/m156) (`onSync`)
    #[allow(clippy::too_many_lines)] // one function in Skia, ported as written
    fn sync(&self) {
        let start_point = self.start_point.get();
        let end_point = self.end_point.get();
        let s_point = Point {
            x: start_point.x,
            y: start_point.y,
        };
        let e_point = Point {
            x: end_point.x,
            y: end_point.y,
        };

        match (&self.gradient, self.gradient_type) {
            (GradientNode::Linear(grad), GradientType::Linear) => {
                grad.set_start_point(s_point);
                grad.set_end_point(e_point);
            }
            (GradientNode::Radial(grad), GradientType::Radial) => {
                // The highlight parameters control the location of the actual gradient start
                // point (equivalent to SVG's radial gradient focal point
                //  https://www.w3.org/TR/SVG11/pservers.html#RadialGradients)
                //
                //   - highlight length determines the position along the |s_point -> e_point|
                //     vector, where 0% corresponds to s_point and 100% corresponds to e_point.
                //   - highlight angle rotates the point around s_point
                //
                let rotated_e_point = Matrix::rotate_deg_pivot(self.highlight_angle.get(), s_point)
                    .map_point(e_point);

                // The valid range for length is [-100% .. 100%], where negative values mirror the
                // positive interval relative to s_point.
                //
                // Edge case: for exactly -100% and 100%, the focal point lies on the end circle
                // and this triggers specific SVG behavior (see
                // SkConicalGrdient::FocalData::isFocalOnCircle and friends), which does not match
                // AE's semantics. To avoid that, we clamp by an epsilon value.
                let eps = SCALAR_NEARLY_ZERO * 2.0;
                let h_len = t_pin(self.highlight_length.get() * 0.01, -1.0 + eps, 1.0 - eps);

                let focal_point = s_point + (rotated_e_point - s_point) * h_len;

                grad.set_start_center(focal_point);
                grad.set_end_center(s_point);
                grad.set_start_radius(0.0);
                grad.set_end_radius(Point::distance(s_point, rotated_e_point));
            }
            _ => unreachable!("the gradient node matches the gradient type"),
        }

        // Gradient color stops are specified as a consolidated float vector holding:
        //
        //   a) an (optional) array of color/RGB stop records (t, r, g, b)
        //
        // followed by
        //
        //   b) an (optional) array of opacity/alpha stop records (t, a)
        //
        // The number of color records is explicit (fColorStopCount),
        // while the number of opacity stops is implicit (based on the size of fStops).
        //
        // |fStops| holds ColorRec x |fColorStopCount| + OpacityRec x N
        let stops = self.stops.borrow();
        let mut safe = SafeMath::new();
        let c_count = self.stop_count;
        let c_size = safe.mul(c_count, 4);
        let o_count = safe.sub(stops.len(), c_size) / 2;
        let o_size = safe.mul(o_count, 2);
        let total_size = safe.add(c_size, o_size);
        if !safe.ok() || stops.len() != total_size {
            // apply() may get called before the stops are set, so only log when we have some
            // stops.
            if !stops.is_empty() {
                eprintln!("!! Invalid gradient stop array size: {}", stops.len());
            }
            return;
        }

        let color_rec = |i: usize| ColorRec {
            t: stops[i * 4],
            r: stops[i * 4 + 1],
            g: stops[i * 4 + 2],
            b: stops[i * 4 + 3],
        };
        let opacity_rec = |i: usize| OpacityRec {
            t: stops[c_size + i * 2],
            a: stops[c_size + i * 2 + 1],
        };

        let mut c_rec: Option<usize> = if c_count > 0 { Some(0) } else { None };
        let mut o_rec: Option<usize> = if o_count > 0 { Some(0) } else { None };

        // ColorStop current_stop
        let mut current_position = 0.0_f32;
        let mut current_color = Color4f {
            r: c_rec.map_or(0.0, |i| color_rec(i).r),
            g: c_rec.map_or(0.0, |i| color_rec(i).g),
            b: c_rec.map_or(0.0, |i| color_rec(i).b),
            a: o_rec.map_or(1.0, |i| opacity_rec(i).a),
        };

        let mut color_stops: Vec<ColorStop> = Vec::with_capacity(c_count);

        // Merge-sort the color and opacity stops, LERP-ing intermediate channel values as needed.
        while c_rec.is_some() || o_rec.is_some() {
            // After exhausting one of color recs / opacity recs, continue propagating the last
            // computed values (as if they were specified at the current position).
            let cs = match c_rec {
                Some(i) => color_rec(i),
                None => ColorRec {
                    t: opacity_rec(o_rec.expect("one of the records is present")).t,
                    r: current_color.r,
                    g: current_color.g,
                    b: current_color.b,
                },
            };
            let os = match o_rec {
                Some(i) => opacity_rec(i),
                None => OpacityRec {
                    t: color_rec(c_rec.expect("one of the records is present")).t,
                    a: current_color.a,
                },
            };

            // Compute component lerp coefficients based on the relative position of the stops
            // being considered. The idea is to select the smaller-pos stop, use its own
            // properties as specified (lerp with t == 1), and lerp (with t < 1) the properties
            // from the larger-pos stop against the previously computed gradient stop values.
            //
            // std::max(a, b) is `(a < b) ? b : a`.
            let std_max = |a: f32, b: f32| if a < b { b } else { a };
            let c_pos = std_max(cs.t, current_position);
            let o_pos = std_max(os.t, current_position);
            let c_pos_rel = c_pos - current_position;
            let o_pos_rel = o_pos - current_position;
            let t_c = t_pin(ieee_float_divide(o_pos_rel, c_pos_rel), 0.0, 1.0);
            let t_o = t_pin(ieee_float_divide(c_pos_rel, o_pos_rel), 0.0, 1.0);

            let lerp = |a: f32, b: f32, t: f32| a + t * (b - a);

            // std::min(a, b) is `(b < a) ? b : a`.
            current_position = if o_pos < c_pos { o_pos } else { c_pos };
            current_color = Color4f {
                r: lerp(current_color.r, cs.r, t_c),
                g: lerp(current_color.g, cs.g, t_c),
                b: lerp(current_color.b, cs.b, t_c),
                a: lerp(current_color.a, os.a, t_o),
            };
            color_stops.push(ColorStop {
                position: current_position,
                color: current_color,
            });

            // Consume one of, or both (for coincident positions) color/opacity stops.
            if is_nan(c_pos) || c_pos <= o_pos {
                c_rec = next_rec(c_rec, c_count);
            }
            if is_nan(o_pos) || o_pos <= c_pos {
                o_rec = next_rec(o_rec, o_count);
            }
        }

        color_stops.shrink_to_fit();
        self.set_color_stops(color_stops);
    }
}

/// The record after `rec`, if there is one before `end_rec` (`next_rec`).
// Port of: modules/skottie/src/layers/shapelayer/Gradient.cpp#L249-L257 (chrome/m156)
fn next_rec(rec: Option<usize>, end_rec: usize) -> Option<usize> {
    let rec = rec?;

    debug_assert!(rec < end_rec);
    let rec = rec + 1;

    if rec < end_rec { Some(rec) } else { None }
}

impl AnimatablePropertyContainer for GradientAdapter {
    fn container(&self) -> &PropertyContainer {
        &self.container
    }

    fn on_sync(&self) {
        self.sync();
    }
}

crate::impl_container_animator!(GradientAdapter);

impl ShapeBuilder {
    /// Attaches a gradient fill.
    // Port of: modules/skottie/src/layers/shapelayer/Gradient.cpp#L261-L268 (chrome/m156) (`AttachGradientFill`)
    #[doc(alias = "AttachGradientFill")]
    #[must_use]
    pub fn attach_gradient_fill(
        jgrad: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
    ) -> Option<Rc<dyn PaintNode>> {
        let adapter = GradientAdapter::make(jgrad, abuilder)?;

        Some(Self::attach_fill(
            jgrad,
            abuilder,
            ShaderPaint::make(Some(adapter.node()))? as Rc<dyn PaintNode>,
            None,
            Some(adapter as Rc<dyn AnimatablePropertyContainer>),
        ))
    }

    /// Attaches a gradient stroke.
    // Port of: modules/skottie/src/layers/shapelayer/Gradient.cpp#L270-L277 (chrome/m156) (`AttachGradientStroke`)
    #[doc(alias = "AttachGradientStroke")]
    #[must_use]
    pub fn attach_gradient_stroke(
        jgrad: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
    ) -> Option<Rc<dyn PaintNode>> {
        let adapter = GradientAdapter::make(jgrad, abuilder)?;

        Some(Self::attach_stroke(
            jgrad,
            abuilder,
            ShaderPaint::make(Some(adapter.node()))? as Rc<dyn PaintNode>,
            None,
            Some(adapter as Rc<dyn AnimatablePropertyContainer>),
        ))
    }
}
