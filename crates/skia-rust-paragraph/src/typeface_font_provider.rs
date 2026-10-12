// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/include/TypefaceFontProvider.h,
// modules/skparagraph/src/TypefaceFontProvider.cpp

//! `TypefaceFontProvider`: a font manager of the typefaces registered with it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use skia_rust_core::data::Data;
use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_mgr::{FontMgr, FontMgrBase, FontStyleSet, FontStyleSetBase};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::stream::StreamAsset;
use skia_rust_core::typeface::Typeface;
use skia_rust_core::utf::Unichar;

// Port of: modules/skparagraph/include/TypefaceFontProvider.h#L20-L40 (chrome/m156)
/// `TypefaceFontStyleSet`: the typefaces of one registered family.
#[derive(Debug)]
pub struct TypefaceFontStyleSet {
    styles: Mutex<Vec<Typeface>>,
    family_name: String,
    alias: String,
}

impl TypefaceFontStyleSet {
    // Port of: modules/skparagraph/src/TypefaceFontProvider.cpp#L92-L93 (chrome/m156)
    /// `TypefaceFontStyleSet(const SkString& familyName)`.
    #[must_use]
    pub fn new(family_name: impl AsRef<str>) -> Self {
        Self {
            styles: Mutex::new(Vec::new()),
            family_name: family_name.as_ref().to_owned(),
            alias: String::new(),
        }
    }

    /// `getFamilyName()`.
    #[must_use]
    #[doc(alias = "getFamilyName")]
    pub fn family_name(&self) -> &str {
        &self.family_name
    }

    /// `getAlias()`.
    #[must_use]
    #[doc(alias = "getAlias")]
    pub fn alias(&self) -> &str {
        &self.alias
    }

    // Port of: modules/skparagraph/src/TypefaceFontProvider.cpp#L116-L120 (chrome/m156)
    /// `appendTypeface(typeface)`.
    #[doc(alias = "appendTypeface")]
    pub fn append_typeface(&self, typeface: Typeface) -> &Self {
        self.styles
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(typeface);
        self
    }

    fn styles(&self) -> Vec<Typeface> {
        self.styles
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl FontStyleSetBase for TypefaceFontStyleSet {
    // Port of: modules/skparagraph/src/TypefaceFontProvider.cpp#L95 (chrome/m156)
    fn count(&self) -> usize {
        self.styles().len()
    }

    // Port of: modules/skparagraph/src/TypefaceFontProvider.cpp#L97-L105 (chrome/m156)
    fn get_style(&self, index: usize) -> (FontStyle, String) {
        let styles = self.styles();
        debug_assert!(index < styles.len());
        (styles[index].font_style(), self.family_name.clone())
    }

    // Port of: modules/skparagraph/src/TypefaceFontProvider.cpp#L107-L110 (chrome/m156)
    fn create_typeface(&self, index: usize) -> Option<Typeface> {
        let styles = self.styles();
        debug_assert!(index < styles.len());
        Some(styles[index].clone())
    }

    // Port of: modules/skparagraph/src/TypefaceFontProvider.cpp#L112-L114 (chrome/m156)
    fn match_style(&self, pattern: &FontStyle) -> Option<Typeface> {
        self.match_style_css3(pattern)
    }
}

// Port of: modules/skparagraph/include/TypefaceFontProvider.h#L42-L75 (chrome/m156)
#[derive(Debug, Default)]
struct ProviderState {
    registered_families: HashMap<String, Arc<TypefaceFontStyleSet>>,
    family_names: Vec<String>,
}

#[derive(Debug, Default)]
struct ProviderInner {
    state: Mutex<ProviderState>,
}

impl ProviderInner {
    fn state(&self) -> std::sync::MutexGuard<'_, ProviderState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn family(&self, family_name: &str) -> Option<Arc<TypefaceFontStyleSet>> {
        self.state().registered_families.get(family_name).cloned()
    }
}

impl FontMgrBase for ProviderInner {
    // Port of: modules/skparagraph/src/TypefaceFontProvider.cpp#L16 (chrome/m156)
    fn on_count_families(&self) -> usize {
        self.state().registered_families.len()
    }

    // Port of: modules/skparagraph/src/TypefaceFontProvider.cpp#L18-L21 (chrome/m156)
    fn on_get_family_name(&self, index: usize) -> String {
        let state = self.state();
        debug_assert!(index < state.registered_families.len());
        state.family_names[index].clone()
    }

    // Port of: modules/skparagraph/src/TypefaceFontProvider.cpp#L34-L42 (chrome/m156)
    fn on_create_style_set(&self, index: usize) -> Option<FontStyleSet> {
        let state = self.state();
        debug_assert!(index < state.registered_families.len());
        state
            .registered_families
            .get(&state.family_names[index])
            .map(|set| FontStyleSet::new(set.clone()))
    }

    // Port of: modules/skparagraph/src/TypefaceFontProvider.cpp#L23-L32 (chrome/m156)
    fn on_match_family(&self, family_name: Option<&str>) -> Option<FontStyleSet> {
        // `fRegisteredFamilies.find(familyName)` with a null name is undefined in C++; nothing
        // is registered under it here.
        let family_name = family_name?;
        self.family(family_name).map(|set| FontStyleSet::new(set))
    }

    // Port of: modules/skparagraph/src/TypefaceFontProvider.cpp#L44-L51 (chrome/m156)
    fn on_match_family_style(
        &self,
        family_name: Option<&str>,
        style: &FontStyle,
    ) -> Option<Typeface> {
        let sset = self.on_match_family(family_name)?;
        sset.match_style(style)
    }

    // Port of: modules/skparagraph/include/TypefaceFontProvider.h#L56-L60 (chrome/m156)
    fn on_match_family_style_character(
        &self,
        _family_name: Option<&str>,
        _style: &FontStyle,
        _bcp47: &[&str],
        _character: Unichar,
    ) -> Option<Typeface> {
        None
    }

    // Port of: modules/skparagraph/include/TypefaceFontProvider.h#L61 (chrome/m156)
    fn on_make_from_data(&self, _data: &Data, _tt_index: i32) -> Option<Typeface> {
        None
    }

    // Port of: modules/skparagraph/include/TypefaceFontProvider.h#L62-L64 (chrome/m156)
    fn on_make_from_stream_index(
        &self,
        _stream: Box<dyn StreamAsset>,
        _tt_index: i32,
    ) -> Option<Typeface> {
        None
    }

    // Port of: modules/skparagraph/include/TypefaceFontProvider.h#L65-L68 (chrome/m156)
    fn on_make_from_stream_args(
        &self,
        _stream: Box<dyn StreamAsset>,
        _args: &FontArguments<'_, '_>,
    ) -> Option<Typeface> {
        None
    }

    // Port of: modules/skparagraph/include/TypefaceFontProvider.h#L69-L71 (chrome/m156)
    fn on_make_from_file(&self, _path: &str, _tt_index: i32) -> Option<Typeface> {
        None
    }

    // Port of: modules/skparagraph/src/TypefaceFontProvider.cpp#L76-L90 (chrome/m156)
    fn on_legacy_make_typeface(
        &self,
        family_name: Option<&str>,
        style: FontStyle,
    ) -> Option<Typeface> {
        if let Some(family_name) = family_name {
            let matched_by_family = self.on_match_family_style(Some(family_name), &style);
            if matched_by_family.is_some() {
                return matched_by_family;
            }
        }
        if self.on_count_families() == 0 {
            return None;
        }
        let default_family = self.on_create_style_set(0)?;
        default_family.match_style(&style)
    }
}

// Port of: modules/skparagraph/include/TypefaceFontProvider.h#L42 (chrome/m156)
/// `TypefaceFontProvider`: a font manager that serves the typefaces registered with it.
#[derive(Debug, Clone, Default)]
pub struct TypefaceFontProvider {
    inner: Arc<ProviderInner>,
}

impl From<TypefaceFontProvider> for FontMgr {
    fn from(provider: TypefaceFontProvider) -> Self {
        FontMgr::new(provider.inner)
    }
}

impl TypefaceFontProvider {
    /// `sk_make_sp<TypefaceFontProvider>()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // Port of: modules/skparagraph/src/TypefaceFontProvider.cpp#L53-L74 (chrome/m156)
    /// `registerTypeface(typeface)` and `registerTypeface(typeface, alias)`: registers the
    /// typeface under its family name, or under `alias`. Returns 1 if the typeface was
    /// registered, else 0.
    #[doc(alias = "registerTypeface")]
    pub fn register_typeface<'a>(
        &mut self,
        typeface: Typeface,
        alias: impl Into<Option<&'a str>>,
    ) -> usize {
        let family_name = match alias.into() {
            Some(alias) => alias.to_owned(),
            None => typeface.family_name(),
        };
        self.register(typeface, &family_name)
    }

    /// `registerTypeface(typeface)` for an optional typeface: `nullptr` registers nothing.
    #[doc(alias = "registerTypeface")]
    pub fn register_optional(&mut self, typeface: Option<Typeface>) -> usize {
        match typeface {
            None => 0,
            Some(typeface) => self.register_typeface(typeface, None),
        }
    }

    fn register(&self, typeface: Typeface, family_name: &str) -> usize {
        if family_name.is_empty() {
            return 0;
        }
        let mut state = self.inner.state();
        if let Some(found) = state.registered_families.get(family_name) {
            found.append_typeface(typeface);
        } else {
            let val = Arc::new(TypefaceFontStyleSet::new(family_name));
            state
                .registered_families
                .insert(family_name.to_owned(), val.clone());
            state.family_names.push(family_name.to_owned());
            val.append_typeface(typeface);
        }
        1
    }

    /// The provider as a [`FontMgr`] handle (`sk_sp<SkFontMgr>`), sharing its state.
    #[must_use]
    pub fn font_mgr(&self) -> FontMgr {
        FontMgr::new(self.inner.clone())
    }

    /// `countFamilies()`.
    #[must_use]
    #[doc(alias = "countFamilies")]
    pub fn count_families(&self) -> usize {
        self.inner.on_count_families()
    }
}
