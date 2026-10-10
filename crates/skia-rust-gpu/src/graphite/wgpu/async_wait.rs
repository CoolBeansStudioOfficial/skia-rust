// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnAsyncWait.h, DawnAsyncWait.cpp

//! Blocking on wgpu futures, and scoped error checks.
//!
//! `DawnAsyncWait` blocks a thread until a Dawn callback fires, ticking the instance meanwhile.
//! wgpu's futures need no ticking on native (the error-scope and compilation-info futures are
//! ready at once), so [`block_on`] is a plain park-until-woken executor.
//!
//! Dawn reports an invalid creation by returning a null object. wgpu reports it through the
//! device's uncaptured-error handler, which panics by default, so the creation functions run
//! inside an error scope ([`ErrorChecker`], `DawnErrorChecker`) and turn an error into `None`;
//! [`create_checked`] is that pattern.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

pub use crate::graphite::wgpu::error_checker::{ErrorChecker, ErrorType};

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
    let mut check = ErrorChecker::new(device);
    let created = create();
    let error = check.pop_error_scopes();
    if error == ErrorType::NO_ERROR {
        Some(created)
    } else {
        crate::gpu::sk_log::skia_log_e!("wgpu object creation failed: {error:?}");
        None
    }
}
