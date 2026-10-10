// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/layers/SolidLayer.cpp (chrome/m156)

use std::rc::Rc;

use skia_rust_core::color::Color;
use skia_rust_core::rect::Rect;
use skia_rust_core::size::Size;
use skia_rust_core::utils::parse::find_hex;
use skia_rust_sksg::{Color as SgColor, Draw, GeometryNode, PaintNode, Rect as SgRect, RenderNode};

use crate::internal::skottie_priv::AnimationBuilder;
use crate::json::ObjectValue;
use crate::skottie::{LayerInfo, LoggerLevel};
use crate::skottie_json::{ValueExt, parse_default};

impl AnimationBuilder<'_> {
    /// Attaches a solid color layer.
    // Port of: modules/skottie/src/layers/SolidLayer.cpp#L26-L51 (chrome/m156) (`attachSolidLayer`)
    #[doc(alias = "attachSolidLayer")]
    #[must_use]
    pub fn attach_solid_layer(
        &self,
        jlayer: &ObjectValue,
        layer_info: &mut LayerInfo,
    ) -> Option<Rc<dyn RenderNode>> {
        layer_info.size = Size::new(
            parse_default::<f32>(jlayer.get("sw"), 0.0),
            parse_default::<f32>(jlayer.get("sh"), 0.0),
        );
        let hex_str = jlayer.get("sc").as_string();
        let hex = hex_str
            .map(|hex_str| hex_str.as_bytes())
            .filter(|bytes| bytes.first() == Some(&b'#'))
            .and_then(|bytes| find_hex(bytes, 1));
        let Some((_, c)) = hex.filter(|_| !layer_info.size.is_empty()) else {
            self.log_json(LoggerLevel::Error, jlayer, "Could not parse solid layer.");
            return None;
        };

        let color = Color::new(0xff00_0000 | c);

        let solid_paint = SgColor::make(color);
        solid_paint.set_anti_alias(true);
        self.dispatch_color_property(&solid_paint);

        Draw::make(
            Some(SgRect::make(Rect::from_wh(layer_info.size.width, layer_info.size.height))
                as Rc<dyn GeometryNode>),
            Some(solid_paint as Rc<dyn PaintNode>),
        )
        .map(|draw| draw as Rc<dyn RenderNode>)
    }
}
