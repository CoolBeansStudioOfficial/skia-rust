// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of the flow of: src/gpu/graphite/SharedContext.cpp#L73-L125 and
// src/gpu/graphite/ResourceProvider.cpp#L44-L60 (chrome/m156). The backend's pipeline creation is
// a closure here, since the wgpu pipeline is G11b.

use std::any::Any;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use skia_rust_core::data::Data;
use skia_rust_gpu::gpu::resource_key::{UniqueKey, UniqueKeyBuilder};
use skia_rust_gpu::graphite::compute_pipeline::ComputePipeline;
use skia_rust_gpu::graphite::context_options::{
    Callback, ContextOptions, PipelineCacheOp, PipelineCachingCallbackFn,
};
use skia_rust_gpu::graphite::graphics_pipeline::{
    GraphicsPipeline, GraphicsPipelineBase, PipelineCreationFlags,
};
use skia_rust_gpu::graphite::wgpu::{make_context, noop_backend_context};

#[derive(Debug)]
struct FakeGraphicsPipeline {
    base: GraphicsPipelineBase,
}

impl GraphicsPipeline for FakeGraphicsPipeline {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn base(&self) -> &GraphicsPipelineBase {
        &self.base
    }
}

#[derive(Debug)]
struct FakeComputePipeline;

impl ComputePipeline for FakeComputePipeline {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn graphics(label: &str, hash: u32, compilation_id: u32) -> Arc<dyn GraphicsPipeline> {
    Arc::new(FakeGraphicsPipeline {
        base: GraphicsPipelineBase::new(label, hash, compilation_id, false),
    })
}

/// A fresh unique key; each has its own domain.
fn key() -> UniqueKey {
    let mut key = UniqueKey::new();
    {
        let _builder = UniqueKeyBuilder::new(&mut key, UniqueKey::generate_domain(), 0, None);
    }
    key
}

#[test]
fn a_miss_creates_once_and_later_lookups_reuse_the_pipeline() {
    let context = make_context(&noop_backend_context(), &ContextOptions::default())
        .expect("a context on the noop device");
    let shared = context.shared_context().base();
    let k = key();
    let creations = AtomicU32::new(0);

    let first = shared
        .find_or_create_graphics_pipeline(&k, PipelineCreationFlags::NONE, |id| {
            creations.fetch_add(1, Ordering::Relaxed);
            assert_ne!(id, 0, "a miss is given a compilation ID");
            Some(graphics("first", 1, id))
        })
        .expect("the pipeline is created");
    let second = shared
        .find_or_create_graphics_pipeline(&k, PipelineCreationFlags::NONE, |_| {
            creations.fetch_add(1, Ordering::Relaxed);
            Some(graphics("second", 2, 0))
        })
        .expect("the cached pipeline is found");

    assert_eq!(creations.load(Ordering::Relaxed), 1);
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(second.label(), "first");
}

#[test]
fn a_failed_creation_caches_nothing() {
    let context = make_context(&noop_backend_context(), &ContextOptions::default())
        .expect("a context on the noop device");
    let shared = context.shared_context().base();
    let k = key();

    assert!(
        shared
            .find_or_create_graphics_pipeline(&k, PipelineCreationFlags::NONE, |_| None)
            .is_none()
    );
    assert_eq!(
        context
            .shared_context()
            .base()
            .global_cache()
            .num_graphics_pipelines(),
        0
    );
}

#[test]
fn the_caching_callback_sees_additions_and_hits() {
    let seen: Arc<Mutex<Vec<(PipelineCacheOp, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let callback: Arc<PipelineCachingCallbackFn> = Arc::new(
        move |op: PipelineCacheOp, label: &str, _hash: u32, _pre: bool, key: Option<&Data>| {
            assert!(
                key.is_none(),
                "no serialized key until SerializationUtils (G14)"
            );
            sink.lock().unwrap().push((op, label.to_string()));
        },
    );
    let options = ContextOptions {
        pipeline_caching_callback: Some(Callback(callback)),
        ..ContextOptions::default()
    };
    let context = make_context(&noop_backend_context(), &options).expect("a context");
    let shared = context.shared_context().base();
    let k = key();

    shared.find_or_create_graphics_pipeline(&k, PipelineCreationFlags::NONE, |id| {
        Some(graphics("cb", 3, id))
    });
    shared.find_or_create_graphics_pipeline(&k, PipelineCreationFlags::NONE, |_| None);

    assert_eq!(
        *seen.lock().unwrap(),
        vec![
            (PipelineCacheOp::AddingPipeline, "cb".to_string()),
            (PipelineCacheOp::PipelineFound, "cb".to_string()),
        ]
    );
}

#[test]
fn compute_pipelines_are_created_once_per_key() {
    let context = make_context(&noop_backend_context(), &ContextOptions::default())
        .expect("a context on the noop device");
    let shared = context.shared_context().base();
    let k = key();
    let creations = AtomicU32::new(0);
    let make = || {
        creations.fetch_add(1, Ordering::Relaxed);
        Some(Arc::new(FakeComputePipeline) as Arc<dyn ComputePipeline>)
    };

    let first = shared
        .find_or_create_compute_pipeline(&k, make)
        .expect("created");
    let second = shared
        .find_or_create_compute_pipeline(&k, || panic!("the cached pipeline is reused"))
        .expect("found");

    assert_eq!(creations.load(Ordering::Relaxed), 1);
    assert!(Arc::ptr_eq(&first, &second));
}
