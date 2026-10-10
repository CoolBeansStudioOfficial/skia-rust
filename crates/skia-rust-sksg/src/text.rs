// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGText.h, modules/sksg/src/SkSGText.cpp (chrome/m156)

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::{FontHinting, TextEncoding};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path as SkPath;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_core::typeface::Typeface;
use skia_rust_core::utils::text_utils::Align;

use crate::geometry_node::{GEOMETRY_TRAITS, GeometryNode};
use crate::invalidation_controller::InvalidationController;
use crate::node::{Node, NodeCore};
use crate::util::scalar_changed;

/// A text geometry: a run of text, drawn and clipped as one blob (`Text`).
// Port of: modules/sksg/include/SkSGText.h#L16-L49 (chrome/m156) (`class Text`)
#[doc(alias = "sksg::Text")]
#[derive(Debug)]
#[allow(clippy::struct_field_names)] // mirrors the Skia header, where the field is prefixed
pub struct Text {
    core: NodeCore,
    typeface: RefCell<Option<Typeface>>,
    text: RefCell<String>,
    position: Cell<Point>,
    size: Cell<f32>,
    scale_x: Cell<f32>,
    skew_x: Cell<f32>,
    align: Cell<Align>,
    edging: Cell<Edging>,
    hinting: Cell<FontHinting>,
    /// The cached text blob, built on revalidation (`fBlob`).
    blob: RefCell<Option<TextBlob>>,
}

impl Text {
    /// `Text::Make(typeface, text)`.
    // Port of: modules/sksg/include/SkSGText.h#L19-L20 (chrome/m156) (`Text::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(typeface: Option<Typeface>, text: &str) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(GEOMETRY_TRAITS, weak.clone()),
            typeface: RefCell::new(typeface),
            text: RefCell::new(text.to_owned()),
            position: Cell::new(Point { x: 0.0, y: 0.0 }),
            size: Cell::new(12.0),
            scale_x: Cell::new(1.0),
            skew_x: Cell::new(0.0),
            align: Cell::new(Align::Left),
            edging: Cell::new(Edging::AntiAlias),
            hinting: Cell::new(FontHinting::Normal),
            blob: RefCell::new(None),
        })
    }

    /// The typeface (`getTypeface`).
    #[must_use]
    pub fn typeface(&self) -> Option<Typeface> {
        self.typeface.borrow().clone()
    }

    /// Sets the typeface, invalidating the node if it changed (`setTypeface`).
    pub fn set_typeface(&self, typeface: Option<Typeface>) {
        if *self.typeface.borrow() != typeface {
            *self.typeface.borrow_mut() = typeface;
            self.invalidate();
        }
    }

    /// The text (`getText`).
    #[must_use]
    pub fn text(&self) -> String {
        self.text.borrow().clone()
    }

    /// Sets the text, invalidating the node if it changed (`setText`).
    pub fn set_text(&self, text: &str) {
        if *self.text.borrow() != text {
            text.clone_into(&mut self.text.borrow_mut());
            self.invalidate();
        }
    }

    /// The position of the text origin (`getPosition`).
    #[must_use]
    pub fn position(&self) -> Point {
        self.position.get()
    }

    /// Sets the position, invalidating the node if it changed (`setPosition`).
    pub fn set_position(&self, p: Point) {
        if self.position.get() != p {
            self.position.set(p);
            self.invalidate();
        }
    }

    /// The text size (`getSize`).
    #[must_use]
    pub fn size(&self) -> f32 {
        self.size.get()
    }

    /// Sets the text size, invalidating the node if it changed (`setSize`).
    pub fn set_size(&self, size: f32) {
        if scalar_changed(self.size.get(), size) {
            self.size.set(size);
            self.invalidate();
        }
    }

    /// The horizontal scale (`getScaleX`).
    #[must_use]
    pub fn scale_x(&self) -> f32 {
        self.scale_x.get()
    }

    /// Sets the horizontal scale, invalidating the node if it changed (`setScaleX`).
    pub fn set_scale_x(&self, scale_x: f32) {
        if scalar_changed(self.scale_x.get(), scale_x) {
            self.scale_x.set(scale_x);
            self.invalidate();
        }
    }

    /// The skew (`getSkewX`).
    #[must_use]
    pub fn skew_x(&self) -> f32 {
        self.skew_x.get()
    }

    /// Sets the skew, invalidating the node if it changed (`setSkewX`).
    pub fn set_skew_x(&self, skew_x: f32) {
        if scalar_changed(self.skew_x.get(), skew_x) {
            self.skew_x.set(skew_x);
            self.invalidate();
        }
    }

    /// The alignment (`getAlign`).
    #[must_use]
    pub fn align(&self) -> Align {
        self.align.get()
    }

    /// Sets the alignment, invalidating the node if it changed (`setAlign`).
    pub fn set_align(&self, align: Align) {
        if self.align.get() != align {
            self.align.set(align);
            self.invalidate();
        }
    }

    /// The edging (`getEdging`).
    #[must_use]
    pub fn edging(&self) -> Edging {
        self.edging.get()
    }

    /// Sets the edging, invalidating the node if it changed (`setEdging`).
    pub fn set_edging(&self, edging: Edging) {
        if self.edging.get() != edging {
            self.edging.set(edging);
            self.invalidate();
        }
    }

    /// The hinting (`getHinting`).
    #[must_use]
    pub fn hinting(&self) -> FontHinting {
        self.hinting.get()
    }

    /// Sets the hinting, invalidating the node if it changed (`setHinting`).
    pub fn set_hinting(&self, hinting: FontHinting) {
        if self.hinting.get() != hinting {
            self.hinting.set(hinting);
            self.invalidate();
        }
    }

    /// The origin of the text, shifted by the alignment for an advance of `advance`
    /// (`alignedPosition`).
    // Port of: modules/sksg/src/SkSGText.cpp#L31-L46 (chrome/m156) (`Text::alignedPosition`)
    fn aligned_position(&self, advance: f32) -> Point {
        let mut aligned = self.position.get();
        match self.align.get() {
            Align::Left => {}
            Align::Center => aligned.x += -advance / 2.0,
            Align::Right => aligned.x += -advance,
        }
        aligned
    }
}

impl Node for Text {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGText.cpp#L48-L72 (chrome/m156) (`Text::onRevalidate`)
    fn on_revalidate(&self, _ic: Option<&mut InvalidationController>, _ctm: &Matrix) -> Rect {
        debug_assert!(self.core.has_inval());
        // TODO: we could potentially track invals which don't require rebuilding the blob.
        let mut font = Font::from_typeface(self.typeface.borrow().clone());
        font.set_size(self.size.get());
        font.set_scale_x(self.scale_x.get());
        font.set_skew_x(self.skew_x.get());
        font.set_edging(self.edging.get());
        font.set_hinting(self.hinting.get());
        // N.B.: align is applied externally (in aligned_position()), because text blobs have
        // trouble computing accurate bounds with alignment, and paint alignment is slated for
        // deprecation.
        let text = self.text.borrow();
        let blob = TextBlob::from_text(text.as_bytes(), TextEncoding::UTF8, &font);
        drop(text);
        let Some(blob) = blob else {
            *self.blob.borrow_mut() = None;
            return Rect::new_empty();
        };
        let bounds = *blob.bounds();
        let aligned_pos = self.aligned_position(bounds.width());
        let result = bounds.with_offset(aligned_pos);
        *self.blob.borrow_mut() = Some(blob);
        result
    }
}

impl GeometryNode for Text {
    // Port of: modules/sksg/src/SkSGText.cpp#L88-L91 (chrome/m156) (`Text::onClip`)
    fn on_clip(&self, canvas: &Canvas, anti_alias: bool) {
        canvas.clip_path(&self.on_as_path(), ClipOp::Intersect, anti_alias);
    }

    // Port of: modules/sksg/src/SkSGText.cpp#L74-L78 (chrome/m156) (`Text::onDraw`)
    fn on_draw(&self, canvas: &Canvas, paint: &Paint) {
        let aligned_pos = self.aligned_position(self.core.bounds().width());
        if let Some(blob) = self.blob.borrow().as_ref() {
            canvas.draw_text_blob(blob, aligned_pos, paint);
        }
    }

    // Port of: modules/sksg/src/SkSGText.cpp#L79-L81 (chrome/m156) (`Text::onContains`)
    fn on_contains(&self, p: Point) -> bool {
        self.on_as_path().contains(p)
    }

    // Port of: modules/sksg/src/SkSGText.cpp#L83-L86 (chrome/m156) (`Text::onAsPath`)
    fn on_as_path(&self) -> SkPath {
        // TODO: as in Skia, text has no path yet.
        SkPath::default()
    }
}
