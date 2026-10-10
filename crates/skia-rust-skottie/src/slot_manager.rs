// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/include/SlotManager.h, modules/skottie/src/SlotManager.cpp
// (chrome/m156)
//
// Slots: values of an animation that are given an ID in the JSON, and that clients change by ID
// after the animation is built. A slot tracks the target of the bound property and a weak handle
// of the adapter that owns it, which it syncs after setting the value. Slot IDs are kept in
// insertion order (Skia iterates a hash table).
//
// The text slots (`setTextSlot`, `getTextSlot`, `SlotInfo::fTextSlotIDs`) come with the text
// layers of M22.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::m44::V2;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_resources::{FrameData, ImageAsset, SizeFit};

use crate::internal::animator::{AnimatablePropertyContainer, Prop, ScalarTarget};
use crate::internal::skottie_priv::SceneGraphRevalidator;
use crate::skottie_value::ColorValue;

/// The ID of a slot.
// Port of: modules/skottie/include/SlotManager.h#L35 (chrome/m156) (`SlotManager::SlotID`)
pub type SlotID = String;

/// A value to change, and the means to invalidate the render tree: the adapter that interprets
/// the value before pushing it to the scene (clamping, normalizing, etc.).
// Port of: modules/skottie/include/SlotManager.h#L77-L83 (chrome/m156) (`SlotManager::ValuePair`)
struct ValuePair<T> {
    value: T,
    adapter: Weak<dyn AnimatablePropertyContainer>,
}

/// Wraps an image asset, so it can be swapped after the animation is built.
// Port of: modules/skottie/src/SlotManager.cpp#L19-L45 (chrome/m156) (`SlotManager::ImageAssetProxy`)
struct ImageAssetProxy {
    image_asset: RefCell<Option<Rc<dyn ImageAsset>>>,
}

impl ImageAssetProxy {
    fn set_image_asset(&self, asset: Option<Rc<dyn ImageAsset>>) {
        *self.image_asset.borrow_mut() = asset;
    }

    fn image_asset(&self) -> Option<Rc<dyn ImageAsset>> {
        self.image_asset.borrow().clone()
    }
}

impl ImageAsset for ImageAssetProxy {
    /// Always true, to force the footage layer to always redraw in case the asset is swapped.
    fn is_multi_frame(&self) -> bool {
        true
    }

    fn get_frame_data(&self, t: f32) -> FrameData {
        if let Some(image_asset) = &*self.image_asset.borrow() {
            return image_asset.get_frame_data(t);
        }
        FrameData {
            image: None,
            sampling: SamplingOptions::new(FilterMode::Linear, MipmapMode::Nearest),
            matrix: Matrix::new_identity(),
            scaling: SizeFit::Center,
        }
    }
}

/// The IDs of all the slots, by value type (`SlotManager::SlotInfo`).
// Port of: modules/skottie/include/SlotManager.h#L56-L62 (chrome/m156)
#[doc(alias = "skottie::SlotManager::SlotInfo")]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SlotInfo {
    /// The color slots (`fColorSlotIDs`).
    pub color_slot_ids: Vec<SlotID>,
    /// The scalar slots (`fScalarSlotIDs`).
    pub scalar_slot_ids: Vec<SlotID>,
    /// The 2D vector slots (`fVec2SlotIDs`).
    pub vec2_slot_ids: Vec<SlotID>,
    /// The image slots (`fImageSlotIDs`).
    pub image_slot_ids: Vec<SlotID>,
    /// The text slots (`fTextSlotIDs`). Text layers are not ported yet: always empty.
    pub text_slot_ids: Vec<SlotID>,
}

/// An insertion-ordered map from slot IDs to the values tracked under them.
struct SlotMap<T>(Vec<(SlotID, Vec<T>)>);

impl<T> SlotMap<T> {
    const fn new() -> Self {
        Self(Vec::new())
    }

    fn find(&self, slot_id: &str) -> Option<&Vec<T>> {
        self.0.iter().find(|(id, _)| id == slot_id).map(|(_, v)| v)
    }

    fn entry(&mut self, slot_id: &str) -> &mut Vec<T> {
        if let Some(i) = self.0.iter().position(|(id, _)| id == slot_id) {
            return &mut self.0[i].1;
        }
        self.0.push((slot_id.to_string(), Vec::new()));
        let last = self.0.len() - 1;
        &mut self.0[last].1
    }

    fn ids(&self) -> Vec<SlotID> {
        self.0.iter().map(|(id, _)| id.clone()).collect()
    }
}

/// Tracks the slots of an animation: values that clients can read and change by slot ID.
// Port of: modules/skottie/include/SlotManager.h#L27-L96 (chrome/m156) (`class SlotManager`)
#[doc(alias = "skottie::SlotManager")]
pub struct SlotManager {
    color_map: RefCell<SlotMap<ValuePair<Prop<ColorValue>>>>,
    scalar_map: RefCell<SlotMap<ValuePair<ScalarTarget>>>,
    vec2_map: RefCell<SlotMap<ValuePair<Prop<V2>>>>,
    image_map: RefCell<SlotMap<Rc<ImageAssetProxy>>>,
    revalidator: Rc<SceneGraphRevalidator>,
}

impl std::fmt::Debug for SlotManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlotManager").finish_non_exhaustive()
    }
}

impl SlotManager {
    /// A slot manager that revalidates the scene graph of `revalidator` after changes.
    // Port of: modules/skottie/src/SlotManager.cpp#L47-L48 (chrome/m156)
    #[must_use]
    pub(crate) fn new(revalidator: Rc<SceneGraphRevalidator>) -> Self {
        Self {
            color_map: RefCell::new(SlotMap::new()),
            scalar_map: RefCell::new(SlotMap::new()),
            vec2_map: RefCell::new(SlotMap::new()),
            image_map: RefCell::new(SlotMap::new()),
            revalidator,
        }
    }

    /// Sets the color of a color slot. Returns false if there is no such slot.
    // Port of: modules/skottie/src/SlotManager.cpp#L52-L65 (chrome/m156)
    #[doc(alias = "setColorSlot")]
    pub fn set_color_slot(&self, slot_id: &str, c: Color) -> bool {
        let c4f = Color4f::from_color(c);
        let v = ColorValue::from_slice(&[c4f.r, c4f.g, c4f.b, c4f.a]);
        let map = self.color_map.borrow();
        let Some(value_group) = map.find(slot_id) else {
            return false;
        };
        for c_pair in value_group {
            c_pair.value.set(v.clone());
            if let Some(adapter) = c_pair.adapter.upgrade() {
                adapter.on_sync();
            }
        }
        self.revalidator.revalidate();
        true
    }

    /// Sets the image asset of an image slot. Returns false if there is no such slot.
    // Port of: modules/skottie/src/SlotManager.cpp#L67-L77 (chrome/m156)
    #[doc(alias = "setImageSlot")]
    pub fn set_image_slot(&self, slot_id: &str, i: Option<&Rc<dyn ImageAsset>>) -> bool {
        let map = self.image_map.borrow();
        let Some(image_group) = map.find(slot_id) else {
            return false;
        };
        for image_asset in image_group {
            image_asset.set_image_asset(i.cloned());
        }
        self.revalidator.revalidate();
        true
    }

    /// Sets the value of a scalar slot. Returns false if there is no such slot.
    // Port of: modules/skottie/src/SlotManager.cpp#L79-L91 (chrome/m156)
    #[doc(alias = "setScalarSlot")]
    pub fn set_scalar_slot(&self, slot_id: &str, s: f32) -> bool {
        let map = self.scalar_map.borrow();
        let Some(value_group) = map.find(slot_id) else {
            return false;
        };
        for s_pair in value_group {
            s_pair.value.set(s);
            if let Some(adapter) = s_pair.adapter.upgrade() {
                adapter.on_sync();
            }
        }
        self.revalidator.revalidate();
        true
    }

    /// Sets the value of a 2D vector slot. Returns false if there is no such slot.
    // Port of: modules/skottie/src/SlotManager.cpp#L93-L105 (chrome/m156)
    #[doc(alias = "setVec2Slot")]
    pub fn set_vec2_slot(&self, slot_id: &str, v: V2) -> bool {
        let map = self.vec2_map.borrow();
        let Some(value_group) = map.find(slot_id) else {
            return false;
        };
        for v_pair in value_group {
            v_pair.value.set(v);
            if let Some(adapter) = v_pair.adapter.upgrade() {
                adapter.on_sync();
            }
        }
        self.revalidator.revalidate();
        true
    }

    /// The color of a color slot.
    // Port of: modules/skottie/src/SlotManager.cpp#L119-L123 (chrome/m156)
    #[doc(alias = "getColorSlot")]
    #[must_use]
    pub fn color_slot(&self, slot_id: &str) -> Option<Color> {
        let map = self.color_map.borrow();
        let value_group = map.find(slot_id)?;
        value_group
            .first()
            .map(|pair| pair.value.borrow().to_color())
    }

    /// The image asset of an image slot.
    // Port of: modules/skottie/src/SlotManager.cpp#L125-L130 (chrome/m156)
    #[doc(alias = "getImageSlot")]
    #[must_use]
    pub fn image_slot(&self, slot_id: &str) -> Option<Rc<dyn ImageAsset>> {
        let map = self.image_map.borrow();
        let image_group = map.find(slot_id)?;
        image_group.first()?.image_asset()
    }

    /// The value of a scalar slot.
    // Port of: modules/skottie/src/SlotManager.cpp#L132-L136 (chrome/m156)
    #[doc(alias = "getScalarSlot")]
    #[must_use]
    pub fn scalar_slot(&self, slot_id: &str) -> Option<f32> {
        let map = self.scalar_map.borrow();
        let value_group = map.find(slot_id)?;
        value_group.first().map(|pair| pair.value.get())
    }

    /// The value of a 2D vector slot.
    // Port of: modules/skottie/src/SlotManager.cpp#L138-L142 (chrome/m156)
    #[doc(alias = "getVec2Slot")]
    #[must_use]
    pub fn vec2_slot(&self, slot_id: &str) -> Option<V2> {
        let map = self.vec2_map.borrow();
        let value_group = map.find(slot_id)?;
        value_group.first().map(|pair| pair.value.get())
    }

    /// Tracks a color value under a slot ID (`trackColorValue`).
    // Port of: modules/skottie/src/SlotManager.cpp#L153-L156 (chrome/m156)
    pub(crate) fn track_color_value(
        &self,
        slot_id: &str,
        color_value: Prop<ColorValue>,
        adapter: Weak<dyn AnimatablePropertyContainer>,
    ) {
        self.color_map.borrow_mut().entry(slot_id).push(ValuePair {
            value: color_value,
            adapter,
        });
    }

    /// Tracks an image asset under a slot ID, and returns the proxy to use in its place
    /// (`trackImageValue`).
    // Port of: modules/skottie/src/SlotManager.cpp#L158-L164 (chrome/m156)
    pub(crate) fn track_image_value(
        &self,
        slot_id: &str,
        image_asset: Option<Rc<dyn ImageAsset>>,
    ) -> Rc<dyn ImageAsset> {
        let proxy = Rc::new(ImageAssetProxy {
            image_asset: RefCell::new(image_asset),
        });
        self.image_map
            .borrow_mut()
            .entry(slot_id)
            .push(Rc::clone(&proxy));
        proxy
    }

    /// Tracks a scalar value under a slot ID (`trackScalarValue`).
    // Port of: modules/skottie/src/SlotManager.cpp#L166-L170 (chrome/m156)
    pub(crate) fn track_scalar_value(
        &self,
        slot_id: &str,
        scalar_value: ScalarTarget,
        adapter: Weak<dyn AnimatablePropertyContainer>,
    ) {
        self.scalar_map.borrow_mut().entry(slot_id).push(ValuePair {
            value: scalar_value,
            adapter,
        });
    }

    /// Tracks a 2D vector value under a slot ID (`trackVec2Value`).
    // Port of: modules/skottie/src/SlotManager.cpp#L172-L176 (chrome/m156)
    pub(crate) fn track_vec2_value(
        &self,
        slot_id: &str,
        vec2_value: Prop<V2>,
        adapter: Weak<dyn AnimatablePropertyContainer>,
    ) {
        self.vec2_map.borrow_mut().entry(slot_id).push(ValuePair {
            value: vec2_value,
            adapter,
        });
    }

    /// All the slot IDs, by value type (`getSlotInfo`).
    // Port of: modules/skottie/src/SlotManager.cpp#L184-L202 (chrome/m156)
    #[doc(alias = "getSlotInfo")]
    #[must_use]
    pub fn slot_info(&self) -> SlotInfo {
        SlotInfo {
            color_slot_ids: self.color_map.borrow().ids(),
            scalar_slot_ids: self.scalar_map.borrow().ids(),
            vec2_slot_ids: self.vec2_map.borrow().ids(),
            image_slot_ids: self.image_map.borrow().ids(),
            text_slot_ids: Vec::new(),
        }
    }
}
