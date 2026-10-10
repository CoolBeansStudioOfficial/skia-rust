// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/core/SkAnnotationKeys.h, src/core/SkAnnotation.cpp, include/core/SkAnnotation.h
// (chrome/m156)

//! Annotations: key-value pairs attached to a rectangle of a canvas
//! ([`Canvas::draw_annotation`]). The PDF device turns the URL and named-destination keys into
//! link annotations and destinations; other devices ignore them.

use crate::canvas::Canvas;
use crate::data::Data;
use crate::point::Point;
use crate::rect::Rect;

/// `SkAnnotationKeys`: the keys of the annotations that Skia itself defines.
// Port of: src/core/SkAnnotationKeys.h, src/core/SkAnnotation.cpp#L14-L24 (chrome/m156)
#[doc(alias = "SkAnnotationKeys")]
#[derive(Debug, Clone, Copy)]
pub struct AnnotationKeys;

impl AnnotationKeys {
    /// The key of a URL link (`URL_Key`).
    #[doc(alias = "URL_Key")]
    #[must_use]
    pub fn url_key() -> &'static str {
        "SkAnnotationKey_URL"
    }

    /// The key of a named destination (`Define_Named_Dest_Key`).
    #[doc(alias = "Define_Named_Dest_Key")]
    #[must_use]
    pub fn define_named_dest_key() -> &'static str {
        "SkAnnotationKey_Define_Named_Dest"
    }

    /// The key of a link to a named destination (`Link_Named_Dest_Key`).
    #[doc(alias = "Link_Named_Dest_Key")]
    #[must_use]
    pub fn link_named_dest_key() -> &'static str {
        "SkAnnotationKey_Link_Named_Dest"
    }
}

impl Canvas {
    /// Annotates `rect` with the URL `value` (`SkAnnotateRectWithURL`).
    // Port of: src/core/SkAnnotation.cpp#L28-L34 (chrome/m156)
    #[doc(alias = "SkAnnotateRectWithURL")]
    pub fn draw_url_annotation(&self, rect: impl AsRef<Rect>, value: &Data) -> &Self {
        self.draw_annotation(rect, AnnotationKeys::url_key(), Some(value))
    }

    /// Defines the named destination `name` at `point` (`SkAnnotateNamedDestination`).
    // Port of: src/core/SkAnnotation.cpp#L36-L42 (chrome/m156)
    #[doc(alias = "SkAnnotateNamedDestination")]
    pub fn draw_named_destination_annotation(&self, point: impl Into<Point>, name: &Data) -> &Self {
        let point = point.into();
        let rect = Rect::from_xywh(point.x, point.y, 0.0, 0.0);
        self.draw_annotation(rect, AnnotationKeys::define_named_dest_key(), Some(name))
    }

    /// Links `rect` to the named destination `name` (`SkAnnotateLinkToDestination`).
    // Port of: src/core/SkAnnotation.cpp#L44-L49 (chrome/m156)
    #[doc(alias = "SkAnnotateLinkToDestination")]
    pub fn draw_link_destination_annotation(&self, rect: impl AsRef<Rect>, name: &Data) -> &Self {
        self.draw_annotation(rect, AnnotationKeys::link_named_dest_key(), Some(name))
    }
}
