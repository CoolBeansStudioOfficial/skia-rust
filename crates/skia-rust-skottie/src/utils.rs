// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/utils/SkottieUtils.cpp, SkottieUtils.h (chrome/m156)
//
// `CustomPropertyManager`: collects the color, opacity and transform property handles of an
// animation by key, and gets and sets them by key. The precomp interceptor (`ExternalAnimationPrecompInterceptor`), which no ported GM uses.

use std::cell::{Ref, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::skottie::MarkerObserver;
use crate::skottie_property::{
    ColorPropertyHandle, ColorPropertyValue, LazyHandle, NodeType, OpacityPropertyHandle,
    OpacityPropertyValue, PropertyObserver, TextPropertyHandle, TextPropertyValue,
    TransformPropertyHandle, TransformPropertyValue,
};

/// A property key (`CustomPropertyManager::PropKey`).
// Port of: modules/skottie/utils/SkottieUtils.h (`CustomPropertyManager::PropKey`)
pub type PropKey = String;

/// How property keys are formed (`CustomPropertyManager::Mode`).
// Port of: modules/skottie/utils/SkottieUtils.h (`CustomPropertyManager::Mode`)
#[doc(alias = "skottie_utils::CustomPropertyManager::Mode")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Keys ignore the ancestor chain and are grouped based on the local node name.
    CollapseProperties,
    /// Keys include the ancestor node names (no grouping).
    #[default]
    NamespacedProperties,
}

/// A marker collected from the composition (`CustomPropertyManager::MarkerInfo`).
// Port of: modules/skottie/utils/SkottieUtils.h (`CustomPropertyManager::MarkerInfo`)
#[derive(Debug, Clone, PartialEq)]
pub struct MarkerInfo {
    /// The marker name.
    pub name: String,
    /// The start time.
    pub t0: f32,
    /// The end time.
    pub t1: f32,
}

/// The access that the generic accessors need from each kind of property handle.
trait HandleAccess {
    type Value: Default;
    fn get_value(&self) -> Self::Value;
    fn set_value(&self, value: &Self::Value);
}

impl HandleAccess for ColorPropertyHandle {
    type Value = ColorPropertyValue;
    fn get_value(&self) -> ColorPropertyValue {
        self.get()
    }
    fn set_value(&self, value: &ColorPropertyValue) {
        self.set(*value);
    }
}

impl HandleAccess for OpacityPropertyHandle {
    type Value = OpacityPropertyValue;
    fn get_value(&self) -> OpacityPropertyValue {
        self.get()
    }
    fn set_value(&self, value: &OpacityPropertyValue) {
        self.set(*value);
    }
}

impl HandleAccess for TextPropertyHandle {
    type Value = TextPropertyValue;
    fn get_value(&self) -> TextPropertyValue {
        self.get()
    }
    fn set_value(&self, value: &TextPropertyValue) {
        self.set(value);
    }
}

impl HandleAccess for TransformPropertyHandle {
    type Value = TransformPropertyValue;
    fn get_value(&self) -> TransformPropertyValue {
        self.get()
    }
    fn set_value(&self, value: &TransformPropertyValue) {
        self.set(value);
    }
}

/// The handles of one kind of property, by key (`PropMap<T>`). Each key has one or more handles.
// Port of: modules/skottie/utils/SkottieUtils.h (`CustomPropertyManager::PropMap`); the C++
// `unordered_map` iterates in no particular order, the keys here are sorted.
type PropMap<T> = BTreeMap<PropKey, Vec<Box<T>>>;

/// The state shared by the manager and its interceptors (`fColorMap`, `fCurrentNode`, ...).
#[derive(Default)]
struct State {
    mode: Mode,
    prefix: String,
    color_map: PropMap<ColorPropertyHandle>,
    opacity_map: PropMap<OpacityPropertyHandle>,
    text_map: PropMap<TextPropertyHandle>,
    transform_map: PropMap<TransformPropertyHandle>,
    markers: Vec<MarkerInfo>,
    current_node: String,
}

impl State {
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L185-L193 (acceptKey)
    fn accept_key(&self, name: Option<&str>, suffix: &str) -> PropKey {
        // A property without a node name is not accepted (C++ would pass a null name on).
        let Some(name) = name else {
            return String::new();
        };
        if !name.starts_with(&self.prefix) {
            return String::new();
        }
        match self.mode {
            Mode::CollapseProperties => name.to_owned(),
            Mode::NamespacedProperties => format!("{}{suffix}", self.current_node),
        }
    }
}

/// Receives the property callbacks and files the handles under their keys
/// (`CustomPropertyManager::PropertyInterceptor`).
// Port of: modules/skottie/utils/SkottieUtils.cpp#L20-L77 (PropertyInterceptor)
struct PropertyInterceptor {
    state: Rc<RefCell<State>>,
}

impl PropertyObserver for PropertyInterceptor {
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L24-L31 (onColorProperty)
    fn on_color_property(
        &self,
        node_name: Option<&str>,
        handle: LazyHandle<'_, ColorPropertyHandle>,
    ) {
        let key = self.state.borrow().accept_key(node_name, ".Color");
        if !key.is_empty() {
            let handle = handle();
            self.state
                .borrow_mut()
                .color_map
                .entry(key)
                .or_default()
                .push(handle);
        }
    }

    // Port of: modules/skottie/utils/SkottieUtils.cpp#L33-L40 (onOpacityProperty)
    fn on_opacity_property(
        &self,
        node_name: Option<&str>,
        handle: LazyHandle<'_, OpacityPropertyHandle>,
    ) {
        let key = self.state.borrow().accept_key(node_name, ".Opacity");
        if !key.is_empty() {
            let handle = handle();
            self.state
                .borrow_mut()
                .opacity_map
                .entry(key)
                .or_default()
                .push(handle);
        }
    }

    // Port of: modules/skottie/utils/SkottieUtils.cpp#L51-L58 (onTextProperty)
    fn on_text_property(
        &self,
        node_name: Option<&str>,
        handle: LazyHandle<'_, TextPropertyHandle>,
    ) {
        let key = self.state.borrow().accept_key(node_name, ".Text");
        if !key.is_empty() {
            let handle = handle();
            self.state
                .borrow_mut()
                .text_map
                .entry(key)
                .or_default()
                .push(handle);
        }
    }

    // Port of: modules/skottie/utils/SkottieUtils.cpp#L42-L49 (onTransformProperty)
    fn on_transform_property(
        &self,
        node_name: Option<&str>,
        handle: LazyHandle<'_, TransformPropertyHandle>,
    ) {
        let key = self.state.borrow().accept_key(node_name, ".Transform");
        if !key.is_empty() {
            let handle = handle();
            self.state
                .borrow_mut()
                .transform_map
                .entry(key)
                .or_default()
                .push(handle);
        }
    }

    // Port of: modules/skottie/utils/SkottieUtils.cpp#L62-L70 (onEnterNode)
    fn on_enter_node(&self, node_name: Option<&str>, _node_type: NodeType) {
        let Some(node_name) = node_name else {
            return;
        };
        let mut state = self.state.borrow_mut();
        state.current_node = if state.current_node.is_empty() {
            node_name.to_owned()
        } else {
            format!("{}.{node_name}", state.current_node)
        };
    }

    // Port of: modules/skottie/utils/SkottieUtils.cpp#L72-L83 (onLeavingNode)
    fn on_leaving_node(&self, node_name: Option<&str>, _node_type: NodeType) {
        let Some(node_name) = node_name else {
            return;
        };
        let length = node_name.len();
        let mut state = self.state.borrow_mut();
        let current = &state.current_node;
        state.current_node = if current.len() > length {
            current
                .get(..current.len() - length - 1)
                .unwrap_or_default()
                .to_owned()
        } else {
            String::new()
        };
    }
}

/// Collects the markers of the composition (`CustomPropertyManager::MarkerInterceptor`).
// Port of: modules/skottie/utils/SkottieUtils.cpp#L85-L94 (MarkerInterceptor)
struct MarkerInterceptor {
    state: Rc<RefCell<State>>,
}

impl MarkerObserver for MarkerInterceptor {
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L89-L92 (onMarker)
    fn on_marker(&self, name: &str, t0: f32, t1: f32) {
        self.state.borrow_mut().markers.push(MarkerInfo {
            name: name.to_owned(),
            t0,
            t1,
        });
    }
}

/// Collects the properties of an animation and gets and sets them by key
/// (`skottie_utils::CustomPropertyManager`).
// Port of: modules/skottie/utils/SkottieUtils.h (`class CustomPropertyManager`)
#[doc(alias = "skottie_utils::CustomPropertyManager")]
pub struct CustomPropertyManager {
    state: Rc<RefCell<State>>,
    property_interceptor: Rc<PropertyInterceptor>,
    marker_interceptor: Rc<MarkerInterceptor>,
}

impl Default for CustomPropertyManager {
    fn default() -> Self {
        Self::new(Mode::NamespacedProperties, None)
    }
}

impl std::fmt::Debug for CustomPropertyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomPropertyManager")
            .finish_non_exhaustive()
    }
}

impl CustomPropertyManager {
    /// A manager for the properties whose node names start with `prefix` (`$` by default).
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L96-L101 (CustomPropertyManager constructor)
    #[doc(alias = "CustomPropertyManager")]
    #[must_use]
    pub fn new(mode: Mode, prefix: Option<&str>) -> Self {
        let state = Rc::new(RefCell::new(State {
            mode,
            prefix: prefix.unwrap_or("$").to_owned(),
            ..State::default()
        }));
        Self {
            property_interceptor: Rc::new(PropertyInterceptor {
                state: Rc::clone(&state),
            }),
            marker_interceptor: Rc::new(MarkerInterceptor {
                state: Rc::clone(&state),
            }),
            state,
        }
    }

    /// The keys of the color properties (`getColorProps`).
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L236-L238 (getColorProps)
    #[must_use]
    pub fn get_color_props(&self) -> Vec<PropKey> {
        get_props(&self.state.borrow().color_map)
    }

    /// The value of a color property, or the default if the key is unknown (`getColor`).
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L240-L242 (getColor)
    #[must_use]
    pub fn get_color(&self, key: &str) -> ColorPropertyValue {
        get_value(key, &self.state.borrow().color_map)
    }

    /// A copy of the handle `index` of a color property (`getColorHandle`).
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L244-L247 (getColorHandle)
    #[must_use]
    pub fn get_color_handle(&self, key: &str, index: usize) -> Option<Box<ColorPropertyHandle>> {
        get_handle(key, index, &self.state.borrow().color_map)
    }

    /// Sets every handle of a color property (`setColor`). False if the key is unknown.
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L249-L251 (setColor)
    #[must_use]
    pub fn set_color(&self, key: &str, c: ColorPropertyValue) -> bool {
        set_value(key, &c, &self.state.borrow().color_map)
    }

    /// The keys of the opacity properties (`getOpacityProps`).
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L257-L259 (getOpacityProps)
    #[must_use]
    pub fn get_opacity_props(&self) -> Vec<PropKey> {
        get_props(&self.state.borrow().opacity_map)
    }

    /// The value of an opacity property, or the default if the key is unknown (`getOpacity`).
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L261-L263 (getOpacity)
    #[must_use]
    pub fn get_opacity(&self, key: &str) -> OpacityPropertyValue {
        get_value(key, &self.state.borrow().opacity_map)
    }

    /// A copy of the handle `index` of an opacity property (`getOpacityHandle`).
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L265-L268 (getOpacityHandle)
    #[must_use]
    pub fn get_opacity_handle(
        &self,
        key: &str,
        index: usize,
    ) -> Option<Box<OpacityPropertyHandle>> {
        get_handle(key, index, &self.state.borrow().opacity_map)
    }

    /// Sets every handle of an opacity property (`setOpacity`). False if the key is unknown.
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L270-L272 (setOpacity)
    #[must_use]
    pub fn set_opacity(&self, key: &str, o: OpacityPropertyValue) -> bool {
        set_value(key, &o, &self.state.borrow().opacity_map)
    }

    /// The keys of the text properties (`getTextProps`).
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L229-L231 (getTextProps)
    #[must_use]
    pub fn get_text_props(&self) -> Vec<PropKey> {
        get_props(&self.state.borrow().text_map)
    }

    /// The value of a text property, or the default if the key is unknown (`getText`).
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L233-L235 (getText)
    #[must_use]
    pub fn get_text(&self, key: &str) -> TextPropertyValue {
        get_value(key, &self.state.borrow().text_map)
    }

    /// A copy of the handle `index` of a text property (`getTextHandle`).
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L237-L240 (getTextHandle)
    #[must_use]
    pub fn get_text_handle(&self, key: &str, index: usize) -> Option<Box<TextPropertyHandle>> {
        get_handle(key, index, &self.state.borrow().text_map)
    }

    /// Sets every handle of a text property (`setText`). False if the key is unknown.
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L242-L244 (setText)
    #[must_use]
    pub fn set_text(&self, key: &str, t: &TextPropertyValue) -> bool {
        set_value(key, t, &self.state.borrow().text_map)
    }

    /// The keys of the transform properties (`getTransformProps`).
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L278-L280 (getTransformProps)
    #[must_use]
    pub fn get_transform_props(&self) -> Vec<PropKey> {
        get_props(&self.state.borrow().transform_map)
    }

    /// The value of a transform property, or the default if the key is unknown (`getTransform`).
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L282-L284 (getTransform)
    #[must_use]
    pub fn get_transform(&self, key: &str) -> TransformPropertyValue {
        get_value(key, &self.state.borrow().transform_map)
    }

    /// A copy of the handle `index` of a transform property (`getTransformHandle`).
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L286-L290 (getTransformHandle)
    #[must_use]
    pub fn get_transform_handle(
        &self,
        key: &str,
        index: usize,
    ) -> Option<Box<TransformPropertyHandle>> {
        get_handle(key, index, &self.state.borrow().transform_map)
    }

    /// Sets every handle of a transform property (`setTransform`). False if the key is unknown.
    // Port of: modules/skottie/utils/SkottieUtils.cpp#L292-L295 (setTransform)
    #[must_use]
    pub fn set_transform(&self, key: &str, t: &TransformPropertyValue) -> bool {
        set_value(key, t, &self.state.borrow().transform_map)
    }

    /// The markers collected while the animation was built (`markers`).
    // Port of: modules/skottie/utils/SkottieUtils.h (`markers`)
    #[must_use]
    pub fn markers(&self) -> Ref<'_, [MarkerInfo]> {
        Ref::map(self.state.borrow(), |state| state.markers.as_slice())
    }

    /// The observer to attach to an animation builder (`getPropertyObserver`).
    // Port of: modules/skottie/utils/SkottieUtils.h (`getPropertyObserver`)
    #[must_use]
    pub fn get_property_observer(&self) -> Rc<dyn PropertyObserver> {
        Rc::clone(&self.property_interceptor) as Rc<dyn PropertyObserver>
    }

    /// The observer to attach to an animation builder (`getMarkerObserver`).
    // Port of: modules/skottie/utils/SkottieUtils.h (`getMarkerObserver`)
    #[must_use]
    pub fn get_marker_observer(&self) -> Rc<dyn MarkerObserver> {
        Rc::clone(&self.marker_interceptor) as Rc<dyn MarkerObserver>
    }
}

// Port of: modules/skottie/utils/SkottieUtils.cpp#L103-L110 (getProps)
fn get_props<T>(container: &PropMap<T>) -> Vec<PropKey> {
    container.keys().cloned().collect()
}

// Port of: modules/skottie/utils/SkottieUtils.cpp#L112-L120 (get)
fn get_value<T: HandleAccess>(key: &str, container: &PropMap<T>) -> T::Value {
    container
        .get(key)
        .and_then(|group| group.first())
        .map_or_else(T::Value::default, |handle| handle.get_value())
}

// Port of: modules/skottie/utils/SkottieUtils.cpp#L122-L131 (getHandle)
fn get_handle<T: Clone>(key: &str, index: usize, container: &PropMap<T>) -> Option<Box<T>> {
    container
        .get(key)
        .and_then(|group| group.get(index))
        .map(|handle| Box::new(handle.as_ref().clone()))
}

// Port of: modules/skottie/utils/SkottieUtils.cpp#L133-L145 (set)
fn set_value<T: HandleAccess>(key: &str, value: &T::Value, container: &PropMap<T>) -> bool {
    let Some(group) = container.get(key) else {
        return false;
    };
    for handle in group {
        handle.set_value(value);
    }
    true
}
