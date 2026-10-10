// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/SkottiePriv.h, and the builder half of
// modules/skottie/src/Skottie.cpp (chrome/m156)
//
// `AnimationBuilder` parses the JSON into a scene graph and its animators. It is `const` in Skia
// (the parse functions are `const`) but keeps `mutable` state; here that state is in cells, so
// the builder is shared by `&`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use skia_rust_core::font_mgr::FontMgr;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::size::Size;
use skia_rust_resources::{ImageAsset, ResourceProvider};
use skia_rust_sksg::{Color as SgColor, OpacityEffect, RenderNode};

use crate::external_layer::PrecompInterceptor;
use crate::json::{ArrayValue, ObjectValue};
use crate::skottie::{
    BuilderFlags, ExpressionManager, LayerInfo, Logger, LoggerLevel, MarkerObserver,
};
use crate::skottie_json::{LogJson, ValueExt, parse_default, string_text};
use crate::skottie_property::{
    ColorPropertyHandle, NodeType, OpacityPropertyHandle, PropertyObserver,
    TransformPropertyHandle,
};
use crate::slot_manager::SlotManager;

use super::animator::{AnimatablePropertyContainer, Animator, DiscardableAdapterBase, Prop};
use super::composition::CompositionBuilder;
use super::transform::TransformAdapter2D;
use crate::impl_container_animator;

/// Close-enough to AE.
// Port of: modules/skottie/src/SkottiePriv.h#L49 (chrome/m156) (`kBlurSizeToSigma`)
pub const BLUR_SIZE_TO_SIGMA: f32 = 0.3;

/// A list of animators: the scope that a layer, a composition or a shape collects its animators in.
// Port of: modules/skottie/src/SkottiePriv.h#L58 (chrome/m156) (`AnimatorScope`)
pub type AnimatorScope = Vec<Rc<dyn Animator>>;

/// Revalidates the scene graph after clients change the properties of an animation.
// Port of: modules/skottie/src/SkottiePriv.h#L60-L68 (chrome/m156) (`class SceneGraphRevalidator`)
#[derive(Debug, Default)]
pub struct SceneGraphRevalidator {
    root: RefCell<Option<Rc<dyn RenderNode>>>,
}

impl SceneGraphRevalidator {
    /// Revalidates the scene graph, if it has a root.
    // Port of: modules/skottie/src/Skottie.cpp#L64-L68 (chrome/m156)
    pub fn revalidate(&self) {
        if let Some(root) = &*self.root.borrow() {
            root.revalidate(None, &Matrix::new_identity());
        }
    }

    /// Sets the root of the scene graph.
    // Port of: modules/skottie/src/Skottie.cpp#L60-L62 (chrome/m156)
    pub fn set_root(&self, root: Option<Rc<dyn RenderNode>>) {
        *self.root.borrow_mut() = root;
    }
}

/// What the parse of an animation produced (`AnimationBuilder::AnimationInfo`).
// Port of: modules/skottie/src/SkottiePriv.h#L81-L86 (chrome/m156)
pub struct AnimationInfo {
    /// The root of the scene graph.
    pub scene_root: Option<Rc<dyn RenderNode>>,
    /// The animators of the animation.
    pub animators: AnimatorScope,
    /// The slot manager.
    pub slot_manager: Rc<SlotManager>,
    /// The layers.
    pub layer_info: Vec<LayerInfo>,
}

/// An asset of the animation, and whether it is being attached (cycle detection).
// Port of: modules/skottie/src/SkottiePriv.h#L240-L243 (chrome/m156) (`AnimationBuilder::AssetInfo`)
struct AssetInfo<'j> {
    asset: &'j ObjectValue,
    /// Used for cycle detection.
    is_attaching: Cell<bool>,
}

/// An image asset and its declared size.
// Port of: modules/skottie/src/SkottiePriv.h#L245-L248 (chrome/m156) (`AnimationBuilder::FootageAssetInfo`)
pub struct FootageAssetInfo {
    /// The asset.
    pub asset: Rc<dyn ImageAsset>,
    /// The declared size.
    pub size: skia_rust_core::size::ISize,
}

/// A reference to the asset a layer points at, locked against cycles while it is alive.
// Port of: modules/skottie/src/SkottiePriv.h#L250-L271 (chrome/m156) (`AnimationBuilder::ScopedAssetRef`)
pub struct ScopedAssetRef<'j> {
    info: Option<Rc<AssetInfo<'j>>>,
}

impl<'j> ScopedAssetRef<'j> {
    /// The reference of the asset of `jlayer`, if there is one that is not being attached.
    // Port of: modules/skottie/src/Composition.cpp#L24-L49 (chrome/m156)
    pub fn new(abuilder: &AnimationBuilder<'j>, jlayer: &ObjectValue) -> Self {
        let ref_id = parse_default::<String>(jlayer.get("refId"), String::new());
        if ref_id.is_empty() {
            abuilder.log(LoggerLevel::Error, "Layer missing refId.");
            return Self { info: None };
        }

        let asset_info = abuilder.assets.borrow().get(&ref_id).cloned();
        let Some(asset_info) = asset_info else {
            abuilder.log(LoggerLevel::Error, &format!("Asset not found: '{ref_id}'."));
            return Self { info: None };
        };

        if asset_info.is_attaching.get() {
            abuilder.log(
                LoggerLevel::Error,
                &format!("Asset cycle detected for: '{ref_id}'"),
            );
            return Self { info: None };
        }

        asset_info.is_attaching.set(true);

        Self {
            info: Some(asset_info),
        }
    }

    /// The asset, if the reference is valid (`operator bool`, `operator*`).
    #[must_use]
    pub fn asset(&self) -> Option<&'j ObjectValue> {
        self.info.as_ref().map(|info| info.asset)
    }
}

impl Drop for ScopedAssetRef<'_> {
    fn drop(&mut self) {
        if let Some(info) = &self.info {
            info.is_attaching.set(false);
        }
    }
}

/// Collects the animators attached while it is alive (`AnimationBuilder::AutoScope`).
// Port of: modules/skottie/src/SkottiePriv.h#L128-L156 (chrome/m156) (`class AutoScope`)
pub struct AutoScope<'a, 'j> {
    builder: &'a AnimationBuilder<'j>,
    prev_scope: Option<AnimatorScope>,
}

impl<'a, 'j> AutoScope<'a, 'j> {
    /// A new, empty scope.
    #[must_use]
    pub fn new(builder: &'a AnimationBuilder<'j>) -> Self {
        Self::with_scope(builder, AnimatorScope::new())
    }

    /// A scope that starts with the given animators.
    #[must_use]
    pub fn with_scope(builder: &'a AnimationBuilder<'j>, scope: AnimatorScope) -> Self {
        let prev_scope = std::mem::replace(&mut *builder.current_animator_scope.borrow_mut(), scope);
        Self {
            builder,
            prev_scope: Some(prev_scope),
        }
    }

    /// Ends the scope, and returns its animators.
    #[must_use]
    pub fn release(mut self) -> AnimatorScope {
        let prev = self.prev_scope.take().expect("the scope is released once");
        std::mem::replace(&mut *self.builder.current_animator_scope.borrow_mut(), prev)
    }
}

impl Drop for AutoScope<'_, '_> {
    fn drop(&mut self) {
        // A scope that is not released is discarded (Skia asserts that it is released).
        if let Some(prev) = self.prev_scope.take() {
            *self.builder.current_animator_scope.borrow_mut() = prev;
        }
    }
}

/// Tracks the name of the node being built, and tells the property observer about the nodes
/// being entered and left (`AnimationBuilder::AutoPropertyTracker`).
// Port of: modules/skottie/src/SkottiePriv.h#L170-L196 (chrome/m156)
pub struct AutoPropertyTracker<'a, 'j> {
    builder: &'a AnimationBuilder<'j>,
    prev_context: Option<String>,
    node_type: NodeType,
}

impl<'a, 'j> AutoPropertyTracker<'a, 'j> {
    /// Enters the node `obj`.
    // Port of: modules/skottie/src/SkottiePriv.h#L172-L181 (chrome/m156)
    #[must_use]
    pub fn new(builder: &'a AnimationBuilder<'j>, obj: &ObjectValue, node_type: NodeType) -> Self {
        let prev_context = builder.property_observer_context.borrow().clone();
        if let Some(observer) = &builder.property_observer {
            // updateContext
            let name = obj.get("nm").as_string().map(string_text);
            let context = name.or_else(|| prev_context.clone());
            *builder.property_observer_context.borrow_mut() = context.clone();
            observer.on_enter_node(context.as_deref(), node_type);
        }
        Self {
            builder,
            prev_context,
            node_type,
        }
    }
}

impl Drop for AutoPropertyTracker<'_, '_> {
    // Port of: modules/skottie/src/SkottiePriv.h#L183-L188 (chrome/m156)
    fn drop(&mut self) {
        if let Some(observer) = &self.builder.property_observer {
            let context = self.builder.property_observer_context.borrow().clone();
            observer.on_leaving_node(context.as_deref(), self.node_type);
            *self.builder.property_observer_context.borrow_mut() = self.prev_context.take();
        }
    }
}

/// Parses an animation JSON into a scene graph.
// Port of: modules/skottie/src/SkottiePriv.h#L70-L282 (chrome/m156) (`class AnimationBuilder`)
#[doc(alias = "skottie::internal::AnimationBuilder")]
pub struct AnimationBuilder<'j> {
    resource_provider: Option<Rc<dyn ResourceProvider>>,
    font_mgr: Option<FontMgr>,
    pub(crate) property_observer: Option<Rc<dyn PropertyObserver>>,
    logger: Option<Rc<dyn Logger>>,
    marker_observer: Option<Rc<dyn MarkerObserver>>,
    precomp_interceptor: Option<Rc<dyn PrecompInterceptor>>,
    expression_manager: Option<Rc<dyn ExpressionManager>>,
    revalidator: Rc<SceneGraphRevalidator>,
    slot_manager: Rc<SlotManager>,
    comp_size: Size,
    duration: f32,
    frame_rate: f32,
    flags: BuilderFlags,
    layer_info: RefCell<Vec<LayerInfo>>,
    pub(crate) current_animator_scope: RefCell<AnimatorScope>,
    pub(crate) property_observer_context: RefCell<Option<String>>,
    has_nontrivial_blending: Cell<bool>,
    assets: RefCell<HashMap<String, Rc<AssetInfo<'j>>>>,
    image_asset_cache: RefCell<HashMap<String, Rc<FootageAssetInfo>>>,
    /// Handle to the "slots" JSON object, used to grab slot values while building.
    slots_root: Cell<Option<&'j ObjectValue>>,
}

impl<'j> AnimationBuilder<'j> {
    /// A builder for an animation of the given size, duration (in seconds), frame rate and flags.
    // Port of: modules/skottie/src/Skottie.cpp#L124-L146 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ constructor
    #[must_use]
    pub fn new(
        rp: Option<Rc<dyn ResourceProvider>>,
        fontmgr: Option<FontMgr>,
        pobserver: Option<Rc<dyn PropertyObserver>>,
        logger: Option<Rc<dyn Logger>>,
        mobserver: Option<Rc<dyn MarkerObserver>>,
        pi: Option<Rc<dyn PrecompInterceptor>>,
        expressionmgr: Option<Rc<dyn ExpressionManager>>,
        comp_size: Size,
        duration: f32,
        framerate: f32,
        flags: BuilderFlags,
    ) -> Self {
        let revalidator = Rc::new(SceneGraphRevalidator::default());
        let slot_manager = Rc::new(SlotManager::new(Rc::clone(&revalidator)));
        Self {
            resource_provider: rp,
            font_mgr: fontmgr,
            property_observer: pobserver,
            logger,
            marker_observer: mobserver,
            precomp_interceptor: pi,
            expression_manager: expressionmgr,
            revalidator,
            slot_manager,
            comp_size,
            duration,
            frame_rate: framerate,
            flags,
            layer_info: RefCell::new(Vec::new()),
            current_animator_scope: RefCell::new(Vec::new()),
            property_observer_context: RefCell::new(None),
            has_nontrivial_blending: Cell::new(false),
            assets: RefCell::new(HashMap::new()),
            image_asset_cache: RefCell::new(HashMap::new()),
            slots_root: Cell::new(None),
        }
    }

    /// Parses the animation `jroot`.
    // Port of: modules/skottie/src/Skottie.cpp#L148-L169 (chrome/m156)
    pub fn parse(&self, jroot: &'j ObjectValue) -> AnimationInfo {
        self.dispatch_markers(jroot.get("markers").as_array());

        let ascope = AutoScope::new(self);
        let _apt = AutoPropertyTracker::new(self, jroot, NodeType::Composition);

        self.parse_assets(jroot.get("assets").as_array());
        self.parse_fonts(jroot.get("fonts").as_object(), jroot.get("chars").as_array());
        self.slots_root.set(jroot.get("slots").as_object());

        let root = CompositionBuilder::new(self, self.comp_size, jroot).build(self);

        let animators = ascope.release();

        // Point the revalidator to our final root, and perform initial revalidation.
        self.revalidator.set_root(root.clone());
        self.revalidator.revalidate();

        AnimationInfo {
            scene_root: root,
            animators,
            slot_manager: Rc::clone(&self.slot_manager),
            layer_info: std::mem::take(&mut *self.layer_info.borrow_mut()),
        }
    }

    // Port of: modules/skottie/src/Skottie.cpp#L171-L181 (chrome/m156) (`parseAssets`)
    fn parse_assets(&self, jassets: Option<&'j ArrayValue>) {
        let Some(jassets) = jassets else {
            return;
        };

        for i in 0..jassets.size() {
            if let Some(asset) = jassets[i].as_object() {
                let id = parse_default::<String>(asset.get("id"), String::new());
                self.assets.borrow_mut().insert(
                    id,
                    Rc::new(AssetInfo {
                        asset,
                        is_attaching: Cell::new(false),
                    }),
                );
            }
        }
    }

    /// Resolves the fonts of the animation. Text layers (M22) own the font table; until they are
    /// ported there is nothing to resolve.
    // Port of: modules/skottie/src/text/Font.cpp (chrome/m156) (`AnimationBuilder::parseFonts`)
    fn parse_fonts(&self, _jfonts: Option<&ObjectValue>, _jchars: Option<&ArrayValue>) {
        // TODO(M22): font table and embedded glyph fonts.
    }

    // Port of: modules/skottie/src/Skottie.cpp#L183-L210 (chrome/m156) (`dispatchMarkers`)
    fn dispatch_markers(&self, jmarkers: Option<&ArrayValue>) {
        let Some(marker_observer) = &self.marker_observer else {
            return;
        };
        let Some(jmarkers) = jmarkers else {
            return;
        };

        // For frame-number -> t conversions.
        let frame_ratio = 1.0_f32 / (self.frame_rate * self.duration);

        for i in 0..jmarkers.size() {
            let Some(m) = jmarkers[i].as_object() else {
                continue;
            };

            let name = m.get("cm").as_string();
            let time = parse_default::<f32>(m.get("tm"), -1.0);
            let duration = parse_default::<f32>(m.get("dr"), -1.0);

            match name {
                Some(name) if time >= 0.0 && duration >= 0.0 => {
                    marker_observer.on_marker(
                        &string_text(name),
                        // "tm" is in frames
                        time * frame_ratio,
                        // ... as is "dr"
                        (time + duration) * frame_ratio,
                    );
                }
                _ => self.log_json(LoggerLevel::Warning, m, "Ignoring unexpected marker."),
            }
        }
    }

    /// Logs a message.
    pub fn log(&self, level: LoggerLevel, msg: &str) {
        self.log_impl(level, None, msg);
    }

    /// Logs a message about a piece of JSON.
    pub fn log_json(&self, level: LoggerLevel, json: &dyn LogJson, msg: &str) {
        self.log_impl(level, Some(json), msg);
    }

    // Port of: modules/skottie/src/Skottie.cpp#L71-L99 (chrome/m156) (`AnimationBuilder::log`)
    fn log_impl(&self, level: LoggerLevel, json: Option<&dyn LogJson>, msg: &str) {
        let Some(logger) = &self.logger else {
            return;
        };

        // The message is formatted into a 1024 byte buffer: longer messages end in "...".
        let mut buff = msg.as_bytes().to_vec();
        if buff.len() >= 1024 {
            buff.truncate(1024 - 4);
            buff.extend_from_slice(b"...");
        }
        let buff = String::from_utf8_lossy(&buff).into_owned();

        let jsonstr = json.map_or_else(String::new, |json| {
            String::from_utf8_lossy(&json.json_text()).into_owned()
        });

        logger.log(level, &buff, Some(&jsonstr));
    }

    /// The expression manager, if there is one (`expression_manager`).
    #[must_use]
    pub fn expression_manager(&self) -> Option<&Rc<dyn ExpressionManager>> {
        self.expression_manager.as_ref()
    }

    /// The "slots" JSON object, if the animation has one (`getSlotsRoot`).
    #[doc(alias = "getSlotsRoot")]
    #[must_use]
    pub fn get_slots_root(&self) -> Option<&'j ObjectValue> {
        self.slots_root.get()
    }

    /// The slot manager of the animation.
    #[must_use]
    pub fn slot_manager(&self) -> &Rc<SlotManager> {
        &self.slot_manager
    }

    /// The scene graph revalidator.
    #[must_use]
    pub fn revalidator(&self) -> &Rc<SceneGraphRevalidator> {
        &self.revalidator
    }

    /// The resource provider, if there is one.
    #[must_use]
    pub fn resource_provider(&self) -> Option<&Rc<dyn ResourceProvider>> {
        self.resource_provider.as_ref()
    }

    /// The font manager, if there is one.
    #[must_use]
    pub fn font_manager(&self) -> Option<&FontMgr> {
        self.font_mgr.as_ref()
    }

    /// The precomp interceptor, if there is one.
    #[must_use]
    pub fn precomp_interceptor(&self) -> Option<&Rc<dyn PrecompInterceptor>> {
        self.precomp_interceptor.as_ref()
    }

    /// The frame rate of the animation.
    #[must_use]
    pub fn frame_rate(&self) -> f32 {
        self.frame_rate
    }

    /// The builder flags.
    #[must_use]
    pub fn flags(&self) -> BuilderFlags {
        self.flags
    }

    /// True if a layer of the animation uses a blend mode other than src-over.
    #[doc(alias = "hasNontrivialBlending")]
    #[must_use]
    pub fn has_nontrivial_blending(&self) -> bool {
        self.has_nontrivial_blending.get()
    }

    pub(crate) fn set_has_nontrivial_blending(&self) {
        self.has_nontrivial_blending.set(true);
    }

    pub(crate) fn track_layer_info(&self, info: &LayerInfo) {
        self.layer_info.borrow_mut().push(info.clone());
    }

    pub(crate) fn cached_footage_asset(&self, id: &str) -> Option<Rc<FootageAssetInfo>> {
        self.image_asset_cache.borrow().get(id).cloned()
    }

    pub(crate) fn cache_footage_asset(&self, id: &str, info: FootageAssetInfo) -> Rc<FootageAssetInfo> {
        let info = Rc::new(info);
        self.image_asset_cache
            .borrow_mut()
            .insert(id.to_string(), Rc::clone(&info));
        info
    }

    /// Adds an animator to the current scope.
    pub fn push_animator(&self, animator: Rc<dyn Animator>) {
        self.current_animator_scope.borrow_mut().push(animator);
    }

    /// The number of animators in the current scope.
    #[must_use]
    pub fn animator_count(&self) -> usize {
        self.current_animator_scope.borrow().len()
    }

    /// Attaches an adapter to the current scope, unless it is static: then it syncs once and is
    /// discarded (`attachDiscardableAdapter`).
    // Port of: modules/skottie/src/SkottiePriv.h#L158-L167 (chrome/m156)
    pub fn attach_discardable_adapter<A: AnimatablePropertyContainer + 'static>(
        &self,
        adapter: &Rc<A>,
    ) {
        if adapter.is_static() {
            // Fire off a synthetic tick to force a single SG sync before discarding.
            adapter.seek(0.0);
        } else {
            self.push_animator(Rc::clone(adapter) as Rc<dyn Animator>);
        }
    }

    /// The `mutable` flag of the property tracker: the node name in effect.
    pub(crate) fn property_observer_context_name(&self) -> Option<String> {
        self.property_observer_context.borrow().clone()
    }

    // Port of: modules/skottie/src/Skottie.cpp#L212-L224 (chrome/m156) (`dispatchColorProperty`)
    pub(crate) fn dispatch_color_property(&self, c: &Rc<SgColor>) -> bool {
        let dispatched = Cell::new(false);
        if let Some(observer) = &self.property_observer {
            let node_name = self.property_observer_context_name();
            observer.on_color_property(node_name.as_deref(), &|| {
                dispatched.set(true);
                Box::new(ColorPropertyHandle::with_revalidator(
                    Rc::clone(c),
                    Rc::clone(&self.revalidator),
                ))
            });
        }

        dispatched.get()
    }

    // Port of: modules/skottie/src/Skottie.cpp#L226-L238 (chrome/m156) (`dispatchOpacityProperty`)
    pub(crate) fn dispatch_opacity_property(&self, o: &Rc<OpacityEffect>) -> bool {
        let dispatched = Cell::new(false);

        if let Some(observer) = &self.property_observer {
            let node_name = self.property_observer_context_name();
            observer.on_opacity_property(node_name.as_deref(), &|| {
                dispatched.set(true);
                Box::new(OpacityPropertyHandle::with_revalidator(
                    Rc::clone(o),
                    Rc::clone(&self.revalidator),
                ))
            });
        }

        dispatched.get()
    }

    // Port of: modules/skottie/src/Skottie.cpp#L260-L272 (chrome/m156) (`dispatchTransformProperty`)
    pub(crate) fn dispatch_transform_property(&self, t: &Rc<TransformAdapter2D>) -> bool {
        let dispatched = Cell::new(false);

        if let Some(observer) = &self.property_observer {
            let node_name = self.property_observer_context_name();
            observer.on_transform_property(node_name.as_deref(), &|| {
                dispatched.set(true);
                Box::new(TransformPropertyHandle::with_revalidator(
                    Rc::clone(t),
                    Rc::clone(&self.revalidator),
                ))
            });
        }

        dispatched.get()
    }

    /// Attaches the opacity of `jobject` on top of `child_node`.
    // Port of: modules/skottie/src/Skottie.cpp#L101-L122 (chrome/m156) (`attachOpacity`)
    #[doc(alias = "attachOpacity")]
    #[must_use]
    pub fn attach_opacity(
        &self,
        jobject: &ObjectValue,
        child_node: Option<Rc<dyn RenderNode>>,
    ) -> Option<Rc<dyn RenderNode>> {
        let child_node = child_node?;

        let adapter = OpacityAdapter::make(jobject, Rc::clone(&child_node), self);
        if adapter.is_static() {
            adapter.seek(0.0);
        }
        let dispatched = self.dispatch_opacity_property(adapter.node());
        if adapter.is_static() {
            if !dispatched && adapter.node().opacity() >= 1.0 {
                // No obeservable effects - we can discard.
                return Some(child_node);
            }
        } else {
            self.push_animator(Rc::clone(&adapter) as Rc<dyn Animator>);
        }

        Some(Rc::clone(adapter.node()) as Rc<dyn RenderNode>)
    }
}

/// Drives the opacity of a layer or shape group.
// Port of: modules/skottie/src/Skottie.cpp#L101-L117 (chrome/m156) (`class OpacityAdapter`)
struct OpacityAdapter {
    base: DiscardableAdapterBase<OpacityEffect>,
    opacity: Prop<f32>,
}

impl OpacityAdapter {
    fn make(
        jobject: &ObjectValue,
        child: Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        let adapter = Rc::new_cyclic(|weak: &std::rc::Weak<Self>| {
            let base = DiscardableAdapterBase::new(
                weak.clone(),
                OpacityEffect::make(Some(child), 1.0).expect("the child is not null"),
            );
            let opacity = Prop::new(100.0);
            base.container().bind(abuilder, jobject.get("o"), &opacity);
            Self { base, opacity }
        });
        adapter.base.container().shrink_to_fit();
        adapter
    }

    fn node(&self) -> &Rc<OpacityEffect> {
        self.base.node()
    }
}

impl AnimatablePropertyContainer for OpacityAdapter {
    fn container(&self) -> &super::animator::PropertyContainer {
        self.base.container()
    }

    fn on_sync(&self) {
        self.node().set_opacity(self.opacity.get() * 0.01);
    }
}

impl_container_animator!(OpacityAdapter);

opaque_debug!(AnimationInfo, FootageAssetInfo, ScopedAssetRef<'j>, AutoScope<'a, 'j>, AutoPropertyTracker<'a, 'j>, AnimationBuilder<'j>);
