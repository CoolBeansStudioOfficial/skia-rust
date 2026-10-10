// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/BlendModes.cpp (chrome/m156)

use std::rc::Rc;

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blender::Blender;
use skia_rust_core::runtime_effect::RuntimeEffect;
use skia_rust_sksg::{BlenderEffect, RenderNode};

use crate::json::ObjectValue;
use crate::skottie::LoggerLevel;
use crate::skottie_json::parse_default;

use super::skottie_priv::AnimationBuilder;

/// The Lottie blend mode of `HARDMIX`: a custom blender.
// Port of: modules/skottie/src/BlendModes.cpp#L28-L30 (chrome/m156) (`CustomBlenders`)
const HARDMIX: usize = 17;

/// The hard-mix blender, built once per thread.
// Port of: modules/skottie/src/BlendModes.cpp#L32-L47 (chrome/m156) (`hardMix`)
fn hard_mix() -> Option<Blender> {
    thread_local! {
        static HARD_MIX_BLENDER: Option<Blender> = {
            const HARD_MIX: &str = "half4 main(half4 src, half4 dst) {\
                src.rgb = unpremul(src).rgb + unpremul(dst).rgb;\
                src.rgb = min(floor(src.rgb), 1) * src.a;\
                return src + (1 - src.a)*dst;\
            }";
            RuntimeEffect::make_for_blender(HARD_MIX, None)
                .ok()
                .and_then(|effect| effect.make_blender(skia_rust_core::data::Data::new_empty(), &[]))
        };
    }

    HARD_MIX_BLENDER.with(Clone::clone)
}

// Port of: modules/skottie/src/BlendModes.cpp#L49-L87 (chrome/m156) (`get_blender`)
fn get_blender(jobject: &ObjectValue, abuilder: &AnimationBuilder<'_>) -> Option<Blender> {
    const BLEND_MODE_MAP: [BlendMode; 17] = [
        BlendMode::SrcOver,    // 0:'normal'
        BlendMode::Multiply,   // 1:'multiply'
        BlendMode::Screen,     // 2:'screen'
        BlendMode::Overlay,    // 3:'overlay
        BlendMode::Darken,     // 4:'darken'
        BlendMode::Lighten,    // 5:'lighten'
        BlendMode::ColorDodge, // 6:'color-dodge'
        BlendMode::ColorBurn,  // 7:'color-burn'
        BlendMode::HardLight,  // 8:'hard-light'
        BlendMode::SoftLight,  // 9:'soft-light'
        BlendMode::Difference, // 10:'difference'
        BlendMode::Exclusion,  // 11:'exclusion'
        BlendMode::Hue,        // 12:'hue'
        BlendMode::Saturation, // 13:'saturation'
        BlendMode::Color,      // 14:'color'
        BlendMode::Luminosity, // 15:'luminosity'
        BlendMode::Plus,       // 16:'add'
    ];

    let mode = parse_default::<usize>(jobject.get("bm"), 0);

    // Special handling of src-over, so we can detect the trivial/no-fancy-blending case
    // (a null blender is equivalent to src-over).
    if mode == 0 {
        return None;
    }

    // Modes that are expressible as SkBlendMode.
    if mode < BLEND_MODE_MAP.len() {
        return Some(Blender::mode(BLEND_MODE_MAP[mode]));
    }

    // Modes that require custom blenders.
    if mode == HARDMIX {
        return hard_mix();
    }

    abuilder.log_json(
        LoggerLevel::Warning,
        jobject,
        &format!("Unsupported blend mode {mode}\n"),
    );
    None
}

impl AnimationBuilder<'_> {
    /// Attaches the blend mode of `jobject` to `child`.
    // Port of: modules/skottie/src/BlendModes.cpp#L91-L100 (chrome/m156) (`attachBlendMode`)
    #[doc(alias = "attachBlendMode")]
    #[must_use]
    pub fn attach_blend_mode(
        &self,
        jobject: &ObjectValue,
        child: Option<Rc<dyn RenderNode>>,
    ) -> Option<Rc<dyn RenderNode>> {
        let mut child = child;
        if let Some(blender) = get_blender(jobject, self) {
            self.set_has_nontrivial_blending();
            child = BlenderEffect::make(child, Some(blender)).map(|b| b as Rc<dyn RenderNode>);
        }

        child
    }
}
