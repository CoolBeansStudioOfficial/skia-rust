// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/text/Font.h, Font.cpp (chrome/m156)

use std::collections::HashMap;
use std::rc::Rc;

use skia_rust_core::font_types::GlyphId;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::rect::Rect;
use skia_rust_core::size::Size;
use skia_rust_core::typeface::Typeface;
use skia_rust_core::utf;
use skia_rust_sksg::transform::Matrix as SgMatrix;
use skia_rust_sksg::{RenderNode, Transform, TransformEffect};
use skia_rust_text::utils::custom_typeface::CustomTypefaceBuilder;

use crate::internal::skottie_priv::{AnimationBuilder, AutoScope};
use crate::json::ObjectValue;
use crate::skottie::LayerInfo;
use crate::skottie_json::{ValueExt, parse_default};

/// Normalizes the paths and advances for 1pt (`kPtScale`, a function-local constant in Skia).
// Port of: modules/skottie/src/text/Font.cpp#L57 and #L172 (chrome/m156)
const PT_SCALE: f32 = 0.01;

/// The glyphs of a font that are compositions, by glyph ID.
// Port of: modules/skottie/src/text/Font.h#L36 (chrome/m156) (`CustomFont::GlyphCompMap`)
type GlyphCompMap = HashMap<GlyphId, Rc<dyn RenderNode>>;

/// Font backed by Lottie character data (glyph paths and glyph compositions).
// Port of: modules/skottie/src/text/Font.h#L29-L77 (chrome/m156) (`class CustomFont`)
#[doc(alias = "skottie::internal::CustomFont")]
pub struct CustomFont {
    glyph_comps: GlyphCompMap,
    typeface: Option<Typeface>,
}

/// Builds a [`CustomFont`] from the glyphs of the `chars` of an animation.
// Port of: modules/skottie/src/text/Font.h#L38-L53 (chrome/m156) (`CustomFont::Builder`)
#[doc(alias = "skottie::internal::CustomFont::Builder")]
#[derive(Default)]
pub struct CustomFontBuilder {
    glyph_comps: GlyphCompMap,
    custom_builder: CustomTypefaceBuilder,
}

impl CustomFontBuilder {
    /// A builder with no glyphs.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses a glyph of the `chars` list (`parseGlyph`).
    // Port of: modules/skottie/src/text/Font.cpp#L30-L90 (chrome/m156)
    #[doc(alias = "parseGlyph")]
    pub fn parse_glyph(&mut self, abuilder: &AnimationBuilder<'_>, jchar: &ObjectValue) -> bool {
        // Glyph encoding:
        //     {
        //         "ch": "t",
        //         "data": <glyph data>,  // Glyph path or composition data
        //         "size": 50,            // apparently ignored
        //         "w": 32.67,            // width/advance (1/100 units)
        //         "t": 1                 // Marker for composition glyphs only.
        //     }
        let jch = jchar.get("ch").as_string();
        let jdata = jchar.get("data").as_object();
        let (Some(jch), Some(jdata)) = (jch, jdata) else {
            return false;
        };

        let ch = jch.as_bytes();
        if utf::count_utf8(ch) != 1 {
            return false;
        }

        let mut ch_ptr = ch;
        let uni = utf::next_utf8(&mut ch_ptr);
        debug_assert!(uni != -1);
        // Custom font keys are SkGlyphIDs.  We could implement a remapping scheme if needed,
        // but for now direct mapping seems to work well enough.
        let Ok(glyph_id) = GlyphId::try_from(uni) else {
            return false;
        };

        // Normalize the path and advance for 1pt.
        let advance = parse_default::<f32>(jchar.get("w"), 0.0) * PT_SCALE;

        // Custom glyphs are either compositions...
        let mut glyph_size = Size::new(0.0, 0.0);
        if let Some(comp_node) = Self::parse_glyph_comp(abuilder, jdata, &mut glyph_size) {
            // With glyph comps, we use the SkCustomTypeface only for shaping -- not for
            // rendering. We still need accurate glyph bounds though, for visual alignment.

            // TODO: This assumes the glyph origin is always in the lower-left corner.
            // Lottie may need to add an origin property, to allow designers full control over
            // glyph comp positioning.
            let glyph_bounds = Rect::from_ltrb(0.0, -glyph_size.height, glyph_size.width, 0.0);
            self.custom_builder
                .set_glyph(glyph_id, advance, &Path::rect(glyph_bounds, None));

            // Rendering is handled explicitly, post shaping,
            // based on info tracked in this GlyphCompMap.
            self.glyph_comps.insert(glyph_id, comp_node);

            return true;
        }

        // ... or paths.
        let mut path = Path::default();
        if !Self::parse_glyph_path(abuilder, jdata, &mut path) {
            return false;
        }

        path = path.make_transform(&Matrix::scale((PT_SCALE, PT_SCALE)));

        self.custom_builder.set_glyph(glyph_id, advance, &path);

        true
    }

    // Port of: modules/skottie/src/text/Font.cpp#L92-L146 (chrome/m156) (`ParseGlyphPath`)
    fn parse_glyph_path(
        abuilder: &AnimationBuilder<'_>,
        jdata: &ObjectValue,
        path: &mut Path,
    ) -> bool {
        // Glyph path encoding:
        //
        //   "data": {
        //       "shapes": [                         // follows the shape layer format
        //           {
        //               "ty": "gr",                 // group shape type
        //               "it": [                     // group items
        //                   {
        //                       "ty": "sh",         // actual shape
        //                       "ks": <path data>   // animatable path format, but always static
        //                   },
        //                   ...
        //               ]
        //           },
        //           ...
        //       ]
        //   }

        let Some(jshapes) = jdata.get("shapes").as_array() else {
            // Space/empty glyph.
            return true;
        };

        let mut builder = PathBuilder::new();
        for i in 0..jshapes.size() {
            let Some(jgrp) = jshapes[i].as_object() else {
                return false;
            };

            let Some(jit) = jgrp.get("it").as_array() else {
                return false;
            };

            for j in 0..jit.size() {
                let Some(jshape) = jit[j].as_object() else {
                    return false;
                };

                // Glyph paths should never be animated.  But they are encoded as
                // animatable properties, so we use the appropriate helpers.
                let ascope = AutoScope::new(abuilder);
                let path_node = abuilder.attach_path(jshape.get("ks"));
                let animators = ascope.release();

                let Some(path_node) = path_node else {
                    return false;
                };
                if !animators.is_empty() {
                    return false;
                }

                builder.add_path(&path_node.path(), None);
            }
        }
        *path = builder.detach();

        true
    }

    // Port of: modules/skottie/src/text/Font.cpp#L148-L208 (chrome/m156) (`ParseGlyphComp`)
    fn parse_glyph_comp(
        abuilder: &AnimationBuilder<'_>,
        jdata: &ObjectValue,
        glyph_size: &mut Size,
    ) -> Option<Rc<dyn RenderNode>> {
        // Glyph comp encoding:
        //
        //   "data": {                     // Follows the precomp layer format.
        //       "ip": <in point>,
        //       "op": <out point>,
        //       "refId": <comp ID>,
        //       "sr": <time remap info>,
        //       "st": <time remap info>,
        //       "ks": <transform info>
        //   }

        let mut linfo = LayerInfo {
            name: parse_default::<String>(jdata.get("nm"), String::new()),
            size: Size::new(0.0, 0.0),
            in_point: parse_default::<f32>(jdata.get("ip"), 0.0),
            out_point: parse_default::<f32>(jdata.get("op"), 0.0),
        };

        #[allow(clippy::float_cmp)] // C++ `!linfo.fInPoint && !linfo.fOutPoint`
        if linfo.in_point == 0.0 && linfo.out_point == 0.0 {
            // Not a comp glyph.
            return None;
        }

        // Since the glyph composition encoding matches the precomp layer encoding, we can
        // pretend we're attaching a precomp here.
        let comp_node = abuilder.attach_precomp_layer(jdata, &mut linfo);
        let _ = comp_node;

        // Normalize for 1pt.

        // For bounds/alignment purposes, we use a glyph size matching the normalized glyph comp
        // size.
        *glyph_size = Size::new(linfo.size.width * PT_SCALE, linfo.size.height * PT_SCALE);

        let mut glyph_transform: Option<Rc<dyn Transform>> = Some(SgMatrix::<Matrix>::make(
            Matrix::scale((PT_SCALE, PT_SCALE)),
        ));

        // Additional/explicit glyph transform (not handled in attachPrecompLayer).
        if let Some(jtransform) = jdata.get("ks").as_object() {
            glyph_transform = abuilder.attach_matrix_2d(jtransform, glyph_transform, false);
        }

        TransformEffect::make(
            abuilder.attach_precomp_layer(jdata, &mut linfo),
            glyph_transform,
        )
        .map(|node| node as Rc<dyn RenderNode>)
    }

    /// Makes the font (`detach`).
    // Port of: modules/skottie/src/text/Font.cpp#L210-L213 (chrome/m156)
    #[must_use]
    pub fn detach(&mut self) -> CustomFont {
        CustomFont::new(
            std::mem::take(&mut self.glyph_comps),
            self.custom_builder.detach(),
        )
    }
}

impl CustomFont {
    // Port of: modules/skottie/src/text/Font.cpp#L215-L218 (chrome/m156)
    fn new(glyph_comps: GlyphCompMap, typeface: Option<Typeface>) -> Self {
        Self {
            glyph_comps,
            typeface,
        }
    }

    /// The typeface of the font (`typeface`).
    #[must_use]
    pub fn typeface(&self) -> Option<&Typeface> {
        self.typeface.as_ref()
    }

    /// The number of glyphs that are compositions (`glyphCompCount`).
    #[doc(alias = "glyphCompCount")]
    #[must_use]
    pub fn glyph_comp_count(&self) -> usize {
        self.glyph_comps.len()
    }
}

/// Helper for resolving (typeface, glyph ID) tuples to a composition root. Used post-shaping, to
/// substitute composition glyphs in the rendering tree.
// Port of: modules/skottie/src/text/Font.h#L55-L68 (chrome/m156) (`CustomFont::GlyphCompMapper`)
#[doc(alias = "skottie::internal::CustomFont::GlyphCompMapper")]
pub struct GlyphCompMapper {
    fonts: Vec<CustomFont>,
}

impl GlyphCompMapper {
    /// A mapper for `fonts`.
    #[must_use]
    pub fn new(fonts: Vec<CustomFont>) -> Self {
        Self { fonts }
    }

    /// The composition of the glyph `gid` of the typeface `tf`, if there is one
    /// (`getGlyphComp`).
    // Port of: modules/skottie/src/text/Font.cpp#L222-L232 (chrome/m156)
    #[doc(alias = "getGlyphComp")]
    #[must_use]
    pub fn get_glyph_comp(&self, tf: &Typeface, gid: GlyphId) -> Option<Rc<dyn RenderNode>> {
        for font in &self.fonts {
            if font.typeface().is_some_and(|t| t.ptr_eq(tf)) {
                return font.glyph_comps.get(&gid).cloned();
            }
        }

        None
    }
}

opaque_debug!(CustomFont, CustomFontBuilder, GlyphCompMapper);
