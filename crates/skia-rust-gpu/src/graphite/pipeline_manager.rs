// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/PipelineManager.h, src/gpu/graphite/PipelineManager.cpp

//! [`PipelineManager`]: finds, or compiles on the executor, the graphics pipelines of a
//! `SharedContext` (`docs/design/gpu.md` §5.4).
//!
//! The threaded manager is set up to work with an executor with two work lists. All work from the
//! first work list must be completed before work from the second list is begun. In-line compiles
//! are put in the high priority list while precompiles are put in the low priority work list. If
//! a single work list executor is provided everything collapses to being interleaved on that
//! single work list. The number of work lists is orthogonal to the number of threads.
//!
//! # Deviations from the C++
//!
//! - The `PipelineCreationTask` holds a `Weak` to the shared context where Skia holds a raw
//!   pointer (see [`PipelineCreationTask`]). The shared context is reached through the
//!   [`PipelineCreationContext`] trait, which is the part of `SharedContext` +
//!   `DawnSharedContext` a task needs: the neutral half, and `findOrCreateGraphicsPipeline` with
//!   the backend's `createGraphicsPipeline` behind it.
//! - `SkSpinlock` is a `Mutex`. Skia's `SkTaskGroup::add` ran under the spinlock; here the task
//!   group is shared out of the lock first, because the trivial executor runs the work in
//!   `add`, and the work takes the lock again.
//! - Dropping the manager shuts it down (waits for in-flight tasks) where the C++ destructor
//!   asserts that `Context` already did.
//! - `wait_TestOnly`, `getStats` and `Stats` exist in every build (Skia's are `GPU_TEST_UTILS`).

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak};

use skia_rust_core::executor::{Executor, TaskGroup};

use crate::gpu::resource_key::UniqueKey;
use crate::gpu::sk_log::skia_log_w;
use crate::graphite::graphics_pipeline::{GraphicsPipeline, PipelineCreationFlags};
use crate::graphite::graphics_pipeline_desc::GraphicsPipelineDesc;
use crate::graphite::graphics_pipeline_desc::GraphicsPipelineHandle;
use crate::graphite::graphics_pipeline_desc::PipelineHandleFactory;
use crate::graphite::pipeline_creation_task::PipelineCreationTask;
use crate::graphite::render_pass_desc::RenderPassDesc;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::shared_context::SharedContext;

/// `kHighPriorityWorkList`.
// Port of: src/gpu/graphite/PipelineManager.cpp#L25 (chrome/m156)
const HIGH_PRIORITY_WORK_LIST: i32 = 0;
/// `kLowPriorityWorkList`.
// Port of: src/gpu/graphite/PipelineManager.cpp#L26 (chrome/m156)
const LOW_PRIORITY_WORK_LIST: i32 = 1;

/// What a [`PipelineCreationTask`] needs of the `SharedContext` it compiles for: the neutral
/// half (caps, global cache, pipeline manager) and the backend's pipeline creation.
///
/// `WgpuSharedContext` implements it. A task holds it by `Weak`.
// Port of: src/gpu/graphite/SharedContext.h#L35-L136, the calls PipelineManager.cpp makes
pub trait PipelineCreationContext: Send + Sync {
    /// The backend-neutral half of the shared context.
    fn shared_context(&self) -> &SharedContext;

    /// `findOrCreateGraphicsPipeline(runtimeDict, pipelineKey, pipelineDesc, renderPassDesc,
    /// flags)`: finds the pipeline in the global cache or creates it with the backend's
    /// `createGraphicsPipeline` and adds it to the cache.
    // Port of: src/gpu/graphite/SharedContext.cpp#L73-L125 (chrome/m156)
    fn find_or_create_graphics_pipeline(
        &self,
        runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
        pipeline_key: &UniqueKey,
        pipeline_desc: &GraphicsPipelineDesc,
        render_pass_desc: &RenderPassDesc,
        flags: PipelineCreationFlags,
    ) -> Option<Arc<dyn GraphicsPipeline>>;
}

/// `PipelineManager::Priority`.
// Port of: src/gpu/graphite/PipelineManager.h#L83 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Priority {
    High = 0,
    Low = 1,
}

/// `PipelineManager::Stats`.
// Port of: src/gpu/graphite/PipelineManager.h#L65-L70 (chrome/m156)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// `fNumPreemptivelyFoundTasks`: the number of times we find a pre-existing task for a
    /// pipeline.
    pub num_preemptively_found_tasks: i32,
    /// `fNumTasksCreated`.
    pub num_tasks_created: i32,
}

/// The state `fSpinLock` guards.
#[derive(Default)]
struct State {
    /// `fActiveTasks`.
    active_tasks: HashMap<UniqueKey, Arc<PipelineCreationTask>>,
    /// `fStats`.
    stats: Stats,
    /// `fTaskGroup`: `None` without an executor, and after `shutDown`.
    task_group: Option<Arc<TaskGroup>>,
}

/// `skgpu::graphite::PipelineManager`.
// Port of: src/gpu/graphite/PipelineManager.h#L31-L121 (chrome/m156)
#[doc(alias = "skgpu::graphite::PipelineManager")]
pub struct PipelineManager {
    /// `fSpinLock` and what it guards.
    state: Mutex<State>,
    /// `fMutex`: we have the mutex and condition variable here to limit the number of
    /// mutexes/semaphores we need for synchronizing access to the pipelines. The `Context` thread
    /// is the only place that resolves handles so we will only ever be waiting on at most one
    /// pipeline at a time and no other thread will need to block on waiting for a different
    /// pipeline. This means we don't need a condition variable in every `PipelineCreationTask`.
    mutex: Mutex<()>,
    /// `fConditionVariable`.
    condition_variable: Condvar,
}

impl std::fmt::Debug for PipelineManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.lock_state();
        f.debug_struct("PipelineManager")
            .field("threaded", &state.task_group.is_some())
            .field("active_tasks", &state.active_tasks.len())
            .field("stats", &state.stats)
            .finish_non_exhaustive()
    }
}

impl PipelineManager {
    /// `PipelineManager(executor)`.
    // Port of: src/gpu/graphite/PipelineManager.cpp#L56-L60 (chrome/m156)
    #[must_use]
    pub fn new(executor: Option<Arc<dyn Executor>>) -> Self {
        let state = State {
            task_group: executor.map(|executor| Arc::new(TaskGroup::new(executor))),
            ..State::default()
        };
        Self {
            state: Mutex::new(state),
            mutex: Mutex::new(()),
            condition_variable: Condvar::new(),
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `createHandle(sharedContext, runtimeDict, pipelineDesc, renderPassDesc,
    /// pipelineCreationFlags)`: if an existing pipeline is found, it is just wrapped in a handle
    /// and returned. Otherwise, a compilation task is created and queued up for execution. If no
    /// executor was provided the compilations occur synchronously, in-line.
    ///
    /// `shared_context` is the context that owns this manager.
    // Port of: src/gpu/graphite/PipelineManager.cpp#L83-L143 (chrome/m156)
    #[doc(alias = "createHandle")]
    pub fn create_handle(
        &self,
        shared_context: &Arc<dyn PipelineCreationContext>,
        runtime_dict: Option<Arc<RuntimeEffectDictionary>>,
        pipeline_desc: &GraphicsPipelineDesc,
        render_pass_desc: &RenderPassDesc,
        pipeline_creation_flags: PipelineCreationFlags,
    ) -> GraphicsPipelineHandle {
        let base = shared_context.shared_context();
        let global_cache = base.global_cache();
        let caps = base.caps();

        let for_precompile =
            pipeline_creation_flags.contains(PipelineCreationFlags::FOR_PRECOMPILATION);
        let cur_priority = if for_precompile {
            Priority::Low
        } else {
            Priority::High
        };

        let pipeline_key = caps.make_graphics_pipeline_key(pipeline_desc, render_pass_desc);

        if let Some(pipeline) =
            global_cache.find_graphics_pipeline(&pipeline_key, pipeline_creation_flags, None)
        {
            return GraphicsPipelineHandle::from_pipeline(Some(pipeline));
        }

        // Although 'findGraphicsPipeline' didn't find a GraphicsPipeline, there could be a race.
        // In 'findOrCreateTask' we will, thread-safely, check if there is a task in flight to
        // create the Pipeline and, failing that, create one. If the race had occurred and
        // there is actually a matching GraphicsPipeline in the GlobalCache then it will be found
        // in 'compileTask'.
        let task = self.find_or_create_task(
            shared_context,
            runtime_dict,
            &pipeline_key,
            *pipeline_desc,
            render_pass_desc,
            cur_priority,
        );

        let should_add_to_work_list = if cur_priority == Priority::High && task.is_low_priority() {
            // If we found an active task for the current Pipeline we know, modulo thread races,
            // that it hasn't completed yet (since it, then, wouldn't be in the active task list).
            // If it was initially added as low priority, but turned out to be high priority,
            // re-add it as a high priority task.
            !task.is_high_priority.swap(true, Ordering::AcqRel)
        } else {
            // Tasks are only removed from the TaskList when they are complete. This means that
            // an in-flight task can be found and resubmitted for compilation. The 'fInWorkList'
            // guard ensures we don't resubmit the same task over and over (modulo the one-off
            // switch from low to high priority above).
            !task.in_work_list.swap(true, Ordering::AcqRel)
        };

        if should_add_to_work_list {
            self.add_task_to_work_list(&task, cur_priority);
        }

        GraphicsPipelineHandle::from_task(task)
    }

    /// `InlineCompile(task)`: returns true if compilation occurred; false otherwise.
    // Port of: src/gpu/graphite/PipelineManager.cpp#L145-L195 (chrome/m156)
    fn inline_compile(task: &PipelineCreationTask) -> bool {
        // Since there might be threaded contention to execute the compilation for the same
        // task (e.g., if a low priority compile got duplicated as a high priority compile
        // or an immediate compile was required), we check the 'fStarted' atomic so only
        // one does the work.
        if task.started.swap(true, Ordering::AcqRel) {
            // If we got here it means some other thread beat us to it so don't compile
            // the pipeline.
            return false;
        }

        let Some(shared_context) = task.shared_context() else {
            // The shared context is gone: nothing can compile, and nothing is left to wake.
            let _ = task.pipeline.set(None);
            task.completed.store(true, Ordering::Release);
            return true;
        };
        let pipeline_manager = shared_context.shared_context().pipeline_manager();

        // This is a bit racy but the exact correctness of the actual cause for the compilation
        // isn't crucial. In essence, this tries to give the SharedContext a best guess about
        // the driver behind the compilation. The exact race is if a precompile compilation
        // was usurped by a normal compilation but we still report a precompile compilation
        // to the SharedContext.
        let flags = if task.is_low_priority() {
            PipelineCreationFlags::FOR_PRECOMPILATION
        } else {
            PipelineCreationFlags::NONE
        };

        let pipeline = shared_context.find_or_create_graphics_pipeline(
            task.runtime_dict.as_ref(),
            &task.pipeline_key,
            &task.graphics_pipeline_desc,
            &task.render_pass_desc,
            flags,
        );

        if pipeline.is_none() {
            skia_log_w!("Failed to create GraphicsPipeline!");
        }
        let _ = task.pipeline.set(pipeline);

        pipeline_manager.signal_completed(task);
        pipeline_manager.remove_task(task);

        task.clear_shared_context();
        true
    }

    /// `addTaskToWorkList(sharedContext, task, priority)`.
    // Port of: src/gpu/graphite/PipelineManager.cpp#L197-L222 (chrome/m156)
    fn add_task_to_work_list(&self, task: &Arc<PipelineCreationTask>, priority: Priority) {
        let task_group = self.lock_state().task_group.clone();
        if let Some(task_group) = task_group {
            let work_list = if priority == Priority::Low {
                LOW_PRIORITY_WORK_LIST
            } else {
                HIGH_PRIORITY_WORK_LIST
            };

            let task = Arc::clone(task);
            task_group.add(
                move || {
                    Self::inline_compile(&task);
                },
                work_list,
            );
            return;
        }

        // Non-SkExecutor fallback. Note that, if multiple Recorders are recording in parallel on
        // multiple threads (w/ no SkExecutor supplied) there could still be a compilation race
        // here. In that case all the thread-safety mechanisms (e.g., 'fStarted', 'fCompleted')
        // will kick in to eliminate duplicate work. This does mean, as in the SkExecutor case,
        // that the task's Pipeline need not be resolved at the end of 'compileTask'. That is,
        // after all, the purview of 'resolveHandle'.
        Self::inline_compile(task);
    }

    /// `resolveHandle(handle)`: the handle's pipeline, waiting for its compilation if needed;
    /// `None` if the compilation failed.
    // Port of: src/gpu/graphite/PipelineManager.cpp#L224-L238 (chrome/m156)
    #[doc(alias = "resolveHandle")]
    #[must_use]
    pub fn resolve_handle(
        &self,
        handle: &GraphicsPipelineHandle,
    ) -> Option<Arc<dyn GraphicsPipeline>> {
        // A handle holds either a pipeline (or none, if creating it failed), or the task that
        // creates it.
        let Some(task) = handle.task() else {
            return handle.pipeline_or_null();
        };
        let Some(task) = task.as_any().downcast_ref::<PipelineCreationTask>() else {
            return handle.pipeline_or_null();
        };

        // For the non-threaded PipelineManager, the GraphicsPipeline will have been compiled
        // in-line so will already have been completed.
        self.potentially_wait_on(task);
        task.pipeline.get().cloned().flatten()
    }

    /// `shutDown()`: waits for any in-flight tasks to complete. Additionally, disables the
    /// addition of any more threaded tasks.
    // Port of: src/gpu/graphite/PipelineManager.cpp#L240-L260 (chrome/m156)
    #[doc(alias = "shutDown")]
    pub fn shut_down(&self) {
        // We take out 'fTaskGroup' so no more threaded work can be added after this point.
        let tmp = self.lock_state().task_group.take();
        if let Some(tmp) = tmp {
            // We have to wait for the remaining tasks to complete bc they rely on the existence
            // of the SharedContext and the PipelineManager (this).
            // TODO(robertphillips) We could discard any unstarted tasks but would need a way to
            // have them still remove themselves from the task list.
            tmp.wait();
        }
        // (The C++ asserts that no task is left active. A task whose shared context was already
        // gone when it ran completes without leaving the list, so the assert is not made here.)
    }

    /// The number of tasks that have been created and not yet completed (`fActiveTasks.count()`).
    #[must_use]
    pub fn num_active_tasks(&self) -> usize {
        self.lock_state().active_tasks.len()
    }

    /// `wait_TestOnly()`: waits for the tasks in the work lists. This isn't safe (since the task
    /// group could be altered on some other thread) but, hopefully, the unit tests know what
    /// they're doing (i.e., don't delete the owning `Context` while in this method).
    // Port of: src/gpu/graphite/PipelineManager.cpp#L262-L278 (chrome/m156)
    pub fn wait_test_only(&self) {
        let tmp = self.lock_state().task_group.clone();
        if let Some(tmp) = tmp {
            tmp.wait();
        }
    }

    /// `getStats()`.
    // Port of: src/gpu/graphite/PipelineManager.cpp#L280-L284 (chrome/m156)
    #[doc(alias = "getStats")]
    #[must_use]
    pub fn get_stats(&self) -> Stats {
        self.lock_state().stats
    }

    /// `findOrCreateTask(...)`.
    // Port of: src/gpu/graphite/PipelineManager.cpp#L287-L318 (chrome/m156)
    fn find_or_create_task(
        &self,
        shared_context: &Arc<dyn PipelineCreationContext>,
        runtime_dict: Option<Arc<RuntimeEffectDictionary>>,
        pipeline_key: &UniqueKey,
        pipeline_desc: GraphicsPipelineDesc,
        render_pass_desc: &RenderPassDesc,
        priority: Priority,
    ) -> Arc<PipelineCreationTask> {
        let mut state = self.lock_state();

        if let Some(task) = state.active_tasks.get(pipeline_key) {
            let task = Arc::clone(task);
            state.stats.num_preemptively_found_tasks += 1;
            return task;
        }

        state.stats.num_tasks_created += 1;

        let new_task = Arc::new(PipelineCreationTask::new(
            Arc::downgrade(shared_context),
            runtime_dict,
            pipeline_key.clone(),
            pipeline_desc,
            render_pass_desc.clone(),
            priority == Priority::High,
        ));
        state
            .active_tasks
            .insert(pipeline_key.clone(), Arc::clone(&new_task));
        new_task
    }

    /// `removeTask(task)`.
    // Port of: src/gpu/graphite/PipelineManager.cpp#L320-L324 (chrome/m156)
    fn remove_task(&self, task: &PipelineCreationTask) {
        self.lock_state().active_tasks.remove(&task.pipeline_key);
    }

    /// `signalCompleted(task)`.
    // Port of: src/gpu/graphite/PipelineManager.cpp#L326-L337 (chrome/m156)
    fn signal_completed(&self, task: &PipelineCreationTask) {
        {
            let _lock = self.mutex.lock().unwrap_or_else(PoisonError::into_inner);

            // Even though 'fCompleted' is atomic it is still required that it be
            // modified within the locked mutex lest the 'wait' in potentiallyWaitOn
            // misses the signal.
            task.completed.store(true, Ordering::Release);
        }
        // potentiallyWaitOn should only ever be called from the main thread (on which
        // Context::insertRecording is called) so only one thread should ever be waiting
        self.condition_variable.notify_one();
    }

    /// `potentiallyWaitOn(task)`.
    // Port of: src/gpu/graphite/PipelineManager.cpp#L340-L354 (chrome/m156)
    fn potentially_wait_on(&self, task: &PipelineCreationTask) {
        // If we can preempt some thread that is scheduled to compile this Pipeline, do so rather
        // than waiting.
        if Self::inline_compile(task) {
            debug_assert!(task.completed.load(Ordering::Acquire));
            return;
        }

        let lock = self.mutex.lock().unwrap_or_else(PoisonError::into_inner);

        if task.completed.load(Ordering::Acquire) {
            return;
        }
        let _lock = self
            .condition_variable
            .wait_while(lock, |()| !task.completed.load(Ordering::Acquire))
            .unwrap_or_else(PoisonError::into_inner);

        debug_assert!(task.completed.load(Ordering::Acquire));
    }
}

impl Drop for PipelineManager {
    // Port of: src/gpu/graphite/PipelineManager.cpp#L62-L69 (chrome/m156): the C++ asserts that
    // `Context` shut the manager down; here the drop does it if nobody did.
    fn drop(&mut self) {
        self.shut_down();
    }
}

/// The [`PipelineHandleFactory`] a draw pass is given: `sharedContext->pipelineManager()` of a
/// shared context. It holds the context weakly, like the creation tasks do; once the context is
/// gone every handle is one of a pipeline that failed to compile.
#[derive(Clone)]
pub struct SharedContextPipelineFactory {
    shared_context: Weak<dyn PipelineCreationContext>,
}

impl std::fmt::Debug for SharedContextPipelineFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedContextPipelineFactory")
            .finish_non_exhaustive()
    }
}

impl SharedContextPipelineFactory {
    /// The factory of `shared_context`'s pipeline manager.
    #[must_use]
    pub fn new(shared_context: &Arc<dyn PipelineCreationContext>) -> Self {
        Self {
            shared_context: Arc::downgrade(shared_context),
        }
    }
}

impl PipelineHandleFactory for SharedContextPipelineFactory {
    fn create_handle(
        &self,
        runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
        pipeline_desc: &GraphicsPipelineDesc,
        render_pass_desc: &RenderPassDesc,
        flags: PipelineCreationFlags,
    ) -> GraphicsPipelineHandle {
        let Some(shared_context) = self.shared_context.upgrade() else {
            return GraphicsPipelineHandle::from_pipeline(None);
        };
        shared_context
            .shared_context()
            .pipeline_manager()
            .create_handle(
                &shared_context,
                runtime_dict.map(Arc::clone),
                pipeline_desc,
                render_pass_desc,
                flags,
            )
    }

    fn resolve_handle(&self, handle: &GraphicsPipelineHandle) -> Option<Arc<dyn GraphicsPipeline>> {
        match self.shared_context.upgrade() {
            Some(shared_context) => shared_context
                .shared_context()
                .pipeline_manager()
                .resolve_handle(handle),
            None => handle.pipeline_or_null(),
        }
    }
}
