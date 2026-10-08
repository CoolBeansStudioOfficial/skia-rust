// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkSerialProcs.h (chrome/m156), the typeface procedures only.

//! [`SerialProcs`] and [`DeserialProcs`] (`SkSerialProcs`, `SkDeserialProcs`): the hooks that
//! let a caller decide how typefaces are written and read back. The image and data procedures
//! arrive with the codec and picture phases.
//!
//! C++ passes a function pointer and a `void*` context. Here the procedure is an `Arc` closure
//! that owns (or shares) its context, so the structs are `Clone` and `Send + Sync`.

use std::fmt;
use std::sync::Arc;

use crate::data::Data;
use crate::stream::Stream;
use crate::typeface::Typeface;

/// `SkSerialProcs::fTypefaceProc`: the custom bytes for a typeface, or `None` to fall back to
/// the default encoding (a typeface index in the buffer's table).
#[doc(alias = "SkSerialTypefaceProc")]
pub type TypefaceSerializer = Arc<dyn Fn(&Typeface) -> Option<Data> + Send + Sync>;

/// `SkDeserialProcs::fTypefaceStreamProc`: makes a typeface from the bytes that
/// [`TypefaceSerializer`] wrote.
#[doc(alias = "SkDeserialTypefaceProc")]
pub type TypefaceDeserializer = Arc<dyn Fn(&mut dyn Stream) -> Option<Typeface> + Send + Sync>;

/// `SkSerialProcs`: how to write the objects of a buffer. The default has no procedures.
// Port of: include/core/SkSerialProcs.h#L20-L40 (chrome/m156), the typeface procedure
#[doc(alias = "SkSerialProcs")]
#[derive(Clone, Default)]
pub struct SerialProcs {
    /// `fTypefaceProc` with its context.
    pub typeface: Option<TypefaceSerializer>,
}

impl fmt::Debug for SerialProcs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SerialProcs")
            .field("typeface", &self.typeface.is_some())
            .finish()
    }
}

/// `SkDeserialProcs`: how to read the objects of a buffer. The default has no procedures.
// Port of: include/core/SkSerialProcs.h#L42-L60 (chrome/m156), the typeface procedure
#[doc(alias = "SkDeserialProcs")]
#[derive(Clone, Default)]
pub struct DeserialProcs {
    /// `fTypefaceStreamProc` with its context.
    pub typeface: Option<TypefaceDeserializer>,
}

impl fmt::Debug for DeserialProcs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeserialProcs")
            .field("typeface", &self.typeface.is_some())
            .finish()
    }
}
