// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/RefCntedCallback.h

//! `skgpu::AutoCallback` and `skgpu::RefCntedCallback`: callbacks that run when dropped.
//!
//! Skia's callbacks are a function pointer plus a `void*` context. Here a callback is a boxed
//! closure that owns its context. The four callback shapes (plain, with stats, with result, with
//! result and stats) are kept as the variants of [`CallbackProc`].

use std::sync::{Arc, Mutex};

use crate::gpu::gpu_types::{CallbackResult, GpuStats};

/// One of the four callback signatures `AutoCallback` accepts.
// Port of: src/gpu/RefCntedCallback.h#L24-L27 (chrome/m156)
pub enum CallbackProc {
    /// `Callback`: `void (*)(Context)`.
    Plain(Box<dyn FnOnce() + Send>),
    /// `CallbackWithStats`: `void (*)(Context, const GpuStats&)`.
    WithStats(Box<dyn FnOnce(&GpuStats) + Send>),
    /// `ResultCallback`: `void (*)(Context, CallbackResult)`.
    Result(Box<dyn FnOnce(CallbackResult) + Send>),
    /// `ResultCallbackWithStats`: `void (*)(Context, CallbackResult, const GpuStats&)`.
    ResultWithStats(Box<dyn FnOnce(CallbackResult, &GpuStats) + Send>),
}

impl std::fmt::Debug for CallbackProc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CallbackProc::Plain(_) => "Plain",
            CallbackProc::WithStats(_) => "WithStats",
            CallbackProc::Result(_) => "Result",
            CallbackProc::ResultWithStats(_) => "ResultWithStats",
        })
    }
}

/// Move-only type that calls a callback from its destructor.
// Port of: src/gpu/RefCntedCallback.h#L22-L112 (chrome/m156)
#[doc(alias = "skgpu::AutoCallback")]
#[derive(Debug, Default)]
pub struct AutoCallback {
    proc: Option<CallbackProc>,
    result: Option<CallbackResult>,
    gpu_stats: GpuStats,
}

impl AutoCallback {
    /// `AutoCallback(proc, ctx)`.
    #[must_use]
    pub fn new(proc: CallbackProc) -> Self {
        Self {
            proc: Some(proc),
            result: None,
            gpu_stats: GpuStats::default(),
        }
    }

    /// `receivesGpuStats()`.
    #[doc(alias = "receivesGpuStats")]
    #[must_use]
    pub fn receives_gpu_stats(&self) -> bool {
        matches!(
            self.proc,
            Some(CallbackProc::WithStats(_) | CallbackProc::ResultWithStats(_))
        )
    }

    /// `setFailureResult()`.
    ///
    /// # Panics
    /// In debug builds, if the callback does not take a result or a result was already set.
    // Port of: src/gpu/RefCntedCallback.h#L78-L83 (chrome/m156)
    #[doc(alias = "setFailureResult")]
    pub fn set_failure_result(&mut self) {
        debug_assert!(matches!(
            self.proc,
            Some(CallbackProc::Result(_) | CallbackProc::ResultWithStats(_))
        ));
        // Shouldn't really be calling this multiple times.
        debug_assert!(self.result.is_none());
        self.result = Some(CallbackResult::Failed);
    }

    /// `setStats()`.
    ///
    /// # Panics
    /// In debug builds, if the callback does not take stats.
    // Port of: src/gpu/RefCntedCallback.h#L84-L87 (chrome/m156)
    #[doc(alias = "setStats")]
    pub fn set_stats(&mut self, stats: &GpuStats) {
        debug_assert!(self.receives_gpu_stats());
        self.gpu_stats = *stats;
    }

    /// `operator bool()`: true if a callback is set.
    #[must_use]
    pub fn is_set(&self) -> bool {
        self.proc.is_some()
    }
}

impl Drop for AutoCallback {
    // Port of: src/gpu/RefCntedCallback.h#L44-L56 (chrome/m156)
    fn drop(&mut self) {
        let result = self.result.unwrap_or(CallbackResult::Success);
        match self.proc.take() {
            Some(CallbackProc::ResultWithStats(proc)) => proc(result, &self.gpu_stats),
            Some(CallbackProc::WithStats(proc)) => proc(&self.gpu_stats),
            Some(CallbackProc::Result(proc)) => proc(result),
            Some(CallbackProc::Plain(proc)) => proc(),
            None => {}
        }
    }
}

/// Ref-counted object that calls a callback from its destructor (held as
/// `Arc<RefCntedCallback>`, Skia's `sk_sp<RefCntedCallback>`).
// Port of: src/gpu/RefCntedCallback.h#L117-L165 (chrome/m156)
#[doc(alias = "skgpu::RefCntedCallback")]
#[derive(Debug)]
pub struct RefCntedCallback {
    callback: Mutex<AutoCallback>,
}

impl RefCntedCallback {
    /// `Make(proc, ctx)`.
    #[must_use]
    pub fn make(proc: CallbackProc) -> Arc<RefCntedCallback> {
        Arc::new(RefCntedCallback {
            callback: Mutex::new(AutoCallback::new(proc)),
        })
    }

    /// `Make(AutoCallback&&)`: `None` if the callback is not set.
    #[must_use]
    pub fn make_from(callback: AutoCallback) -> Option<Arc<RefCntedCallback>> {
        if !callback.is_set() {
            return None;
        }
        Some(Arc::new(RefCntedCallback {
            callback: Mutex::new(callback),
        }))
    }

    fn callback(&self) -> std::sync::MutexGuard<'_, AutoCallback> {
        self.callback
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `receivesGpuStats()`.
    #[doc(alias = "receivesGpuStats")]
    #[must_use]
    pub fn receives_gpu_stats(&self) -> bool {
        self.callback().receives_gpu_stats()
    }

    /// `setFailureResult()`.
    #[doc(alias = "setFailureResult")]
    pub fn set_failure_result(&self) {
        self.callback().set_failure_result();
    }

    /// `setStats()`.
    #[doc(alias = "setStats")]
    pub fn set_stats(&self, stats: &GpuStats) {
        self.callback().set_stats(stats);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn result_callback_runs_once_on_last_drop_with_failure() {
        let seen = Arc::new(AtomicU32::new(0));
        let seen2 = seen.clone();
        let cb = RefCntedCallback::make(CallbackProc::Result(Box::new(move |r| {
            seen2.store(
                if r == CallbackResult::Failed { 2 } else { 1 },
                Ordering::SeqCst,
            );
        })));
        let cb2 = cb.clone();
        cb.set_failure_result();
        drop(cb);
        assert_eq!(seen.load(Ordering::SeqCst), 0);
        drop(cb2);
        assert_eq!(seen.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn stats_are_delivered() {
        let seen = Arc::new(AtomicU32::new(0));
        let seen2 = seen.clone();
        let cb = RefCntedCallback::make(CallbackProc::WithStats(Box::new(move |s| {
            seen2.store(u32::try_from(s.elapsed_time).unwrap(), Ordering::SeqCst);
        })));
        assert!(cb.receives_gpu_stats());
        cb.set_stats(&GpuStats {
            elapsed_time: 7,
            num_occlusion_pass_samples: 0,
        });
        drop(cb);
        assert_eq!(seen.load(Ordering::SeqCst), 7);
    }
}
