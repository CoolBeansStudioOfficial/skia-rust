// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/private/SkIDChangeListener.h, src/core/SkIDChangeListener.cpp

//! [`IdChangeListener`]: notification that a gen/unique ID was invalidated.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

/// Used to be notified when a gen/unique ID is invalidated, typically to preemptively purge
/// associated items from a cache that are no longer reachable. The listener can be marked for
/// deregistration if the cached item is removed before the listener is triggered. This prevents
/// unbounded listener growth when cache items are routinely removed before the gen ID/unique ID
/// is invalidated.
///
/// skia-rust: in C++ this is an abstract class with a virtual `changed()`; here the callback is a
/// closure given to [`IdChangeListener::new`]. The shared (`sk_sp`) handle is an
/// `Arc<IdChangeListener>`; `unique()` is `Arc::strong_count(..) == 1`.
// Port of: include/private/SkIDChangeListener.h#L24-L74 (chrome/m156)
#[doc(alias = "SkIDChangeListener")]
pub struct IdChangeListener {
    changed: Box<dyn Fn() + Send + Sync>,
    should_deregister: AtomicBool,
}

impl fmt::Debug for IdChangeListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdChangeListener")
            .field("should_deregister", &self.should_deregister())
            .finish_non_exhaustive()
    }
}

impl IdChangeListener {
    /// Creates a listener that calls `changed` when the ID it is registered with is invalidated.
    // Port of: src/core/SkIDChangeListener.cpp#L14 (chrome/m156)
    #[must_use]
    pub fn new(changed: impl Fn() + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self {
            changed: Box::new(changed),
            should_deregister: AtomicBool::new(false),
        })
    }

    /// Calls the listener's callback (`changed()`).
    #[doc(alias = "changed")]
    pub fn notify_changed(&self) {
        (self.changed)();
    }

    /// Mark the listener is no longer needed. It should be removed and `changed()` should not be
    /// called.
    // Port of: include/private/SkIDChangeListener.h#L40 (chrome/m156)
    #[doc(alias = "markShouldDeregister")]
    pub fn mark_should_deregister(&self) {
        self.should_deregister.store(true, Ordering::Relaxed);
    }

    /// Indicates whether [`Self::mark_should_deregister`] was called.
    // Port of: include/private/SkIDChangeListener.h#L43 (chrome/m156)
    #[doc(alias = "shouldDeregister")]
    #[must_use]
    pub fn should_deregister(&self) -> bool {
        self.should_deregister.load(Ordering::Acquire)
    }
}

/// Manages a list of [`IdChangeListener`]s.
// Port of: include/private/SkIDChangeListener.h#L46-L72 (chrome/m156)
#[doc(alias = "SkIDChangeListener::List")]
#[derive(Debug, Default)]
pub struct IdChangeListenerList {
    listeners: Mutex<Vec<Arc<IdChangeListener>>>,
}

impl IdChangeListenerList {
    /// Creates an empty list.
    // Port of: src/core/SkIDChangeListener.cpp#L18 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Arc<IdChangeListener>>> {
        self.listeners
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Add a new listener to the list. It must not already be deregistered. Also clears out
    /// previously deregistered listeners.
    // Port of: src/core/SkIDChangeListener.cpp#L30-L45 (chrome/m156)
    pub fn add(&self, listener: Arc<IdChangeListener>) {
        debug_assert!(!listener.should_deregister());

        let mut listeners = self.lock();
        // Clean out any stale listeners before we append the new one.
        let mut i = 0;
        while i < listeners.len() {
            if listeners[i].should_deregister() {
                // No need to preserve the order after i.
                listeners.swap_remove(i);
            } else {
                i += 1;
            }
        }
        listeners.push(listener);
    }

    /// The number of registered listeners (including deregistered listeners that are
    /// yet-to-be removed).
    // Port of: src/core/SkIDChangeListener.cpp#L47-L50 (chrome/m156)
    #[must_use]
    pub fn count(&self) -> usize {
        self.lock().len()
    }

    /// Calls `changed()` on all listeners that haven't been deregistered and resets the list.
    // Port of: src/core/SkIDChangeListener.cpp#L52-L59 (chrome/m156)
    pub fn changed(&self) {
        let mut listeners = self.lock();
        for listener in listeners.iter() {
            if !listener.should_deregister() {
                listener.notify_changed();
            }
        }
        listeners.clear();
    }

    /// Resets without calling `changed()` on the listeners.
    // Port of: src/core/SkIDChangeListener.cpp#L61-L64 (chrome/m156)
    pub fn reset(&self) {
        self.lock().clear();
    }
}

// Port of: src/core/SkIDChangeListener.cpp#L20-L28 (chrome/m156)
impl Drop for IdChangeListenerList {
    fn drop(&mut self) {
        // We don't need the mutex. No other thread should have this list while it's being
        // destroyed.
        let listeners = self
            .listeners
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner);
        for listener in listeners.iter() {
            if !listener.should_deregister() {
                listener.notify_changed();
            }
        }
    }
}
