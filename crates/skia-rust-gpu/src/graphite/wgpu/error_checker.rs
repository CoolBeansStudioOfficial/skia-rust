// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnErrorChecker.h, DawnErrorChecker.cpp

//! [`ErrorChecker`]: `DawnErrorChecker` on wgpu error scopes.
//!
//! The checker pushes a validation, an out-of-memory and an internal error scope when it is made
//! and reports what the scopes caught when they are popped. Dawn reports an invalid creation by
//! returning a null object; wgpu reports it through the device's uncaptured-error handler, which
//! panics by default, so the creation functions run inside an `ErrorChecker` and turn an error
//! into `None`.
//!
//! wgpu keeps error scopes per thread (a scope guard is `!Send`), so checkers on different threads
//! (a pipeline compiled on the executor while the recorder creates a buffer) do not see each
//! other's errors, which is what Dawn's per-thread scope stacks give too.

use bitflags::bitflags;

use crate::gpu::sk_log::skia_log_e;
use crate::graphite::wgpu::async_wait::block_on;

bitflags! {
    /// The kinds of error a scope can catch (`DawnErrorType`), as a bit mask.
    // Port of: src/gpu/graphite/dawn/DawnErrorChecker.h#L20-L26 (chrome/m156)
    #[doc(alias = "DawnErrorType")]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct ErrorType: u32 {
        /// `kNoError` is the empty mask.
        const NO_ERROR = 0b000;
        /// `kValidation`.
        const VALIDATION = 0b001;
        /// `kOutOfMemory`.
        const OUT_OF_MEMORY = 0b010;
        /// `kInternal`.
        const INTERNAL = 0b100;
    }
}

/// `kErrorScopeNames`, in the order the scopes are pushed.
// Port of: src/gpu/graphite/dawn/DawnErrorChecker.cpp#L18-L19 (chrome/m156)
const ERROR_SCOPE_NAMES: [&str; 3] = ["validation", "out-of-memory", "internal"];
/// `kErrorScopeTypes`, in the order the scopes are pushed.
// Port of: src/gpu/graphite/dawn/DawnErrorChecker.cpp#L20-L21 (chrome/m156)
const ERROR_SCOPE_TYPES: [ErrorType; 3] = [
    ErrorType::VALIDATION,
    ErrorType::OUT_OF_MEMORY,
    ErrorType::INTERNAL,
];

/// `DawnErrorChecker` immediately pushes error scopes for all known error filter types
/// (validation, out-of-memory, internal) upon construction and detects any errors that are
/// reported within those scopes. Errors are detected synchronously by calling
/// [`pop_error_scopes`](Self::pop_error_scopes), or by dropping the checker, which asserts (in
/// debug builds) if any errors were reported that were not caught by calling it directly.
// Port of: src/gpu/graphite/dawn/DawnErrorChecker.h#L28-L47 (chrome/m156)
#[doc(alias = "DawnErrorChecker")]
#[must_use = "the scopes must be popped"]
pub struct ErrorChecker {
    /// `fArmed`: the scopes are pushed and not yet popped.
    armed: bool,
    /// The scopes, the innermost last.
    scopes: Vec<wgpu::ErrorScopeGuard>,
}

impl std::fmt::Debug for ErrorChecker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ErrorChecker")
            .field("armed", &self.armed)
            .finish_non_exhaustive()
    }
}

impl ErrorChecker {
    /// `DawnErrorChecker(sharedContext)`: pushes the three error scopes on `device`.
    // Port of: src/gpu/graphite/dawn/DawnErrorChecker.cpp#L25-L29 (chrome/m156)
    pub fn new(device: &wgpu::Device) -> Self {
        let scopes = vec![
            device.push_error_scope(wgpu::ErrorFilter::Validation),
            device.push_error_scope(wgpu::ErrorFilter::OutOfMemory),
            device.push_error_scope(wgpu::ErrorFilter::Internal),
        ];
        Self {
            armed: true,
            scopes,
        }
    }

    /// `popErrorScopes()`: pops the three scopes (the internal one first) and returns the kinds
    /// of the scopes that caught an error, logging each error. Popping twice reports no error the
    /// second time.
    ///
    /// # Panics
    /// If the checker lost one of its scopes, which only popping them removes.
    // Port of: src/gpu/graphite/dawn/DawnErrorChecker.cpp#L38-L131 (chrome/m156)
    #[doc(alias = "popErrorScopes")]
    pub fn pop_error_scopes(&mut self) -> ErrorType {
        if !self.armed {
            return ErrorType::NO_ERROR;
        }

        let mut error = ErrorType::NO_ERROR;
        // Pop all three error scopes, starting with the one pushed last (`fScopeIdx` counts down
        // from the last).
        for scope_idx in (0..ERROR_SCOPE_TYPES.len()).rev() {
            let scope = self
                .scopes
                .pop()
                .expect("one scope for each of the error scope types");
            // wgpu's pop futures are ready at once on native (the pop takes effect immediately).
            if let Some(message) = block_on(scope.pop()) {
                skia_log_e!(
                    "Failed in error scope ({}): {message}",
                    ERROR_SCOPE_NAMES[scope_idx]
                );
                error |= ERROR_SCOPE_TYPES[scope_idx];
            }
        }

        self.armed = false;
        error
    }
}

impl Drop for ErrorChecker {
    // Port of: src/gpu/graphite/dawn/DawnErrorChecker.cpp#L31-L36 (chrome/m156)
    fn drop(&mut self) {
        let err = self.pop_error_scopes();
        if !std::thread::panicking() {
            debug_assert_eq!(err, ErrorType::NO_ERROR);
        }
    }
}
