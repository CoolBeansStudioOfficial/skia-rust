// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/include/FontCollection.h,
// modules/skparagraph/src/FontCollection.cpp

//! `FontCollection`: the font managers a paragraph resolves its typefaces with.

use std::sync::{Arc, Mutex, PoisonError, RwLock};

use skia_rust_core::font_mgr::FontMgr;
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::typeface::{Typeface, TypefaceId};
use skia_rust_core::utf::Unichar;

use crate::font_arguments::FontArguments;
use crate::paragraph_cache::ParagraphCache;
use crate::text_style::DEFAULT_FONT_FAMILY;

// Port of: modules/skparagraph/src/FontCollection.cpp#L18-L25 (chrome/m156)
/// `kColorEmojiLocale`: the BCP-47 tag of the color emoji fonts (`und-Zsye`).
const COLOR_EMOJI_LOCALE: &str = "und-Zsye";

// Port of: modules/skparagraph/src/FontCollection.cpp#L32-L61 (chrome/m156)
/// The key of `FaceCache`. `THashMap` finds entries by hash and then `operator==`; the equality
/// decides, and the hash only speeds the lookup up, so the cache is searched by equality.
#[derive(Debug, Clone, PartialEq)]
struct FamilyKey {
    family_names: Vec<String>,
    font_style: FontStyle,
    font_arguments: Option<FontArguments>,
}

// Port of: modules/skparagraph/src/FontCollection.cpp#L63-L83 (chrome/m156)
/// The key of `VariationCache`.
#[derive(Debug, Clone, PartialEq)]
struct VariationKey {
    typeface_id: TypefaceId,
    font_arguments: FontArguments,
}

#[derive(Debug, Default)]
struct Caches {
    // `FaceCache`
    face_cache: Vec<(FamilyKey, Vec<Typeface>)>,
    // `VariationCache`
    variation_cache: Vec<(VariationKey, Typeface)>,
}

#[derive(Debug)]
struct Config {
    enable_font_fallback: bool,
    default_font_manager: Option<FontMgr>,
    asset_font_manager: Option<FontMgr>,
    dynamic_font_manager: Option<FontMgr>,
    test_font_manager: Option<FontMgr>,
    default_family_names: Vec<String>,
}

#[derive(Debug)]
struct FontCollectionInner {
    config: RwLock<Config>,
    caches: Mutex<Caches>,
    paragraph_cache: ParagraphCache,
}

// Port of: modules/skparagraph/include/FontCollection.h#L24-L74 (chrome/m156)
/// `FontCollection`: a shared set of font managers (`sk_sp<FontCollection>`). Cloning it shares
/// the collection, its caches and its paragraph cache.
#[derive(Debug, Clone)]
pub struct FontCollection {
    inner: Arc<FontCollectionInner>,
}

impl Default for FontCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl FontCollection {
    // Port of: modules/skparagraph/src/FontCollection.cpp#L85-L89 (chrome/m156)
    /// `FontCollection()`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(FontCollectionInner {
                config: RwLock::new(Config {
                    enable_font_fallback: true,
                    default_font_manager: None,
                    asset_font_manager: None,
                    dynamic_font_manager: None,
                    test_font_manager: None,
                    default_family_names: vec![DEFAULT_FONT_FAMILY.to_owned()],
                }),
                caches: Mutex::new(Caches::default()),
                paragraph_cache: ParagraphCache::new(),
            }),
        }
    }

    fn config(&self) -> std::sync::RwLockReadGuard<'_, Config> {
        self.inner
            .config
            .read()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn config_mut(&self) -> std::sync::RwLockWriteGuard<'_, Config> {
        self.inner
            .config
            .write()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn caches(&self) -> std::sync::MutexGuard<'_, Caches> {
        self.inner
            .caches
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L93 (chrome/m156)
    /// `getFontManagersCount()`.
    #[must_use]
    #[doc(alias = "getFontManagersCount")]
    pub fn font_managers_count(&self) -> usize {
        self.font_manager_order().len()
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L95-L97 (chrome/m156)
    /// `setAssetFontManager(fontManager)`.
    #[doc(alias = "setAssetFontManager")]
    pub fn set_asset_font_manager(&mut self, font_manager: impl Into<Option<FontMgr>>) {
        self.config_mut().asset_font_manager = font_manager.into();
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L98-L100 (chrome/m156)
    /// `setDynamicFontManager(fontManager)`.
    #[doc(alias = "setDynamicFontManager")]
    pub fn set_dynamic_font_manager(&mut self, font_manager: impl Into<Option<FontMgr>>) {
        self.config_mut().dynamic_font_manager = font_manager.into();
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L101-L103 (chrome/m156)
    /// `setTestFontManager(fontManager)`.
    #[doc(alias = "setTestFontManager")]
    pub fn set_test_font_manager(&mut self, font_manager: impl Into<Option<FontMgr>>) {
        self.config_mut().test_font_manager = font_manager.into();
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L117-L119 (chrome/m156)
    /// `setDefaultFontManager(fontManager)`.
    #[doc(alias = "setDefaultFontManager")]
    pub fn set_default_font_manager(&mut self, font_manager: impl Into<Option<FontMgr>>) {
        self.config_mut().default_font_manager = font_manager.into();
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L104-L108 (chrome/m156)
    /// `setDefaultFontManager(fontManager, defaultFamilyName)`: also adds the family name to the
    /// default family names.
    #[doc(alias = "setDefaultFontManager")]
    pub fn set_default_font_manager_and_family_name(
        &mut self,
        font_manager: impl Into<Option<FontMgr>>,
        default_family_name: impl AsRef<str>,
    ) {
        let mut config = self.config_mut();
        config.default_font_manager = font_manager.into();
        config
            .default_family_names
            .push(default_family_name.as_ref().to_owned());
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L110-L115 (chrome/m156)
    /// `setDefaultFontManager(fontManager, defaultFamilyNames)`: replaces the default family
    /// names.
    #[doc(alias = "setDefaultFontManager")]
    pub fn set_default_font_manager_and_family_names(
        &mut self,
        font_manager: impl Into<Option<FontMgr>>,
        default_family_names: &[impl AsRef<str>],
    ) {
        let mut config = self.config_mut();
        config.default_font_manager = font_manager.into();
        config.default_family_names = default_family_names
            .iter()
            .map(|name| name.as_ref().to_owned())
            .collect();
    }

    // Port of: modules/skparagraph/include/FontCollection.h#L43 (chrome/m156)
    /// `getFallbackManager()`.
    #[must_use]
    #[doc(alias = "getFallbackManager")]
    pub fn fallback_manager(&self) -> Option<FontMgr> {
        self.config().default_font_manager.clone()
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L121-L137 (chrome/m156)
    /// `getFontManagerOrder()`: the font managers in the order they should be queried.
    fn font_manager_order(&self) -> Vec<FontMgr> {
        let config = self.config();
        let mut order = Vec::new();
        if let Some(manager) = &config.dynamic_font_manager {
            order.push(manager.clone());
        }
        if let Some(manager) = &config.asset_font_manager {
            order.push(manager.clone());
        }
        if let Some(manager) = &config.test_font_manager {
            order.push(manager.clone());
        }
        if let Some(manager) = &config.default_font_manager {
            if config.enable_font_fallback {
                order.push(manager.clone());
            }
        }
        order
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L139-L141 (chrome/m156)
    /// `findTypefaces(familyNames, fontStyle)`.
    #[doc(alias = "findTypefaces")]
    pub fn find_typefaces(
        &self,
        family_names: &[impl AsRef<str>],
        font_style: FontStyle,
    ) -> Vec<Typeface> {
        self.find_typefaces_with_font_arguments(family_names, font_style, None)
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L143-L193 (chrome/m156)
    /// `findTypefaces(familyNames, fontStyle, fontArgs)`.
    #[doc(alias = "findTypefaces")]
    pub fn find_typefaces_with_font_arguments(
        &self,
        family_names: &[impl AsRef<str>],
        font_style: FontStyle,
        font_args: Option<&FontArguments>,
    ) -> Vec<Typeface> {
        // Look inside the font collections cache first
        let family_key = FamilyKey {
            family_names: family_names
                .iter()
                .map(|name| name.as_ref().to_owned())
                .collect(),
            font_style,
            font_arguments: font_args.cloned(),
        };
        {
            let caches = self.caches();
            if let Some((_, found)) = caches.face_cache.iter().find(|(key, _)| *key == family_key) {
                return found.clone();
            }
        }

        let mut typefaces: Vec<Typeface> = Vec::new();
        for family_name in family_names {
            let mut matched = self.match_typeface(family_name.as_ref(), font_style);
            if let (Some(typeface), Some(args)) = (&matched, font_args) {
                matched = Some(self.clone_typeface(typeface, args));
            }
            if let Some(matched) = matched {
                typefaces.push(matched);
            }
        }

        if typefaces.is_empty() {
            let mut matched: Option<Typeface> = None;
            let default_family_names = self.config().default_family_names.clone();
            for family_name in &default_family_names {
                matched = self.match_typeface(family_name, font_style);
                if matched.is_some() {
                    break;
                }
            }
            if matched.is_none() {
                for manager in self.font_manager_order() {
                    matched = manager.legacy_make_typeface(None, font_style);
                    if matched.is_some() {
                        break;
                    }
                }
            }
            if let Some(mut matched) = matched {
                if let Some(args) = font_args {
                    matched = self.clone_typeface(&matched, args);
                }
                typefaces.push(matched);
            }
        }

        // `fFaceCache->fTypefaces.set(familyKey, typefaces)`: replaces an equal key.
        let mut caches = self.caches();
        if let Some(entry) = caches
            .face_cache
            .iter_mut()
            .find(|(key, _)| *key == family_key)
        {
            entry.1.clone_from(&typefaces);
        } else {
            caches.face_cache.push((family_key, typefaces.clone()));
        }
        typefaces
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L195-L208 (chrome/m156)
    /// `matchTypeface(familyName, fontStyle)`.
    fn match_typeface(&self, family_name: &str, font_style: FontStyle) -> Option<Typeface> {
        for manager in self.font_manager_order() {
            let set = manager.match_family(Some(family_name));
            if set.count() == 0 {
                continue;
            }
            let matched = set.match_style(&font_style);
            if matched.is_some() {
                return matched;
            }
        }
        None
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L210-L236 (chrome/m156)
    /// `defaultFallback(unicode, families, fontStyle, locale, fontArgs)`: finds ANY font in the
    /// available font managers that resolves the unicode codepoint.
    #[doc(alias = "defaultFallback")]
    pub fn default_fallback_char(
        &self,
        unicode: Unichar,
        families: &[impl AsRef<str>],
        font_style: FontStyle,
        locale: &str,
        font_args: Option<&FontArguments>,
    ) -> Option<Typeface> {
        for manager in self.font_manager_order() {
            let mut bcp47: Vec<&str> = Vec::new();
            if !locale.is_empty() {
                bcp47.push(locale);
            }
            let family_name = families.first().map(AsRef::as_ref);
            let typeface =
                manager.match_family_style_character(family_name, &font_style, &bcp47, unicode);
            if let Some(mut typeface) = typeface {
                if let Some(args) = font_args {
                    typeface = self.clone_typeface(&typeface, args);
                }
                return Some(typeface);
            }
        }
        None
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L238-L275 (chrome/m156)
    /// `defaultEmojiFallback(emojiStart, fontStyle, locale)`: finds ANY font in the available
    /// font managers that resolves this emojiStart.
    #[doc(alias = "defaultEmojiFallback")]
    #[must_use]
    pub fn default_emoji_fallback(
        &self,
        emoji_start: Unichar,
        font_style: FontStyle,
        locale: &str,
    ) -> Option<Typeface> {
        for manager in self.font_manager_order() {
            let mut bcp47: Vec<&str> = Vec::new();
            // `#if defined(SK_BUILD_FOR_MAC) || defined(SK_BUILD_FOR_IOS)` uses the
            // "Apple Color Emoji" family instead; the oracle is not a Mac build.
            bcp47.push(COLOR_EMOJI_LOCALE);
            if !locale.is_empty() {
                bcp47.push(locale);
            }
            // Not really ideal since the first codepoint may not be the best one
            // but we start from a good colored emoji at least
            let typeface =
                manager.match_family_style_character(None, &font_style, &bcp47, emoji_start);
            if typeface.is_some() {
                // ... and stop as soon as we find something in hope it will work for all of them
                return typeface;
            }
        }
        None
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L277-L290 (chrome/m156)
    /// `defaultFallback()`.
    #[doc(alias = "defaultFallback")]
    #[must_use]
    pub fn default_fallback(&self) -> Option<Typeface> {
        let (manager, default_family_names) = {
            let config = self.config();
            (
                config.default_font_manager.clone()?,
                config.default_family_names.clone(),
            )
        };
        for family_name in &default_family_names {
            let matched = manager.match_family_style(Some(family_name), &FontStyle::default());
            if matched.is_some() {
                return matched;
            }
        }
        None
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L292-L302 (chrome/m156)
    /// `cloneTypeface(typeface, args)`, with the `VariationCache`.
    fn clone_typeface(&self, typeface: &Typeface, args: &FontArguments) -> Typeface {
        let variation_key = VariationKey {
            typeface_id: typeface.unique_id(),
            font_arguments: args.clone(),
        };
        {
            let caches = self.caches();
            if let Some((_, found)) = caches
                .variation_cache
                .iter()
                .find(|(key, _)| *key == variation_key)
            {
                return found.clone();
            }
        }
        let clone = args.clone_typeface(typeface);
        let mut caches = self.caches();
        if let Some(entry) = caches
            .variation_cache
            .iter_mut()
            .find(|(key, _)| *key == variation_key)
        {
            entry.1 = clone.clone();
        } else {
            caches.variation_cache.push((variation_key, clone.clone()));
        }
        clone
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L304 (chrome/m156)
    /// `disableFontFallback()`.
    #[doc(alias = "disableFontFallback")]
    pub fn disable_font_fallback(&mut self) {
        self.config_mut().enable_font_fallback = false;
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L305 (chrome/m156)
    /// `enableFontFallback()`.
    #[doc(alias = "enableFontFallback")]
    pub fn enable_font_fallback(&mut self) {
        self.config_mut().enable_font_fallback = true;
    }

    // Port of: modules/skparagraph/include/FontCollection.h#L52 (chrome/m156)
    /// `fontFallbackEnabled()`.
    #[must_use]
    #[doc(alias = "fontFallbackEnabled")]
    pub fn font_fallback_enabled(&self) -> bool {
        self.config().enable_font_fallback
    }

    // Port of: modules/skparagraph/include/FontCollection.h#L54 (chrome/m156)
    /// `getParagraphCache()`.
    #[must_use]
    #[doc(alias = "getParagraphCache")]
    pub fn paragraph_cache(&self) -> &ParagraphCache {
        &self.inner.paragraph_cache
    }

    // Port of: modules/skparagraph/src/FontCollection.cpp#L307-L312 (chrome/m156)
    /// `clearCaches()`.
    #[doc(alias = "clearCaches")]
    pub fn clear_caches(&mut self) {
        self.inner.paragraph_cache.reset();
        {
            let mut caches = self.caches();
            caches.face_cache.clear();
            caches.variation_cache.clear();
        }
        skia_rust_shaper::harfbuzz::purge_caches();
    }

    /// Whether `other` is this collection (the same `FontCollection` object).
    #[must_use]
    pub fn ptr_eq(&self, other: &FontCollection) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}
