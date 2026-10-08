// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file.
// Ported from Skia: src/sksl/SkSLErrorReporter.{h,cpp}, the `CompilerErrorReporter` of
// src/sksl/SkSLCompiler.h, `NoOpErrorReporter` (src/sksl/analysis/SkSLNoOpErrorReporter.h) and the
// parser checkpoint's `ForwardingErrorReporter` (src/sksl/SkSLParser.cpp).

//! [`ErrorReporter`]: counts errors and hands them to a sink.

use std::sync::Arc;

use crate::compiler::{self, Compiler};
use crate::position::Position;

/// Where an [`ErrorReporter`] sends errors: Skia's `ErrorReporter` subclasses.
#[derive(Debug)]
pub enum ErrorSink {
    /// `Compiler::CompilerErrorReporter`: formats each error into the compiler's error text
    /// (`Compiler::handleError`). The text is bytes because Skia echoes source bytes verbatim.
    Compiler {
        /// `Compiler::fErrorText`.
        error_text: Vec<u8>,
    },
    /// The parser checkpoint's `ForwardingErrorReporter`: keeps errors to forward or drop later.
    Forwarding {
        /// The recorded `(message, position)` pairs.
        errors: Vec<(String, Position)>,
    },
    /// `NoOpErrorReporter`: discards errors (analysis passes that only probe).
    NoOp,
    /// `TestingOnly_AbortErrorReporter`: panics with the message.
    TestingOnlyAbort,
}

/// `SkSL::ErrorReporter`.
///
/// Skia's class is abstract, with one virtual `handleError`. Here the subclasses are the
/// [`ErrorSink`] variants, so a [`Context`](crate::context::Context) can own its reporter by
/// value. Swapping reporters (Skia's `Context::setErrorReporter`, used by parser checkpoints)
/// is `std::mem::swap` of two `ErrorReporter`s.
// Port of: src/sksl/SkSLErrorReporter.h#L23-L56 (chrome/m156)
#[doc(alias = "SkSL::ErrorReporter")]
#[derive(Debug)]
pub struct ErrorReporter {
    source: Arc<str>,
    error_count: i32,
    sink: ErrorSink,
}

impl ErrorReporter {
    /// A reporter that sends errors to `sink`, with an empty source.
    #[must_use]
    pub fn new(sink: ErrorSink) -> Self {
        Self {
            source: Arc::from(""),
            error_count: 0,
            sink,
        }
    }

    /// The compiler's reporter (`CompilerErrorReporter`), with empty error text.
    #[must_use]
    pub fn compiler() -> Self {
        Self::new(ErrorSink::Compiler {
            error_text: Vec::new(),
        })
    }

    /// A `ForwardingErrorReporter` with no errors yet.
    #[must_use]
    pub fn forwarding() -> Self {
        Self::new(ErrorSink::Forwarding { errors: Vec::new() })
    }

    /// `NoOpErrorReporter`.
    #[must_use]
    pub fn no_op() -> Self {
        Self::new(ErrorSink::NoOp)
    }

    /// `TestingOnly_AbortErrorReporter`.
    #[must_use]
    pub fn testing_only_abort() -> Self {
        Self::new(ErrorSink::TestingOnlyAbort)
    }

    /// `error(position, msg)`: counts and reports an error, unless the message mentions a
    /// poison value.
    // Port of: src/sksl/SkSLErrorReporter.cpp#L16-L23 (chrome/m156)
    pub fn error(&mut self, position: Position, msg: &str) {
        if msg.contains(Compiler::POISON_TAG) {
            // Don't report errors on poison values.
            return;
        }
        self.error_count += 1;
        self.handle_error(msg, position);
    }

    /// `handleError(msg, position)`: the subclass hook.
    ///
    /// # Panics
    ///
    /// For [`ErrorSink::TestingOnlyAbort`], with the message.
    fn handle_error(&mut self, msg: &str, position: Position) {
        match &mut self.sink {
            ErrorSink::Compiler { error_text } => {
                compiler::handle_error(error_text, self.source.as_bytes(), msg, position);
            }
            ErrorSink::Forwarding { errors } => errors.push((msg.to_owned(), position)),
            ErrorSink::NoOp => {}
            ErrorSink::TestingOnlyAbort => panic!("{msg}"),
        }
    }

    /// `source()`: the text being compiled.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The shared handle to the source text.
    #[must_use]
    pub fn source_arc(&self) -> &Arc<str> {
        &self.source
    }

    /// `setSource(source)`.
    pub fn set_source(&mut self, source: Arc<str>) {
        self.source = source;
    }

    /// `errorCount()`.
    #[must_use]
    pub fn error_count(&self) -> i32 {
        self.error_count
    }

    /// `resetErrorCount()`.
    pub fn reset_error_count(&mut self) {
        self.error_count = 0;
    }

    /// The sink, for the code that owns this reporter (the compiler reads its error text, a
    /// parser checkpoint takes its forwarded errors).
    #[must_use]
    pub fn sink(&self) -> &ErrorSink {
        &self.sink
    }

    /// The sink, mutable.
    pub fn sink_mut(&mut self) -> &mut ErrorSink {
        &mut self.sink
    }
}

/// Forwards the errors that an [`ErrorSink::Forwarding`] reporter recorded to `to`, in order.
/// The analyses with an optional error reporter record into a forwarding reporter first, so
/// that they can borrow the context mutably at the same time.
pub fn forward_errors(from: &ErrorReporter, to: &mut ErrorReporter) {
    if let ErrorSink::Forwarding { errors } = from.sink() {
        for (msg, position) in errors {
            to.error(*position, msg);
        }
    }
}
