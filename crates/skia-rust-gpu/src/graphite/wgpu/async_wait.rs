// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnAsyncWait.h, DawnAsyncWait.cpp,
//                   src/gpu/graphite/dawn/DawnErrorChecker.h, DawnErrorChecker.cpp (the scoped
//                   error check the resource creation uses)

//! Blocking on wgpu futures, and scoped error checks.
//!
//! `DawnAsyncWait` blocks a thread until a Dawn callback fires, ticking the instance meanwhile.
//! wgpu's futures need no ticking on native (the error-scope and compilation-info futures are
//! ready at once), so [`block_on`] is a plain park-until-woken executor.
//!
//! Dawn reports an invalid creation by returning a null object. wgpu reports it through the
//! device's uncaptured-error handler, which panics by default, so the creation functions run
//! inside an error scope ([`ScopedErrorCheck`], a small `DawnErrorChecker`) and turn an error
//! into `None`.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Blocks the calling thread until `future` completes.
///
/// Not available in the browser (a blocked main thread never gets to run the future), where
/// the caps disable every code path that calls it (`allow_scoped_error_checks`).
// Port of: src/gpu/graphite/dawn/DawnAsyncWait.cpp#L13-L45 (chrome/m156)
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}

/// The kind of error a scope caught (`DawnErrorType`).
// Port of: src/gpu/graphite/dawn/DawnErrorChecker.h#L18-L26 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorType {
    /// `kNoError`.
    NoError,
    /// `kOutOfMemory`.
    OutOfMemory,
    /// `kValidation`.
    Validation,
    /// `kInternal` (any other error).
    Internal,
}

/// Runs the `wgpu` calls made between `new` and [`pop_error_scopes`](Self::pop_error_scopes)
/// inside validation, out-of-memory and internal error scopes.
// Port of: src/gpu/graphite/dawn/DawnErrorChecker.cpp#L14-L70 (chrome/m156)
#[doc(alias = "DawnErrorChecker")]
#[must_use = "the scopes must be popped"]
pub struct ScopedErrorCheck {
    scopes: Vec<wgpu::ErrorScopeGuard>,
}

impl std::fmt::Debug for ScopedErrorCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScopedErrorCheck")
            .field("scopes", &self.scopes.len())
            .finish()
    }
}

impl ScopedErrorCheck {
    /// Pushes the error scopes on `device`.
    pub fn new(device: &wgpu::Device) -> Self {
        // Popped in reverse order.
        let scopes = vec![
            device.push_error_scope(wgpu::ErrorFilter::Internal),
            device.push_error_scope(wgpu::ErrorFilter::OutOfMemory),
            device.push_error_scope(wgpu::ErrorFilter::Validation),
        ];
        Self { scopes }
    }

    /// Pops the scopes and reports the first error any of them caught.
    // Port of: src/gpu/graphite/dawn/DawnErrorChecker.cpp#L38-L70 (chrome/m156)
    #[doc(alias = "popErrorScopes")]
    #[must_use]
    pub fn pop_error_scopes(mut self) -> ErrorType {
        let mut result = ErrorType::NoError;
        while let Some(scope) = self.scopes.pop() {
            let error = block_on(scope.pop());
            if result != ErrorType::NoError {
                continue;
            }
            result = match error {
                None => ErrorType::NoError,
                Some(wgpu::Error::OutOfMemory { .. }) => ErrorType::OutOfMemory,
                Some(wgpu::Error::Validation { .. }) => ErrorType::Validation,
                Some(_) => ErrorType::Internal,
            };
        }
        result
    }
}

/// Calls `create` inside error scopes (when `scoped` is true, `Caps::allowScopedErrorChecks()`)
/// and returns its result unless an error was raised.
pub(crate) fn create_checked<T>(
    device: &wgpu::Device,
    scoped: bool,
    create: impl FnOnce() -> T,
) -> Option<T> {
    if !scoped {
        return Some(create());
    }
    let check = ScopedErrorCheck::new(device);
    let created = create();
    match check.pop_error_scopes() {
        ErrorType::NoError => Some(created),
        error => {
            crate::gpu::sk_log::skia_log_e!("wgpu object creation failed: {error:?}");
            None
        }
    }
}
