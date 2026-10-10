// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/layers/shapelayer/ShapeLayer.h, ShapeLayer.cpp
// (chrome/m156)

use std::rc::Rc;

use skia_rust_core::path_types::PathFillType;
use skia_rust_sksg::{
    Draw, FillTypeOverride, GeometryNode, GeometryTransform, Group, MergeMode, PaintNode,
    RenderNode, Transform, TransformEffect,
};

use crate::internal::skottie_priv::{AnimationBuilder, AutoPropertyTracker, AutoScope};
use crate::json::{ArrayValue, ObjectValue};
use crate::skottie::{LayerInfo, LoggerLevel};
use crate::skottie_json::{ValueExt, parse_default, string_text};
use crate::skottie_property::NodeType;

mod fill_stroke;
mod geometry;
mod gradient;
mod repeater;

pub use geometry::ShapeBuilder;

/// A list of geometry nodes.
type Geometries = Vec<Rc<dyn GeometryNode>>;
/// A list of render nodes.
type Draws = Vec<Rc<dyn RenderNode>>;

/// Attaches a geometry node for a shape.
type GeometryAttacher = fn(&ObjectValue, &AnimationBuilder<'_>) -> Option<Rc<dyn GeometryNode>>;
/// Attaches a geometry effect to the geometries.
type GeometryEffectAttacher = fn(&ObjectValue, &AnimationBuilder<'_>, Geometries) -> Geometries;
/// Attaches a paint node for a shape.
type PaintAttacher = fn(&ObjectValue, &AnimationBuilder<'_>) -> Option<Rc<dyn PaintNode>>;
/// Attaches a draw effect to the draws.
type DrawEffectAttacher = fn(&ObjectValue, &AnimationBuilder<'_>, Draws) -> Draws;

// Port of: modules/skottie/src/layers/shapelayer/ShapeLayer.cpp#L40-L47 (chrome/m156) (`gGeometryAttachers`)
const GEOMETRY_ATTACHERS: [GeometryAttacher; 4] = [
    ShapeBuilder::attach_path_geometry,
    ShapeBuilder::attach_rrect_geometry,
    ShapeBuilder::attach_ellipse_geometry,
    ShapeBuilder::attach_polystar_geometry,
];

// Port of: modules/skottie/src/layers/shapelayer/ShapeLayer.cpp#L49-L59 (chrome/m156) (`gGeometryEffectAttachers`)
const GEOMETRY_EFFECT_ATTACHERS: [GeometryEffectAttacher; 5] = [
    ShapeBuilder::attach_merge_geometry_effect,
    ShapeBuilder::attach_trim_geometry_effect,
    ShapeBuilder::attach_round_geometry_effect,
    ShapeBuilder::attach_offset_geometry_effect,
    ShapeBuilder::attach_pucker_bloat_geometry_effect,
];

// Port of: modules/skottie/src/layers/shapelayer/ShapeLayer.cpp#L61-L68 (chrome/m156) (`gPaintAttachers`)
const PAINT_ATTACHERS: [PaintAttacher; 4] = [
    ShapeBuilder::attach_color_fill,
    ShapeBuilder::attach_color_stroke,
    ShapeBuilder::attach_gradient_fill,
    ShapeBuilder::attach_gradient_stroke,
];

// Some paint types (looking at you dashed-stroke) mess with the local geometry.
// Port of: modules/skottie/src/layers/shapelayer/ShapeLayer.cpp#L70-L77 (chrome/m156) (`gPaintGeometryAdjusters`)
const PAINT_GEOMETRY_ADJUSTERS: [Option<GeometryEffectAttacher>; 4] = [
    None,                                       // color fill
    Some(ShapeBuilder::adjust_stroke_geometry), // color stroke
    None,                                       // gradient fill
    Some(ShapeBuilder::adjust_stroke_geometry), // gradient stroke
];

// Port of: modules/skottie/src/layers/shapelayer/ShapeLayer.cpp#L79-L85 (chrome/m156) (`gDrawEffectAttachers`)
const DRAW_EFFECT_ATTACHERS: [DrawEffectAttacher; 1] = [ShapeBuilder::attach_repeater_draw_effect];

// Port of: modules/skottie/src/layers/shapelayer/ShapeLayer.cpp#L87-L94 (chrome/m156) (`ShapeType`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapeType {
    Geometry,
    GeometryEffect,
    Paint,
    Group,
    Transform,
    DrawEffect,
}

// Port of: modules/skottie/src/layers/shapelayer/ShapeLayer.cpp#L96-L99 (chrome/m156) (`ShapeFlags`)
const NONE: u16 = 0x00;
const SUPPRESS_DRAWS: u16 = 0x01;

// Port of: modules/skottie/src/layers/shapelayer/ShapeLayer.cpp#L101-L106 (chrome/m156) (`ShapeInfo`)
#[derive(Debug)]
struct ShapeInfo {
    type_string: &'static str,
    shape_type: ShapeType,
    /// Index into respective attacher tables.
    attacher_index: usize,
    flags: u16,
}

// Alphabetized for binary search lookup.
static SHAPE_INFO: [ShapeInfo; 16] = [
    ShapeInfo {
        type_string: "el",
        shape_type: ShapeType::Geometry,
        attacher_index: 2,
        flags: NONE,
    }, // ellipse
    ShapeInfo {
        type_string: "fl",
        shape_type: ShapeType::Paint,
        attacher_index: 0,
        flags: NONE,
    }, // fill
    ShapeInfo {
        type_string: "gf",
        shape_type: ShapeType::Paint,
        attacher_index: 2,
        flags: NONE,
    }, // gfill
    ShapeInfo {
        type_string: "gr",
        shape_type: ShapeType::Group,
        attacher_index: 0,
        flags: NONE,
    }, // group
    ShapeInfo {
        type_string: "gs",
        shape_type: ShapeType::Paint,
        attacher_index: 3,
        flags: NONE,
    }, // gstroke
    ShapeInfo {
        type_string: "mm",
        shape_type: ShapeType::GeometryEffect,
        attacher_index: 0,
        flags: SUPPRESS_DRAWS,
    }, // merge
    ShapeInfo {
        type_string: "op",
        shape_type: ShapeType::GeometryEffect,
        attacher_index: 3,
        flags: NONE,
    }, // offset
    ShapeInfo {
        type_string: "pb",
        shape_type: ShapeType::GeometryEffect,
        attacher_index: 4,
        flags: NONE,
    }, // pucker/bloat
    ShapeInfo {
        type_string: "rc",
        shape_type: ShapeType::Geometry,
        attacher_index: 1,
        flags: NONE,
    }, // rrect
    ShapeInfo {
        type_string: "rd",
        shape_type: ShapeType::GeometryEffect,
        attacher_index: 2,
        flags: NONE,
    }, // round
    ShapeInfo {
        type_string: "rp",
        shape_type: ShapeType::DrawEffect,
        attacher_index: 0,
        flags: NONE,
    }, // repeater
    ShapeInfo {
        type_string: "sh",
        shape_type: ShapeType::Geometry,
        attacher_index: 0,
        flags: NONE,
    }, // shape
    ShapeInfo {
        type_string: "sr",
        shape_type: ShapeType::Geometry,
        attacher_index: 3,
        flags: NONE,
    }, // polystar
    ShapeInfo {
        type_string: "st",
        shape_type: ShapeType::Paint,
        attacher_index: 1,
        flags: NONE,
    }, // stroke
    ShapeInfo {
        type_string: "tm",
        shape_type: ShapeType::GeometryEffect,
        attacher_index: 1,
        flags: NONE,
    }, // trim
    ShapeInfo {
        type_string: "tr",
        shape_type: ShapeType::Transform,
        attacher_index: 0,
        flags: NONE,
    }, // transform
];

// Port of: modules/skottie/src/layers/shapelayer/ShapeLayer.cpp#L108-L141 (chrome/m156) (`FindShapeInfo`)
fn find_shape_info(jshape: &ObjectValue) -> Option<&'static ShapeInfo> {
    let ty = jshape.get("ty").as_string()?;
    let key = string_text(ty);

    SHAPE_INFO
        .binary_search_by(|info| info.type_string.cmp(key.as_str()))
        .ok()
        .map(|i| &SHAPE_INFO[i])
}

// Port of: modules/skottie/src/layers/shapelayer/ShapeLayer.cpp#L143-L146 (chrome/m156) (`GeometryEffectRec`)
struct GeometryEffectRec<'a> {
    json: &'a ObjectValue,
    attach: GeometryEffectAttacher,
}

/// Unlike Skia, Lottie specifies the fill rule on paint, not on geometry. This is the transfer
/// point, where we relocate the property to the geometry node as a local wrapper.
// Port of: modules/skottie/src/layers/shapelayer/ShapeLayer.cpp#L148-L160 (chrome/m156) (`AdjustGeometryFillRule`)
fn adjust_geometry_fill_rule(
    geo: Rc<dyn GeometryNode>,
    jpaint: &ObjectValue,
) -> Rc<dyn GeometryNode> {
    const FILL_TYPES: [PathFillType; 2] = [
        PathFillType::Winding, // "r": 1
        PathFillType::EvenOdd, // "r": 2
    ];
    // size_t arithmetic: "r": 0 wraps around, and pins to the last fill type.
    let r = parse_default::<usize>(jpaint.get("r"), 1).wrapping_sub(1);
    let ft = FILL_TYPES[r.min(FILL_TYPES.len() - 1)];
    // `ft == SkPathFillType::kDefault`: the winding rule is the default.
    if ft == PathFillType::Winding {
        geo
    } else {
        FillTypeOverride::make(Some(geo), ft).expect("the geometry is not null")
    }
}

/// The state shared by the nested `attach_shape` calls.
// Port of: modules/skottie/src/layers/shapelayer/ShapeLayer.cpp#L168-L180 (chrome/m156) (`AttachShapeContext`)
struct AttachShapeContext<'a, 'b> {
    geometry_stack: &'a mut Geometries,
    geometry_effect_stack: &'a mut Vec<GeometryEffectRec<'b>>,
    committed_animators: usize,
}

struct ShapeRec<'a> {
    json: &'a ObjectValue,
    info: &'static ShapeInfo,
    suppressed: bool,
}

impl AnimationBuilder<'_> {
    // Port of: modules/skottie/src/layers/shapelayer/ShapeLayer.cpp#L182-L359 (chrome/m156) (`attachShape`)
    #[allow(clippy::too_many_lines)] // one function in Skia, ported as written
    fn attach_shape<'a>(
        &self,
        jshape: Option<&'a ArrayValue>,
        ctx: &mut AttachShapeContext<'_, 'a>,
        mut suppress_draws: bool,
    ) -> Option<Rc<dyn RenderNode>> {
        let jshape = jshape?;

        let initial_geometry_effects = ctx.geometry_effect_stack.len();

        let mut jtransform: Option<&ObjectValue> = None;

        // First pass (bottom->top):
        //
        //   * pick up the group transform and opacity
        //   * push local geometry effects onto the stack
        //   * store recs for next pass
        //
        let mut recs: Vec<ShapeRec<'a>> = Vec::new();
        for i in 0..jshape.size() {
            let Some(shape) = jshape[jshape.size() - 1 - i].as_object() else {
                continue;
            };

            let Some(info) = find_shape_info(shape) else {
                self.log_json(LoggerLevel::Error, shape.get("ty"), "Unknown shape.");
                continue;
            };

            if parse_default::<bool>(shape.get("hd"), false) {
                // Ignore hidden shapes.
                continue;
            }

            recs.push(ShapeRec {
                json: shape,
                info,
                suppressed: suppress_draws,
            });

            // Some effects (merge) suppress any paints above them.
            suppress_draws |= (info.flags & SUPPRESS_DRAWS) != 0;

            match info.shape_type {
                ShapeType::Transform => {
                    // Just track the transform property for now -- we'll deal with it later.
                    jtransform = Some(shape);
                }
                ShapeType::GeometryEffect => {
                    debug_assert!(info.attacher_index < GEOMETRY_EFFECT_ATTACHERS.len());
                    ctx.geometry_effect_stack.push(GeometryEffectRec {
                        json: shape,
                        attach: GEOMETRY_EFFECT_ATTACHERS[info.attacher_index],
                    });
                }
                _ => {}
            }
        }

        // Second pass (top -> bottom, after 2x reverse):
        //
        //   * track local geometry
        //   * emit local paints
        //
        let mut geos: Geometries = Vec::new();
        let mut draws: Draws = Vec::new();

        let add_draw = |draws: &mut Draws, draw: Rc<dyn RenderNode>, rec: &ShapeRec<'_>| {
            // All draws can have an optional blend mode.
            if let Some(draw) = self.attach_blend_mode(rec.json, Some(draw)) {
                draws.push(draw);
            }
        };

        for rec in recs.iter().rev() {
            let _apt = AutoPropertyTracker::new(self, rec.json, NodeType::Other);

            match rec.info.shape_type {
                ShapeType::Geometry => {
                    debug_assert!(rec.info.attacher_index < GEOMETRY_ATTACHERS.len());
                    if let Some(geo) = GEOMETRY_ATTACHERS[rec.info.attacher_index](rec.json, self) {
                        geos.push(geo);
                    }
                }
                ShapeType::GeometryEffect => {
                    // Apply the current effect and pop from the stack.
                    debug_assert!(rec.info.attacher_index < GEOMETRY_EFFECT_ATTACHERS.len());
                    if !geos.is_empty() {
                        geos = GEOMETRY_EFFECT_ATTACHERS[rec.info.attacher_index](
                            rec.json,
                            self,
                            std::mem::take(&mut geos),
                        );
                    }

                    debug_assert!(std::ptr::eq(
                        ctx.geometry_effect_stack.last().map(|r| r.json).unwrap(),
                        rec.json
                    ));
                    ctx.geometry_effect_stack.pop();
                }
                ShapeType::Group => {
                    let mut group_shape_ctx = AttachShapeContext {
                        geometry_stack: &mut geos,
                        geometry_effect_stack: &mut *ctx.geometry_effect_stack,
                        committed_animators: ctx.committed_animators,
                    };
                    if let Some(subgroup) = self.attach_shape(
                        rec.json.get("it").as_array(),
                        &mut group_shape_ctx,
                        rec.suppressed,
                    ) {
                        let committed = group_shape_ctx.committed_animators;
                        add_draw(&mut draws, subgroup, rec);
                        debug_assert!(committed >= ctx.committed_animators);
                        ctx.committed_animators = committed;
                    }
                }
                ShapeType::Paint => {
                    debug_assert!(rec.info.attacher_index < PAINT_ATTACHERS.len());
                    let paint = PAINT_ATTACHERS[rec.info.attacher_index](rec.json, self);
                    let Some(paint) = paint else {
                        continue;
                    };
                    if geos.is_empty() || rec.suppressed {
                        continue;
                    }

                    let mut draw_geos = geos.clone();

                    // Apply all pending effects from the stack.
                    for it in ctx.geometry_effect_stack.iter().rev() {
                        draw_geos = (it.attach)(it.json, self, draw_geos);
                    }

                    // Apply local paint geometry adjustments (e.g. dashing).
                    debug_assert!(rec.info.attacher_index < PAINT_GEOMETRY_ADJUSTERS.len());
                    if let Some(adjuster) = PAINT_GEOMETRY_ADJUSTERS[rec.info.attacher_index] {
                        draw_geos = adjuster(rec.json, self, draw_geos);
                    }

                    // If we still have multiple geos, reduce using 'merge'.
                    let mut geo = if draw_geos.len() > 1 {
                        ShapeBuilder::merge_geometry(draw_geos, MergeMode::Merge)
                            as Rc<dyn GeometryNode>
                    } else {
                        Rc::clone(&draw_geos[0])
                    };

                    // Apply paint-specific fill rule if needed.
                    geo = adjust_geometry_fill_rule(geo, rec.json);

                    if let Some(draw) = Draw::make(Some(geo), Some(paint)) {
                        add_draw(&mut draws, draw as Rc<dyn RenderNode>, rec);
                    }
                    ctx.committed_animators = self.animator_count();
                }
                ShapeType::DrawEffect => {
                    debug_assert!(rec.info.attacher_index < DRAW_EFFECT_ATTACHERS.len());
                    if !draws.is_empty() {
                        draws = DRAW_EFFECT_ATTACHERS[rec.info.attacher_index](
                            rec.json,
                            self,
                            std::mem::take(&mut draws),
                        );
                        ctx.committed_animators = self.animator_count();
                    }
                }
                ShapeType::Transform => {}
            }
        }

        // By now we should have popped all local geometry effects.
        debug_assert_eq!(ctx.geometry_effect_stack.len(), initial_geometry_effects);

        let mut shape_wrapper: Option<Rc<dyn RenderNode>> = None;
        if draws.len() == 1 {
            // For a single draw, we don't need a group.
            shape_wrapper = draws.pop();
        } else if !draws.is_empty() {
            // Emit local draws reversed (bottom->top, per spec).
            draws.reverse();
            draws.shrink_to_fit();

            // We need a group to dispatch multiple draws.
            shape_wrapper = Some(Group::make_with_children(&draws) as Rc<dyn RenderNode>);
        }

        let mut shape_transform: Option<Rc<dyn Transform>> = None;
        if let Some(jtransform) = jtransform {
            let _apt = AutoPropertyTracker::new(self, jtransform, NodeType::Other);

            // This is tricky due to the interaction with ctx->fCommittedAnimators: we want any
            // animators related to tranform/opacity to be committed => they must be inserted in
            // front of the dangling/uncommitted ones.
            let ascope = AutoScope::new(self);

            shape_transform = self.attach_matrix_2d(jtransform, None, false);
            if let Some(shape_transform) = &shape_transform {
                shape_wrapper =
                    TransformEffect::make(shape_wrapper, Some(Rc::clone(shape_transform)))
                        .map(|effect| effect as Rc<dyn RenderNode>);
            }
            shape_wrapper = self.attach_opacity(jtransform, shape_wrapper);

            let local_scope = ascope.release();
            let local_count = local_scope.len();
            self.current_animator_scope.borrow_mut().splice(
                ctx.committed_animators..ctx.committed_animators,
                local_scope,
            );
            ctx.committed_animators += local_count;
        }

        // Push transformed local geometries to parent list, for subsequent paints.
        for geo in geos {
            ctx.geometry_stack.push(match &shape_transform {
                Some(shape_transform) => {
                    GeometryTransform::make(Some(geo), Some(Rc::clone(shape_transform)))
                        .map(|transformed| transformed as Rc<dyn GeometryNode>)
                        .expect("the geometry and transform are not null")
                }
                None => geo,
            });
        }

        shape_wrapper
    }

    /// Attaches a shape layer.
    // Port of: modules/skottie/src/layers/shapelayer/ShapeLayer.cpp#L361-L379 (chrome/m156) (`attachShapeLayer`)
    #[doc(alias = "attachShapeLayer")]
    #[must_use]
    pub fn attach_shape_layer(
        &self,
        layer: &ObjectValue,
        _info: &mut LayerInfo,
    ) -> Option<Rc<dyn RenderNode>> {
        let mut geometry_stack: Geometries = Vec::new();
        let mut geometry_effect_stack: Vec<GeometryEffectRec<'_>> = Vec::new();
        let mut shape_ctx = AttachShapeContext {
            geometry_stack: &mut geometry_stack,
            geometry_effect_stack: &mut geometry_effect_stack,
            committed_animators: self.animator_count(),
        };
        let shape_node = self.attach_shape(layer.get("shapes").as_array(), &mut shape_ctx, false);

        // Trim uncommitted animators: AttachShape consumes effects on the fly, and greedily
        // attaches geometries => at the end, we can end up with unused geometries, which are
        // nevertheless alive due to attached animators. To avoid this, we track committed
        // animators and discard the orphans here.
        let committed = shape_ctx.committed_animators;
        debug_assert!(committed <= self.animator_count());
        self.current_animator_scope.borrow_mut().truncate(committed);

        shape_node
    }
}
