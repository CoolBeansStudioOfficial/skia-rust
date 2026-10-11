// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/svg/SkSVGCanvas.h, src/svg/SkSVGCanvas.cpp

//! `SkSVGCanvas`: a canvas that generates SVG commands from its draw calls.
//!
//! The shape of [`Canvas`] mirrors `skia_safe::svg::Canvas` (`new`, `end`). [`make`] is
//! `SkSVGCanvas::Make` over a shared stream.

use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

use skia_rust_core::canvas::Canvas as CoreCanvas;
use skia_rust_core::data::Data;
use skia_rust_core::rect::{Rect, RoundOut};
use skia_rust_core::size::ISize;
use skia_rust_core::stream::{DynamicMemoryWStream, WStream};
use skia_rust_core::xml::{K_NO_PRETTY_FLAG, XmlStreamWriter};

pub use crate::device::{EncodePngCallback, Flags, Options, SvgDevice};

/// A stream that is shared between the canvas that writes it and its owner.
///
/// skia-rust: `SkSVGCanvas::Make` takes a raw `SkWStream*` that "must remain valid for the
/// lifetime of the returned canvas"; a shared handle stands in for that pointer.
pub type SharedStream = Rc<RefCell<dyn WStream>>;

/// A [`SharedStream`] as an owned [`WStream`].
struct SharedWStream(SharedStream);

impl WStream for SharedWStream {
    fn write(&mut self, buffer: &[u8]) -> bool {
        self.0.borrow_mut().write(buffer)
    }

    fn flush(&mut self) {
        self.0.borrow_mut().flush();
    }

    fn bytes_written(&self) -> usize {
        self.0.borrow().bytes_written()
    }
}

/// Returns a new canvas that will generate SVG commands from its draw calls, and send them to
/// the provided stream.
///
/// The canvas may buffer some drawing calls, so the output is not guaranteed to be valid or
/// complete until the canvas instance is dropped.
///
/// The `bounds` parameter defines an initial SVG viewport (`viewBox` attribute on the root SVG
/// element).
// Port of: src/svg/SkSVGCanvas.cpp#L23-L61 (chrome/m156)
#[doc(alias = "SkSVGCanvas::Make")]
#[doc(alias = "Make")]
#[must_use]
pub fn make(bounds: &Rect, stream: SharedStream, opts: Options) -> CoreCanvas {
    // TODO: pass full bounds to the device
    let rounded: skia_rust_core::rect::IRect = bounds.round_out();
    let size = ISize::new(rounded.width(), rounded.height());
    let xml_flags = if opts.flags.contains(Flags::NO_PRETTY_XML) {
        K_NO_PRETTY_FLAG
    } else {
        0
    };

    let svg_device = SvgDevice::make(
        size,
        Box::new(XmlStreamWriter::new_owned(
            Box::new(SharedWStream(stream)),
            xml_flags,
        )),
        opts,
    );

    CoreCanvas::from_device(svg_device)
}

/// An SVG canvas that collects its output in memory (`skia_safe::svg::Canvas`).
pub struct Canvas {
    canvas: Option<CoreCanvas>,
    stream: Rc<RefCell<DynamicMemoryWStream>>,
}

impl std::fmt::Debug for Canvas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Canvas").finish_non_exhaustive()
    }
}

impl Deref for Canvas {
    type Target = CoreCanvas;

    fn deref(&self) -> &CoreCanvas {
        self.canvas.as_ref().expect("the canvas is alive until end")
    }
}

impl Canvas {
    /// Creates a new SVG canvas.
    #[must_use]
    pub fn new(bounds: impl AsRef<Rect>, flags: impl Into<Option<Flags>>) -> Canvas {
        let opts = Options {
            flags: flags.into().unwrap_or_default(),
            png_encoder: None,
        };
        let stream = Rc::new(RefCell::new(DynamicMemoryWStream::new()));
        let canvas = make(bounds.as_ref(), stream.clone(), opts);
        Canvas {
            canvas: Some(canvas),
            stream,
        }
    }

    /// Ends the canvas drawing and returns the resulting SVG.
    #[must_use]
    pub fn end(mut self) -> Data {
        // The canvas is dropped first: that closes the open elements and flushes the writer.
        self.canvas = None;
        self.stream.borrow_mut().detach_as_data()
    }
}
