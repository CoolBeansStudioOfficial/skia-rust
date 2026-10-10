// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! G11c: the command trace (`trace` feature, `docs/design/gpu.md` §3.2). Runs on the noop
//! adapter, which records and "executes" command buffers without rendering.

#![cfg(all(feature = "trace", not(target_arch = "wasm32")))]

use skia_rust_core::color::Color4f;
use skia_rust_core::device::Device as CoreDevice;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect as SkRect;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_gpu::gpu::backing_fit::BackingFit;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, Mipmapped};
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::device::Device;
use skia_rust_gpu::graphite::graphite_types::{
    InsertRecordingInfo, InsertStatus, SubmitInfo, SyncToCpu,
};
use skia_rust_gpu::graphite::resource_types::LoadOp;
use skia_rust_gpu::graphite::wgpu::trace::{MemorySink, Record, Value};
use skia_rust_gpu::graphite::wgpu::{WgpuContext, WgpuSharedContext, noop_backend_context};

fn ops(records: &[Record]) -> Vec<&'static str> {
    records.iter().map(|record| record.op).collect()
}

fn field<'a>(record: &'a Record, name: &str) -> &'a Value {
    &record
        .fields
        .iter()
        .find(|(field, _)| *field == name)
        .unwrap_or_else(|| panic!("{} has no {name}", record.op))
        .1
}

#[test]
fn the_initialization_traces_the_static_buffers_and_their_copies() {
    let sink = MemorySink::default();
    let records = sink.records.clone();
    let options = ContextOptions::default();
    let shared = WgpuSharedContext::make(&noop_backend_context(), &options).unwrap();
    shared.set_trace_sink(Some(Box::new(sink)));
    let mut context = WgpuContext::new(shared, &options);
    assert!(context.finish_initialization());

    let records = records.lock().unwrap();
    let ops = ops(&records);
    // The transfer buffers and the static buffers are created, filled, copied, and submitted.
    assert!(ops.contains(&"create_buffer"));
    assert!(ops.contains(&"flush_mapped_buffer"));
    assert!(ops.contains(&"copy_buffer_to_buffer"));
    assert_eq!(ops.last(), Some(&"submit"));
    // The copies come before the submission, and the buffers before the copies.
    let first_copy = ops
        .iter()
        .position(|op| *op == "copy_buffer_to_buffer")
        .unwrap();
    let first_buffer = ops.iter().position(|op| *op == "create_buffer").unwrap();
    assert!(first_buffer < first_copy);
}

#[test]
fn a_draw_traces_its_pipeline_pass_and_commands() {
    let sink = MemorySink::default();
    let records = sink.records.clone();
    let options = ContextOptions::default();
    let shared = WgpuSharedContext::make(&noop_backend_context(), &options).unwrap();
    shared.set_trace_sink(Some(Box::new(sink)));
    let mut context = WgpuContext::new(shared, &options);
    assert!(context.finish_initialization());
    records.lock().unwrap().clear();

    let mut recorder = context.make_recorder(None);
    let mut device = Device::make_with_info(
        Some(&recorder),
        &ImageInfo::new_n32_premul((16, 16), None),
        Budgeted::Yes,
        Mipmapped::No,
        BackingFit::Exact,
        &SurfaceProps::default(),
        LoadOp::Clear,
        "Trace",
        true,
        false,
    )
    .unwrap();
    let mut paint = Paint::new(Color4f::new(1.0, 0.0, 0.0, 1.0), None);
    paint.set_anti_alias(false);
    device.draw_rect(&SkRect::new(2.0, 2.0, 10.0, 10.0), &paint);
    device.flush_pending_work();
    let mut recording = recorder.snap().unwrap();
    assert_eq!(
        context.insert_recording(InsertRecordingInfo::new(&mut recording)),
        InsertStatus::Success
    );
    assert!(context.submit(SubmitInfo::new(SyncToCpu::Yes)));

    let records = records.lock().unwrap();
    let ops = ops(&records);
    let position = |op: &str| {
        ops.iter()
            .position(|o| *o == op)
            .unwrap_or_else(|| panic!("{op}"))
    };
    assert!(position("create_texture") < position("begin_render_pass"));
    assert!(position("create_pipeline") < position("set_pipeline"));
    assert!(position("begin_render_pass") < position("set_pipeline"));
    assert!(position("set_pipeline") < position("draw"));
    assert!(position("draw") < position("end_render_pass"));
    assert!(position("end_render_pass") < position("submit"));

    // The pass loads nothing: the device's initial op is a clear.
    let begin = &records[position("begin_render_pass")];
    assert!(matches!(field(begin, "color_load"), Value::S(load) if load.starts_with("Clear")));
    // A JSON line is an object with the operation first.
    assert!(
        begin
            .to_json()
            .starts_with("{\"op\":\"begin_render_pass\",")
    );
}
