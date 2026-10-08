// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkTypeface.h, src/core/SkTypeface.cpp

//! [`Typeface`]: the font face a glyph comes from (`SkTypeface`).
//!
//! `Typeface` is a cheap-clone handle over `Arc<dyn TypefaceBase>`, as `Shader` is. The state
//! every typeface has (id, style, fixed pitch) lives in [`TypefaceCore`], and each backend
//! implements [`TypefaceBase`] for its own `on*` methods.

use std::any::Any;
use std::fmt;
use std::sync::{Arc, OnceLock};

use crate::data::Data;
use crate::font_arguments::variation_position::Coordinate;
use crate::font_descriptor::{FactoryId, FontDescriptor};
use crate::font_style::{FontStyle, Slant, Weight};
use crate::font_types::set_four_byte_tag;
use crate::stream::{DynamicMemoryWStream, StreamAsset, WStream};
use crate::typeface_cache::new_typeface_id;

/// A unique id of a typeface (`SkTypefaceID`).
// Port of: include/core/SkTypeface.h#L40 (chrome/m156)
#[doc(alias = "SkTypefaceID")]
pub type TypefaceId = u32;

/// How much of a typeface [`Typeface::serialize`] writes (`SkTypeface::SerializeBehavior`).
// Port of: include/core/SkTypeface.h#L127-L131 (chrome/m156)
#[doc(alias = "SkTypeface::SerializeBehavior")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SerializeBehavior {
    /// Always include the font data.
    DoIncludeData,
    /// Never include the font data; write the descriptor only.
    DontIncludeData,
    /// Include the data if the typeface reports that it is local to the process.
    IncludeDataIfLocal,
}

/// One localized name of a typeface (`SkTypeface::LocalizedString`).
// Port of: include/core/SkTypeface.h#L261-L264 (chrome/m156)
#[doc(alias = "SkTypeface::LocalizedString")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalizedString {
    /// The name, in UTF-8.
    pub string: String,
    /// The BCP 47 language tag of the name.
    pub language: String,
}

/// An iterator over the localized names of a typeface (`SkTypeface::LocalizedStrings`).
// Port of: include/core/SkTypeface.h#L265-L271 (chrome/m156)
#[doc(alias = "SkTypeface::LocalizedStrings")]
pub trait LocalizedStrings {
    /// Returns the next name, or `None` at the end (`next` returning false in C++).
    // Port of: include/core/SkTypeface.h#L269 (chrome/m156)
    fn next(&mut self) -> Option<LocalizedString>;
}

/// The state every typeface has (`SkTypeface`'s data members).
// Port of: include/core/SkTypeface.h#L460-L464 (chrome/m156)
#[derive(Debug)]
pub struct TypefaceCore {
    unique_id: TypefaceId,
    style: FontStyle,
    is_fixed_pitch: bool,
}

impl TypefaceCore {
    /// `SkTypeface::SkTypeface(style, isFixedPitch)`: assigns the next unique id.
    // Port of: src/core/SkTypeface.cpp#L57-L58 (chrome/m156)
    #[must_use]
    pub fn new(style: FontStyle, is_fixed_pitch: bool) -> Self {
        Self {
            unique_id: new_typeface_id(),
            style,
            is_fixed_pitch,
        }
    }
}

/// The per-backend half of a typeface: the `on*` virtuals of `SkTypeface`.
///
/// This carries the methods the first text slice needs (serialization, the empty typeface and
/// the cache). The rest of `SkTypeface`'s virtuals (glyph lookup, metrics, scaler contexts,
/// tables) are added with the phases that need them: the scaler context with T6, glyph mapping
/// and tables with T9.
// Port of: include/core/SkTypeface.h#L365-L440 (chrome/m156), the subset named above
#[doc(alias = "SkTypeface")]
pub trait TypefaceBase: Any + Send + Sync + fmt::Debug {
    /// The common state of the typeface.
    fn core(&self) -> &TypefaceCore;

    /// `SkTypeface::onGetFontDescriptor`: the descriptor, and whether the data is local to this
    /// process (`isLocal`).
    // Port of: include/core/SkTypeface.h#L414 (chrome/m156)
    fn on_get_font_descriptor(&self) -> (FontDescriptor, bool);

    /// `SkTypeface::onOpenStream`: the font data and its collection index, or `None`.
    // Port of: include/core/SkTypeface.h#L400 (chrome/m156)
    fn on_open_stream(&self) -> Option<(Box<dyn StreamAsset>, i32)>;

    /// `SkTypeface::onGetFamilyName`.
    // Port of: include/core/SkTypeface.h#L426 (chrome/m156)
    fn on_get_family_name(&self) -> String;

    /// `SkTypeface::onGetVariationDesignPosition`: the axes of the typeface, or `None` if the
    /// number of axes is unknown (C++ returns -1).
    // Port of: include/core/SkTypeface.h#L406-L408 (chrome/m156)
    fn on_get_variation_design_position(&self) -> Option<Vec<Coordinate>>;
}

/// A typeface handle (`sk_sp<SkTypeface>`). Cloning it shares the typeface.
// Port of: include/core/SkTypeface.h#L54 (chrome/m156)
#[doc(alias = "SkTypeface")]
#[derive(Clone)]
pub struct Typeface(Arc<dyn TypefaceBase>);

impl Typeface {
    /// Wraps a backend in a handle.
    #[must_use]
    pub fn new(base: Arc<dyn TypefaceBase>) -> Self {
        Self(base)
    }

    /// `SkTypeface::MakeEmpty()`: the typeface that draws nothing and has no glyphs. It is the
    /// same instance every time.
    // Port of: src/core/SkTypeface.cpp#L144-L146 (chrome/m156)
    #[doc(alias = "MakeEmpty")]
    #[must_use]
    pub fn empty() -> Self {
        static EMPTY: OnceLock<Typeface> = OnceLock::new();
        EMPTY
            .get_or_init(|| Self::new(Arc::new(EmptyTypeface::new())))
            .clone()
    }

    /// `SkTypeface::uniqueID`.
    // Port of: include/core/SkTypeface.h#L104 (chrome/m156)
    #[doc(alias = "uniqueID")]
    #[must_use]
    pub fn unique_id(&self) -> TypefaceId {
        self.0.core().unique_id
    }

    /// `SkTypeface::fontStyle`.
    // Port of: src/core/SkTypeface.cpp#L483-L485 (chrome/m156)
    #[doc(alias = "fontStyle")]
    #[must_use]
    pub fn font_style(&self) -> FontStyle {
        self.0.core().style
    }

    /// `SkTypeface::isFixedPitch`.
    // Port of: src/core/SkTypeface.cpp#L499-L501 (chrome/m156)
    #[doc(alias = "isFixedPitch")]
    #[must_use]
    pub fn is_fixed_pitch(&self) -> bool {
        self.0.core().is_fixed_pitch
    }

    /// `SkTypeface::isBold`: the weight is at least semi-bold.
    // Port of: src/core/SkTypeface.cpp#L491-L493 (chrome/m156)
    #[doc(alias = "isBold")]
    #[must_use]
    pub fn is_bold(&self) -> bool {
        *self.font_style().weight() >= *Weight::SEMI_BOLD
    }

    /// `SkTypeface::isItalic`: the slant is not upright.
    // Port of: src/core/SkTypeface.cpp#L495-L497 (chrome/m156)
    #[doc(alias = "isItalic")]
    #[must_use]
    pub fn is_italic(&self) -> bool {
        self.font_style().slant() != Slant::Upright
    }

    /// True if this handle is the only reference to the typeface (`SkRefCnt::unique`). The
    /// typeface cache purges only such typefaces.
    #[doc(alias = "unique")]
    #[must_use]
    pub fn is_unique(&self) -> bool {
        Arc::strong_count(&self.0) == 1
    }

    /// `SkTypeface::getFontDescriptor`: the descriptor, and whether the data is local.
    // Port of: include/core/SkTypeface.h#L348-L350 (chrome/m156)
    #[doc(alias = "getFontDescriptor")]
    #[must_use]
    pub fn font_descriptor(&self) -> (FontDescriptor, bool) {
        self.0.on_get_font_descriptor()
    }

    /// `SkTypeface::getFamilyName`.
    // Port of: src/core/SkTypeface.cpp#L466-L469 (chrome/m156)
    #[doc(alias = "getFamilyName")]
    #[must_use]
    pub fn family_name(&self) -> String {
        self.0.on_get_family_name()
    }

    /// `SkTypeface::openStream`: the font data and its collection index, or `None`.
    // Port of: src/core/SkTypeface.cpp#L333-L340 (chrome/m156)
    #[doc(alias = "openStream")]
    #[must_use]
    pub fn open_stream(&self) -> Option<(Box<dyn StreamAsset>, i32)> {
        self.0.on_open_stream()
    }

    /// `SkTypeface::getVariationDesignPosition`: the axes, or `None` when unknown.
    // Port of: src/core/SkTypeface.cpp#L290-L294 (chrome/m156)
    #[doc(alias = "getVariationDesignPosition")]
    #[must_use]
    pub fn variation_design_position(&self) -> Option<Vec<Coordinate>> {
        self.0.on_get_variation_design_position()
    }

    /// `SkTypeface::serialize(SkWStream*, behavior)`: writes the descriptor, and the font data
    /// when `behavior` asks for it. Returns false if a write fails.
    // Port of: src/core/SkTypeface.cpp#L201-L233 (chrome/m156)
    #[doc(alias = "serialize")]
    pub fn serialize_to(&self, stream: &mut dyn WStream, behavior: SerializeBehavior) -> bool {
        let (mut desc, is_local_data) = self.0.on_get_font_descriptor();
        let should_serialize_data = match behavior {
            SerializeBehavior::DoIncludeData => true,
            SerializeBehavior::DontIncludeData => false,
            SerializeBehavior::IncludeDataIfLocal => is_local_data,
        };
        if should_serialize_data {
            let opened = self.open_stream();
            let has_stream = opened.is_some();
            let (font_stream, index) = match opened {
                Some((font_stream, index)) => (Some(font_stream), index),
                None => (None, 0),
            };
            desc.set_stream(font_stream);
            if has_stream {
                desc.set_collection_index(index);
            }
            if let Some(coordinates) = self.variation_design_position()
                && !coordinates.is_empty()
            {
                desc.set_variation_coordinates(coordinates.len())
                    .copy_from_slice(&coordinates);
            }
        }
        desc.serialize(stream)
    }

    /// `SkTypeface::serialize(SerializeBehavior)`: the serialized form as data, or `None` if
    /// writing fails.
    // Port of: src/core/SkTypeface.cpp#L235-L238 (chrome/m156)
    #[must_use]
    pub fn serialize(&self, behavior: SerializeBehavior) -> Option<Data> {
        let mut stream = DynamicMemoryWStream::new();
        if self.serialize_to(&mut stream, behavior) {
            Some(stream.detach_as_data())
        } else {
            None
        }
    }
}

impl PartialEq for Typeface {
    /// `SkTypeface::Equal`: the same handle, or the same unique id.
    // Port of: src/core/SkTypeface.cpp#L148-L156 (chrome/m156)
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.unique_id() == other.unique_id()
    }
}

impl Eq for Typeface {}

impl fmt::Debug for Typeface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Typeface")
            .field("unique_id", &self.unique_id())
            .field("style", &self.font_style())
            .finish()
    }
}

/// `SkEmptyTypeface`: no glyphs, no data, a fixed pitch, and the normal style.
// Port of: src/core/SkTypeface.cpp#L66-L140 (chrome/m156)
#[derive(Debug)]
struct EmptyTypeface {
    core: TypefaceCore,
}

impl EmptyTypeface {
    /// `SkEmptyTypeface() : SkTypeface(SkFontStyle(), true)`.
    // Port of: src/core/SkTypeface.cpp#L83 (chrome/m156)
    fn new() -> Self {
        Self {
            core: TypefaceCore::new(FontStyle::default(), true),
        }
    }
}

/// `SkEmptyTypeface::FactoryId`.
// Port of: src/core/SkTypeface.cpp#L73 (chrome/m156)
const EMPTY_FACTORY_ID: FactoryId = set_four_byte_tag(b'e', b'm', b't', b'y');

impl TypefaceBase for EmptyTypeface {
    fn core(&self) -> &TypefaceCore {
        &self.core
    }

    /// `SkEmptyTypeface::onGetFontDescriptor`: the factory id, and not serialized as data.
    // Port of: src/core/SkTypeface.cpp#L98-L101 (chrome/m156)
    fn on_get_font_descriptor(&self) -> (FontDescriptor, bool) {
        let mut desc = FontDescriptor::new();
        desc.set_factory_id(EMPTY_FACTORY_ID);
        (desc, false)
    }

    /// `SkEmptyTypeface::onOpenStream`: there is no data.
    // Port of: src/core/SkTypeface.cpp#L85 (chrome/m156)
    fn on_open_stream(&self) -> Option<(Box<dyn StreamAsset>, i32)> {
        None
    }

    /// `SkEmptyTypeface::onGetFamilyName`: the empty name.
    // Port of: src/core/SkTypeface.cpp#L115-L117 (chrome/m156)
    fn on_get_family_name(&self) -> String {
        String::new()
    }

    /// `SkEmptyTypeface::onGetVariationDesignPosition`: no axes.
    // Port of: src/core/SkTypeface.cpp#L127-L130 (chrome/m156)
    fn on_get_variation_design_position(&self) -> Option<Vec<Coordinate>> {
        Some(Vec::new())
    }
}
