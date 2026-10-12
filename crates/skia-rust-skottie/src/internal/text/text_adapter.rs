// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/text/TextAdapter.h, TextAdapter.cpp (chrome/m156)
//
// `SHOW_LAYOUT_BOXES` (a compile-time debugging switch, 0 in Skia) is not ported.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::contour_measure::{ContourMeasure, ContourMeasureIter};
use skia_rust_core::font_mgr::FontMgr;
use skia_rust_core::m44::{M44, V2, V3};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_priv::reverse_path;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{degrees_to_radians, scalar_round_to_int};
use skia_rust_core::t_pin::t_pin;
use skia_rust_core::utils::text_utils::Align;
use skia_rust_shaper::factory::Factory;
use skia_rust_sksg::InvalidationController;
use skia_rust_sksg::geometry_node::GEOMETRY_TRAITS;
use skia_rust_sksg::node::NodeCore;
use skia_rust_sksg::render_node::Hit;
use skia_rust_sksg::transform::Matrix as SgMatrix;
use skia_rust_sksg::{
    BlurImageFilter, Color as SgColor, Draw, GeometryNode, Group, ImageFilterEffect, Node,
    PaintNode, RenderContext, RenderNode, ScopedRenderContext, Transform, TransformEffect,
};

use crate::impl_container_animator;
use crate::internal::animator::{AnimatablePropertyContainer, Prop, PropertyContainer};
use crate::internal::skottie_priv::{AnimationBuilder, BLUR_SIZE_TO_SIGMA};
use crate::internal::text::font::GlyphCompMapper;
use crate::internal::text::text_animator::{
    DomainMap, DomainMaps, DomainSpan, ModulatorBuffer, ResolvedProps, TextAnimator,
};
use crate::internal::text::text_value::TextValue;
use crate::json::ObjectValue;
use crate::skottie::{Logger, LoggerLevel};
use crate::skottie_json::{Parse, ValueExt, parse_default};
use crate::skottie_property::{GlyphDecorator, GlyphInfo, TextInfo, TextPaintOrder};
use crate::skottie_value::{ShapeValue, Vec2Value};
use crate::text_shaper::{
    BoundsType, Flags, Fragment, ShapeResult, ShapedGlyphs, Shaper, TextDesc,
};

// Port of: modules/skottie/src/text/TextAdapter.cpp#L64-L103 (chrome/m156) (`GlyphTextNode`)
struct GlyphTextNode {
    core: NodeCore,
    glyphs: ShapedGlyphs,
}

impl GlyphTextNode {
    // Port of: modules/skottie/src/text/TextAdapter.cpp#L67 (chrome/m156)
    fn new(glyphs: ShapedGlyphs) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(GEOMETRY_TRAITS, weak.clone()),
            glyphs,
        })
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L71 (chrome/m156)
    fn glyphs(&self) -> &ShapedGlyphs {
        &self.glyphs
    }
}

impl std::fmt::Debug for GlyphTextNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlyphTextNode").finish_non_exhaustive()
    }
}

impl Node for GlyphTextNode {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L74-L76 (chrome/m156)
    fn on_revalidate(&self, _ic: Option<&mut InvalidationController>, _ctm: &Matrix) -> Rect {
        self.glyphs.compute_bounds(BoundsType::Tight)
    }
}

impl GeometryNode for GlyphTextNode {
    // Port of: modules/skottie/src/text/TextAdapter.cpp#L82-L84 (chrome/m156)
    fn on_clip(&self, canvas: &Canvas, anti_alias: bool) {
        canvas.clip_path(&self.on_as_path(), ClipOp::Intersect, anti_alias);
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L78-L80 (chrome/m156)
    fn on_draw(&self, canvas: &Canvas, paint: &Paint) {
        self.glyphs.draw(canvas, Point::new(0.0, 0.0), paint);
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L86-L88 (chrome/m156)
    fn on_contains(&self, p: Point) -> bool {
        self.on_as_path().contains(p)
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L90-L93 (chrome/m156)
    fn on_as_path(&self) -> Path {
        // TODO
        Path::default()
    }
}

// Port of: modules/skottie/src/text/TextAdapter.cpp#L96-L105 (chrome/m156) (`align_factor`)
fn align_factor(a: Align) -> f32 {
    match a {
        Align::Left => 0.0,
        Align::Center => 0.5,
        Align::Right => 1.0,
    }
}

/// What the glyph decorator node needs to know about a fragment.
// Port of: modules/skottie/src/text/TextAdapter.cpp#L152-L156 (chrome/m156) (`GlyphDecoratorNode::FragmentInfo`)
struct DecoratedFragment {
    glyphs: Rc<GlyphTextNode>,
    matrix_node: Rc<SgMatrix<M44>>,
    advance: f32,
}

// A group that calls the glyph decorator after its children rendered. `sksg::Group` is not
// subclassable here: this node owns a group and delegates to it.
// Port of: modules/skottie/src/text/TextAdapter.cpp#L107-L168 (chrome/m156)
// (`TextAdapter::GlyphDecoratorNode`)
struct GlyphDecoratorNode {
    core: NodeCore,
    group: Rc<Group>,
    decorator: Rc<dyn GlyphDecorator>,
    scale: f32,
    frag_info: RefCell<Vec<DecoratedFragment>>,
    decorator_info: RefCell<Vec<GlyphInfo>>,
}

impl GlyphDecoratorNode {
    // Port of: modules/skottie/src/text/TextAdapter.cpp#L110-L113 (chrome/m156)
    fn new(decorator: Rc<dyn GlyphDecorator>, scale: f32) -> Rc<Self> {
        let group = Group::make();
        let node = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            group: Rc::clone(&group),
            decorator,
            scale,
            frag_info: RefCell::new(Vec::new()),
            decorator_info: RefCell::new(Vec::new()),
        });
        node.observe_inval(group.as_ref());
        node
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L117-L128 (chrome/m156)
    fn update_fragment_data(&self, recs: &[FragmentRec]) {
        debug_assert!(self.frag_info.borrow().is_empty());
        *self.frag_info.borrow_mut() = recs
            .iter()
            .map(|rec| DecoratedFragment {
                glyphs: Rc::clone(&rec.glyphs),
                matrix_node: Rc::clone(&rec.matrix_node),
                advance: rec.advance,
            })
            .collect();

        debug_assert!(self.decorator_info.borrow().is_empty());
        *self.decorator_info.borrow_mut() = recs
            .iter()
            .map(|_| GlyphInfo {
                bounds: Rect::new_empty(),
                matrix: Matrix::new_identity(),
                cluster: 0,
                advance: 0.0,
            })
            .collect();
    }
}

impl std::fmt::Debug for GlyphDecoratorNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlyphDecoratorNode").finish_non_exhaustive()
    }
}

impl Node for GlyphDecoratorNode {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L130-L142 (chrome/m156)
    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        let child_bounds = self.group.revalidate(ic, ctm);

        let frag_info = self.frag_info.borrow();
        let mut decorator_info = self.decorator_info.borrow_mut();
        for (frag, info) in frag_info.iter().zip(decorator_info.iter_mut()) {
            let glyphs = frag.glyphs.glyphs();
            info.bounds = glyphs.compute_bounds(BoundsType::Tight);
            info.matrix = frag.matrix_node.as_matrix();

            info.cluster = glyphs.clusters.first().copied().unwrap_or(0);
            info.advance = frag.advance;
        }

        child_bounds
    }
}

impl RenderNode for GlyphDecoratorNode {
    // Port of: modules/skottie/src/text/TextAdapter.cpp#L144-L153 (chrome/m156)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        let local_ctx = ScopedRenderContext::new(canvas, ctx).set_isolation(
            &self.core.bounds(),
            &canvas.total_matrix(),
            true,
        );
        self.group.on_render(canvas, Some(local_ctx.context()));

        self.decorator.on_decorate(
            canvas,
            &TextInfo {
                glyphs: &self.decorator_info.borrow(),
                scale: self.scale,
            },
        );
    }

    fn on_node_at(&self, p: Point) -> Option<Hit> {
        self.group.on_node_at(p)
    }
}

/// Where fragments are anchored (`TextAdapter::AnchorPointGrouping`).
// Port of: modules/skottie/src/text/TextAdapter.h#L59-L64 (chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnchorPointGrouping {
    Character,
    Word,
    Line,
    All,
}

// Port of: modules/skottie/src/text/TextAdapter.h#L68-L79 (chrome/m156) (`TextAdapter::FragmentRec`)
struct FragmentRec {
    /// Fragment position.
    origin: Point,

    glyphs: Rc<GlyphTextNode>,
    matrix_node: Rc<SgMatrix<M44>>,
    fill_color_node: Option<Rc<SgColor>>,
    stroke_color_node: Option<Rc<SgColor>>,
    blur: Option<Rc<BlurImageFilter>>,

    /// Used for transform anchor point calculations.
    advance: f32,
    ascent: f32,
}

// The cached contour data of the text path.
// Port of: modules/skottie/src/text/TextAdapter.cpp#L218-L226 (chrome/m156) (`PathInfo`)
#[derive(Default)]
#[allow(clippy::struct_field_names)] // mirrors the `fCurrent*` members of C++
struct ContourCache {
    current_path: ShapeValue,
    current_measure: Option<ContourMeasure>,
    current_reversed: bool,
    current_closed: bool,
}

// Text path semantics
//
//   * glyphs are positioned on the path based on their horizontal/x anchor point, interpreted as
//     a distance along the path
//
//   * horizontal alignment is applied relative to the path start/end points
//
//   * "Reverse Path" allows reversing the path direction
//
//   * "Perpendicular To Path" determines whether glyphs are rotated to be perpendicular
//      to the path tangent, or not (just positioned).
//
//   * two controls ("First Margin" and "Last Margin") allow arbitrary offseting along the path,
//     depending on horizontal alignement:
//       - left:   offset = first margin
//       - center: offset = first margin + last margin
//       - right:  offset = last margin
//
//   * extranormal path positions (d < 0, d > path len) are allowed
//       - closed path: the position wraps around in both directions
//       - open path: extrapolates from extremes' positions/slopes
//
// Port of: modules/skottie/src/text/TextAdapter.cpp#L170-L226 (chrome/m156) (`TextAdapter::PathInfo`)
struct PathInfo {
    path: Prop<ShapeValue>,
    path_f_margin: Prop<f32>,
    path_l_margin: Prop<f32>,
    path_perpendicular: Prop<f32>,
    path_reverse: Prop<f32>,

    cache: RefCell<ContourCache>,
}

impl PathInfo {
    // Port of: modules/skottie/src/text/TextAdapter.cpp#L180-L199 (chrome/m156)
    fn update_contour_data(&self) {
        let reverse = self.path_reverse.get() != 0.0;

        let mut cache = self.cache.borrow_mut();
        if *self.path.borrow() != cache.current_path || reverse != cache.current_reversed {
            // reinitialize cached contour data
            let mut path = self.path.borrow().to_path();
            if reverse {
                path = reverse_path(&path);
            }

            let mut iter = ContourMeasureIter::new(&path, false, None);
            cache.current_measure = iter.next();
            cache.current_closed = path.is_last_contour_closed();
            cache.current_reversed = reverse;
            cache.current_path = self.path.get();

            // AE paths are always single-contour (no moves allowed).
            debug_assert!(iter.next().is_none());
        }
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L201-L206 (chrome/m156)
    fn path_length(&self) -> f32 {
        let cache = self.cache.borrow();
        debug_assert_eq!(*self.path.borrow(), cache.current_path);
        debug_assert_eq!(self.path_reverse.get() != 0.0, cache.current_reversed);

        cache
            .current_measure
            .as_ref()
            .map_or(0.0, ContourMeasure::length)
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L208-L216 (chrome/m156)
    fn get_matrix(&self, distance: f32, alignment: Align) -> M44 {
        let cache = self.cache.borrow();
        debug_assert_eq!(*self.path.borrow(), cache.current_path);
        debug_assert_eq!(self.path_reverse.get() != 0.0, cache.current_reversed);

        let Some(measure) = &cache.current_measure else {
            return M44::default();
        };
        let mut distance = distance;

        let path_len = measure.length();

        // First/last margin adjustment also depends on alignment.
        match alignment {
            Align::Left => distance += self.path_f_margin.get(),
            Align::Center => distance += self.path_f_margin.get() + self.path_l_margin.get(),
            Align::Right => distance += self.path_l_margin.get(),
        }

        // For closed paths, extranormal distances wrap around the contour.
        if cache.current_closed {
            // skia-rust: libm (std::fmod is exact: the remainder has no rounding)
            distance %= path_len;
            if distance < 0.0 {
                distance += path_len;
            }
            debug_assert!(0.0 <= distance && distance <= path_len);
        }

        let mut pos = Point::new(0.0, 0.0);
        let mut tan = Vector::new(0.0, 0.0);
        if !measure.get_pos_tan(distance, Some(&mut pos), Some(&mut tan)) {
            return M44::default();
        }

        // For open paths, extranormal distances are extrapolated from extremes.
        // Note:
        //   - getPosTan above clamps to the extremes
        //   - the extrapolation below only kicks in for extranormal values
        // std::min(0.0f, distance), std::max(0.0f, distance - path_len)
        let underflow = if distance < 0.0 { distance } else { 0.0 };
        let overflow = {
            let d = distance - path_len;
            if 0.0 < d { d } else { 0.0 }
        };
        let extrapolation = underflow + overflow;
        pos = Point::new(pos.x + tan.x * extrapolation, pos.y + tan.y * extrapolation);

        let mut m = M44::translate(pos.x, pos.y, 0.0);

        // The "perpendicular" flag controls whether fragments are positioned and rotated,
        // or just positioned.
        if self.path_perpendicular.get() != 0.0 {
            // skia-rust: libm
            m = &m
                * &M44::rotate(
                    V3 {
                        x: 0.0,
                        y: 0.0,
                        z: 1.0,
                    },
                    tan.y.atan2(tan.x),
                );
        }

        m
    }
}

/// Binds the text of a text layer to scene graph nodes: one fragment per glyph, word or line, and
/// the animators that move them.
// Port of: modules/skottie/src/text/TextAdapter.h#L40-L152 (chrome/m156) (`class TextAdapter`)
#[doc(alias = "skottie::internal::TextAdapter")]
pub struct TextAdapter {
    container: PropertyContainer,

    root: Rc<Group>,
    font_mgr: Option<FontMgr>,
    custom_glyph_mapper: Option<Rc<GlyphCompMapper>>,
    logger: RefCell<Option<Rc<dyn Logger>>>,
    shaping_factory: Arc<dyn Factory>,
    anchor_point_grouping: AnchorPointGrouping,

    animators: Vec<Rc<TextAnimator>>,
    fragments: RefCell<Vec<FragmentRec>>,
    maps: RefCell<DomainMaps>,

    // `TextValueTracker`: helps detect external value changes.
    text: Prop<TextValue>,
    prev_text: RefCell<TextValue>,
    grouping_alignment: Prop<Vec2Value>,
    /// Size adjustment from auto-scaling (`fTextShapingScale`).
    text_shaping_scale: Cell<f32>,

    // Optional text path.
    path_info: Option<PathInfo>,

    has_blur_animator: bool,
    requires_anchor_point: bool,
    requires_line_adjustments: bool,
}

impl TextAdapter {
    /// Makes the adapter of a text layer; `None` if the layer is invalid.
    ///
    /// # Panics
    /// Never: the grouping index is pinned to the table.
    // Port of: modules/skottie/src/text/TextAdapter.cpp#L228-L396 (chrome/m156) (`Make`)
    #[must_use]
    #[allow(clippy::items_after_statements)] // mirrors the function-local table of C++
    pub fn make(
        jlayer: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        fontmgr: Option<FontMgr>,
        custom_glyph_mapper: Option<Rc<GlyphCompMapper>>,
        logger: Option<Rc<dyn Logger>>,
        factory: Arc<dyn Factory>,
    ) -> Option<Rc<Self>> {
        // General text node format:
        // "t": {
        //    "a": [], // animators (see TextAnimator)
        //    "d": {
        //        "k": [
        //            {
        //                "s": {
        //                    "f": "Roboto-Regular",
        //                    "fc": [
        //                        0.42,
        //                        0.15,
        //                        0.15
        //                    ],
        //                    "j": 1,
        //                    "lh": 60,
        //                    "ls": 0,
        //                    "s": 50,
        //                    "t": "text align right",
        //                    "tr": 0
        //                },
        //                "t": 0
        //            }
        //        ],
        //        "sid": "optionalSlotID"
        //    },
        //    "m": { // more options
        //           "g": 1,     // Anchor Point Grouping
        //           "a": {...}  // Grouping Alignment
        //         },
        //    "p": { // path options
        //           "a": 0,   // force alignment
        //           "f": {},  // first margin
        //           "l": {},  // last margin
        //           "m": 1,   // mask index
        //           "p": 1,   // perpendicular
        //           "r": 0    // reverse path
        //         }

        // },

        let jt = jlayer.get("t").as_object();
        let jd = jt.and_then(|jt| jt.get("d").as_object());
        let (Some(jt), Some(jd)) = (jt, jd) else {
            abuilder.log_json(LoggerLevel::Error, jlayer, "Invalid text layer.");
            return None;
        };

        // "More options"
        let jm = jt.get("m").as_object();
        const GROUPING_MAP: [AnchorPointGrouping; 4] = [
            AnchorPointGrouping::Character, // 'g': 1
            AnchorPointGrouping::Word,      // 'g': 2
            AnchorPointGrouping::Line,      // 'g': 3
            AnchorPointGrouping::All,       // 'g': 4
        ];
        let apg = jm.map_or(1, |jm| {
            t_pin(
                parse_default::<i32>(jm.get("g"), 1),
                1,
                i32::try_from(GROUPING_MAP.len()).expect("small"),
            )
        });

        let adapter = Rc::new_cyclic(|weak: &Weak<Self>| {
            let container = PropertyContainer::new(weak.clone());

            let text = Prop::new(TextValue::default());
            let grouping_alignment = Prop::new(Vec2Value { x: 0.0, y: 0.0 });

            container.bind(abuilder, jd, &text);
            if let Some(jm) = jm {
                container.bind(abuilder, jm.get("a"), &grouping_alignment);
            }

            // Animators
            let mut animators: Vec<Rc<TextAnimator>> = Vec::new();
            let mut has_blur_animator = false;
            let mut requires_anchor_point = false;
            let mut requires_line_adjustments = false;
            if let Some(janimators) = jt.get("a").as_array() {
                animators.reserve(janimators.size());

                for i in 0..janimators.size() {
                    if let Some(animator) =
                        TextAnimator::make(janimators[i].as_object(), abuilder, &container)
                    {
                        has_blur_animator |= animator.has_blur();
                        requires_anchor_point |= animator.requires_anchor_point();
                        requires_line_adjustments |= animator.requires_line_adjustments();

                        animators.push(animator);
                    }
                }
            }

            // Optional text path
            let path_info = Self::attach_path(
                jt.get("p").as_object(),
                jlayer,
                abuilder,
                &container,
                &mut requires_anchor_point,
            );

            Self {
                container,
                root: Group::make(),
                font_mgr: fontmgr,
                custom_glyph_mapper,
                logger: RefCell::new(logger),
                shaping_factory: factory,
                anchor_point_grouping: GROUPING_MAP
                    [usize::try_from(apg - 1).expect("pinned to 1..=4")],
                animators,
                fragments: RefCell::new(Vec::new()),
                maps: RefCell::new(DomainMaps::default()),
                text,
                prev_text: RefCell::new(TextValue::default()),
                grouping_alignment,
                text_shaping_scale: Cell::new(1.0),
                path_info,
                has_blur_animator,
                requires_anchor_point,
                requires_line_adjustments,
            }
        });

        abuilder.dispatch_text_property(&adapter, Some(jd));

        Some(adapter)
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L348-L389 (chrome/m156) (`attach_path`)
    fn attach_path(
        jpath: Option<&ObjectValue>,
        jlayer: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        container: &PropertyContainer,
        requires_anchor_point: &mut bool,
    ) -> Option<PathInfo> {
        let jpath = jpath?;

        // the actual path is identified as an index in the layer mask stack
        let mask_index = parse_default::<usize>(jpath.get("m"), usize::MAX);
        let jmasks = jlayer.get("masksProperties").as_array();
        let jmasks = jmasks?;
        if mask_index >= jmasks.size() {
            return None;
        }

        let mask = jmasks[mask_index].as_object()?;

        let pinfo = PathInfo {
            path: Prop::new(ShapeValue::new()),
            path_f_margin: Prop::new(0.0),
            path_l_margin: Prop::new(0.0),
            path_perpendicular: Prop::new(0.0),
            path_reverse: Prop::new(0.0),
            cache: RefCell::new(ContourCache::default()),
        };
        container.bind(abuilder, mask.get("pt"), &pinfo.path);
        container.bind(abuilder, jpath.get("f"), &pinfo.path_f_margin);
        container.bind(abuilder, jpath.get("l"), &pinfo.path_l_margin);
        container.bind(abuilder, jpath.get("p"), &pinfo.path_perpendicular);
        container.bind(abuilder, jpath.get("r"), &pinfo.path_reverse);

        // TODO: force align support

        // Historically, these used to be exported as static properties.
        // Attempt parsing both ways, for backward compat.
        let mut perpendicular = pinfo.path_perpendicular.get();
        if f32::parse(jpath.get("p"), &mut perpendicular) {
            pinfo.path_perpendicular.set(perpendicular);
        }
        let mut reverse = pinfo.path_reverse.get();
        if f32::parse(jpath.get("r"), &mut reverse) {
            pinfo.path_reverse.set(reverse);
        }

        // Path positioning requires anchor point info.
        *requires_anchor_point = true;

        Some(pinfo)
    }

    /// The root node of the text (`node`).
    #[must_use]
    pub fn node(&self) -> &Rc<Group> {
        &self.root
    }

    /// The current text value (`getText`).
    #[doc(alias = "getText")]
    #[must_use]
    pub fn get_text(&self) -> TextValue {
        self.text.get()
    }

    /// Sets the text value, and updates the scene graph (`setText`).
    // Port of: modules/skottie/src/text/TextAdapter.cpp#L628-L631 (chrome/m156)
    #[doc(alias = "setText")]
    pub fn set_text(&self, txt: &TextValue) {
        self.text.set(txt.clone());
        self.on_sync();
    }

    // `TextValueTracker::hasChanged`.
    // Port of: modules/skottie/src/text/TextAdapter.h#L118-L125 (chrome/m156)
    fn text_has_changed(&self) -> bool {
        let current = self.text.borrow();
        let mut prev = self.prev_text.borrow_mut();
        if *current != *prev {
            prev.clone_from(&current);
            return true;
        }
        false
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L398-L442 (chrome/m156)
    // (`buildGlyphCompNodes`)
    fn build_glyph_comp_nodes(&self, glyphs: &mut ShapedGlyphs) -> Vec<Rc<dyn RenderNode>> {
        let mut draws: Vec<Rc<dyn RenderNode>> = Vec::new();

        if let Some(mapper) = &self.custom_glyph_mapper {
            let text = self.text.borrow();
            let mut run_offset = 0;
            for run_index in 0..glyphs.runs.len() {
                let mut i = 0;
                while i < glyphs.runs[run_index].size {
                    let goffset = run_offset + i;
                    let gid = glyphs.glyph_ids[goffset];

                    if let Some(gcomp) =
                        mapper.get_glyph_comp(glyphs.runs[run_index].font.typeface(), gid)
                    {
                        // Position and scale the "glyph".
                        let scale = text.text_size * self.text_shaping_scale.get();
                        let m = Matrix::concat(
                            &Matrix::translate(glyphs.glyph_pos[goffset]),
                            &Matrix::scale((scale, scale)),
                        );

                        draws.extend(
                            TransformEffect::make_with_matrix(Some(gcomp), m)
                                .map(|node| node as Rc<dyn RenderNode>),
                        );

                        // Remove all related data from the fragment, so we don't attempt to
                        // render this as a regular glyph.
                        debug_assert!(glyphs.glyph_ids.len() > goffset);
                        glyphs.glyph_ids.remove(goffset);
                        debug_assert!(glyphs.glyph_pos.len() > goffset);
                        glyphs.glyph_pos.remove(goffset);
                        if !glyphs.clusters.is_empty() {
                            debug_assert!(glyphs.clusters.len() > goffset);
                            glyphs.clusters.remove(goffset);
                        }
                        // C++ `i -= 1` (wrapping) and the `++i` of the loop cancel out.
                        glyphs.runs[run_index].size -= 1;
                    } else {
                        i += 1;
                    }
                }
                run_offset += glyphs.runs[run_index].size;
            }
        }

        draws
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L444-L541 (chrome/m156)
    fn add_fragment(&self, frag: &mut Fragment, container: &Group) {
        // For a given shaped fragment, build a corresponding SG fragment:
        //
        //   [TransformEffect] -> [Transform]
        //     [Group]
        //       [Draw] -> [GlyphTextNode*] [FillPaint]    // SkTypeface-based glyph.
        //       [Draw] -> [GlyphTextNode*] [StrokePaint]  // SkTypeface-based glyph.
        //       [CompRenderTree]                          // Comp glyph.
        //       ...
        //

        let text = self.text.borrow();
        let matrix_node = SgMatrix::<M44>::make(M44::translate(frag.origin.x, frag.origin.y, 0.0));

        // Start off substituting existing comp nodes for all composition-based glyphs.
        let mut draws: Vec<Rc<dyn RenderNode>> = self.build_glyph_comp_nodes(&mut frag.glyphs);

        // Use a regular GlyphTextNode for the remaining glyphs (backed by a real SkTypeface).
        let text_node = GlyphTextNode::new(std::mem::take(&mut frag.glyphs));

        draws.reserve(usize::from(text.has_fill) + usize::from(text.has_stroke));

        debug_assert!(text.has_fill || text.has_stroke);

        let mut fill_color_node = None;
        let mut stroke_color_node = None;

        let mut add_fill = |draws: &mut Vec<Rc<dyn RenderNode>>| {
            if text.has_fill {
                let node = SgColor::make(text.fill_color);
                node.set_anti_alias(true);
                draws.extend(
                    Draw::make(
                        Some(Rc::clone(&text_node) as Rc<dyn GeometryNode>),
                        Some(Rc::clone(&node) as Rc<dyn PaintNode>),
                    )
                    .map(|draw| draw as Rc<dyn RenderNode>),
                );
                fill_color_node = Some(node);
            }
        };
        let mut add_stroke = |draws: &mut Vec<Rc<dyn RenderNode>>| {
            if text.has_stroke {
                let node = SgColor::make(text.stroke_color);
                node.set_anti_alias(true);
                node.set_style(Style::Stroke);
                node.set_stroke_width(text.stroke_width * self.text_shaping_scale.get());
                node.set_stroke_join(text.stroke_join);
                draws.extend(
                    Draw::make(
                        Some(Rc::clone(&text_node) as Rc<dyn GeometryNode>),
                        Some(Rc::clone(&node) as Rc<dyn PaintNode>),
                    )
                    .map(|draw| draw as Rc<dyn RenderNode>),
                );
                stroke_color_node = Some(node);
            }
        };

        if text.paint_order == TextPaintOrder::FillStroke {
            add_fill(&mut draws);
            add_stroke(&mut draws);
        } else {
            add_stroke(&mut draws);
            add_fill(&mut draws);
        }

        debug_assert!(!draws.is_empty());

        draws.shrink_to_fit();

        let mut draws_node: Rc<dyn RenderNode> = if draws.len() > 1 {
            Group::make_with_children(&draws)
        } else {
            draws.swap_remove(0)
        };

        let mut blur = None;
        if self.has_blur_animator {
            // Optional blur effect.
            let node = BlurImageFilter::make();
            draws_node = ImageFilterEffect::make(
                draws_node,
                Some(Rc::clone(&node) as Rc<dyn skia_rust_sksg::render_effect::ImageFilterNode>),
            );
            blur = Some(node);
        }

        container.add_child(
            TransformEffect::make(
                Some(draws_node),
                Some(Rc::clone(&matrix_node) as Rc<dyn Transform>),
            )
            .expect("the child and the transform exist") as Rc<dyn RenderNode>,
        );
        self.fragments.borrow_mut().push(FragmentRec {
            origin: frag.origin,
            glyphs: text_node,
            matrix_node,
            fill_color_node,
            stroke_color_node,
            blur,
            advance: frag.advance,
            ascent: frag.ascent,
        });
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L543-L598 (chrome/m156)
    fn build_domain_maps(&self, shape_result: &ShapeResult) {
        let mut non_whitespace_map = DomainMap::new();
        let mut words_map = DomainMap::new();
        let mut lines_map = DomainMap::new();

        let mut i = 0;
        let mut line = 0;
        let mut line_start = 0;
        let mut word_start = 0;

        let mut word_advance = 0.0_f32;
        let mut word_ascent = 0.0_f32;
        let mut line_advance = 0.0_f32;
        let mut line_ascent = 0.0_f32;

        let mut in_word = false;

        // TODO: use ICU for building the word map?
        while i < shape_result.fragments.len() {
            let frag = &shape_result.fragments[i];
            let is_new_line = frag.line_index != line;

            if frag.is_whitespace || is_new_line {
                // Both whitespace and new lines break words.
                if in_word {
                    in_word = false;
                    words_map.push(DomainSpan {
                        offset: word_start,
                        count: i - word_start,
                        advance: word_advance,
                        ascent: word_ascent,
                    });
                }
            }

            if !frag.is_whitespace {
                non_whitespace_map.push(DomainSpan {
                    offset: i,
                    count: 1,
                    advance: 0.0,
                    ascent: 0.0,
                });

                if !in_word {
                    in_word = true;
                    word_start = i;
                    word_advance = 0.0;
                    word_ascent = 0.0;
                }

                word_advance += frag.advance;
                // std::min(word_ascent, frag.fAscent): negative ascent
                word_ascent = if frag.ascent < word_ascent {
                    frag.ascent
                } else {
                    word_ascent
                };
            }

            if is_new_line {
                debug_assert_eq!(frag.line_index, line + 1);
                lines_map.push(DomainSpan {
                    offset: line_start,
                    count: i - line_start,
                    advance: line_advance,
                    ascent: line_ascent,
                });
                line = frag.line_index;
                line_start = i;
                line_advance = 0.0;
                line_ascent = 0.0;
            }

            line_advance += frag.advance;
            // std::min(line_ascent, frag.fAscent): negative ascent
            line_ascent = if frag.ascent < line_ascent {
                frag.ascent
            } else {
                line_ascent
            };

            i += 1;
        }

        if i > word_start {
            words_map.push(DomainSpan {
                offset: word_start,
                count: i - word_start,
                advance: word_advance,
                ascent: word_ascent,
            });
        }

        if i > line_start {
            lines_map.push(DomainSpan {
                offset: line_start,
                count: i - line_start,
                advance: line_advance,
                ascent: line_ascent,
            });
        }

        let maps = DomainMaps {
            non_whitespace_map: Rc::new(non_whitespace_map),
            words_map: Rc::new(words_map),
            lines_map: Rc::new(lines_map),
        };
        for animator in &self.animators {
            animator.update_domain_maps(&maps, shape_result.fragments.len());
        }
        *self.maps.borrow_mut() = maps;
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L633-L656 (chrome/m156)
    fn shaper_flags(&self) -> Flags {
        let text = self.text.borrow();
        let mut flags = Flags::empty();

        // We need granular fragments (as opposed to consolidated blobs):
        //   - when animating
        //   - when positioning on a path
        //   - when clamping the number or lines (for accurate line count)
        //   - when a text decorator is present
        if !self.animators.is_empty()
            || self.path_info.is_some()
            || text.max_lines != 0
            || text.decorator.is_some()
        {
            flags |= Flags::FRAGMENT_GLYPHS;
        }

        if self.requires_anchor_point || text.decorator.is_some() {
            flags |= Flags::TRACK_FRAGMENT_ADVANCE_ASCENT;
        }

        if text.decorator.is_some() {
            flags |= Flags::CLUSTERS;
        }

        flags
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L658-L774 (chrome/m156)
    fn reshape(&self) {
        // AE clamps the font size to a reasonable range.
        // We do the same, since HB is susceptible to int overflows for degenerate values.
        const MIN_SIZE: f32 = 0.1;
        const MAX_SIZE: f32 = 1296.0;
        let text = self.text.get();
        let text_desc = TextDesc {
            typeface: text.typeface.clone(),
            text_size: t_pin(text.text_size, MIN_SIZE, MAX_SIZE),
            min_text_size: t_pin(text.min_text_size, MIN_SIZE, MAX_SIZE),
            max_text_size: t_pin(text.max_text_size, MIN_SIZE, MAX_SIZE),
            line_height: text.line_height,
            line_shift: text.line_shift,
            ascent: text.ascent,
            h_align: text.h_align,
            v_align: text.v_align,
            resize: text.resize,
            linebreak: text.line_break,
            direction: text.direction,
            capitalization: text.capitalization,
            max_lines: text.max_lines,
            flags: self.shaper_flags(),
            locale: if text.locale.is_empty() {
                None
            } else {
                Some(text.locale.clone())
            },
            font_family: if text.font_family.is_empty() {
                None
            } else {
                Some(text.font_family.clone())
            },
            text_tracking: text.text_tracking,
        };
        let mut shape_result = Shaper::shape_in_box(
            &text.text,
            &text_desc,
            &text.box_,
            self.font_mgr.as_ref(),
            &self.shaping_factory,
        );

        {
            let mut logger_slot = self.logger.borrow_mut();
            if let Some(logger) = logger_slot.clone() {
                if shape_result.fragments.is_empty() && !text.text.is_empty() {
                    let msg = format!("Text layout failed for '{}'.", text.text);
                    logger.log(LoggerLevel::Error, &msg, None);

                    // These may trigger repeatedly when the text is animating.
                    // To avoid spamming, only log once.
                    *logger_slot = None;
                }

                // C++ dereferences `fLogger` here even after it was cleared above, which cannot
                // happen: no fragments means no missing glyphs.
                if shape_result.missing_glyph_count > 0
                    && let Some(logger) = logger_slot.clone()
                {
                    let msg = format!(
                        "Missing {} glyphs for '{}'.",
                        shape_result.missing_glyph_count, text.text
                    );
                    logger.log(LoggerLevel::Warning, &msg, None);
                    *logger_slot = None;
                }
            }
        }

        // Save the text shaping scale for later adjustments.
        self.text_shaping_scale.set(shape_result.scale);

        // Rebuild all fragments.
        // TODO: we can be smarter here and try to reuse the existing SG structure if needed.

        self.root.clear();
        self.fragments.borrow_mut().clear();

        // Depending on whether a GlyphDecorator is present, we either add the glyph render nodes
        // directly to the root group, or to an intermediate GlyphDecoratorNode container.
        let mut decorator_node: Option<Rc<GlyphDecoratorNode>> = None;
        if let Some(decorator) = &text.decorator {
            decorator_node = Some(GlyphDecoratorNode::new(
                Rc::clone(decorator),
                self.text_shaping_scale.get(),
            ));
        }
        let container: Rc<Group> = match &decorator_node {
            Some(node) => Rc::clone(&node.group),
            None => Rc::clone(&self.root),
        };

        // N.B. addFragment moves shaped glyph data out of the fragment, so only the fragment
        // metrics are valid after this block.
        for frag in &mut shape_result.fragments {
            self.add_fragment(frag, &container);
        }

        if let Some(decorator_node) = decorator_node {
            decorator_node.update_fragment_data(&self.fragments.borrow());
            self.root.add_child(decorator_node);
        }

        if !self.animators.is_empty() || self.path_info.is_some() {
            // Range selectors and text paths require fragment domain maps.
            self.build_domain_maps(&shape_result);
        }
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L776-L899 (chrome/m156)
    // (`TextAdapter::onSync`, via the container)
    #[allow(clippy::too_many_lines)] // one function in C++ as well
    fn sync(&self) {
        {
            let text = self.text.borrow();
            if !text.has_fill && !text.has_stroke {
                return;
            }
        }

        if self.text_has_changed() {
            self.reshape();
        }

        if self.fragments.borrow().is_empty() {
            return;
        }

        // Update the path contour measure, if needed.
        if let Some(path_info) = &self.path_info {
            path_info.update_contour_data();
        }

        let text = self.text.get();

        // Seed props from the current text value.
        let seed_props = ResolvedProps {
            fill_color: text.fill_color,
            stroke_color: text.stroke_color,
            stroke_width: text.stroke_width,
            ..ResolvedProps::default()
        };

        let fragments = self.fragments.borrow();
        let mut buf: ModulatorBuffer = vec![
            crate::internal::text::text_animator::AnimatedPropsModulator {
                props: seed_props,
                coverage: 0.0,
            };
            fragments.len()
        ];

        // Apply all animators to the modulator buffer.
        for animator in &self.animators {
            animator.modulate_props(&mut buf);
        }

        let maps = self.maps.borrow();
        let grouping_domain: Option<&DomainMap> = match self.anchor_point_grouping {
            // for word/line grouping, we rely on domain map info
            AnchorPointGrouping::Word => Some(&maps.words_map),
            AnchorPointGrouping::Line => Some(&maps.lines_map),
            // remaining grouping modes (character/all) do not need (or have) domain map data
            _ => None,
        };

        let mut grouping_span_index = 0_usize;
        let mut current_line_offset = V2 { x: 0.0, y: 0.0 }; // cumulative line spacing

        let compute_linewide_props = |buf: &ModulatorBuffer, line_span: &DomainSpan| {
            let mut total_spacing = V2 { x: 0.0, y: 0.0 };
            let mut total_tracking = 0.0_f32;

            // Only compute these when needed.
            if self.requires_line_adjustments && line_span.count != 0 {
                for item in &buf[line_span.offset..line_span.offset + line_span.count] {
                    let props = &item.props;
                    total_spacing += props.line_spacing;
                    total_tracking += props.tracking;
                }

                // The first glyph does not contribute |before| tracking, and the last one does
                // not contribute |after| tracking.
                #[allow(clippy::manual_midpoint)] // the arithmetic of C++: `0.5f * (a + b)`
                {
                    total_tracking -= 0.5_f32
                        * (buf[line_span.offset].props.tracking
                            + buf[line_span.offset + line_span.count - 1].props.tracking);
                }
            }

            (total_spacing, total_tracking)
        };

        // Finally, push all props to their corresponding fragment.
        for (line_index, line_span) in maps.lines_map.iter().enumerate() {
            let (line_spacing, line_tracking) = compute_linewide_props(&buf, line_span);
            let align_offset = -line_tracking * align_factor(text.h_align);

            // line spacing of the first line is ignored (nothing to "space" against)
            if line_index != 0 && line_span.count != 0 {
                // For each line, the actual spacing is an average of individual fragment spacing
                // (to preserve the "line").
                #[allow(clippy::cast_precision_loss)] // mirrors the size_t to float conversion
                {
                    current_line_offset += line_spacing / (line_span.count as f32);
                }
            }

            let mut tracking_acc = 0.0_f32;
            for i in line_span.offset..line_span.offset + line_span.count {
                // Track the grouping domain span in parallel.
                if let Some(grouping_domain) = grouping_domain
                    && i >= grouping_domain[grouping_span_index].offset
                        + grouping_domain[grouping_span_index].count
                {
                    grouping_span_index += 1;
                    debug_assert!(
                        i < grouping_domain[grouping_span_index].offset
                            + grouping_domain[grouping_span_index].count
                    );
                }

                let props = &buf[i].props;
                let frag = &fragments[i];

                // AE tracking is defined per glyph, based on two components: |before| and
                // |after|. BodyMovin only exports "balanced" tracking values, where
                // before = after = tracking/2.
                //
                // Tracking is applied as a local glyph offset, and contributes to the line width
                // for alignment purposes.
                //
                // No |before| tracking for the first glyph, nor |after| tracking for the last
                // one.
                let track_before = if i > line_span.offset {
                    props.tracking * 0.5_f32
                } else {
                    0.0
                };
                let track_after = if i < line_span.offset + line_span.count - 1 {
                    props.tracking * 0.5_f32
                } else {
                    0.0
                };

                let frag_offset = current_line_offset
                    + V2 {
                        x: align_offset + tracking_acc + track_before,
                        y: 0.0,
                    };

                tracking_acc += track_before + track_after;

                self.push_props_to_fragment(
                    &text,
                    props,
                    frag,
                    frag_offset,
                    self.grouping_alignment.get() * 0.01_f32, // %
                    grouping_domain.map(|domain| &domain[grouping_span_index]),
                    &fragments,
                );
            }
        }
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L901-L964 (chrome/m156)
    fn fragment_anchor_point(
        &self,
        text: &TextValue,
        rec: &FragmentRec,
        grouping_alignment: V2,
        grouping_span: Option<&DomainSpan>,
        fragments: &[FragmentRec],
    ) -> V2 {
        // Construct the following 2x ascent box:
        //
        //      -------------
        //     |             |
        //     |             | ascent
        //     |             |
        // ----+-------------+---------- baseline
        //   (pos)           |
        //     |             | ascent
        //     |             |
        //      -------------
        //         advance

        let make_box = |pos: Point, advance: f32, ascent: f32| {
            // note: negative ascent
            Rect::from_xywh(pos.x, pos.y + ascent, advance, -2.0 * ascent)
        };

        // Compute a grouping-dependent anchor point box.
        // The default anchor point is at the center, and gets adjusted relative to the bounds
        // based on |grouping_alignment|.
        let anchor_box = || -> Rect {
            match self.anchor_point_grouping {
                AnchorPointGrouping::Character => {
                    // Anchor box relative to each individual fragment.
                    make_box(rec.origin, rec.advance, rec.ascent)
                }
                AnchorPointGrouping::Word | AnchorPointGrouping::Line => {
                    let grouping_span = grouping_span.expect("word and line grouping have a span");
                    // Anchor box relative to the first fragment in the word/line.
                    let first_span_fragment = &fragments[grouping_span.offset];
                    make_box(
                        first_span_fragment.origin,
                        grouping_span.advance,
                        grouping_span.ascent,
                    )
                }
                AnchorPointGrouping::All => {
                    // Anchor box is the same as the text box.
                    text.box_
                }
            }
        };

        let ab = anchor_box();

        // Apply grouping alignment.
        let ap = V2 {
            x: ab.center_x() + ab.width() * 0.5_f32 * grouping_alignment.x,
            y: ab.center_y() + ab.height() * 0.5_f32 * grouping_alignment.y,
        };

        // The anchor point is relative to the fragment position.
        ap - V2 {
            x: rec.origin.x,
            y: rec.origin.y,
        }
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L966-L1002 (chrome/m156)
    fn fragment_matrix(
        &self,
        text: &TextValue,
        props: &ResolvedProps,
        rec: &FragmentRec,
        frag_offset: V2,
    ) -> M44 {
        let pos = V3 {
            x: props.position.x + rec.origin.x + frag_offset.x,
            y: props.position.y + rec.origin.y + frag_offset.y,
            z: props.position.z,
        };

        let Some(path_info) = &self.path_info else {
            return M44::translate(pos.x, pos.y, pos.z);
        };

        // "Align" the paragraph box left/center/right to path start/mid/end, respectively.
        let align_offset =
            align_factor(text.h_align) * (path_info.path_length() - text.box_.width());

        // Path positioning is based on the fragment position relative to the paragraph box
        // upper-left corner:
        //
        //   - the horizontal component determines the distance on path
        //
        //   - the vertical component is post-applied after orienting on path
        //
        // Note: in point-text mode, the box adjustments have no effect as fBox is {0,0,0,0}.
        //
        let rel_pos = V2 { x: pos.x, y: pos.y }
            - V2 {
                x: text.box_.left,
                y: text.box_.top,
            };
        let path_distance = rel_pos.x + align_offset;

        &path_info.get_matrix(path_distance, text.h_align) * &M44::translate(0.0, rel_pos.y, pos.z)
    }

    // Port of: modules/skottie/src/text/TextAdapter.cpp#L1004-L1040 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ parameters, plus the fragment list
    fn push_props_to_fragment(
        &self,
        text: &TextValue,
        props: &ResolvedProps,
        rec: &FragmentRec,
        frag_offset: V2,
        grouping_alignment: V2,
        grouping_span: Option<&DomainSpan>,
        fragments: &[FragmentRec],
    ) {
        let anchor_point =
            self.fragment_anchor_point(text, rec, grouping_alignment, grouping_span, fragments);

        let m = &self.fragment_matrix(
            text,
            props,
            rec,
            V2 {
                x: anchor_point.x + frag_offset.x,
                y: anchor_point.y + frag_offset.y,
            },
        ) * &M44::rotate(
            V3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            degrees_to_radians(props.rotation.x),
        );
        let m = &m
            * &M44::rotate(
                V3 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                },
                degrees_to_radians(props.rotation.y),
            );
        let m = &m
            * &M44::rotate(
                V3 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                degrees_to_radians(props.rotation.z),
            );
        let m = &m * &M44::scale(props.scale.x, props.scale.y, props.scale.z);
        let m = &m * &M44::translate(-anchor_point.x, -anchor_point.y, 0.0);
        rec.matrix_node.set_matrix(m);

        let scale_alpha = |c: skia_rust_core::color::Color, o: f32| {
            c.with_a(color_alpha_from_round(scalar_round_to_int(
                o * f32::from(c.a()),
            )))
        };

        if let Some(fill_color_node) = &rec.fill_color_node {
            fill_color_node.set_color(scale_alpha(props.fill_color, props.opacity));
        }
        if let Some(stroke_color_node) = &rec.stroke_color_node {
            stroke_color_node.set_color(scale_alpha(props.stroke_color, props.opacity));
            stroke_color_node.set_stroke_width(props.stroke_width * self.text_shaping_scale.get());
        }
        if let Some(blur) = &rec.blur {
            blur.set_sigma((
                props.blur.x * BLUR_SIZE_TO_SIGMA,
                props.blur.y * BLUR_SIZE_TO_SIGMA,
            ));
        }
    }
}

// `SkColorSetA(c, a)` takes a U8CPU and keeps its low byte.
fn color_alpha_from_round(a: i32) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // mirrors the implicit int to U8CPU to uint8_t narrowing of SkColorSetA
    {
        (a as u32 & 0xff) as u8
    }
}

impl AnimatablePropertyContainer for TextAdapter {
    fn container(&self) -> &PropertyContainer {
        &self.container
    }

    fn on_sync(&self) {
        self.sync();
    }
}

impl_container_animator!(TextAdapter);

opaque_debug!(TextAdapter);
