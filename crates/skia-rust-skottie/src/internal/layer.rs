// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/Layer.h, modules/skottie/src/Layer.cpp (chrome/m156)

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color;
use skia_rust_core::m44::V2;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::Rect;
use skia_rust_core::size::Size;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_sksg::render_effect::ImageFilterNode;
use skia_rust_sksg::transform::make_inverse;
use skia_rust_sksg::{
    BlurImageFilter, ClipEffect, Color as SgColor, Draw, GeometryNode, Group, ImageFilterEffect,
    LayerEffect, MaskEffect, MaskMode, Merge, MergeMode, MergeRec, PaintNode, Path as SgPath,
    Rect as SgRect, RenderNode, Transform, TransformEffect,
};

use crate::json::{ArrayValue, ObjectValue};
use crate::skottie::{LayerInfo, LoggerLevel};
use crate::skottie_json::{ValueExt, parse_default, parse_value};
use crate::skottie_property::NodeType;

use super::animator::{
    AnimatablePropertyContainer, Animator, Prop, PropertyContainer, StateChanged,
};
use super::composition::CompositionBuilder;
use super::effects::EffectBuilder;
use super::skottie_priv::{AnimationBuilder, AnimatorScope, AutoPropertyTracker, AutoScope};
use crate::impl_container_animator;

/// How a mask mode is realized.
// Port of: modules/skottie/src/Layer.cpp#L60-L64 (chrome/m156) (`MaskInfo`)
struct MaskInfo {
    /// Used when masking with layers/blending.
    blend_mode: BlendMode,
    /// Used when clipping.
    merge_mode: MergeMode,
    invert_geometry: bool,
}

// Port of: modules/skottie/src/Layer.cpp#L66-L88 (chrome/m156) (`GetMaskInfo`)
fn get_mask_info(mode: u8) -> Option<&'static MaskInfo> {
    static ADD_INFO: MaskInfo = MaskInfo {
        blend_mode: BlendMode::SrcOver,
        merge_mode: MergeMode::Union,
        invert_geometry: false,
    };
    static INT_INFO: MaskInfo = MaskInfo {
        blend_mode: BlendMode::SrcIn,
        merge_mode: MergeMode::Intersect,
        invert_geometry: false,
    };
    static SUB_INFO: MaskInfo = MaskInfo {
        blend_mode: BlendMode::DstOut,
        merge_mode: MergeMode::Difference,
        invert_geometry: true,
    };
    static DIF_INFO: MaskInfo = MaskInfo {
        blend_mode: BlendMode::Xor,
        merge_mode: MergeMode::Xor,
        invert_geometry: false,
    };

    match mode {
        b'a' => Some(&ADD_INFO),
        b'f' => Some(&DIF_INFO),
        b'i' => Some(&INT_INFO),
        b's' => Some(&SUB_INFO),
        _ => None,
    }
}

/// Drives the paint (opacity) and the feather of a mask.
// Port of: modules/skottie/src/Layer.cpp#L90-L156 (chrome/m156) (`class MaskAdapter`)
struct MaskAdapter {
    container: PropertyContainer,
    mask_paint: Rc<SgColor>,
    blend_mode: BlendMode,
    /// Optional "feather".
    mask_filter: Option<Rc<BlurImageFilter>>,
    feather: Prop<V2>,
    opacity: Prop<f32>,
}

impl MaskAdapter {
    // Port of: modules/skottie/src/Layer.cpp#L92-L108 (chrome/m156)
    fn new(jmask: &ObjectValue, abuilder: &AnimationBuilder<'_>, bm: BlendMode) -> Rc<Self> {
        let adapter = Rc::new_cyclic(|weak: &Weak<Self>| {
            let container = PropertyContainer::new(weak.clone());
            let mask_paint = SgColor::make(Color::BLACK);
            mask_paint.set_anti_alias(true);
            if !Self::requires_isolation_for(bm) {
                // We can mask at draw time.
                mask_paint.set_blend_mode(bm);
            }

            let opacity = Prop::new(100.0);
            let feather = Prop::new(V2::new(0.0, 0.0));
            container.bind(abuilder, jmask.get("o"), &opacity);

            let mut mask_filter = None;
            if container.bind(abuilder, jmask.get("f"), &feather) {
                let filter = BlurImageFilter::make();
                // Mask feathers don't repeat edge pixels.
                filter.set_tile_mode(TileMode::Decal);
                mask_filter = Some(filter);
            }

            Self {
                container,
                mask_paint,
                blend_mode: bm,
                mask_filter,
                feather,
                opacity,
            }
        });
        adapter.container.shrink_to_fit();
        adapter
    }

    // Port of: modules/skottie/src/Layer.cpp#L110-L114 (chrome/m156) (`hasEffect`)
    fn has_effect(&self) -> bool {
        !self.is_static() || self.opacity.get() < 100.0 || self.feather.get() != V2::new(0.0, 0.0)
    }

    // Port of: modules/skottie/src/Layer.cpp#L116-L130 (chrome/m156) (`makeMask`)
    fn make_mask(&self, mask_path: Rc<SgPath>) -> Rc<dyn RenderNode> {
        let mut mask: Rc<dyn RenderNode> = Draw::make(
            Some(mask_path as Rc<dyn GeometryNode>),
            Some(Rc::clone(&self.mask_paint) as Rc<dyn PaintNode>),
        )
        .expect("the mask path and paint are not null");

        // Optional mask blur (feather).
        mask = ImageFilterEffect::make(
            mask,
            self.mask_filter
                .clone()
                .map(|filter| filter as Rc<dyn ImageFilterNode>),
        );

        if self.requires_isolation() {
            mask = LayerEffect::make(Some(mask), self.blend_mode).expect("the mask is not null");
        }

        mask
    }

    // Port of: modules/skottie/src/Layer.cpp#L141-L153 (chrome/m156) (`requires_isolation`)
    fn requires_isolation_for(blend_mode: BlendMode) -> bool {
        debug_assert!(matches!(
            blend_mode,
            BlendMode::Src
                | BlendMode::SrcOver
                | BlendMode::SrcIn
                | BlendMode::DstOut
                | BlendMode::Xor
        ));

        // Some mask modes touch pixels outside the immediate draw geometry.
        // These require a layer.
        matches!(blend_mode, BlendMode::SrcIn)
    }

    fn requires_isolation(&self) -> bool {
        Self::requires_isolation_for(self.blend_mode)
    }
}

impl AnimatablePropertyContainer for MaskAdapter {
    fn container(&self) -> &PropertyContainer {
        &self.container
    }

    // Port of: modules/skottie/src/Layer.cpp#L132-L139 (chrome/m156) (`onSync`)
    fn on_sync(&self) {
        self.mask_paint.set_opacity(self.opacity.get() * 0.01);
        if let Some(mask_filter) = &self.mask_filter {
            // Close enough to AE.
            const FEATHER_TO_SIGMA: f32 = 0.38;
            let feather = self.feather.get();
            mask_filter.set_sigma((feather.x * FEATHER_TO_SIGMA, feather.y * FEATHER_TO_SIGMA));
        }
    }
}

impl_container_animator!(MaskAdapter);

/// One mask of a layer.
// Port of: modules/skottie/src/Layer.cpp#L162-L166 (chrome/m156) (`AttachMask::MaskRecord`)
struct MaskRecord {
    /// For clipping and masking.
    mask_path: Rc<SgPath>,
    /// For masking.
    mask_adapter: Rc<MaskAdapter>,
    /// For clipping.
    merge_mode: MergeMode,
}

// Port of: modules/skottie/src/Layer.cpp#L158-L262 (chrome/m156) (`AttachMask`)
fn attach_mask(
    jmask: Option<&ArrayValue>,
    abuilder: &AnimationBuilder<'_>,
    child_node: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let Some(jmask) = jmask else {
        return child_node;
    };

    let mut mask_stack: Vec<MaskRecord> = Vec::with_capacity(4);
    let mut has_effect = false;

    for i in 0..jmask.size() {
        let Some(m) = jmask[i].as_object() else {
            continue;
        };

        let jmode = m.get("mode").as_string();
        let Some(jmode) = jmode.filter(|jmode| jmode.size() == 1) else {
            abuilder.log_json(LoggerLevel::Error, m.get("mode"), "Invalid mask mode.");
            continue;
        };

        let mode = jmode.as_bytes()[0];
        if mode == b'n' {
            // "None" masks have no effect.
            continue;
        }

        let Some(mask_info) = get_mask_info(mode) else {
            abuilder.log(
                LoggerLevel::Warning,
                &format!("Unsupported mask mode: '{}'.", char::from(mode)),
            );
            continue;
        };

        let Some(mask_path) = abuilder.attach_path(m.get("pt")) else {
            abuilder.log_json(LoggerLevel::Error, m, "Could not parse mask path.");
            continue;
        };

        let mut mask_blend_mode = mask_info.blend_mode;
        let mut mask_merge_mode = mask_info.merge_mode;
        let mut mask_inverted = parse_default::<bool>(m.get("inv"), false);

        if mask_stack.is_empty() {
            // First mask adjustments:
            //   - always draw in source mode
            //   - invert geometry if needed
            mask_blend_mode = BlendMode::Src;
            mask_merge_mode = MergeMode::Merge;
            mask_inverted = mask_inverted != mask_info.invert_geometry;
        }

        mask_path.set_fill_type(if mask_inverted {
            PathFillType::InverseWinding
        } else {
            PathFillType::Winding
        });

        let mask_adapter = MaskAdapter::new(m, abuilder, mask_blend_mode);
        abuilder.attach_discardable_adapter(&mask_adapter);

        has_effect |= mask_adapter.has_effect();

        mask_stack.push(MaskRecord {
            mask_path,
            mask_adapter,
            merge_mode: mask_merge_mode,
        });
    }

    if mask_stack.is_empty() {
        return child_node;
    }

    // If the masks are fully opaque, we can clip.
    if !has_effect {
        let clip_node: Rc<dyn GeometryNode> = if mask_stack.len() == 1 {
            // Single path -> just clip.
            Rc::clone(&mask_stack[0].mask_path) as Rc<dyn GeometryNode>
        } else {
            // Multiple clip paths -> merge.
            let merge_recs: Vec<MergeRec> = mask_stack
                .iter()
                .map(|mask| MergeRec {
                    geo: Rc::clone(&mask.mask_path) as Rc<dyn GeometryNode>,
                    mode: mask.merge_mode,
                })
                .collect();
            Merge::make(&merge_recs)
        };

        return ClipEffect::make(child_node, Some(clip_node), true, false)
            .map(|clip| clip as Rc<dyn RenderNode>);
    }

    // Complex masks (non-opaque or blurred) turn into a mask node stack.
    let mask_node: Rc<dyn RenderNode> = if mask_stack.len() == 1 {
        // no group needed for single mask
        let rec = &mask_stack[0];
        rec.mask_adapter.make_mask(Rc::clone(&rec.mask_path))
    } else {
        let masks: Vec<Rc<dyn RenderNode>> = mask_stack
            .iter()
            .map(|rec| rec.mask_adapter.make_mask(Rc::clone(&rec.mask_path)))
            .collect();

        Group::make_with_children(&masks)
    };

    MaskEffect::make(child_node, Some(mask_node), MaskMode::AlphaNormal)
        .map(|mask| mask as Rc<dyn RenderNode>)
}

/// Seeks the animators of a layer, and shows the layer while it is active.
// Port of: modules/skottie/src/Layer.cpp#L264-L304 (chrome/m156) (`class LayerController`)
struct LayerController {
    layer_animators: AnimatorScope,
    layer_node: Option<Rc<dyn RenderNode>>,
    transform_animators_count: usize,
    in_point: f32,
    out_point: f32,
}

impl Animator for LayerController {
    // Port of: modules/skottie/src/Layer.cpp#L276-L296 (chrome/m156) (`onSeek`)
    fn seek(&self, t: f32) -> StateChanged {
        // in/out may be inverted for time-reversed layers
        let active = (t >= self.in_point && t < self.out_point)
            || (t > self.out_point && t <= self.in_point);

        let mut changed = false;
        if let Some(layer_node) = &self.layer_node {
            changed |= layer_node.is_visible() != active;
            layer_node.set_visible(active);
        }

        // When active, dispatch ticks to all layer animators.
        // When inactive, we must still dispatch ticks to the layer transform animators
        // (active child layers depend on transforms being updated).
        let dispatch_count = if active {
            self.layer_animators.len()
        } else {
            self.transform_animators_count
        };
        for animator in self.layer_animators.iter().take(dispatch_count) {
            changed |= animator.seek(t);
        }

        changed
    }
}

// AE is annoyingly inconsistent in how effects interact with layer transforms: depending on
// the layer type, effects are applied before or after the content is transformed.
//
// Empirically, pre-rendered layers (for some loose meaning of "pre-rendered") are in the
// former category (effects are subject to transformation), while the remaining types are in
// the latter.

/// The layer transform also applies to its effects.
const TRANSFORM_EFFECTS: u32 = 0x01;
/// Dispatch all `seek()` events even when the layer is inactive.
const FORCE_SEEK: u32 = 0x02;

/// The kinds of layers that have content builders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayerKind {
    Precomp,
    Solid,
    Footage,
    Null,
    Shape,
    Text,
    Audio,
}

#[derive(Debug, Clone, Copy, Default)]
struct BuilderInfo {
    kind: Option<LayerKind>,
    flags: u32,
}

/// The type of a transform chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransformType {
    K2D = 0,
    K3D = 1,
}

// Port of: modules/skottie/src/Layer.h#L61-L68 (chrome/m156) (`LayerBuilder::Flags`)
/// 3D layer ("ddd": 1) or camera layer.
const IS_3D: u32 = 0x04;
/// The content tree has been built.
const BUILT_CONTENT: u32 = 0x08;

/// Builds the transform chain and the render tree of one layer.
// Port of: modules/skottie/src/Layer.h#L33-L113 (chrome/m156) (`class LayerBuilder`)
#[doc(alias = "skottie::internal::LayerBuilder")]
pub struct LayerBuilder<'j> {
    jlayer: &'j ObjectValue,
    index: i32,
    parent_index: i32,
    layer_type: i32,
    auto_orient: bool,

    info: RefCell<LayerInfo>,
    builder_info: BuilderInfo,
    /// This layer's transform node.
    layer_transform: RefCell<Option<Rc<dyn Transform>>>,
    /// Cached 2D/3D chain for the local node.
    transform_cache: RefCell<[Option<Rc<dyn Transform>>; 2]>,
    /// Render tree for layer content, excluding mask/matte and blending.
    content_tree: RefCell<Option<Rc<dyn RenderNode>>>,

    /// Layer-scoped animators.
    layer_scope: RefCell<AnimatorScope>,
    /// Transform-related animator count.
    transform_animator_count: Cell<usize>,
    flags: Cell<u32>,
}

impl<'j> LayerBuilder<'j> {
    /// The builder of the layer `jlayer` of a composition of size `comp_size`.
    // Port of: modules/skottie/src/Layer.cpp#L306-L347 (chrome/m156)
    #[must_use]
    pub fn new(jlayer: &'j ObjectValue, comp_size: Size) -> Self {
        let layer_type = parse_default::<i32>(jlayer.get("ty"), -1);

        // This table maps the 'ty' field to the appropriate layer building member function
        let builder_info = match layer_type {
            0 => BuilderInfo {
                kind: Some(LayerKind::Precomp),
                flags: TRANSFORM_EFFECTS,
            }, // precomp
            1 => BuilderInfo {
                kind: Some(LayerKind::Solid),
                flags: TRANSFORM_EFFECTS,
            }, // solid
            2 | 9 => BuilderInfo {
                kind: Some(LayerKind::Footage),
                flags: TRANSFORM_EFFECTS,
            }, // image, video
            3 | 13 => BuilderInfo {
                kind: Some(LayerKind::Null),
                flags: 0,
            }, // null, camera
            4 => BuilderInfo {
                kind: Some(LayerKind::Shape),
                flags: 0,
            }, // shape
            5 => BuilderInfo {
                kind: Some(LayerKind::Text),
                flags: 0,
            }, // text
            6 => BuilderInfo {
                kind: Some(LayerKind::Audio),
                flags: FORCE_SEEK,
            }, // audio
            // 7 pholderVideo, 8 imageSeq, 10 pholderStill, 11 guide, 12 adjustment, 14 light
            _ => BuilderInfo::default(),
        };

        let this = Self {
            jlayer,
            index: parse_default::<i32>(jlayer.get("ind"), -1),
            parent_index: parse_default::<i32>(jlayer.get("parent"), -1),
            layer_type,
            auto_orient: parse_default::<i32>(jlayer.get("ao"), 0) != 0,
            info: RefCell::new(LayerInfo {
                name: parse_default::<String>(jlayer.get("nm"), String::new()),
                size: comp_size,
                in_point: parse_default::<f32>(jlayer.get("ip"), 0.0),
                out_point: parse_default::<f32>(jlayer.get("op"), 0.0),
            }),
            builder_info,
            layer_transform: RefCell::new(None),
            transform_cache: RefCell::new([None, None]),
            content_tree: RefCell::new(None),
            layer_scope: RefCell::new(Vec::new()),
            transform_animator_count: Cell::new(0),
            flags: Cell::new(0),
        };

        if this.is_camera() || parse_default::<i32>(jlayer.get("ddd"), 0) != 0 {
            this.flags.set(this.flags.get() | IS_3D);
        }

        this
    }

    /// The "ind" of the layer.
    #[must_use]
    pub fn index(&self) -> i32 {
        self.index
    }

    /// True if this is a camera layer.
    // Port of: modules/skottie/src/Layer.cpp#L353-L357 (chrome/m156) (`isCamera`)
    #[doc(alias = "isCamera")]
    #[must_use]
    pub fn is_camera(&self) -> bool {
        const CAMERA_LAYER_TYPE: i32 = 13;

        self.layer_type == CAMERA_LAYER_TYPE
    }

    /// The size of the layer.
    #[must_use]
    pub fn size(&self) -> Size {
        self.info.borrow().size
    }

    fn is_3d(&self) -> bool {
        self.flags.get() & IS_3D != 0
    }

    /// Attaches the local and ancestor transform chain for the layer "native" type.
    // Port of: modules/skottie/src/Layer.cpp#L359-L368 (chrome/m156) (`buildTransform`)
    #[doc(alias = "buildTransform")]
    pub fn build_transform(
        &self,
        abuilder: &AnimationBuilder<'j>,
        cbuilder: &CompositionBuilder<'j>,
    ) -> Option<Rc<dyn Transform>> {
        // Depending on the leaf node type, we treat the whole transform chain as either 2D or 3D.
        let transform_chain_type = if self.is_3d() {
            TransformType::K3D
        } else {
            TransformType::K2D
        };
        let layer_transform = self.get_transform(abuilder, cbuilder, transform_chain_type);
        self.layer_transform
            .borrow_mut()
            .clone_from(&layer_transform);

        layer_transform
    }

    /// Attaches (if needed) and caches the transform chain for a given layer, as either a 2D or
    /// 3D chain type. Called transitively (and possibly repeatedly) to resolve layer parenting.
    // Port of: modules/skottie/src/Layer.cpp#L370-L389 (chrome/m156) (`getTransform`)
    fn get_transform(
        &self,
        abuilder: &AnimationBuilder<'j>,
        cbuilder: &CompositionBuilder<'j>,
        ttype: TransformType,
    ) -> Option<Rc<dyn Transform>> {
        let cache_valid_mask = 1_u32 << (ttype as u32);
        if self.flags.get() & cache_valid_mask == 0 {
            // Set valid flag upfront to break cycles.
            self.flags.set(self.flags.get() | cache_valid_mask);

            let _apt = AutoPropertyTracker::new(abuilder, self.jlayer, NodeType::Layer);
            let scope = std::mem::take(&mut *self.layer_scope.borrow_mut());
            let ascope = AutoScope::with_scope(abuilder, scope);
            let transform = self.do_attach_transform(abuilder, cbuilder, ttype);
            self.transform_cache.borrow_mut()[ttype as usize] = transform;
            let layer_scope = ascope.release();
            self.transform_animator_count.set(layer_scope.len());
            *self.layer_scope.borrow_mut() = layer_scope;
        }

        self.transform_cache.borrow()[ttype as usize].clone()
    }

    // Port of: modules/skottie/src/Layer.cpp#L391-L406 (chrome/m156) (`getParentTransform`)
    fn get_parent_transform(
        &self,
        abuilder: &AnimationBuilder<'j>,
        cbuilder: &CompositionBuilder<'j>,
        ttype: TransformType,
    ) -> Option<Rc<dyn Transform>> {
        if let Some(parent_builder) = cbuilder.layer_builder(self.parent_index) {
            // Explicit parent layer.
            return parent_builder.get_transform(abuilder, cbuilder, ttype);
        }

        // Camera layers have no implicit parent transform,
        // while regular 3D transform chains are implicitly rooted onto the camera.
        if ttype == TransformType::K3D && !self.is_camera() {
            return cbuilder.camera_transform();
        }

        None
    }

    // Port of: modules/skottie/src/Layer.cpp#L408-L434 (chrome/m156) (`doAttachTransform`)
    fn do_attach_transform(
        &self,
        abuilder: &AnimationBuilder<'j>,
        cbuilder: &CompositionBuilder<'j>,
        ttype: TransformType,
    ) -> Option<Rc<dyn Transform>> {
        let jtransform = self.jlayer.get("ks").as_object()?;

        let parent_transform = self.get_parent_transform(abuilder, cbuilder, ttype);

        if self.is_camera() {
            // parent_transform applies to the camera itself => it pre-composes inverted to the
            // camera/view/adapter transform.
            //
            //   T_camera' = T_camera x Inv(parent_transform)
            //
            return abuilder.attach_camera(
                self.jlayer,
                jtransform,
                make_inverse(parent_transform),
                cbuilder.size(),
            );
        }

        if self.is_3d() {
            abuilder.attach_matrix_3d(jtransform, parent_transform, self.auto_orient)
        } else {
            abuilder.attach_matrix_2d(jtransform, parent_transform, self.auto_orient)
        }
    }

    /// The layer content tree: built once per layer.
    // Port of: modules/skottie/src/Layer.cpp#L436-L446 (chrome/m156) (`getContentTree`)
    #[doc(alias = "getContentTree")]
    pub fn get_content_tree(
        &self,
        abuilder: &AnimationBuilder<'j>,
        cbuilder: &CompositionBuilder<'j>,
    ) -> Option<Rc<dyn RenderNode>> {
        if self.flags.get() & BUILT_CONTENT == 0 {
            // Set the flag first to prevent reference cycles.
            self.flags.set(self.flags.get() | BUILT_CONTENT);

            let content = self.build_content_tree(abuilder, cbuilder);
            *self.content_tree.borrow_mut() = content;
        }

        self.content_tree.borrow().clone()
    }

    /// Attaches the layer content (excluding motion blur, layer controller, and mattes). Can be
    /// called transitively, but only once per layer (via `get_content_tree`, which caches the
    /// result).
    // Port of: modules/skottie/src/Layer.cpp#L448-L541 (chrome/m156) (`buildContentTree`)
    fn build_content_tree(
        &self,
        abuilder: &AnimationBuilder<'j>,
        cbuilder: &CompositionBuilder<'j>,
    ) -> Option<Rc<dyn RenderNode>> {
        let _apt = AutoPropertyTracker::new(abuilder, self.jlayer, NodeType::Layer);

        // Switch to the layer animator scope (which at this point holds transform-only animators).
        let scope = std::mem::take(&mut *self.layer_scope.borrow_mut());
        let ascope = AutoScope::with_scope(abuilder, scope);

        // Potentially null.
        let mut layer: Option<Rc<dyn RenderNode>> = None;

        // Build the layer content fragment.
        if let Some(kind) = self.builder_info.kind {
            let mut info = self.info.borrow().clone();
            layer = match kind {
                LayerKind::Precomp => abuilder.attach_precomp_layer(self.jlayer, &mut info),
                LayerKind::Solid => abuilder.attach_solid_layer(self.jlayer, &mut info),
                LayerKind::Footage => abuilder.attach_footage_layer(self.jlayer, &mut info),
                LayerKind::Null => abuilder.attach_null_layer(self.jlayer, &mut info),
                LayerKind::Shape => abuilder.attach_shape_layer(self.jlayer, &mut info),
                LayerKind::Text => abuilder.attach_text_layer(self.jlayer, &mut info),
                LayerKind::Audio => abuilder.attach_audio_layer(self.jlayer, &mut info),
            };
            *self.info.borrow_mut() = info;
        }

        // Clip layers with explicit dimensions.
        if let (Some(w), Some(h)) = (
            parse_value::<f32>(self.jlayer.get("w")),
            parse_value::<f32>(self.jlayer.get("h")),
        ) {
            layer = ClipEffect::make(
                layer,
                Some(SgRect::make(Rect::from_wh(w, h)) as Rc<dyn GeometryNode>),
                /*aa=*/ true,
                /*force_clip=*/ true,
            )
            .map(|clip| clip as Rc<dyn RenderNode>);
        }

        // Optional layer mask.
        layer = attach_mask(
            self.jlayer.get("masksProperties").as_array(),
            abuilder,
            layer,
        );

        // Does the transform apply to effects also?
        // (AE quirk: it doesn't - except for solid layers)
        let transform_effects = self.builder_info.flags & TRANSFORM_EFFECTS != 0;

        let layer_transform = self.layer_transform.borrow().clone();

        // Attach the transform before effects, when needed.
        if let (Some(layer_transform), false) = (&layer_transform, transform_effects) {
            layer = TransformEffect::make(layer, Some(Rc::clone(layer_transform)))
                .map(|effect| effect as Rc<dyn RenderNode>);
        }

        let layer_size = self.info.borrow().size;

        // Optional layer effects.
        if let Some(jeffects) = self.jlayer.get("ef").as_array() {
            layer =
                EffectBuilder::new(abuilder, layer_size, cbuilder).attach_effects(jeffects, layer);
        }

        // Attach the transform after effects, when needed.
        if let (Some(layer_transform), true) = (layer_transform, transform_effects) {
            layer = TransformEffect::make(layer, Some(layer_transform))
                .map(|effect| effect as Rc<dyn RenderNode>);
        }

        // Optional layer styles.
        if let Some(jstyles) = self.jlayer.get("sy").as_array() {
            layer =
                EffectBuilder::new(abuilder, layer_size, cbuilder).attach_styles(jstyles, layer);
        }

        // Optional layer opacity.
        // TODO: de-dupe this "ks" lookup with matrix above.
        if let Some(jtransform) = self.jlayer.get("ks").as_object() {
            layer = abuilder.attach_opacity(jtransform, layer);
        }

        // Stash the layer animator scope, to be picked up later in buildRenderTree().
        *self.layer_scope.borrow_mut() = ascope.release();

        abuilder.track_layer_info(&self.info.borrow());
        layer
    }

    /// Attaches the actual layer content and finalizes its render tree. Called once per layer.
    // Port of: modules/skottie/src/Layer.cpp#L543-L588 (chrome/m156) (`buildRenderTree`)
    pub fn build_render_tree(
        &self,
        abuilder: &AnimationBuilder<'j>,
        cbuilder: &CompositionBuilder<'j>,
        prev_layer_index: i32,
    ) -> Option<Rc<dyn RenderNode>> {
        let mut layer = self.get_content_tree(abuilder, cbuilder);

        let layer_scope = std::mem::take(&mut *self.layer_scope.borrow_mut());
        let force_seek_count = if self.builder_info.flags & FORCE_SEEK != 0 {
            layer_scope.len()
        } else {
            self.transform_animator_count.get()
        };

        let (in_point, out_point) = {
            let info = self.info.borrow();
            (info.in_point, info.out_point)
        };
        abuilder.push_animator(Rc::new(LayerController {
            layer_animators: layer_scope,
            layer_node: layer.clone(),
            transform_animators_count: force_seek_count,
            in_point,
            out_point,
        }));

        let is_hidden = || {
            // If present, the 'hd' property controls visibility.
            if let Some(jhidden) = self.jlayer.get("hd").as_bool() {
                return jhidden.value();
            }

            // Legacy track matte flag, not supported in Lottie >= 1.0.
            // We only observe this in the absence of an explicit `hd` property.
            parse_default::<bool>(self.jlayer.get("td"), false)
        };

        if is_hidden() {
            return None;
        }

        // Optional matte.
        let matte_mode = parse_default::<usize>(self.jlayer.get("tt"), 0);
        if matte_mode != 0 {
            const MATTE_MODES: [MaskMode; 4] = [
                MaskMode::AlphaNormal, // tt: 1
                MaskMode::AlphaInvert, // tt: 2
                MaskMode::LumaNormal,  // tt: 3
                MaskMode::LumaInvert,  // tt: 4
            ];

            if matte_mode <= MATTE_MODES.len() {
                let mut matte_index = parse_default::<i32>(self.jlayer.get("tp"), -1);
                if matte_index < 0 {
                    // When 'tp' is not present, assume the matte source is the previous layer
                    // (legacy assets).
                    matte_index = prev_layer_index;
                }

                if matte_index >= 0 {
                    layer = MaskEffect::make(
                        layer,
                        cbuilder.layer_content(abuilder, matte_index),
                        MATTE_MODES[matte_mode - 1],
                    )
                    .map(|effect| effect as Rc<dyn RenderNode>);
                }
            } else {
                abuilder.log(
                    LoggerLevel::Error,
                    &format!("Unknown track matte mode: {matte_mode}\n"),
                );
            }
        }

        // Finally, attach an optional blend mode.
        // NB: blend modes are never applied to matte sources (layer content only).
        abuilder.attach_blend_mode(self.jlayer, layer)
    }
}

opaque_debug!(LayerBuilder<'j>);
