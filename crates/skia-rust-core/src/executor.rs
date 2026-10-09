// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkExecutor.h, src/core/SkExecutor.cpp, src/core/SkTaskGroup.h,
// src/core/SkTaskGroup.cpp (chrome/m156)

//! `SkExecutor`: work handed to a thread pool, and `SkTaskGroup`, which waits for a batch of it.
//!
//! Skia's default executor (`SkExecutor::GetDefault`) is a global. This port has no global
//! state: a client passes the executor it wants, and [`TrivialExecutor`] runs work on the
//! calling thread, as Skia's trivial executor does. `SkTaskGroup` holds its executor by `Arc`
//! where Skia holds a reference.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;

/// A unit of work handed to an executor.
pub type Work = Box<dyn FnOnce() + Send + 'static>;

/// `SkExecutor`: runs work, possibly on other threads.
// Port of: include/core/SkExecutor.h#L16-L44 (chrome/m156)
#[doc(alias = "SkExecutor")]
pub trait Executor: Send + Sync {
    /// `add(fn, workList)`: runs `work` in the given work list.
    fn add_to_work_list(&self, work: Work, work_list: i32);

    /// `add(fn)`: runs `work` in work list 0.
    fn add(&self, work: Work) {
        self.add_to_work_list(work, 0);
    }

    /// `borrow()`: lets the calling thread run one queued unit of work, if the executor allows it.
    fn borrow(&self) {}
}

/// `SkTrivialExecutor`: runs every unit of work immediately, on the calling thread.
// Port of: src/core/SkExecutor.cpp#L26-L35 (chrome/m156)
#[derive(Debug, Default, Clone, Copy)]
pub struct TrivialExecutor;

impl Executor for TrivialExecutor {
    fn add_to_work_list(&self, work: Work, _work_list: i32) {
        work();
    }
}

/// The order a thread pool takes work from its work lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkOrder {
    /// `MakeFIFOThreadPool`: the oldest work first.
    Fifo,
    /// `MakeLIFOThreadPool`: the newest work first.
    Lifo,
}

/// The work lists and the shutdown flag the pool's lock guards.
struct PoolState {
    work_lists: Vec<VecDeque<Work>>,
    shutdown: bool,
}

struct PoolShared {
    state: Mutex<PoolState>,
    work_available: Condvar,
    order: WorkOrder,
    allow_borrowing: bool,
}

impl PoolShared {
    fn lock(&self) -> std::sync::MutexGuard<'_, PoolState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Takes one unit of work from the first non-empty work list, in the pool's order. `None` if
    /// every list is empty.
    // Port of: src/core/SkExecutor.cpp#L80-L97 (`do_work`, the pop, chrome/m156)
    fn pop(&self, state: &mut PoolState) -> Option<Work> {
        state
            .work_lists
            .iter_mut()
            .find_map(|list| match self.order {
                WorkOrder::Fifo => list.pop_front(),
                WorkOrder::Lifo => list.pop_back(),
            })
    }
}

/// `SkThreadPool`: a pool of worker threads over one or more work lists.
// Port of: src/core/SkExecutor.cpp#L107-L166 (chrome/m156)
#[doc(alias = "SkExecutor::MakeFIFOThreadPool")]
#[doc(alias = "SkExecutor::MakeLIFOThreadPool")]
pub struct ThreadPool {
    shared: Arc<PoolShared>,
    threads: Vec<JoinHandle<()>>,
}

impl std::fmt::Debug for ThreadPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreadPool")
            .field("order", &self.shared.order)
            .field("threads", &self.threads.len())
            .finish_non_exhaustive()
    }
}

impl ThreadPool {
    /// A pool with `num_work_lists` work lists (at least one), `threads` worker threads
    /// (`0` means one per available core, as Skia's `num_cores()`), and `allow_borrowing`, which
    /// lets [`Executor::borrow`] run work on the calling thread.
    // Port of: src/core/SkExecutor.cpp#L107-L118 and #L168-L185 (chrome/m156)
    #[must_use]
    pub fn new(
        order: WorkOrder,
        num_work_lists: usize,
        threads: usize,
        allow_borrowing: bool,
    ) -> Self {
        let threads = if threads > 0 {
            threads
        } else {
            std::thread::available_parallelism().map_or(1, usize::from)
        };
        let shared = Arc::new(PoolShared {
            state: Mutex::new(PoolState {
                work_lists: (0..num_work_lists.max(1))
                    .map(|_| VecDeque::new())
                    .collect(),
                shutdown: false,
            }),
            work_available: Condvar::new(),
            order,
            allow_borrowing,
        });
        let threads = (0..threads)
            .map(|_| {
                let shared = Arc::clone(&shared);
                std::thread::spawn(move || worker_loop(&shared))
            })
            .collect();
        Self { shared, threads }
    }

    /// `SkExecutor::MakeFIFOThreadPool(threads, allowBorrowing)`.
    #[must_use]
    pub fn fifo(threads: usize, allow_borrowing: bool) -> Self {
        Self::new(WorkOrder::Fifo, 1, threads, allow_borrowing)
    }

    /// `SkExecutor::MakeLIFOThreadPool(threads, allowBorrowing)`.
    #[must_use]
    pub fn lifo(threads: usize, allow_borrowing: bool) -> Self {
        Self::new(WorkOrder::Lifo, 1, threads, allow_borrowing)
    }
}

/// A worker's loop: take work while there is any, and stop once the pool shuts down with no work
/// left.
// Port of: src/core/SkExecutor.cpp#L134-L140 (`Loop`, chrome/m156)
fn worker_loop(shared: &PoolShared) {
    loop {
        let work = {
            let mut state = shared.lock();
            loop {
                if let Some(work) = shared.pop(&mut state) {
                    break Some(work);
                }
                if state.shutdown {
                    break None;
                }
                state = shared
                    .work_available
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        };
        match work {
            Some(work) => work(),
            None => return,
        }
    }
}

impl Executor for ThreadPool {
    // Port of: src/core/SkExecutor.cpp#L142-L151 (chrome/m156)
    fn add_to_work_list(&self, work: Work, work_list: i32) {
        {
            let mut state = self.shared.lock();
            let last = state.work_lists.len() - 1;
            let index = usize::try_from(work_list).unwrap_or(0).min(last);
            state.work_lists[index].push_back(work);
        }
        self.shared.work_available.notify_one();
    }

    // Port of: src/core/SkExecutor.cpp#L154-L158 (chrome/m156)
    fn borrow(&self) {
        if !self.shared.allow_borrowing {
            return;
        }
        let work = {
            let mut state = self.shared.lock();
            self.shared.pop(&mut state)
        };
        if let Some(work) = work {
            work();
        }
    }
}

impl Drop for ThreadPool {
    // Port of: src/core/SkExecutor.cpp#L119-L131 (chrome/m156)
    fn drop(&mut self) {
        self.shared.lock().shutdown = true;
        self.shared.work_available.notify_all();
        for thread in self.threads.drain(..) {
            // A worker that panicked has already reported the panic.
            let _ = thread.join();
        }
    }
}

/// `SkTaskGroup`: runs a batch of work on an executor and waits for all of it.
// Port of: src/core/SkTaskGroup.h#L18-L46 and src/core/SkTaskGroup.cpp (chrome/m156)
#[doc(alias = "SkTaskGroup")]
pub struct TaskGroup {
    /// `fPending`: the units of work added and not yet finished.
    pending: Arc<AtomicUsize>,
    /// `fExecutor`.
    executor: Arc<dyn Executor>,
}

impl std::fmt::Debug for TaskGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskGroup")
            .field("pending", &self.pending.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl TaskGroup {
    /// `SkTaskGroup(executor)`.
    #[must_use]
    pub fn new(executor: Arc<dyn Executor>) -> Self {
        Self {
            pending: Arc::new(AtomicUsize::new(0)),
            executor,
        }
    }

    /// `add(fn, workList)`: runs `work` on the executor, in the given work list.
    // Port of: src/core/SkTaskGroup.cpp#L16-L24 (chrome/m156)
    pub fn add(&self, work: impl FnOnce() + Send + 'static, work_list: i32) {
        self.pending.fetch_add(1, Ordering::Relaxed);
        let pending = Arc::clone(&self.pending);
        self.executor.add_to_work_list(
            Box::new(move || {
                work();
                pending.fetch_sub(1, Ordering::Release);
            }),
            work_list,
        );
    }

    /// `done()`: whether every unit of work added so far has run.
    // Port of: src/core/SkTaskGroup.cpp#L26-L28 (chrome/m156)
    #[must_use]
    pub fn done(&self) -> bool {
        self.pending.load(Ordering::Acquire) == 0
    }

    /// `wait()`: blocks until [`TaskGroup::done`], borrowing the executor's threads meanwhile.
    // Port of: src/core/SkTaskGroup.cpp#L30-L34 (chrome/m156)
    pub fn wait(&self) {
        while !self.done() {
            self.executor.borrow();
            std::thread::yield_now();
        }
    }
}

impl Drop for TaskGroup {
    // Port of: src/core/SkTaskGroup.h#L24 (`~SkTaskGroup() { this->wait(); }`, chrome/m156)
    fn drop(&mut self) {
        self.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recorder() -> (Arc<Mutex<Vec<u32>>>, impl Fn(u32) -> Work) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&log);
        (log, move |n| -> Work {
            let sink = Arc::clone(&sink);
            Box::new(move || sink.lock().unwrap().push(n))
        })
    }

    #[test]
    fn trivial_executor_runs_work_immediately() {
        let (log, work) = recorder();
        TrivialExecutor.add(work(1));
        assert_eq!(*log.lock().unwrap(), vec![1]);
    }

    /// The shared state of a pool with no worker threads, so that the test drains it in order.
    fn bare_pool(order: WorkOrder) -> PoolShared {
        PoolShared {
            state: Mutex::new(PoolState {
                work_lists: vec![VecDeque::new()],
                shutdown: false,
            }),
            work_available: Condvar::new(),
            order,
            allow_borrowing: true,
        }
    }

    fn drain(shared: &PoolShared) {
        loop {
            let mut state = shared.lock();
            let Some(work) = shared.pop(&mut state) else {
                return;
            };
            drop(state);
            work();
        }
    }

    #[test]
    fn fifo_takes_work_in_submission_order() {
        let shared = bare_pool(WorkOrder::Fifo);
        let (log, work) = recorder();
        for n in 0..5 {
            shared.lock().work_lists[0].push_back(work(n));
        }
        drain(&shared);
        assert_eq!(*log.lock().unwrap(), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn lifo_takes_newest_work_first() {
        let shared = bare_pool(WorkOrder::Lifo);
        let (log, work) = recorder();
        for n in 0..4 {
            shared.lock().work_lists[0].push_back(work(n));
        }
        drain(&shared);
        assert_eq!(*log.lock().unwrap(), vec![3, 2, 1, 0]);
    }

    #[test]
    fn task_group_wait_borrows_work_from_the_pool() {
        // Every unit runs once, on a worker or on this thread, which `wait` lends to the pool.
        let pool = Arc::new(ThreadPool::fifo(1, true));
        let (log, work) = recorder();
        let group = TaskGroup::new(pool.clone());
        for n in 0..5 {
            let w = work(n);
            group.add(move || w(), 0);
        }
        group.wait();
        assert!(group.done());
        assert_eq!(log.lock().unwrap().len(), 5);
    }

    #[test]
    fn worker_threads_finish_every_task_before_the_group_is_done() {
        let pool = Arc::new(ThreadPool::fifo(3, false));
        let count = Arc::new(AtomicUsize::new(0));
        let group = TaskGroup::new(pool.clone());
        for _ in 0..200 {
            let count = Arc::clone(&count);
            group.add(
                move || {
                    count.fetch_add(1, Ordering::Relaxed);
                },
                0,
            );
        }
        group.wait();
        assert!(group.done());
        assert_eq!(count.load(Ordering::Relaxed), 200);
    }

    #[test]
    fn out_of_range_work_lists_are_pinned_to_the_last_list() {
        let pool = ThreadPool::new(WorkOrder::Fifo, 2, 0, true);
        let (log, work) = recorder();
        let group = TaskGroup::new(Arc::new(pool));
        let w = work(7);
        group.add(move || w(), 9);
        group.wait();
        assert_eq!(*log.lock().unwrap(), vec![7]);
    }

    #[test]
    fn dropping_a_pool_runs_queued_work_first() {
        let (log, work) = recorder();
        {
            let pool = ThreadPool::fifo(1, false);
            for n in 0..10 {
                pool.add(work(n));
            }
        }
        assert_eq!(*log.lock().unwrap(), (0..10).collect::<Vec<_>>());
    }
}
