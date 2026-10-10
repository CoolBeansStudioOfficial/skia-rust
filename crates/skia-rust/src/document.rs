// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// API shape of: third_party/rust-skia/skia-safe/src/core/document.rs (`skia_safe::Document`)

//! High-level API for creating a document-based [`Canvas`], e.g. for PDF output; pages are
//! created with [`Document::begin_page`].

use skia_rust_core::canvas::Canvas;
use skia_rust_core::rect::Rect;
use skia_rust_core::size::Size;

pub mod state {
    //! The state of a [`crate::Document`]: [`Open`] while pages may be added, [`OnPage`] while a
    //! page is being drawn onto.

    /// Document is currently open. May contain several pages.
    #[derive(Debug)]
    pub struct Open {
        pub(crate) pages: usize,
    }

    /// Document is currently on a page and can be drawn onto.
    #[derive(Debug)]
    pub struct OnPage {
        pub(crate) page: usize,
    }
}

/// A document, `SkDocument`: a sequence of pages drawn to canvases, written to a stream.
pub struct Document<'a, State = state::Open> {
    document: skia_rust_pdf::Document<'a>,
    state: State,
}

impl<State: std::fmt::Debug> std::fmt::Debug for Document<'_, State> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl<State> Document<'_, State> {
    /// Stops writing the document. No trailer is written.
    #[doc(alias = "abort")]
    pub fn abort(mut self) {
        self.document.abort();
    }
}

impl<'a> Document<'a, state::Open> {
    pub(crate) fn new(document: skia_rust_pdf::Document<'a>, state: state::Open) -> Self {
        Document { document, state }
    }

    /// The number of pages in this document.
    #[must_use]
    pub fn pages(&self) -> usize {
        self.state.pages
    }

    /// Begins a page, `beginPage`. This function consumes the document and returns a document
    /// containing a canvas that represents the page it's currently drawing on.
    ///
    /// # Panics
    /// If the size is empty, or `content` does not intersect the page.
    #[doc(alias = "beginPage")]
    #[must_use]
    pub fn begin_page(
        mut self,
        size: impl Into<Size>,
        content: Option<&Rect>,
    ) -> Document<'a, state::OnPage> {
        let size = size.into();
        let page = self.state.pages + 1;
        self.document
            .begin_page(size.width, size.height, content)
            .expect("a page with a canvas");
        Document {
            document: self.document,
            state: state::OnPage { page },
        }
    }

    /// Closes the document, `close`, and writes it out.
    ///
    /// This function consumes and drops the document.
    #[doc(alias = "close")]
    pub fn close(mut self) {
        self.document.close();
    }
}

impl<'a> Document<'a, state::OnPage> {
    /// The current page we are currently drawing on.
    #[must_use]
    pub fn page(&self) -> usize {
        self.state.page
    }

    /// Borrows the canvas for the current page on the document.
    ///
    /// # Panics
    /// Never: a document on a page has its canvas.
    pub fn canvas(&mut self) -> &Canvas {
        self.document.canvas().expect("the canvas of the page")
    }

    /// Ends the page, `endPage`.
    ///
    /// This function consumes the document and returns a new open document that contains the
    /// pages drawn so far.
    #[doc(alias = "endPage")]
    #[must_use]
    pub fn end_page(mut self) -> Document<'a> {
        self.document.end_page();
        Document {
            document: self.document,
            state: state::Open {
                pages: self.state.page,
            },
        }
    }
}
