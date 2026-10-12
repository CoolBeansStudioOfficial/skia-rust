// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/layers/TextLayer.cpp (chrome/m156)
//
// The text layer: the font table of the animation (`parseFonts` and the typeface resolution), and
// `attachTextLayer`, which builds a [`TextAdapter`].

use std::rc::Rc;
use std::sync::Arc;

use skia_rust_core::font_arguments::{FontArguments, VariationPosition, variation_position};
use skia_rust_core::font_style::{FontStyle, Slant, Weight, Width};
use skia_rust_core::font_types::set_four_byte_tag;
use skia_rust_core::typeface::Typeface;
use skia_rust_sksg::RenderNode;

use crate::internal::skottie_priv::{AnimationBuilder, FontInfo};
use crate::internal::text::font::{CustomFont, CustomFontBuilder, GlyphCompMapper};
use crate::internal::text::text_adapter::TextAdapter;
use crate::json::{ArrayValue, ObjectValue};
use crate::skottie::{BuilderFlags, LayerInfo, LoggerLevel};
use crate::skottie_json::{ValueExt, parse_default, string_text};

// Port of: modules/skottie/src/layers/TextLayer.cpp#L41-L60 (chrome/m156) (`parse_map`)
fn parse_map<'a, T: Copy>(map: &[(&str, T)], str_: &'a str, result: &mut T) -> &'a str {
    // ignore leading whitespace
    let str_ = str_.trim_start_matches(' ');

    let len = str_.find(' ').unwrap_or(str_.len());

    if len != 0 {
        for (key, value) in map {
            if key.as_bytes() == &str_.as_bytes()[..len] {
                *result = *value;
                return &str_[len..];
            }
        }
    }

    str_
}

// Port of: modules/skottie/src/layers/TextLayer.cpp#L62-L115 (chrome/m156) (`FontStyle`)
fn font_style(abuilder: &AnimationBuilder<'_>, style: &str) -> FontStyle {
    const WEIGHT_MAP: [(&str, Weight); 24] = [
        ("regular", Weight::NORMAL),
        ("medium", Weight::MEDIUM),
        ("bold", Weight::BOLD),
        ("light", Weight::LIGHT),
        ("black", Weight::BLACK),
        ("thin", Weight::THIN),
        ("extra", Weight::EXTRA_BOLD),
        ("extrabold", Weight::EXTRA_BOLD),
        ("extralight", Weight::EXTRA_LIGHT),
        ("extrablack", Weight::EXTRA_BLACK),
        ("semibold", Weight::SEMI_BOLD),
        ("hairline", Weight::THIN),
        ("normal", Weight::NORMAL),
        ("plain", Weight::NORMAL),
        ("standard", Weight::NORMAL),
        ("roman", Weight::NORMAL),
        ("heavy", Weight::BLACK),
        ("demi", Weight::SEMI_BOLD),
        ("demibold", Weight::SEMI_BOLD),
        ("ultra", Weight::EXTRA_BOLD),
        ("ultrabold", Weight::EXTRA_BOLD),
        ("ultrablack", Weight::EXTRA_BLACK),
        ("ultraheavy", Weight::EXTRA_BLACK),
        ("ultralight", Weight::EXTRA_LIGHT),
    ];
    const SLANT_MAP: [(&str, Slant); 2] = [("italic", Slant::Italic), ("oblique", Slant::Oblique)];

    let mut weight = Weight::NORMAL;
    let mut slant = Slant::Upright;

    // style is case insensitive.
    let lc_style = style.to_ascii_lowercase();
    let rest = parse_map(&WEIGHT_MAP, &lc_style, &mut weight);
    let rest = parse_map(&SLANT_MAP, rest, &mut slant);

    // ignore trailing whitespace
    let rest = rest.trim_start_matches(' ');

    if !rest.is_empty() {
        abuilder.log(
            LoggerLevel::Warning,
            &format!("Unknown font style: {rest}."),
        );
    }

    FontStyle::new(weight, Width::NORMAL, slant)
}

// Port of: modules/skottie/src/layers/TextLayer.cpp#L118-L146 (chrome/m156) (`ParseVariation`)
fn parse_variation(jvar: Option<&ObjectValue>) -> Vec<variation_position::Coordinate> {
    let mut var = Vec::new();
    let Some(jvar) = jvar else {
        return var;
    };

    let make_tag = |s: &[u8]| -> Option<u32> {
        // https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/font-variation-settings#string_number
        if s.len() != 4 || !s.iter().all(|&c| (0x20..=0x7e).contains(&c)) {
            return None;
        }
        Some(set_four_byte_tag(s[0], s[1], s[2], s[3]))
    };

    for jaxis in jvar.members() {
        let tag = make_tag(jaxis.key.as_bytes());
        let jnumber = jaxis.value.as_number();
        if let (Some(tag), Some(jnumber)) = (tag, jnumber) {
            #[allow(clippy::cast_possible_truncation)] // mirrors static_cast<float>(double)
            var.push(variation_position::Coordinate {
                axis: tag,
                value: jnumber.value() as f32,
            });
        }
    }
    var
}

impl FontInfo {
    // Port of: modules/skottie/src/layers/TextLayer.cpp#L148-L151 (chrome/m156)
    fn matches(&self, family: &str, style: &str) -> bool {
        self.family == family && self.style == style
    }
}

impl AnimationBuilder<'_> {
    /// Resolves the fonts of the animation: the `fonts` list, and the embedded glyphs of `chars`
    /// (`parseFonts`).
    // Port of: modules/skottie/src/layers/TextLayer.cpp#L153-L246 (chrome/m156)
    #[doc(alias = "parseFonts")]
    pub(crate) fn parse_fonts(&self, jfonts: Option<&ObjectValue>, jchars: Option<&ArrayValue>) {
        // Optional array of font entries, referenced (by name) from text layer document nodes.
        // E.g.
        // "fonts": {
        //        "list": [
        //            {
        //                "ascent": 75,
        //                "fClass": "",
        //                "fFamily": "Roboto",
        //                "fName": "Roboto-Regular",
        //                "fPath": "https://fonts.googleapis.com/css?family=Roboto",
        //                "fPath": "",
        //                "fStyle": "Regular",
        //                "fWeight": "",
        //                "origin": 1
        //            }
        //        ]
        //    },
        let jlist = jfonts.and_then(|jfonts| jfonts.get("list").as_array());
        let Some(jlist) = jlist else {
            return;
        };

        // First pass: collect font info.
        for i in 0..jlist.size() {
            let Some(jfont) = jlist[i].as_object() else {
                continue;
            };

            let jname = jfont.get("fName").as_string();
            let jfamily = jfont.get("fFamily").as_string();
            let jstyle = jfont.get("fStyle").as_string();
            let jpath = jfont.get("fPath").as_string();

            let (Some(jname), Some(jfamily), Some(jstyle)) = (jname, jfamily, jstyle) else {
                self.log_json(LoggerLevel::Error, jfont, "Invalid font.");
                continue;
            };
            if jname.size() == 0 || jfamily.size() == 0 {
                self.log_json(LoggerLevel::Error, jfont, "Invalid font.");
                continue;
            }

            self.set_font(
                string_text(jname),
                FontInfo {
                    family: string_text(jfamily),
                    style: string_text(jstyle),
                    path: jpath.map(string_text).unwrap_or_default(),
                    ascent_pct: parse_default::<f32>(jfont.get("ascent"), 0.0),
                    typeface: None, // placeholder
                    custom_font_builder: CustomFontBuilder::new(),
                    variation: parse_variation(jfont.get("fVariation").as_object()),
                },
            );
        }

        let has_comp_glyphs = |jchars: Option<&ArrayValue>| {
            let Some(jchars) = jchars else {
                return false;
            };

            for i in 0..jchars.size() {
                let Some(jchar) = jchars[i].as_object() else {
                    continue;
                };
                if parse_default::<i32>(jchar.get("t"), 0) == 1 {
                    return true;
                }
            }

            false
        };

        // Historically, Skottie has been loading native fonts before embedded glyphs, unless
        // the opposite is explicitly requested via kPreferEmbeddedFonts.  That's mostly because
        // embedded glyphs used to be just a path representation of system fonts at export time,
        // (and thus lower quality).
        //
        // OTOH embedded glyph *compositions* must be prioritized, as they are presumably more
        // expressive than the system font equivalent.
        let prioritize_embedded_fonts =
            self.flags().contains(BuilderFlags::PREFER_EMBEDDED_FONTS) || has_comp_glyphs(jchars);

        // Optional pass.
        if let Some(jchars) = jchars
            && prioritize_embedded_fonts
            && self.resolve_embedded_typefaces(jchars)
        {
            return;
        }

        // Native typeface resolution.
        if self.resolve_native_typefaces() {
            return;
        }

        // Embedded typeface fallback.
        if let Some(jchars) = jchars
            && !prioritize_embedded_fonts
        {
            self.resolve_embedded_typefaces(jchars);
        }
    }

    // Port of: modules/skottie/src/layers/TextLayer.cpp#L248-L312 (chrome/m156)
    // (`resolveNativeTypefaces`)
    fn resolve_native_typefaces(&self) -> bool {
        let mut has_unresolved = false;

        let font_mgr = self.font_manager().cloned();
        // A missing provider is Skia's `NullResourceProvider`: it loads nothing.
        let resource_provider = self.resource_provider().cloned();

        // `fFonts.foreach`: the callback can log, which does not touch the font table.
        let mut fonts = self.fonts.borrow_mut();
        for (name, finfo) in fonts.iter_mut() {
            if finfo.typeface.is_some() {
                // Already resolved from glyph paths.
                continue;
            }

            // Typeface fallback order:
            //   1) externally-loaded font (provided by the embedder)
            //   2) system font (family/style)
            //   3) system default

            finfo.typeface = resource_provider
                .as_ref()
                .and_then(|rp| rp.load_typeface(name, &finfo.path));

            // legacy API fallback
            // TODO: remove after client migration
            if finfo.typeface.is_none()
                && let Some(font_mgr) = &font_mgr
            {
                let font_data = resource_provider
                    .as_ref()
                    .and_then(|rp| rp.load_font(name, &finfo.path));
                finfo.typeface = font_mgr.make_from_data(font_data.as_ref(), 0);
            }

            if finfo.typeface.is_none()
                && let Some(font_mgr) = &font_mgr
            {
                finfo.typeface = font_mgr
                    .match_family_style(Some(&finfo.family), &font_style(self, &finfo.style));

                if finfo.typeface.is_none() {
                    self.log(
                        LoggerLevel::Error,
                        &format!(
                            "Could not create typeface for {}|{}.",
                            finfo.family, finfo.style
                        ),
                    );
                    // Last resort.
                    finfo.typeface =
                        font_mgr.legacy_make_typeface(None, font_style(self, &finfo.style));

                    has_unresolved |= finfo.typeface.is_none();
                }
            }
            if finfo.typeface.is_none() && font_mgr.is_none() {
                self.log(
                    LoggerLevel::Error,
                    &format!(
                        "Could not load typeface for {}|{} because no SkFontMgr provided.",
                        finfo.family, finfo.style
                    ),
                );
            }

            if let Some(typeface) = &finfo.typeface
                && !finfo.variation.is_empty()
            {
                // Apply optional variation coords.
                let mut args = FontArguments::new();
                args.set_variation_design_position(VariationPosition {
                    coordinates: &finfo.variation,
                });
                let tf: Typeface = typeface.make_clone(&args);

                finfo.typeface = Some(tf);
            }
        }

        !has_unresolved
    }

    // Port of: modules/skottie/src/layers/TextLayer.cpp#L314-L384 (chrome/m156)
    // (`resolveEmbeddedTypefaces`)
    fn resolve_embedded_typefaces(&self, jchars: &ArrayValue) -> bool {
        // Optional array of glyphs, to be associated with one of the declared fonts. E.g.
        // "chars": [
        //     {
        //         "fFamily": "Roboto",       // part of the font key
        //         "style": "Regular",        // part of the font key
        //         ...                        // glyph data
        //    }
        // ]
        // The font the previous glyph belonged to (`current_font`): an index in the font table.
        let mut current_font: Option<usize> = None;

        for i in 0..jchars.size() {
            let Some(jchar) = jchars[i].as_object() else {
                continue;
            };

            let jfamily = jchar.get("fFamily").as_string();
            let jstyle = jchar.get("style").as_string(); // "style", not "fStyle"...

            let (Some(jfamily), Some(jstyle)) = (jfamily, jstyle) else {
                self.log_json(LoggerLevel::Error, jchar, "Invalid glyph.");
                continue;
            };
            let family = string_text(jfamily);
            let style = string_text(jstyle);

            // Locate (and cache) the font info. Unlike text nodes, glyphs reference the font by
            // (family, style) -- not by name :(  For now this performs a linear search over
            // *all* fonts: generally there are few of them, and glyph definitions are
            // font-clustered. If problematic, we can refactor as a two-level hashmap.
            let matches_current = current_font
                .is_some_and(|index| self.fonts.borrow()[index].1.matches(&family, &style));
            if !matches_current {
                current_font = None;
                for (index, (_, finfo)) in self.fonts.borrow().iter().enumerate() {
                    if finfo.matches(&family, &style) {
                        current_font = Some(index);
                        // TODO: would be nice to break early here...
                    }
                }
                if current_font.is_none() {
                    self.log(
                        LoggerLevel::Error,
                        &format!("Font not found ({family}, {style})."),
                    );
                    continue;
                }
            }

            // The glyph parsing attaches paths and precomps, which use the builder (but not the
            // font table).
            let index = current_font.expect("the font was found");
            let mut builder =
                std::mem::take(&mut self.fonts.borrow_mut()[index].1.custom_font_builder);
            let parsed = builder.parse_glyph(self, jchar);
            self.fonts.borrow_mut()[index].1.custom_font_builder = builder;
            if !parsed {
                self.log_json(LoggerLevel::Error, jchar, "Invalid glyph.");
            }
        }

        // Final pass to commit custom typefaces.
        let mut has_unresolved = false;
        let mut custom_fonts: Vec<CustomFont> = Vec::new();
        for (_, finfo) in self.fonts.borrow_mut().iter_mut() {
            if finfo.typeface.is_some() {
                continue; // already resolved
            }

            let font = finfo.custom_font_builder.detach();

            finfo.typeface = font.typeface().cloned();

            if font.glyph_comp_count() > 0 {
                custom_fonts.push(font);
            }

            has_unresolved |= finfo.typeface.is_none();
        }

        // Stash custom font data for later use.
        if !custom_fonts.is_empty() {
            custom_fonts.shrink_to_fit();
            *self.custom_glyph_mapper.borrow_mut() =
                Some(Rc::new(GlyphCompMapper::new(custom_fonts)));
        }

        !has_unresolved
    }

    /// Attaches a text layer (`attachTextLayer`).
    // Port of: modules/skottie/src/layers/TextLayer.cpp#L386-L394 (chrome/m156)
    #[doc(alias = "attachTextLayer")]
    #[must_use]
    pub fn attach_text_layer(
        &self,
        layer: &ObjectValue,
        _info: &mut LayerInfo,
    ) -> Option<Rc<dyn RenderNode>> {
        let adapter = TextAdapter::make(
            layer,
            self,
            self.font_manager().cloned(),
            self.custom_glyph_mapper.borrow().clone(),
            self.logger_handle(),
            Arc::clone(self.shaping_factory()),
        )?;
        let node = Rc::clone(adapter.node()) as Rc<dyn RenderNode>;
        self.attach_discardable_adapter(&adapter);
        Some(node)
    }

    /// The font table entry named `font_name`, if there is one (`findFont`).
    // Port of: modules/skottie/src/layers/TextLayer.cpp#L396-L398 (chrome/m156)
    #[doc(alias = "findFont")]
    #[must_use]
    pub fn find_font(&self, font_name: &str) -> Option<std::cell::Ref<'_, FontInfo>> {
        std::cell::Ref::filter_map(self.fonts.borrow(), |fonts| {
            fonts
                .iter()
                .find(|(name, _)| name == font_name)
                .map(|(_, finfo)| finfo)
        })
        .ok()
    }
}
