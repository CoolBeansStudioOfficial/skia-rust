// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/utils/TestFontCollection.h,
// modules/skparagraph/utils/TestFontCollection.cpp

//! `TestFontCollection`: a font collection of the fonts in a directory.
//!
//! C++ makes the typefaces with the typeface factory the build selects (FreeType, CoreText or
//! DirectWrite); here the typeface factory is Fontations.

use std::path::Path;

use skia_rust_core::font_arguments::FontArguments as CoreFontArguments;
use skia_rust_core::stream;
use skia_rust_text::ports::fontations::typeface::make_from_stream;

use crate::font_collection::FontCollection;
use crate::typeface_font_provider::TypefaceFontProvider;

// Port of: modules/skparagraph/utils/TestFontCollection.h#L15-L31 (chrome/m156)
/// `TestFontCollection`: a `FontCollection` of the fonts of a resource directory. It dereferences
/// to its [`FontCollection`].
#[derive(Debug, Clone)]
pub struct TestFontCollection {
    font_collection: FontCollection,
    resource_dir: String,
    fonts_found: usize,
    font_provider: TypefaceFontProvider,
    dirs: String,
}

impl std::ops::Deref for TestFontCollection {
    type Target = FontCollection;

    fn deref(&self) -> &FontCollection {
        &self.font_collection
    }
}

impl std::ops::DerefMut for TestFontCollection {
    fn deref_mut(&mut self) -> &mut FontCollection {
        &mut self.font_collection
    }
}

impl From<TestFontCollection> for FontCollection {
    fn from(collection: TestFontCollection) -> Self {
        collection.font_collection
    }
}

impl From<&TestFontCollection> for FontCollection {
    fn from(collection: &TestFontCollection) -> Self {
        collection.font_collection.clone()
    }
}

impl TestFontCollection {
    // Port of: modules/skparagraph/utils/TestFontCollection.cpp#L30-L54 (chrome/m156)
    /// `TestFontCollection(resourceDir, testOnly, loadFonts)`: if `load_fonts` is true, loads the
    /// fonts from `resource_dir`.
    #[must_use]
    pub fn new(resource_dir: &str, test_only: bool, load_fonts: bool) -> Self {
        let mut collection = Self {
            font_collection: FontCollection::new(),
            resource_dir: resource_dir.to_owned(),
            fonts_found: 0,
            font_provider: TypefaceFontProvider::new(),
            dirs: String::new(),
        };
        if collection.dirs == resource_dir {
            return collection;
        }

        collection.font_provider = TypefaceFontProvider::new();

        if load_fonts {
            // `SkOSFile::Iter` visits the files of the directory.
            if let Ok(entries) = std::fs::read_dir(&collection.resource_dir) {
                for entry in entries.flatten() {
                    if entry.file_type().is_ok_and(|file_type| file_type.is_file()) {
                        if let Some(name) = entry.file_name().to_str() {
                            collection.add_font_from_file(name, "");
                        }
                    }
                }
            }
        }

        collection.fonts_found = collection.font_provider.count_families();
        let font_provider = collection.font_provider.font_mgr();
        if test_only {
            collection
                .font_collection
                .set_test_font_manager(font_provider);
        } else {
            collection
                .font_collection
                .set_asset_font_manager(font_provider);
        }
        collection.font_collection.disable_font_fallback();
        resource_dir.clone_into(&mut collection.dirs);
        collection
    }

    // Port of: modules/skparagraph/utils/TestFontCollection.h#L22 (chrome/m156)
    /// `fontsFound()`.
    #[must_use]
    #[doc(alias = "fontsFound")]
    pub fn fonts_found(&self) -> usize {
        self.fonts_found
    }

    // Port of: modules/skparagraph/utils/TestFontCollection.cpp#L56-L78 (chrome/m156)
    /// `addFontFromFile(path, familyName)`: registers the font of the file, under `family_name`
    /// when it is not empty. Returns false if the file cannot be read.
    #[doc(alias = "addFontFromFile")]
    pub fn add_font_from_file(&mut self, path: &str, family_name: &str) -> bool {
        let file_path = format!("{}/{}", self.resource_dir, path);

        let Some(file) = stream::make_from_file(Path::new(&file_path)) else {
            return false;
        };
        let face = make_from_stream(file, &CoreFontArguments::new());
        if let Some(face) = face {
            if family_name.is_empty() {
                self.font_provider.register_typeface(face, None);
            } else {
                self.font_provider.register_typeface(face, family_name);
            }
        }

        true
    }
}
