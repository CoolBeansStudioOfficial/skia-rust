// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sfnt/SkOTUtils.h (LocalizedStrings_SingleName)

//! `SkOTUtils::LocalizedStrings_SingleName`: a [`LocalizedStrings`] iterator over one name.

use crate::typeface::{LocalizedString, LocalizedStrings};

/// Yields one localized name, once (`SkOTUtils::LocalizedStrings_SingleName`).
// Port of: src/sfnt/SkOTUtils.h#L80-L97 (chrome/m156)
#[doc(alias = "SkOTUtils::LocalizedStrings_SingleName")]
#[derive(Debug, Clone)]
pub struct LocalizedStringsSingleName {
    name: String,
    language: String,
    has_next: bool,
}

impl LocalizedStringsSingleName {
    /// `LocalizedStrings_SingleName(SkString name, SkString language)`.
    // Port of: src/sfnt/SkOTUtils.h#L82-L84 (chrome/m156)
    #[must_use]
    pub fn new(name: impl Into<String>, language: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            language: language.into(),
            has_next: true,
        }
    }
}

impl LocalizedStrings for LocalizedStringsSingleName {
    /// `LocalizedStrings_SingleName::next`: the name on the first call, then nothing.
    // Port of: src/sfnt/SkOTUtils.h#L85-L92 (chrome/m156)
    fn next(&mut self) -> Option<LocalizedString> {
        let had_next = self.has_next;
        self.has_next = false;
        had_next.then(|| LocalizedString {
            string: self.name.clone(),
            language: self.language.clone(),
        })
    }
}
