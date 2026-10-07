// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Test-only hooks (feature `testing`). Never enable this feature in release builds of
//! downstream crates.
//!
//! The forced selection is **thread-local**: tests running in parallel on different threads
//! cannot see each other's selection, and each GM render or pipeline run happens on the thread
//! that forced it. A pipeline compiled under a forced selection keeps it (Skia also binds its
//! `SkOpts` table at compile time), so it may be run from another thread afterwards.

use std::cell::RefCell;
use std::marker::PhantomData;

use crate::tier::{Selection, Unsupported};

thread_local! {
    /// The stack of forced selections on this thread; the last one is in effect.
    static FORCED: RefCell<Vec<Selection>> = const { RefCell::new(Vec::new()) };
}

/// The current thread's forced selection, if any.
pub(crate) fn forced() -> Option<Selection> {
    FORCED.with(|f| f.borrow().last().copied())
}

/// Forces `sel` for the current thread until the returned guard drops.
///
/// Guards nest; they must be dropped in reverse order of creation (dropping an outer guard while
/// an inner one is alive panics).
///
/// # Errors
/// [`Unsupported`] if the host cannot execute `sel` (see [`Selection::check`]), e.g.
/// `Backend::Native` for a tier the CPU lacks.
pub fn force_tier(sel: Selection) -> Result<TierGuard, Unsupported> {
    let sel = sel.check()?;
    let depth = FORCED.with(|f| {
        let mut stack = f.borrow_mut();
        stack.push(sel);
        stack.len()
    });
    Ok(TierGuard {
        depth,
        _not_send: PhantomData,
    })
}

/// Restores the previous selection of this thread when dropped. Not `Send`: it must drop on the
/// thread that created it.
#[must_use = "the tier is only forced while the guard is alive"]
#[derive(Debug)]
pub struct TierGuard {
    /// Stack depth after this guard's push.
    depth: usize,
    _not_send: PhantomData<*const ()>,
}

impl TierGuard {
    /// The selection this guard forces.
    #[must_use]
    pub fn selection(&self) -> Selection {
        FORCED.with(|f| f.borrow()[self.depth - 1])
    }
}

impl Drop for TierGuard {
    fn drop(&mut self) {
        let top = FORCED.with(|f| {
            let mut stack = f.borrow_mut();
            let top = stack.len();
            if top == self.depth {
                stack.pop();
            }
            top
        });
        assert!(
            top == self.depth || std::thread::panicking(),
            "TierGuard dropped out of order (guard depth {}, stack depth {top})",
            self.depth
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Barrier;

    use super::*;
    use crate::tier::{Estimates, Tier, selection};

    #[test]
    fn force_and_restore() {
        let default = selection();
        {
            let g = force_tier(Selection::native(Tier::Scalar)).unwrap();
            assert_eq!(selection(), Selection::native(Tier::Scalar));
            assert_eq!(g.selection(), Selection::native(Tier::Scalar));
            {
                let sel = Selection::model(Tier::Sse41, Estimates::AmdZen4);
                let _inner = force_tier(sel).unwrap();
                assert_eq!(selection(), sel);
            }
            assert_eq!(selection(), Selection::native(Tier::Scalar));
        }
        assert_eq!(selection(), default);
    }

    #[test]
    fn rejects_unsupported() {
        let before = selection();
        let bad = Selection::model(Tier::Neon, Estimates::AmdZen4);
        assert_eq!(force_tier(bad).unwrap_err().selection, bad);
        // A native tier the host lacks.
        if let Some(t) = Tier::ALL.into_iter().find(|t| !t.is_native()) {
            assert!(force_tier(Selection::native(t)).is_err());
        }
        assert_eq!(selection(), before);
    }

    #[test]
    fn threads_do_not_interfere() {
        // Every thread forces a different model selection at the same time and checks it sees
        // only its own; the barrier makes the guards overlap in time.
        let sels = [
            Selection::native(Tier::Scalar),
            Selection::model(Tier::Sse2, Estimates::AmdZen4),
            Selection::model(Tier::Sse41, Estimates::AmdZen4),
            Selection::model(Tier::Ml3, Estimates::AmdZen4),
            Selection::model(Tier::Ml4, Estimates::AmdZen4),
            Selection::model(Tier::Neon, Estimates::Arm),
        ];
        let barrier = Barrier::new(sels.len() + 1);
        std::thread::scope(|s| {
            for sel in sels {
                let barrier = &barrier;
                s.spawn(move || {
                    let _g = force_tier(sel).unwrap();
                    barrier.wait();
                    for _ in 0..100 {
                        assert_eq!(selection(), sel);
                        std::thread::yield_now();
                    }
                    barrier.wait();
                });
            }
            // This thread forces nothing and must keep the default while the others force.
            barrier.wait();
            assert_eq!(selection(), Selection::native(Tier::detect()));
            barrier.wait();
        });
    }

    #[test]
    fn out_of_order_drop_panics() {
        let r = std::thread::spawn(|| {
            let outer = force_tier(Selection::native(Tier::Scalar)).unwrap();
            let inner = force_tier(Selection::native(Tier::Scalar)).unwrap();
            drop(outer);
            drop(inner);
        })
        .join();
        assert!(r.is_err());
    }
}
