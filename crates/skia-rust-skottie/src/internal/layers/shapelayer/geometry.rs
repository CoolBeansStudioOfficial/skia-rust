// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/layers/shapelayer/Ellipse.cpp, Rectangle.cpp,
// Polystar.cpp, MergePaths.cpp, TrimPaths.cpp, RoundCorners.cpp, OffsetPaths.cpp, PuckerBloat.cpp
// (chrome/m156)

use std::cell::Cell;
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::floating_point::{float_degrees_to_radians, ieee_float_divide};
use skia_rust_core::geometry::convert_quad_to_cubic;
use skia_rust_core::m44::V2;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Join, Paint};
use skia_rust_core::path::{Iter as PathIter, Path as SkPath};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathDirection, PathVerb};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect as SkRRect;
use skia_rust_core::scalar::{
    SCALAR_PI, SCALAR_ROOT_2_OVER_2, Scalar, scalar_cos, scalar_floor_to_scalar,
    scalar_round_to_int, scalar_sin,
};
use skia_rust_core::t_pin::t_pin;
use skia_rust_effects::trim_path_effect::Mode as TrimMode;
use skia_rust_sksg::geometry_effect::{
    GeometryEffectState, geometry_effect_as_path, geometry_effect_clip, geometry_effect_contains,
    geometry_effect_draw, geometry_effect_revalidate, make_geometry_effect,
};
use skia_rust_sksg::node::{Node, NodeCore};
use skia_rust_sksg::util::scalar_changed;
use skia_rust_sksg::{
    GeometryNode, InvalidationController, Merge, MergeMode, MergeRec, OffsetEffect, Path as SgPath,
    RRect as SgRRect, RoundEffect, TrimEffect,
};

use crate::internal::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use crate::internal::skottie_priv::AnimationBuilder;
use crate::json::ObjectValue;
use crate::skottie::LoggerLevel;
use crate::skottie_json::parse_default;

use super::Geometries;

/// The shape layer builders (`ShapeBuilder`).
// Port of: modules/skottie/src/layers/shapelayer/ShapeLayer.h#L36-L87 (chrome/m156) (`class ShapeBuilder`)
#[doc(alias = "skottie::internal::ShapeBuilder")]
#[derive(Debug)]
pub struct ShapeBuilder;

/// Attaches an adapter to the builder, and returns the node it drives.
fn attach_adapter<A: AnimatablePropertyContainer + 'static, N: ?Sized>(
    abuilder: &AnimationBuilder<'_>,
    adapter: &Rc<A>,
    node: &Rc<N>,
) -> Rc<N> {
    abuilder.attach_discardable_adapter(adapter);
    Rc::clone(node)
}

/// Implements the traits of an adapter that has `base` and `sync`.
macro_rules! shape_adapter {
    ($ty:ty) => {
        impl AnimatablePropertyContainer for $ty {
            fn container(&self) -> &PropertyContainer {
                self.base.container()
            }

            fn on_sync(&self) {
                self.sync();
            }
        }

        crate::impl_container_animator!($ty);
    };
}
pub(super) use shape_adapter;

// ---- Ellipse ----

/// Drives an ellipse geometry.
// Port of: modules/skottie/src/layers/shapelayer/Ellipse.cpp#L22-L48 (chrome/m156) (`class EllipseGeometryAdapter`)
struct EllipseGeometryAdapter {
    base: DiscardableAdapterBase<SgRRect>,
    size: Prop<V2>,
    position: Prop<V2>,
}

impl EllipseGeometryAdapter {
    fn make(jellipse: &ObjectValue, abuilder: &AnimationBuilder<'_>) -> Rc<Self> {
        let adapter = Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), SgRRect::make_empty());
            base.node()
                .set_direction(if parse_default::<i32>(jellipse.get("d"), -1) == 3 {
                    PathDirection::CCW
                } else {
                    PathDirection::CW
                });
            base.node().set_initial_point_index(1); // starting point: (Center, Top)

            let size = Prop::new(V2::new(0.0, 0.0));
            let position = Prop::new(V2::new(0.0, 0.0)); // center
            base.container().bind(abuilder, jellipse.get("s"), &size);
            base.container()
                .bind(abuilder, jellipse.get("p"), &position);
            Self {
                base,
                size,
                position,
            }
        });
        adapter.base.container().shrink_to_fit();
        adapter
    }

    // Port of: modules/skottie/src/layers/shapelayer/Ellipse.cpp#L35-L42 (chrome/m156) (`onSync`)
    fn sync(&self) {
        let size = self.size.get();
        let position = self.position.get();
        let bounds = Rect::from_xywh(
            position.x - size.x / 2.0,
            position.y - size.y / 2.0,
            size.x,
            size.y,
        );

        self.base.node().set_rrect(SkRRect::new_oval(bounds));
    }
}

shape_adapter!(EllipseGeometryAdapter);

// ---- Rectangle ----

/// Drives a (rounded) rectangle geometry.
// Port of: modules/skottie/src/layers/shapelayer/Rectangle.cpp#L22-L52 (chrome/m156) (`class RectangleGeometryAdapter`)
struct RectangleGeometryAdapter {
    base: DiscardableAdapterBase<SgRRect>,
    size: Prop<V2>,
    position: Prop<V2>,
    roundness: Prop<f32>,
}

impl RectangleGeometryAdapter {
    fn make(jrect: &ObjectValue, abuilder: &AnimationBuilder<'_>) -> Rc<Self> {
        let adapter = Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), SgRRect::make_empty());
            base.node()
                .set_direction(if parse_default::<i32>(jrect.get("d"), -1) == 3 {
                    PathDirection::CCW
                } else {
                    PathDirection::CW
                });
            base.node().set_initial_point_index(2); // starting point: (Right, Top - radius.y)

            let size = Prop::new(V2::new(0.0, 0.0));
            let position = Prop::new(V2::new(0.0, 0.0)); // center
            let roundness = Prop::new(0.0);
            base.container().bind(abuilder, jrect.get("s"), &size);
            base.container().bind(abuilder, jrect.get("p"), &position);
            base.container().bind(abuilder, jrect.get("r"), &roundness);
            Self {
                base,
                size,
                position,
                roundness,
            }
        });
        adapter.base.container().shrink_to_fit();
        adapter
    }

    // Port of: modules/skottie/src/layers/shapelayer/Rectangle.cpp#L38-L46 (chrome/m156) (`onSync`)
    fn sync(&self) {
        let size = self.size.get();
        let position = self.position.get();
        let bounds = Rect::from_xywh(
            position.x - size.x / 2.0,
            position.y - size.y / 2.0,
            size.x,
            size.y,
        );

        let roundness = self.roundness.get();
        self.base
            .node()
            .set_rrect(SkRRect::new_rect_xy(bounds, roundness, roundness));
    }
}

shape_adapter!(RectangleGeometryAdapter);

// ---- Polystar ----

/// The type of a polystar.
// Port of: modules/skottie/src/layers/shapelayer/Polystar.cpp#L28-L30 (chrome/m156) (`PolystarGeometryAdapter::Type`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PolystarType {
    Star,
    Poly,
}

/// Drives a star or polygon geometry.
// Port of: modules/skottie/src/layers/shapelayer/Polystar.cpp#L24-L88 (chrome/m156) (`class PolystarGeometryAdapter`)
struct PolystarGeometryAdapter {
    base: DiscardableAdapterBase<SgPath>,
    poly_type: PolystarType,
    position: Prop<V2>,
    point_count: Prop<f32>,
    rotation: Prop<f32>,
    inner_radius: Prop<f32>,
    outer_radius: Prop<f32>,
    // TODO: inner/outer "roundness"? Bound, but unused, as in Skia.
    #[allow(dead_code)]
    inner_roundness: Prop<f32>,
    #[allow(dead_code)]
    outer_roundness: Prop<f32>,
}

impl PolystarGeometryAdapter {
    fn make(jstar: &ObjectValue, abuilder: &AnimationBuilder<'_>, t: PolystarType) -> Rc<Self> {
        let adapter = Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), SgPath::make_empty());
            let position = Prop::new(V2::new(0.0, 0.0));
            let point_count = Prop::new(0.0);
            let rotation = Prop::new(0.0);
            let inner_radius = Prop::new(0.0);
            let outer_radius = Prop::new(0.0);
            let inner_roundness = Prop::new(0.0);
            let outer_roundness = Prop::new(0.0);

            let c = base.container();
            c.bind(abuilder, jstar.get("pt"), &point_count);
            c.bind(abuilder, jstar.get("p"), &position);
            c.bind(abuilder, jstar.get("r"), &rotation);
            c.bind(abuilder, jstar.get("ir"), &inner_radius);
            c.bind(abuilder, jstar.get("or"), &outer_radius);
            c.bind(abuilder, jstar.get("is"), &inner_roundness);
            c.bind(abuilder, jstar.get("os"), &outer_roundness);

            Self {
                base,
                poly_type: t,
                position,
                point_count,
                rotation,
                inner_radius,
                outer_radius,
                inner_roundness,
                outer_roundness,
            }
        });
        adapter.base.container().shrink_to_fit();
        adapter
    }

    // Port of: modules/skottie/src/layers/shapelayer/Polystar.cpp#L51-L84 (chrome/m156) (`onSync`)
    fn sync(&self) {
        const MAX_POINT_COUNT: i32 = 100_000;
        let count = u32::try_from(t_pin(
            scalar_round_to_int(self.point_count.get()),
            0,
            MAX_POINT_COUNT,
        ))
        .expect("the count is pinned to a non-negative range");
        #[allow(clippy::cast_precision_loss)] // mirrors the implicit unsigned -> float
        let arc = ieee_float_divide(SCALAR_PI * 2.0, count as f32);

        let position = self.position.get();
        let pt_on_circle = |c: V2, r: f32, a: f32| {
            // skia-rust: libm (std::cos, std::sin)
            Point {
                x: c.x + r * scalar_cos(a),
                y: c.y + r * scalar_sin(a),
            }
        };

        let mut poly = PathBuilder::new();

        let mut angle = float_degrees_to_radians(self.rotation.get() - 90.0);
        poly.move_to(pt_on_circle(position, self.outer_radius.get(), angle));
        let reserve = if self.poly_type == PolystarType::Star {
            count * 2
        } else {
            count
        };
        let reserve = i32::try_from(reserve).unwrap_or(i32::MAX);
        poly.inc_reserve(reserve, reserve, 0);

        for _ in 0..count {
            if self.poly_type == PolystarType::Star {
                poly.line_to(pt_on_circle(
                    position,
                    self.inner_radius.get(),
                    angle + arc * 0.5,
                ));
            }
            angle += arc;
            poly.line_to(pt_on_circle(position, self.outer_radius.get(), angle));
        }

        poly.close();
        self.base.node().set_path(poly.detach());
    }
}

shape_adapter!(PolystarGeometryAdapter);

// ---- Trim ----

/// Drives a trim effect.
// Port of: modules/skottie/src/layers/shapelayer/TrimPaths.cpp#L22-L69 (chrome/m156) (`class TrimEffectAdapter`)
struct TrimEffectAdapter {
    base: DiscardableAdapterBase<TrimEffect>,
    start: Prop<f32>,
    end: Prop<f32>,
    offset: Prop<f32>,
}

impl TrimEffectAdapter {
    fn make(
        jtrim: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        child: Rc<dyn GeometryNode>,
    ) -> Rc<Self> {
        let adapter = Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(
                weak.clone(),
                TrimEffect::make(Some(child)).expect("the child is not null"),
            );
            let start = Prop::new(0.0);
            let end = Prop::new(100.0);
            let offset = Prop::new(0.0);
            base.container().bind(abuilder, jtrim.get("s"), &start);
            base.container().bind(abuilder, jtrim.get("e"), &end);
            base.container().bind(abuilder, jtrim.get("o"), &offset);
            Self {
                base,
                start,
                end,
                offset,
            }
        });
        adapter.base.container().shrink_to_fit();
        adapter
    }

    // Port of: modules/skottie/src/layers/shapelayer/TrimPaths.cpp#L38-L66 (chrome/m156) (`onSync`)
    fn sync(&self) {
        // BM semantics: start/end are percentages, offset is "degrees" (?!).
        let start = self.start.get() / 100.0;
        let end = self.end.get() / 100.0;
        let offset = self.offset.get() / 360.0;

        // std::min(a, b) is `(b < a) ? b : a`; std::max(a, b) is `(a < b) ? b : a`.
        let std_min = if end < start { end } else { start };
        let std_max = if start < end { end } else { start };
        let mut start_t = std_min + offset;
        let mut stop_t = std_max + offset;
        let mut mode = TrimMode::Normal;

        if stop_t - start_t < 1.0 {
            start_t -= scalar_floor_to_scalar(start_t);
            stop_t -= scalar_floor_to_scalar(stop_t);

            if start_t > stop_t {
                std::mem::swap(&mut start_t, &mut stop_t);
                mode = TrimMode::Inverted;
            }
        } else {
            start_t = 0.0;
            stop_t = 1.0;
        }

        self.base.node().set_start(start_t);
        self.base.node().set_stop(stop_t);
        self.base.node().set_mode(mode);
    }
}

shape_adapter!(TrimEffectAdapter);

// ---- Round corners ----

/// Drives a round corners effect.
// Port of: modules/skottie/src/layers/shapelayer/RoundCorners.cpp#L21-L42 (chrome/m156) (`class RoundCornersAdapter`)
struct RoundCornersAdapter {
    base: DiscardableAdapterBase<RoundEffect>,
    radius: Prop<f32>,
}

impl RoundCornersAdapter {
    fn make(
        jround: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        child: Rc<dyn GeometryNode>,
    ) -> Rc<Self> {
        let adapter = Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(
                weak.clone(),
                RoundEffect::make(Some(child)).expect("the child is not null"),
            );
            let radius = Prop::new(0.0);
            base.container().bind(abuilder, jround.get("r"), &radius);
            Self { base, radius }
        });
        adapter.base.container().shrink_to_fit();
        adapter
    }

    fn sync(&self) {
        self.base.node().set_radius(self.radius.get());
    }
}

shape_adapter!(RoundCornersAdapter);

// ---- Offset paths ----

const JOIN_MAP: [Join; 3] = [
    Join::Miter, // 'lj': 1
    Join::Round, // 'lj': 2
    Join::Bevel, // 'lj': 3
];

/// Drives an offset effect.
// Port of: modules/skottie/src/layers/shapelayer/OffsetPaths.cpp#L21-L51 (chrome/m156) (`class OffsetPathsAdapter`)
struct OffsetPathsAdapter {
    base: DiscardableAdapterBase<OffsetEffect>,
    amount: Prop<f32>,
    miter_limit: Prop<f32>,
}

impl OffsetPathsAdapter {
    fn make(
        joffset: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        child: Rc<dyn GeometryNode>,
    ) -> Rc<Self> {
        let adapter = Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(
                weak.clone(),
                OffsetEffect::make(Some(child)).expect("the child is not null"),
            );
            let join = parse_default::<i32>(joffset.get("lj"), 1) - 1;
            let index = usize::try_from(t_pin(join, 0, 2)).expect("pinned to 0..=2");
            base.node().set_join(JOIN_MAP[index]);

            let amount = Prop::new(0.0);
            let miter_limit = Prop::new(0.0);
            base.container().bind(abuilder, joffset.get("a"), &amount);
            base.container()
                .bind(abuilder, joffset.get("ml"), &miter_limit);
            Self {
                base,
                amount,
                miter_limit,
            }
        });
        adapter.base.container().shrink_to_fit();
        adapter
    }

    fn sync(&self) {
        self.base.node().set_offset(self.amount.get());
        self.base.node().set_miter_limit(self.miter_limit.get());
    }
}

shape_adapter!(OffsetPathsAdapter);

// ---- Pucker/bloat ----

/// `p0 + (p1 - p0) * t`.
// Port of: modules/skottie/src/layers/shapelayer/PuckerBloat.cpp#L27-L29 (chrome/m156) (`lerp`)
fn lerp(p0: Point, p1: Point, t: f32) -> Point {
    p0 + (p1 - p0) * t
}

/// Operates on the cubic representation of a shape. Pulls vertices towards the shape center, and
/// cubic control points away from the center. The general shape center is the vertex average.
// Port of: modules/skottie/src/layers/shapelayer/PuckerBloat.cpp#L31-L129 (chrome/m156) (`class PuckerBloatEffect`)
#[derive(Debug)]
struct PuckerBloatEffect {
    core: NodeCore,
    state: GeometryEffectState,
    /// Fraction of the transition to center. I.e.
    ///
    /// - 0: no effect
    /// - 1: vertices collapsed to center
    ///
    /// Negative values are allowed (inverse direction), as are extranormal values.
    amount: Cell<f32>,
}

/// `1 - 0.551915024494f`: <http://spencermortensen.com/articles/bezier-circle/>
const CUBIC_CIRCLE_COEFF: f32 = 1.0 - 0.551_915_05_f32;

/// The parameters of a `cubicTo()`.
struct CubicInfo {
    ctrl0: Point,
    ctrl1: Point,
    pt: Point,
}

impl PuckerBloatEffect {
    fn make(geo: &Rc<dyn GeometryNode>) -> Rc<Self> {
        make_geometry_effect(geo, |core, state| Self {
            core,
            state,
            amount: Cell::new(0.0),
        })
    }

    /// `SG_ATTRIBUTE(Amount, float, fAmount)`.
    fn set_amount(&self, amount: f32) {
        if scalar_changed(self.amount.get(), amount) {
            self.amount.set(amount);
            self.invalidate();
        }
    }

    // Port of: modules/skottie/src/layers/shapelayer/PuckerBloat.cpp#L53-L128 (chrome/m156) (`onRevalidateEffect`)
    fn revalidate_effect(&self, geo: &Rc<dyn GeometryNode>) -> SkPath {
        let amount = self.amount.get();

        let input = geo.as_path();
        if amount.nearly_zero(None) {
            return input;
        }

        let input_bounds = input.compute_tight_bounds();
        let center = Point {
            x: input_bounds.center_x(),
            y: input_bounds.center_y(),
        };

        let mut builder = PathBuilder::new();

        let mut contour_start = Point { x: 0.0, y: 0.0 };
        let mut cubics: Vec<CubicInfo> = Vec::new();

        let commit_contour =
            |builder: &mut PathBuilder, contour_start: Point, cubics: &mut Vec<CubicInfo>| {
                builder.move_to(lerp(contour_start, center, amount));
                for c in cubics.iter() {
                    builder.cubic_to(
                        lerp(c.ctrl0, center, -amount),
                        lerp(c.ctrl1, center, -amount),
                        lerp(c.pt, center, amount),
                    );
                }
                builder.close();

                cubics.clear();
            };

        // Normalize all verbs to cubic representation.
        let mut iter = PathIter::new(&input, true);
        while let Some(rec) = iter.next_rec() {
            let pts = rec.points();
            match rec.verb() {
                PathVerb::Move => {
                    commit_contour(&mut builder, contour_start, &mut cubics);
                    contour_start = pts[0];
                }
                PathVerb::Line => {
                    // Empirically, straight lines are treated as cubics with control points
                    // located length/100 away from extremities.
                    const CTRL_POS_FRACTION: f32 = 1.0 / 100.0;
                    let line_start = pts[0];
                    let line_end = pts[1];
                    cubics.push(CubicInfo {
                        ctrl0: lerp(line_start, line_end, CTRL_POS_FRACTION),
                        ctrl1: lerp(line_start, line_end, 1.0 - CTRL_POS_FRACTION),
                        pt: line_end,
                    });
                }
                PathVerb::Quad => {
                    let mut quad = [Point::default(); 4];
                    convert_quad_to_cubic(pts, &mut quad);
                    cubics.push(CubicInfo {
                        ctrl0: quad[1],
                        ctrl1: quad[2],
                        pt: quad[3],
                    });
                }
                PathVerb::Conic => {
                    // We should only ever encounter conics from circles/ellipses.
                    debug_assert!(f32::nearly_equal(
                        rec.conic_weight(),
                        SCALAR_ROOT_2_OVER_2,
                        None
                    ));

                    // http://spencermortensen.com/articles/bezier-circle/
                    let conic_start = cubics.last().map_or(contour_start, |c| c.pt);
                    let conic_end = pts[2];

                    cubics.push(CubicInfo {
                        ctrl0: lerp(pts[1], conic_start, CUBIC_CIRCLE_COEFF),
                        ctrl1: lerp(pts[1], conic_end, CUBIC_CIRCLE_COEFF),
                        pt: conic_end,
                    });
                }
                PathVerb::Cubic => {
                    cubics.push(CubicInfo {
                        ctrl0: pts[1],
                        ctrl1: pts[2],
                        pt: pts[3],
                    });
                }
                PathVerb::Close => {
                    commit_contour(&mut builder, contour_start, &mut cubics);
                }
            }
        }

        builder.detach()
    }
}

impl Drop for PuckerBloatEffect {
    fn drop(&mut self) {
        self.unobserve_inval(self.state.child().as_ref());
    }
}

impl Node for PuckerBloatEffect {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        geometry_effect_revalidate(&self.state, ic, ctm, |child, _| {
            self.revalidate_effect(child)
        })
    }
}

impl GeometryNode for PuckerBloatEffect {
    fn on_clip(&self, canvas: &Canvas, anti_alias: bool) {
        geometry_effect_clip(&self.state, canvas, anti_alias);
    }

    fn on_draw(&self, canvas: &Canvas, paint: &Paint) {
        geometry_effect_draw(&self.state, canvas, paint);
    }

    fn on_contains(&self, p: Point) -> bool {
        geometry_effect_contains(&self.state, p)
    }

    fn on_as_path(&self) -> SkPath {
        geometry_effect_as_path(&self.state)
    }
}

/// Drives a pucker/bloat effect.
// Port of: modules/skottie/src/layers/shapelayer/PuckerBloat.cpp#L131-L152 (chrome/m156) (`class PuckerBloatAdapter`)
struct PuckerBloatAdapter {
    base: DiscardableAdapterBase<PuckerBloatEffect>,
    amount: Prop<f32>,
}

impl PuckerBloatAdapter {
    fn make(
        joffset: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        child: &Rc<dyn GeometryNode>,
    ) -> Rc<Self> {
        let adapter = Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), PuckerBloatEffect::make(child));
            let amount = Prop::new(0.0);
            base.container().bind(abuilder, joffset.get("a"), &amount);
            Self { base, amount }
        });
        adapter.base.container().shrink_to_fit();
        adapter
    }

    fn sync(&self) {
        // AE amount is percentage-based.
        self.base.node().set_amount(self.amount.get() / 100.0);
    }
}

shape_adapter!(PuckerBloatAdapter);

// ---- ShapeBuilder ----

impl ShapeBuilder {
    /// Merges the geometries: the first one in `Merge` mode, the others in `mode`.
    // Port of: modules/skottie/src/layers/shapelayer/MergePaths.cpp#L23-L33 (chrome/m156) (`MergeGeometry`)
    #[doc(alias = "MergeGeometry")]
    #[must_use]
    pub fn merge_geometry(geos: Geometries, mode: MergeMode) -> Rc<Merge> {
        let mut merge_recs: Vec<MergeRec> = Vec::with_capacity(geos.len());

        for geo in geos {
            let rec_mode = if merge_recs.is_empty() {
                MergeMode::Merge
            } else {
                mode
            };
            merge_recs.push(MergeRec {
                geo,
                mode: rec_mode,
            });
        }

        Merge::make(&merge_recs)
    }

    /// Attaches a path geometry.
    // Port of: modules/skottie/src/layers/shapelayer/ShapeLayer.cpp#L162-L166 (chrome/m156) (`AttachPathGeometry`)
    #[doc(alias = "AttachPathGeometry")]
    #[must_use]
    pub fn attach_path_geometry(
        jpath: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
    ) -> Option<Rc<dyn GeometryNode>> {
        abuilder
            .attach_path(jpath.get("ks"))
            .map(|path| path as Rc<dyn GeometryNode>)
    }

    /// Attaches a rectangle geometry.
    // Port of: modules/skottie/src/layers/shapelayer/Rectangle.cpp#L54-L60 (chrome/m156) (`AttachRRectGeometry`)
    #[doc(alias = "AttachRRectGeometry")]
    #[must_use]
    pub fn attach_rrect_geometry(
        jrect: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
    ) -> Option<Rc<dyn GeometryNode>> {
        let adapter = RectangleGeometryAdapter::make(jrect, abuilder);
        Some(attach_adapter(abuilder, &adapter, adapter.base.node()) as Rc<dyn GeometryNode>)
    }

    /// Attaches an ellipse geometry.
    // Port of: modules/skottie/src/layers/shapelayer/Ellipse.cpp#L52-L58 (chrome/m156) (`AttachEllipseGeometry`)
    #[doc(alias = "AttachEllipseGeometry")]
    #[must_use]
    pub fn attach_ellipse_geometry(
        jellipse: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
    ) -> Option<Rc<dyn GeometryNode>> {
        let adapter = EllipseGeometryAdapter::make(jellipse, abuilder);
        Some(attach_adapter(abuilder, &adapter, adapter.base.node()) as Rc<dyn GeometryNode>)
    }

    /// Attaches a polystar geometry.
    // Port of: modules/skottie/src/layers/shapelayer/Polystar.cpp#L90-L106 (chrome/m156) (`AttachPolystarGeometry`)
    #[doc(alias = "AttachPolystarGeometry")]
    #[must_use]
    pub fn attach_polystar_geometry(
        jstar: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
    ) -> Option<Rc<dyn GeometryNode>> {
        const TYPES: [PolystarType; 2] = [
            PolystarType::Star, // "sy": 1
            PolystarType::Poly, // "sy": 2
        ];

        // size_t arithmetic: "sy": 0 wraps around, and is out of range.
        let poly_type = parse_default::<usize>(jstar.get("sy"), 0).wrapping_sub(1);
        if poly_type >= TYPES.len() {
            abuilder.log_json(LoggerLevel::Error, jstar, "Unknown polystar type.");
            return None;
        }

        let adapter = PolystarGeometryAdapter::make(jstar, abuilder, TYPES[poly_type]);
        Some(attach_adapter(abuilder, &adapter, adapter.base.node()) as Rc<dyn GeometryNode>)
    }

    /// Attaches the merge effect.
    // Port of: modules/skottie/src/layers/shapelayer/MergePaths.cpp#L35-L55 (chrome/m156) (`AttachMergeGeometryEffect`)
    #[doc(alias = "AttachMergeGeometryEffect")]
    #[must_use]
    pub fn attach_merge_geometry_effect(
        jmerge: &ObjectValue,
        _abuilder: &AnimationBuilder<'_>,
        geos: Geometries,
    ) -> Geometries {
        const MODES: [MergeMode; 5] = [
            MergeMode::Merge,      // "mm": 1
            MergeMode::Union,      // "mm": 2
            MergeMode::Difference, // "mm": 3
            MergeMode::Intersect,  // "mm": 4
            MergeMode::Xor,        // "mm": 5
        ];

        // size_t arithmetic: "mm": 0 wraps around, and pins to the last mode.
        let index = parse_default::<usize>(jmerge.get("mm"), 1).wrapping_sub(1);
        let mode = MODES[index.min(MODES.len() - 1)];

        vec![Self::merge_geometry(geos, mode) as Rc<dyn GeometryNode>]
    }

    /// Attaches the trim effect.
    // Port of: modules/skottie/src/layers/shapelayer/TrimPaths.cpp#L71-L109 (chrome/m156) (`AttachTrimGeometryEffect`)
    #[doc(alias = "AttachTrimGeometryEffect")]
    #[must_use]
    pub fn attach_trim_geometry_effect(
        jtrim: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        geos: Geometries,
    ) -> Geometries {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Mode {
            /// "m": 1 (Trim Multiple Shapes: Simultaneously)
            Parallel,
            /// "m": 2 (Trim Multiple Shapes: Individually)
            Serial,
        }
        const MODES: [Mode; 2] = [Mode::Parallel, Mode::Serial];

        // size_t arithmetic: "m": 0 wraps around, and pins to the last mode.
        let index = parse_default::<usize>(jtrim.get("m"), 1).wrapping_sub(1);
        let mode = MODES[index.min(MODES.len() - 1)];

        let inputs: Geometries = if mode == Mode::Serial {
            vec![Self::merge_geometry(geos, MergeMode::Merge) as Rc<dyn GeometryNode>]
        } else {
            geos
        };

        let mut trimmed: Geometries = Vec::with_capacity(inputs.len());

        for input in inputs {
            let adapter = TrimEffectAdapter::make(jtrim, abuilder, input);
            let node = attach_adapter(abuilder, &adapter, adapter.base.node());
            trimmed.push(node as Rc<dyn GeometryNode>);
        }

        trimmed
    }

    /// Attaches the round corners effect.
    // Port of: modules/skottie/src/layers/shapelayer/RoundCorners.cpp#L44-L60 (chrome/m156) (`AttachRoundGeometryEffect`)
    #[doc(alias = "AttachRoundGeometryEffect")]
    #[must_use]
    pub fn attach_round_geometry_effect(
        jround: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        geos: Geometries,
    ) -> Geometries {
        let mut rounded: Geometries = Vec::with_capacity(geos.len());

        for g in geos {
            let adapter = RoundCornersAdapter::make(jround, abuilder, g);
            let node = attach_adapter(abuilder, &adapter, adapter.base.node());
            rounded.push(node as Rc<dyn GeometryNode>);
        }

        rounded
    }

    /// Attaches the offset effect.
    // Port of: modules/skottie/src/layers/shapelayer/OffsetPaths.cpp#L53-L70 (chrome/m156) (`AttachOffsetGeometryEffect`)
    #[doc(alias = "AttachOffsetGeometryEffect")]
    #[must_use]
    pub fn attach_offset_geometry_effect(
        jround: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        geos: Geometries,
    ) -> Geometries {
        let mut offsetted: Geometries = Vec::with_capacity(geos.len());

        for g in geos {
            let adapter = OffsetPathsAdapter::make(jround, abuilder, g);
            let node = attach_adapter(abuilder, &adapter, adapter.base.node());
            offsetted.push(node as Rc<dyn GeometryNode>);
        }

        offsetted
    }

    /// Attaches the pucker/bloat effect.
    // Port of: modules/skottie/src/layers/shapelayer/PuckerBloat.cpp#L154-L170 (chrome/m156) (`AttachPuckerBloatGeometryEffect`)
    #[doc(alias = "AttachPuckerBloatGeometryEffect")]
    #[must_use]
    pub fn attach_pucker_bloat_geometry_effect(
        jround: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        geos: Geometries,
    ) -> Geometries {
        let mut bloated: Geometries = Vec::with_capacity(geos.len());

        for g in geos {
            let adapter = PuckerBloatAdapter::make(jround, abuilder, &g);
            let node = attach_adapter(abuilder, &adapter, adapter.base.node());
            bloated.push(node as Rc<dyn GeometryNode>);
        }

        bloated
    }
}
