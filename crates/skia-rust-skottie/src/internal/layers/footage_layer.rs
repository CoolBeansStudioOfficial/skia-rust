// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/layers/FootageLayer.cpp (chrome/m156)

use std::rc::Rc;

use skia_rust_core::matrix::{Matrix, ScaleToFit};
use skia_rust_core::rect::Rect;
use skia_rust_core::size::{ISize, Size};
use skia_rust_resources::{FrameData, ImageAsset, SizeFit};
use skia_rust_sksg::transform::Matrix as SgMatrix;
use skia_rust_sksg::{Image as SgImage, RenderNode, Transform, TransformEffect};

use crate::internal::animator::{Animator, StateChanged};
use crate::internal::skottie_priv::{AnimationBuilder, FootageAssetInfo, ScopedAssetRef};
use crate::json::ObjectValue;
use crate::skottie::{BuilderFlags, LayerInfo, LoggerLevel};
use crate::skottie_json::{ValueExt, parse_default, string_text};

// Port of: modules/skottie/src/layers/FootageLayer.cpp#L33-L47 (chrome/m156) (`image_matrix`)
fn image_matrix(frame_data: &FrameData, dest_size: ISize) -> Matrix {
    let Some(image) = &frame_data.image else {
        return Matrix::new_identity();
    };

    let size_fit_matrix = match frame_data.scaling {
        SizeFit::None => Matrix::new_identity(),
        scaling => Matrix::rect_to_rect_or_identity(
            Rect::from(image.bounds()),
            Rect::from(dest_size),
            match scaling {
                SizeFit::Fill => ScaleToFit::Fill,
                SizeFit::Start => ScaleToFit::Start,
                SizeFit::Center => ScaleToFit::Center,
                SizeFit::End => ScaleToFit::End,
                SizeFit::None => unreachable!("handled above"),
            },
        ),
    };

    &frame_data.matrix * &size_fit_matrix
}

/// Resolves the frame of an image asset for the animation time.
// Port of: modules/skottie/src/layers/FootageLayer.cpp#L49-L98 (chrome/m156) (`class FootageAnimator`)
struct FootageAnimator {
    asset: Rc<dyn ImageAsset>,
    image_node: Rc<SgImage>,
    image_transform_node: Rc<SgMatrix<Matrix>>,
    asset_size: ISize,
    time_bias: f32,
    time_scale: f32,
    is_multiframe: bool,
}

impl Animator for FootageAnimator {
    // Port of: modules/skottie/src/layers/FootageLayer.cpp#L67-L87 (chrome/m156) (`onSeek`)
    fn seek(&self, t: f32) -> StateChanged {
        if !self.is_multiframe && self.image_node.image().is_some() {
            // Single frame already resolved.
            return false;
        }

        let frame_data = self
            .asset
            .get_frame_data((t + self.time_bias) * self.time_scale);
        let m = image_matrix(&frame_data, self.asset_size);
        if frame_data.image != self.image_node.image()
            || frame_data.sampling != self.image_node.sampling_options()
            || m != self.image_transform_node.matrix()
        {
            self.image_node.set_image(frame_data.image);
            self.image_node.set_sampling_options(frame_data.sampling);
            self.image_transform_node.set_matrix(m);
            return true;
        }

        false
    }
}

impl AnimationBuilder<'_> {
    /// Loads the image asset of `default_jimage` (or the one a slot replaces it with), and caches
    /// it.
    // Port of: modules/skottie/src/layers/FootageLayer.cpp#L102-L145 (chrome/m156) (`loadFootageAsset`)
    #[doc(alias = "loadFootageAsset")]
    fn load_footage_asset(&self, default_jimage: &ObjectValue) -> Option<Rc<FootageAssetInfo>> {
        let mut jimage = Some(default_jimage);
        let slot_id = default_jimage.get("sid").as_string();
        if let Some(slot_id) = slot_id {
            match self.get_slots_root() {
                None => {
                    self.log(
                        LoggerLevel::Warning,
                        "Slotid found but no slots were found in the json. Using default asset.",
                    );
                }
                Some(slots_root) => {
                    let slot = slots_root.get(&string_text(slot_id)).as_object();
                    match slot {
                        None => self.log(
                            LoggerLevel::Warning,
                            "Specified slotID not found in 'slots'. Using default asset.",
                        ),
                        Some(slot) => jimage = slot.get("p").as_object(),
                    }
                }
            }
        }

        let jimage = jimage?;
        let name = jimage.get("p").as_string();
        let path = jimage.get("u").as_string();
        let id = jimage.get("id").as_string();
        let (Some(name), Some(path), Some(id)) = (name, path, id) else {
            return None;
        };
        let (name, path, id) = (string_text(name), string_text(path), string_text(id));

        if let Some(cached_info) = self.cached_footage_asset(&id) {
            return Some(cached_info);
        }

        let mut asset = self
            .resource_provider()
            .and_then(|rp| rp.load_image_asset(&path, &name, &id));
        if asset.is_none() && slot_id.is_none() {
            self.log(
                LoggerLevel::Error,
                &format!("Could not load image asset: {path}/{name} (id: '{id}')."),
            );
            return None;
        }

        if let Some(slot_id) = slot_id {
            asset = Some(
                self.slot_manager()
                    .track_image_value(&string_text(slot_id), asset),
            );
        }
        let size = ISize {
            width: parse_default::<i32>(jimage.get("w"), 0),
            height: parse_default::<i32>(jimage.get("h"), 0),
        };
        Some(self.cache_footage_asset(
            &id,
            FootageAssetInfo {
                asset: asset.expect("a slotted asset is wrapped in a proxy"),
                size,
            },
        ))
    }

    /// Attaches the image asset `jimage` as an image node.
    // Port of: modules/skottie/src/layers/FootageLayer.cpp#L147-L197 (chrome/m156) (`attachFootageAsset`)
    fn attach_footage_asset(
        &self,
        jimage: &ObjectValue,
        layer_info: &mut LayerInfo,
    ) -> Option<Rc<dyn RenderNode>> {
        let asset_info = self.load_footage_asset(jimage)?;

        let image_node = SgImage::make(None);

        // Optional image transform (mapping the intrinsic image size to declared asset size).
        let mut image_transform: Option<Rc<SgMatrix<Matrix>>> = None;

        let requires_animator = self.flags().contains(BuilderFlags::DEFER_IMAGE_LOADING)
            || asset_info.asset.is_multi_frame();
        if requires_animator {
            // We don't know the intrinsic image size yet (plus, in the general case,
            // the size may change from frame to frame) -> we always prepare a scaling transform.
            let transform = SgMatrix::<Matrix>::make(Matrix::new_identity());
            image_transform = Some(Rc::clone(&transform));
            self.push_animator(Rc::new(FootageAnimator {
                asset: Rc::clone(&asset_info.asset),
                image_node: Rc::clone(&image_node),
                image_transform_node: transform,
                asset_size: asset_info.size,
                time_bias: -layer_info.in_point,
                time_scale: 1.0 / self.frame_rate(),
                is_multiframe: asset_info.asset.is_multi_frame(),
            }));
        } else {
            // No animator needed, resolve the (only) frame upfront.
            let frame_data = asset_info.asset.get_frame_data(0.0);
            if frame_data.image.is_none() {
                self.log(
                    LoggerLevel::Error,
                    "Could not load single-frame image asset.",
                );
                return None;
            }

            let m = image_matrix(&frame_data, asset_info.size);
            if !m.is_identity() {
                image_transform = Some(SgMatrix::<Matrix>::make(m));
            }

            image_node.set_image(frame_data.image);
            image_node.set_sampling_options(frame_data.sampling);
        }

        // Image layers are sized explicitly.
        layer_info.size = Size::from(asset_info.size);

        let Some(image_transform) = image_transform else {
            // No resize needed.
            return Some(image_node as Rc<dyn RenderNode>);
        };

        TransformEffect::make(
            Some(image_node as Rc<dyn RenderNode>),
            Some(image_transform as Rc<dyn Transform>),
        )
        .map(|effect| effect as Rc<dyn RenderNode>)
    }

    /// Attaches a footage (image or video) layer.
    // Port of: modules/skottie/src/layers/FootageLayer.cpp#L199-L205 (chrome/m156) (`attachFootageLayer`)
    #[doc(alias = "attachFootageLayer")]
    #[must_use]
    pub fn attach_footage_layer(
        &self,
        jlayer: &ObjectValue,
        layer_info: &mut LayerInfo,
    ) -> Option<Rc<dyn RenderNode>> {
        let footage_asset = ScopedAssetRef::new(self, jlayer);

        footage_asset
            .asset()
            .and_then(|asset| self.attach_footage_asset(asset, layer_info))
    }
}
