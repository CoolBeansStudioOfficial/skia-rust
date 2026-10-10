// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/ComputeTest.cpp (chrome/m156)

#![cfg(test)]
// The tests check exact float results, as the C++ does (there are no tolerances in this project).
// The C++ tests are long, convert integer problem sizes to floats and declare their constants
// between statements; the allows mirror that.
#![allow(
    clippy::items_after_statements,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_lossless,
    clippy::similar_names,
    clippy::unreadable_literal,
    clippy::needless_range_loop
)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::IRect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::size::ISize;
use skia_rust_core::swizzle::Swizzle;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_gpu::gpu::buffer_writer::BufferWriter;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, Mipmapped, Protected, Renderable};
use skia_rust_gpu::graphite::buffer::Buffer;
use skia_rust_gpu::graphite::compute::compute_step::{
    ComputeStep, ComputeStepBase, DataFlow, INDIRECT_DISPATCH_ARGUMENT_SIZE, NativeShaderFormat,
    NativeShaderSource, ResourceDesc, ResourcePolicy, ResourceType, WorkgroupSize,
};
use skia_rust_gpu::graphite::compute::dispatch_group::{BindingResource, Builder};
use skia_rust_gpu::graphite::graphite_types::{InsertRecordingInfo, SubmitInfo, SyncToCpu};
use skia_rust_gpu::graphite::recorder::Recorder;
use skia_rust_gpu::graphite::recording::Recording;
use skia_rust_gpu::graphite::resource::ResourceRef;
use skia_rust_gpu::graphite::resource_types::{
    AccessPattern, BufferType, ClearBuffer, SamplerDesc,
};
use skia_rust_gpu::graphite::task::compute_task::ComputeTask;
use skia_rust_gpu::graphite::task::copy_task::CopyBufferToBufferTask;
use skia_rust_gpu::graphite::task::synchronize_to_cpu_task::SynchronizeToCpuTask;
use skia_rust_gpu::graphite::task::upload_task::{
    ImageUploadContext, MipLevel, UploadInstance, UploadSource, UploadTask,
};
use skia_rust_gpu::graphite::texture_proxy::TextureProxy;
use skia_rust_gpu::graphite::texture_proxy_view::TextureProxyView;
use skia_rust_gpu::graphite::uniform::{K_NON_ARRAY, Uniform};
use skia_rust_gpu::graphite::uniform_manager::UniformManager;
use skia_rust_gpu::graphite::wgpu::WgpuContext;
use skia_rust_gpu::sksl_type_shared::SkSLType;

use crate::{def_graphite_adapter_test, errorf, reporter_assert};

/// `kFloatToFloat4Padding`: the bytes to zero out to keep the `BufferWriter` in sync when a float
/// is written into a `float4`-aligned field.
const K_FLOAT_TO_FLOAT4_PADDING: usize = 3 * std::mem::size_of::<f32>();

/// `map_buffer()`: maps `buffer` (waiting for an asynchronous map if the backend needs one) and
/// returns its contents from `offset` on.
// Port of: tests/graphite/ComputeTest.cpp#L44-L56 (chrome/m156)
fn map_buffer(context: &mut WgpuContext, buffer: &Buffer, offset: usize) -> Vec<u8> {
    if context.shared_context().caps().buffer_maps_are_async() {
        let finished = Arc::new(AtomicBool::new(false));
        let flag = finished.clone();
        buffer.async_map(Some(Box::new(move |_| flag.store(true, Ordering::Release))));
        while !finished.load(Ordering::Acquire) {
            context.shared_context().tick();
        }
    }
    let mut data = buffer
        .map()
        .expect("the buffer maps once its map completes");
    data.split_off(offset)
}

/// Reads `bytes` as native-endian `f32`s, the way the tests read the output buffers.
fn read_f32s(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_ne_bytes(*c))
        .collect()
}

/// `sync_buffer_to_cpu()`: makes `buffer` readable on the CPU, through a transfer buffer if the
/// backend cannot map it directly. Returns the buffer to map after submission.
// Port of: tests/graphite/ComputeTest.cpp#L58-L84 (chrome/m156)
fn sync_buffer_to_cpu(
    recorder: &mut Recorder,
    buffer: &ResourceRef<Buffer>,
) -> ResourceRef<Buffer> {
    if recorder.priv_().caps().draw_buffer_can_be_mapped() {
        // `buffer` can be mapped directly, however it may still require a synchronization step
        // by the underlying API (e.g. a managed buffer in Metal). SynchronizeToCpuTask
        // automatically handles this for us.
        recorder
            .priv_()
            .add(SynchronizeToCpuTask::make(buffer.clone()));
        return buffer.clone();
    }

    // The backend requires a transfer buffer for CPU read-back
    let xfer_buffer = recorder
        .priv_()
        .resource_provider()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .find_or_create_non_shareable_buffer(
            buffer.size(),
            BufferType::XferGpuToCpu,
            AccessPattern::HostVisible,
            "ComputeTest_TransferToCpu",
        )
        .expect("a transfer buffer");

    recorder.priv_().add(CopyBufferToBufferTask::make(
        buffer.as_arc(),
        0,
        xfer_buffer.clone(),
        0,
        buffer.size(),
    ));
    xfer_buffer
}

/// `submit_recording()`: snaps `recorder`, inserts the recording and waits for the GPU.
// Port of: tests/graphite/ComputeTest.cpp#L86-L99 (chrome/m156)
fn submit_recording(context: &mut WgpuContext, recorder: &mut Recorder) -> Option<Recording> {
    let mut recording = recorder.snap()?;
    let _ = context.insert_recording(InsertRecordingInfo::new(&mut recording));
    let _ = context.submit(SubmitInfo {
        sync: SyncToCpu::Yes,
        ..SubmitInfo::default()
    });
    Some(recording)
}

// Port of: tests/graphite/ComputeTest.cpp#L120-L247 (chrome/m156)
def_graphite_adapter_test!(Compute_SingleDispatchTest, |reporter, context| {
    const K_PROBLEM_SIZE: u32 = 512;
    const K_FACTOR: f32 = 4.0;

    // The ComputeStep packs kProblemSize floats into kProblemSize / 4 vectors and each thread
    // processes 1 vector at a time.
    const K_WORKGROUP_SIZE: u32 = K_PROBLEM_SIZE / 4;

    // TODO(skbug.com/40045541): SkSL doesn't support std430 layout well, so the buffers below all
    // pack their data into vectors to be compatible with SPIR-V/WGSL.
    #[derive(Debug)]
    struct TestComputeStep {
        base: ComputeStepBase,
    }

    impl ComputeStep for TestComputeStep {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        // A kernel that multiplies a large array of floats by a supplied factor.
        fn compute_sksl(&self) -> String {
            r"
                void main() {
                    out_data[sk_GlobalInvocationID.x] = in_data[sk_GlobalInvocationID.x] * factor;
                }
            "
            .to_owned()
        }

        fn calculate_buffer_size(&self, index: usize, r: &ResourceDesc) -> usize {
            if index == 0 {
                debug_assert_eq!(r.flow, DataFlow::Private);
                return std::mem::size_of::<f32>() * (K_PROBLEM_SIZE as usize + 4);
            }
            debug_assert_eq!(index, 1);
            debug_assert_eq!(r.slot, 0);
            debug_assert_eq!(r.flow, DataFlow::Shared);
            std::mem::size_of::<f32>() * K_PROBLEM_SIZE as usize
        }

        fn prepare_storage_buffer(
            &self,
            resource_index: usize,
            r: &ResourceDesc,
            mut writer: BufferWriter<'_>,
        ) {
            // Only initialize the input buffer.
            if resource_index != 0 {
                return;
            }
            debug_assert_eq!(r.flow, DataFlow::Private);

            writer.write(&K_FACTOR);
            writer.zero_bytes(K_FLOAT_TO_FLOAT4_PADDING);
            for i in 0..K_PROBLEM_SIZE {
                writer.write(&((i + 1) as f32));
            }
        }

        fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
            WorkgroupSize::new(1, 1, 1)
        }
    }

    let resources = [
        // Input buffer:
        ResourceDesc::with_sksl(
            // TODO(b/299979165): Declare this binding as read-only.
            ResourceType::StorageBuffer,
            DataFlow::Private,
            ResourcePolicy::Mapped,
            "inputBlock {\n    float factor;\n    layout(offset=16) float4 in_data[];\n}",
        ),
        // Output buffer:
        // shared to allow us to access it from the Builder
        ResourceDesc::with_slot_and_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Shared,
            // mappable for read-back
            ResourcePolicy::Mapped,
            0,
            "outputBlock { float4 out_data[]; }",
        ),
    ];
    let step: Arc<dyn ComputeStep> = Arc::new(TestComputeStep {
        base: ComputeStepBase::new(
            "TestArrayMultiply",
            WorkgroupSize::new(K_WORKGROUP_SIZE, 1, 1),
            &resources,
            &[],
            false,
        ),
    });

    let mut recorder = context.make_recorder(None);

    let mut builder = Builder::new(&recorder);
    if !builder.append_step(&step, None) {
        errorf!(reporter, "Failed to add ComputeStep to DispatchGroup");
        return;
    }

    // The output buffer should have been placed in the right output slot.
    let output_info = builder.get_shared_buffer_resource(0);
    if !output_info.is_valid() {
        errorf!(reporter, "Failed to allocate an output buffer at slot 0");
        return;
    }

    // Record the compute task
    let groups = vec![builder.finalize()];
    recorder.priv_().add(ComputeTask::make(groups));

    // Ensure the output buffer is synchronized to the CPU once the GPU submission has finished.
    let output_buffer = sync_buffer_to_cpu(&mut recorder, &output_info.ref_buffer());

    // Submit the work and wait for it to complete.
    let Some(_recording) = submit_recording(context, &mut recorder) else {
        errorf!(reporter, "Failed to make recording");
        return;
    };

    // Verify the contents of the output buffer.
    let out_bytes = map_buffer(context, &output_buffer, output_info.offset as usize);
    let out_data = read_f32s(&out_bytes);
    debug_assert!(output_buffer.is_mapped());
    for i in 0..K_PROBLEM_SIZE as usize {
        let expected = (i + 1) as f32 * K_FACTOR;
        let found = out_data[i];
        reporter_assert!(
            reporter,
            expected == found,
            "expected '{}', found '{}'",
            expected,
            found
        );
    }
});

// Port of: tests/graphite/ComputeTest.cpp#L251-L494 (chrome/m156)
def_graphite_adapter_test!(Compute_DispatchGroupTest, |reporter, context| {
    const K_PROBLEM_SIZE: u32 = 512;
    const K_FACTOR1: f32 = 4.0;
    const K_FACTOR2: f32 = 3.0;

    // The ComputeStep packs kProblemSize floats into kProblemSize / 4 vectors and each thread
    // processes 1 vector at a time.
    const K_WORKGROUP_SIZE: u32 = K_PROBLEM_SIZE / 4;

    // Define two steps that perform two multiplication passes over the same input.

    #[derive(Debug)]
    struct TestComputeStep1 {
        base: ComputeStepBase,
    }

    impl ComputeStep for TestComputeStep1 {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        // A kernel that multiplies a large array of floats by a supplied factor.
        fn compute_sksl(&self) -> String {
            r"
                void main() {
                    uint idx = sk_GlobalInvocationID.x;
                    forward_data[idx] = in_data[idx] * factor;
                    if (idx == 0) {
                        extra_data.x = factor;
                        extra_data.y = 2 * factor;
                    }
                }
            "
            .to_owned()
        }

        fn calculate_buffer_size(&self, index: usize, r: &ResourceDesc) -> usize {
            if index == 0 {
                debug_assert_eq!(r.flow, DataFlow::Private);
                return std::mem::size_of::<f32>() * (K_PROBLEM_SIZE as usize + 4);
            }
            if index == 1 {
                debug_assert_eq!(r.flow, DataFlow::Shared);
                debug_assert_eq!(r.slot, 0);
                return std::mem::size_of::<f32>() * K_PROBLEM_SIZE as usize;
            }

            debug_assert_eq!(index, 2);
            debug_assert_eq!(r.slot, 1);
            debug_assert_eq!(r.flow, DataFlow::Shared);
            2 * std::mem::size_of::<f32>()
        }

        fn prepare_storage_buffer(
            &self,
            resource_index: usize,
            _r: &ResourceDesc,
            mut writer: BufferWriter<'_>,
        ) {
            if resource_index != 0 {
                return;
            }

            writer.write(&K_FACTOR1);
            writer.zero_bytes(K_FLOAT_TO_FLOAT4_PADDING);
            for i in 0..K_PROBLEM_SIZE {
                writer.write(&((i + 1) as f32));
            }
        }

        fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
            WorkgroupSize::new(1, 1, 1)
        }
    }

    #[derive(Debug)]
    struct TestComputeStep2 {
        base: ComputeStepBase,
    }

    impl ComputeStep for TestComputeStep2 {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        // A kernel that multiplies a large array of floats by a supplied factor.
        fn compute_sksl(&self) -> String {
            r"
                void main() {
                    out_data[sk_GlobalInvocationID.x] = in_data[sk_GlobalInvocationID.x] * factor;
                }
            "
            .to_owned()
        }

        fn calculate_buffer_size(&self, index: usize, r: &ResourceDesc) -> usize {
            debug_assert_ne!(index, 0);
            if index == 1 {
                debug_assert_eq!(r.flow, DataFlow::Private);
                return std::mem::size_of::<f32>() * 4;
            }
            debug_assert_eq!(index, 2);
            debug_assert_eq!(r.slot, 2);
            debug_assert_eq!(r.flow, DataFlow::Shared);
            std::mem::size_of::<f32>() * K_PROBLEM_SIZE as usize
        }

        fn prepare_storage_buffer(
            &self,
            resource_index: usize,
            r: &ResourceDesc,
            mut writer: BufferWriter<'_>,
        ) {
            if resource_index != 1 {
                return;
            }
            debug_assert_eq!(r.flow, DataFlow::Private);
            writer.write(&K_FACTOR2);
        }

        fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
            WorkgroupSize::new(1, 1, 1)
        }
    }

    let resources1 = [
        // Input buffer:
        // TODO(b/299979165): Declare this binding as read-only.
        ResourceDesc::with_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Private,
            // mappable for read-back
            ResourcePolicy::Mapped,
            "inputBlock {\n    float factor;\n    layout(offset=16) float4 in_data[];\n}",
        ),
        // Output buffers:
        // GPU-only, read by second step
        ResourceDesc::with_slot_and_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Shared,
            ResourcePolicy::None,
            0,
            "outputBlock1 { float4 forward_data[]; }",
        ),
        // mappable for read-back
        ResourceDesc::with_slot_and_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Shared,
            ResourcePolicy::Mapped,
            1,
            "outputBlock2 { float2 extra_data; }",
        ),
    ];
    let resources2 = [
        // Input buffer:
        // this is the output from the first step
        ResourceDesc::with_slot_and_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Shared,
            // GPU-only
            ResourcePolicy::None,
            0,
            "inputBlock { float4 in_data[]; }",
        ),
        ResourceDesc::with_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Private,
            ResourcePolicy::Mapped,
            "factorBlock { float factor; }",
        ),
        // Output buffer:
        // mappable for read-back
        ResourceDesc::with_slot_and_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Shared,
            ResourcePolicy::Mapped,
            2,
            "outputBlock { float4 out_data[]; }",
        ),
    ];
    let step1: Arc<dyn ComputeStep> = Arc::new(TestComputeStep1 {
        base: ComputeStepBase::new(
            "TestArrayMultiplyFirstPass",
            WorkgroupSize::new(K_WORKGROUP_SIZE, 1, 1),
            &resources1,
            &[],
            false,
        ),
    });
    let step2: Arc<dyn ComputeStep> = Arc::new(TestComputeStep2 {
        base: ComputeStepBase::new(
            "TestArrayMultiplySecondPass",
            WorkgroupSize::new(K_WORKGROUP_SIZE, 1, 1),
            &resources2,
            &[],
            false,
        ),
    });

    let mut recorder = context.make_recorder(None);

    let mut builder = Builder::new(&recorder);
    let _ = builder.append_step(&step1, None);
    let _ = builder.append_step(&step2, None);

    // Slots 0, 1, and 2 should all contain shared buffers. Slot 1 contains the extra output buffer
    // from step 1 while slot 2 contains the result of the second multiplication pass from step 1.
    // Slot 0 is not mappable.
    reporter_assert!(
        reporter,
        matches!(
            builder.output_table().shared_slots[0],
            Some(BindingResource::Buffer(_))
        ),
        "shared resource at slot 0 is missing"
    );
    let output_info = builder.get_shared_buffer_resource(2);
    if !output_info.is_valid() {
        errorf!(reporter, "Failed to allocate an output buffer at slot 0");
        return;
    }

    // Extra output buffer from step 1 (corresponding to 'outputBlock2')
    let extra_output_info = builder.get_shared_buffer_resource(1);
    if !extra_output_info.is_valid() {
        errorf!(reporter, "shared resource at slot 1 is missing");
        return;
    }

    // Record the compute task
    let groups = vec![builder.finalize()];
    recorder.priv_().add(ComputeTask::make(groups));

    // Ensure the output buffers get synchronized to the CPU once the GPU submission has finished.
    let output_buffer = sync_buffer_to_cpu(&mut recorder, &output_info.ref_buffer());
    let extra_output_buffer = sync_buffer_to_cpu(&mut recorder, &extra_output_info.ref_buffer());

    // Submit the work and wait for it to complete.
    let Some(_recording) = submit_recording(context, &mut recorder) else {
        errorf!(reporter, "Failed to make recording");
        return;
    };

    // Verify the contents of the output buffer from step 2
    let out_bytes = map_buffer(context, &output_buffer, output_info.offset as usize);
    let out_data = read_f32s(&out_bytes);
    for i in 0..K_PROBLEM_SIZE as usize {
        let expected = (i + 1) as f32 * K_FACTOR1 * K_FACTOR2;
        let found = out_data[i];
        reporter_assert!(
            reporter,
            expected == found,
            "expected '{}', found '{}'",
            expected,
            found
        );
    }

    // Verify the contents of the extra output buffer from step 1
    let extra_bytes = map_buffer(
        context,
        &extra_output_buffer,
        extra_output_info.offset as usize,
    );
    let extra_out_data = read_f32s(&extra_bytes);
    reporter_assert!(
        reporter,
        K_FACTOR1 == extra_out_data[0],
        "expected '{}', found '{}'",
        K_FACTOR1,
        extra_out_data[0]
    );
    reporter_assert!(
        reporter,
        2.0 * K_FACTOR1 == extra_out_data[1],
        "expected '{}', found '{}'",
        2.0 * K_FACTOR2,
        extra_out_data[1]
    );
});

// Port of: tests/graphite/ComputeTest.cpp#L498-L643 (chrome/m156)
def_graphite_adapter_test!(Compute_UniformBufferTest, |reporter, context| {
    const K_PROBLEM_SIZE: u32 = 512;
    const K_FACTOR: f32 = 4.0;

    // The ComputeStep packs kProblemSize floats into kProblemSize / 4 vectors and each thread
    // processes 1 vector at a time.
    const K_WORKGROUP_SIZE: u32 = K_PROBLEM_SIZE / 4;

    #[derive(Debug)]
    struct TestComputeStep {
        base: ComputeStepBase,
    }

    impl ComputeStep for TestComputeStep {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        // A kernel that multiplies a large array of floats by a supplied factor.
        fn compute_sksl(&self) -> String {
            r"
                void main() {
                    out_data[sk_GlobalInvocationID.x] = in_data[sk_GlobalInvocationID.x] * factor;
                }
            "
            .to_owned()
        }

        fn calculate_buffer_size(&self, index: usize, r: &ResourceDesc) -> usize {
            if index == 0 {
                debug_assert_eq!(r.flow, DataFlow::Private);
                return std::mem::size_of::<f32>();
            }
            if index == 1 {
                debug_assert_eq!(r.flow, DataFlow::Private);
                return std::mem::size_of::<f32>() * K_PROBLEM_SIZE as usize;
            }

            debug_assert_eq!(index, 2);
            debug_assert_eq!(r.slot, 0);
            debug_assert_eq!(r.flow, DataFlow::Shared);
            std::mem::size_of::<f32>() * K_PROBLEM_SIZE as usize
        }

        fn prepare_storage_buffer(
            &self,
            resource_index: usize,
            r: &ResourceDesc,
            mut writer: BufferWriter<'_>,
        ) {
            // Only initialize the input storage buffer.
            if resource_index != 1 {
                return;
            }
            debug_assert_eq!(r.flow, DataFlow::Private);
            for i in 0..K_PROBLEM_SIZE {
                writer.write(&((i + 1) as f32));
            }
        }

        fn prepare_uniform_buffer(
            &self,
            resource_index: usize,
            _r: &ResourceDesc,
            mgr: &mut UniformManager,
        ) {
            debug_assert_eq!(resource_index, 0);
            #[cfg(debug_assertions)]
            {
                let uniforms = [Uniform::new_owned(
                    "factor".to_owned(),
                    SkSLType::Float,
                    K_NON_ARRAY,
                )];
                mgr.set_expected_uniforms(&uniforms, /*is_substruct=*/ false);
            }
            mgr.write_f32(K_FACTOR);
        }

        fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
            WorkgroupSize::new(1, 1, 1)
        }
    }

    let resources = [
        // Uniform buffer:
        ResourceDesc::with_sksl(
            ResourceType::UniformBuffer,
            DataFlow::Private,
            ResourcePolicy::Mapped,
            "uniformBlock { float factor; }",
        ),
        // Input buffer:
        ResourceDesc::with_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Private,
            ResourcePolicy::Mapped,
            "inputBlock { float4 in_data[]; }",
        ),
        // Output buffer:
        // shared to allow us to access it from the Builder
        // mappable for read-back
        ResourceDesc::with_slot_and_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Shared,
            ResourcePolicy::Mapped,
            0,
            "outputBlock { float4 out_data[]; }",
        ),
    ];
    let step: Arc<dyn ComputeStep> = Arc::new(TestComputeStep {
        base: ComputeStepBase::new(
            "TestArrayMultiply",
            WorkgroupSize::new(K_WORKGROUP_SIZE, 1, 1),
            &resources,
            &[],
            false,
        ),
    });

    let mut recorder = context.make_recorder(None);

    let mut builder = Builder::new(&recorder);
    if !builder.append_step(&step, None) {
        errorf!(reporter, "Failed to add ComputeStep to DispatchGroup");
        return;
    }

    // The output buffer should have been placed in the right output slot.
    let output_info = builder.get_shared_buffer_resource(0);
    if !output_info.is_valid() {
        errorf!(reporter, "Failed to allocate an output buffer at slot 0");
        return;
    }

    // Record the compute task
    let groups = vec![builder.finalize()];
    recorder.priv_().add(ComputeTask::make(groups));

    // Ensure the output buffer is synchronized to the CPU once the GPU submission has finished.
    let output_buffer = sync_buffer_to_cpu(&mut recorder, &output_info.ref_buffer());

    // Submit the work and wait for it to complete.
    let Some(_recording) = submit_recording(context, &mut recorder) else {
        errorf!(reporter, "Failed to make recording");
        return;
    };

    // Verify the contents of the output buffer.
    let out_bytes = map_buffer(context, &output_buffer, output_info.offset as usize);
    let out_data = read_f32s(&out_bytes);
    for i in 0..K_PROBLEM_SIZE as usize {
        let expected = (i + 1) as f32 * K_FACTOR;
        let found = out_data[i];
        reporter_assert!(
            reporter,
            expected == found,
            "expected '{}', found '{}'",
            expected,
            found
        );
    }
});

// Port of: tests/graphite/ComputeTest.cpp#L647-L762 (chrome/m156)
def_graphite_adapter_test!(Compute_ExternallyAssignedBuffer, |reporter, context| {
    const K_PROBLEM_SIZE: u32 = 512;
    const K_FACTOR: f32 = 4.0;

    // The ComputeStep packs kProblemSize floats into kProblemSize / 4 vectors and each thread
    // processes 1 vector at a time.
    const K_WORKGROUP_SIZE: u32 = K_PROBLEM_SIZE / 4;

    #[derive(Debug)]
    struct TestComputeStep {
        base: ComputeStepBase,
    }

    impl ComputeStep for TestComputeStep {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        // A kernel that multiplies a large array of floats by a supplied factor.
        fn compute_sksl(&self) -> String {
            r"
                void main() {
                    out_data[sk_GlobalInvocationID.x] = in_data[sk_GlobalInvocationID.x] * factor;
                }
            "
            .to_owned()
        }

        fn calculate_buffer_size(&self, resource_index: usize, r: &ResourceDesc) -> usize {
            debug_assert_eq!(resource_index, 0);
            debug_assert_eq!(r.flow, DataFlow::Private);
            std::mem::size_of::<f32>() * (K_PROBLEM_SIZE as usize + 4)
        }

        fn prepare_storage_buffer(
            &self,
            resource_index: usize,
            r: &ResourceDesc,
            mut writer: BufferWriter<'_>,
        ) {
            debug_assert_eq!(resource_index, 0);
            debug_assert_eq!(r.flow, DataFlow::Private);
            writer.write(&K_FACTOR);
            writer.zero_bytes(K_FLOAT_TO_FLOAT4_PADDING);
            for i in 0..K_PROBLEM_SIZE {
                writer.write(&((i + 1) as f32));
            }
        }
    }

    let resources = [
        // Input buffer:
        ResourceDesc::with_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Private,
            ResourcePolicy::Mapped,
            "inputBlock {\n    float factor;\n    layout(offset = 16) float4 in_data[];\n}\n",
        ),
        // Output buffer:
        // shared to allow us to access it from the Builder
        // mappable for read-back
        ResourceDesc::with_slot_and_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Shared,
            ResourcePolicy::Mapped,
            0,
            "outputBlock { float4 out_data[]; }",
        ),
    ];
    let step: Arc<dyn ComputeStep> = Arc::new(TestComputeStep {
        base: ComputeStepBase::new(
            "ExternallyAssignedBuffer",
            WorkgroupSize::new(K_WORKGROUP_SIZE, 1, 1),
            &resources,
            &[],
            false,
        ),
    });

    let mut recorder = context.make_recorder(None);

    // We allocate a buffer and directly assign it to the DispatchGroup::Builder. The ComputeStep
    // will not participate in the creation of this buffer.
    let mapped = recorder
        .priv_()
        .draw_buffer_manager()
        .get_mapped_storage_buffer(K_PROBLEM_SIZE as usize, std::mem::size_of::<f32>())
        .expect("a mapped output buffer");
    let output_info = mapped.binding.clone();
    reporter_assert!(
        reporter,
        output_info.is_valid(),
        "Failed to allocate output buffer"
    );
    drop(mapped);

    let mut builder = Builder::new(&recorder);
    builder.assign_shared_buffer(output_info.clone(), 0, ClearBuffer::No);

    // Initialize the step with a pre-determined global size
    if !builder.append_step(&step, Some(WorkgroupSize::new(1, 1, 1))) {
        errorf!(reporter, "Failed to add ComputeStep to DispatchGroup");
        return;
    }

    // Record the compute task
    let groups = vec![builder.finalize()];
    recorder.priv_().add(ComputeTask::make(groups));

    // Ensure the output buffer is synchronized to the CPU once the GPU submission has finished.
    let output_buffer = sync_buffer_to_cpu(&mut recorder, &output_info.ref_buffer());

    // Submit the work and wait for it to complete.
    let Some(_recording) = submit_recording(context, &mut recorder) else {
        errorf!(reporter, "Failed to make recording");
        return;
    };

    // Verify the contents of the output buffer.
    let out_bytes = map_buffer(context, &output_buffer, output_info.offset as usize);
    let out_data = read_f32s(&out_bytes);
    for i in 0..K_PROBLEM_SIZE as usize {
        let expected = (i + 1) as f32 * K_FACTOR;
        let found = out_data[i];
        reporter_assert!(
            reporter,
            expected == found,
            "expected '{}', found '{}'",
            expected,
            found
        );
    }
});

// Tests the storage texture binding for a compute dispatch that writes the same color to every
// pixel of a storage texture.
// Port of: tests/graphite/ComputeTest.cpp#L766-L867 (chrome/m156)
def_graphite_adapter_test!(Compute_StorageTexture, |reporter, context| {
    // For this test we allocate a 8x8 tile which is written to by a single workgroup of the same
    // size.
    const K_DIM: u32 = 8;

    #[derive(Debug)]
    struct TestComputeStep {
        base: ComputeStepBase,
    }

    impl ComputeStep for TestComputeStep {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        fn compute_sksl(&self) -> String {
            r"
                void main() {
                    textureWrite(dst, sk_LocalInvocationID.xy, half4(0.0, 1.0, 0.0, 1.0));
                }
            "
            .to_owned()
        }

        fn calculate_texture_parameters(
            &self,
            _index: usize,
            _r: &ResourceDesc,
        ) -> (ISize, ColorType) {
            (ISize::new(K_DIM as i32, K_DIM as i32), ColorType::RGBA8888)
        }

        fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
            WorkgroupSize::new(1, 1, 1)
        }
    }

    let resources = [ResourceDesc::with_slot_and_sksl(
        ResourceType::WriteOnlyStorageTexture,
        DataFlow::Shared,
        ResourcePolicy::None,
        0,
        "dst",
    )];
    let step: Arc<dyn ComputeStep> = Arc::new(TestComputeStep {
        base: ComputeStepBase::new(
            "TestStorageTexture",
            WorkgroupSize::new(K_DIM, K_DIM, 1),
            &resources,
            &[],
            false,
        ),
    });

    let mut recorder = context.make_recorder(None);

    let mut builder = Builder::new(&recorder);
    if !builder.append_step(&step, None) {
        errorf!(reporter, "Failed to add ComputeStep to DispatchGroup");
        return;
    }

    let Some(texture) = builder.get_shared_texture_resource(0) else {
        errorf!(reporter, "Shared resource at slot 0 is missing");
        return;
    };

    // Record the compute task
    let groups = vec![builder.finalize()];
    recorder.priv_().add(ComputeTask::make(groups));

    // Submit the work and wait for it to complete.
    let Some(_recording) = submit_recording(context, &mut recorder) else {
        errorf!(reporter, "Failed to make recording");
        return;
    };

    let image_info = ImageInfo::new(
        (K_DIM as i32, K_DIM as i32),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(&image_info, None);
    let mut pixels = bitmap.peek_pixels_mut().expect("the bitmap has pixels");
    let read_pixels_success = context.read_pixels_into(
        &mut pixels,
        &TextureProxyView::new(Some(texture), Swizzle::rgba()),
        &image_info,
        0,
        0,
    );
    reporter_assert!(reporter, read_pixels_success);

    for x in 0..K_DIM {
        for y in 0..K_DIM {
            let expected = color4f_from_color(0xFF00_FF00);
            let color = pixels.get_color_4f((x as i32, y as i32));
            reporter_assert!(
                reporter,
                expected == color,
                "At position {{{}, {}}}, expected {{{:.1}, {:.1}, {:.1}, {:.1}}}, found {{{:.1}, {:.1}, {:.1}, {:.1}}}",
                x,
                y,
                expected.r,
                expected.g,
                expected.b,
                expected.a,
                color.r,
                color.g,
                color.b,
                color.a
            );
        }
    }
});

// Tests the readonly texture binding for a compute dispatch that random-access reads from a
// CPU-populated texture and copies it to a storage texture.
// Port of: tests/graphite/ComputeTest.cpp#L871-L1034 (chrome/m156)
def_graphite_adapter_test!(Compute_StorageTextureReadAndWrite, |reporter, context| {
    // For this test we allocate a 8x8 tile which is written to by a single workgroup of the same
    // size.
    const K_DIM: u32 = 8;

    #[derive(Debug)]
    struct TestComputeStep {
        base: ComputeStepBase,
    }

    impl ComputeStep for TestComputeStep {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        fn compute_sksl(&self) -> String {
            r"
                void main() {
                    half4 color = textureRead(src, sk_LocalInvocationID.xy);
                    textureWrite(dst, sk_LocalInvocationID.xy, color);
                }
            "
            .to_owned()
        }

        fn calculate_texture_parameters(
            &self,
            index: usize,
            _r: &ResourceDesc,
        ) -> (ISize, ColorType) {
            debug_assert_eq!(index, 1);
            (ISize::new(K_DIM as i32, K_DIM as i32), ColorType::RGBA8888)
        }

        fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
            WorkgroupSize::new(1, 1, 1)
        }
    }

    let resources = [
        ResourceDesc::with_slot_and_sksl(
            ResourceType::ReadOnlyTexture,
            DataFlow::Shared,
            ResourcePolicy::None,
            0,
            "src",
        ),
        ResourceDesc::with_slot_and_sksl(
            ResourceType::WriteOnlyStorageTexture,
            DataFlow::Shared,
            ResourcePolicy::None,
            1,
            "dst",
        ),
    ];
    let step: Arc<dyn ComputeStep> = Arc::new(TestComputeStep {
        base: ComputeStepBase::new(
            "TestStorageTextureReadAndWrite",
            WorkgroupSize::new(K_DIM, K_DIM, 1),
            &resources,
            &[],
            false,
        ),
    });

    let mut recorder = context.make_recorder(None);

    // Create and populate an input texture.
    let src_info = ImageInfo::new(
        (K_DIM as i32, K_DIM as i32),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    let mut src_bitmap = Bitmap::new();
    src_bitmap.alloc_pixels_info(&src_info, None);
    let mut src_pixels = src_bitmap.peek_pixels_mut().expect("the bitmap has pixels");
    for x in 0..K_DIM {
        for y in 0..K_DIM {
            let color = color_set_argb(255, x * 256 / K_DIM, y * 256 / K_DIM, 0);
            write_addr32(&mut src_pixels, x, y, color);
        }
    }

    let caps = recorder.priv_().caps().clone();
    let tex_info = caps.get_default_sampled_texture_info(
        ColorType::RGBA8888,
        Mipmapped::No,
        Protected::No,
        Renderable::No,
    );
    let src_proxy = {
        let shared = recorder.priv_().resource_provider().clone();
        let mut rp = shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        TextureProxy::make(
            caps.as_ref(),
            &mut rp,
            ISize::new(K_DIM as i32, K_DIM as i32),
            &tex_info,
            Budgeted::No,
            "ComputeTestSrcProxy",
        )
        .expect("a source texture proxy")
    };
    let mip_level = MipLevel {
        pixels: src_pixels.bytes(),
        row_bytes: src_pixels.row_bytes(),
    };
    let upload_source = UploadSource::make(
        caps.as_ref(),
        &TextureProxyView::new(Some(src_proxy.clone()), Swizzle::rgba()),
        src_pixels.info().color_info(),
        src_pixels.info().color_info(),
        &[mip_level],
        IRect::from_wh(K_DIM as i32, K_DIM as i32),
    );
    if !upload_source.is_valid() {
        errorf!(reporter, "Could not create UploadSource");
        return;
    }
    let upload = {
        let upload_buffer_manager = recorder.priv_().upload_buffer_manager().clone();
        let mut upload_buffer_manager = upload_buffer_manager.borrow_mut();
        UploadInstance::make(
            caps.as_ref(),
            &mut upload_buffer_manager,
            &upload_source,
            Some(Box::new(ImageUploadContext)),
        )
    };
    if !upload.is_valid() {
        errorf!(reporter, "Could not create UploadInstance");
        return;
    }
    recorder
        .priv_()
        .add(UploadTask::make_instance(upload).expect("an upload task"));

    let mut builder = Builder::new(&recorder);
    // Assign the input texture to slot 0. This corresponds to the ComputeStep's "src" texture
    // binding.
    builder.assign_shared_texture(src_proxy, 0);
    if !builder.append_step(&step, None) {
        errorf!(reporter, "Failed to add ComputeStep to DispatchGroup");
        return;
    }

    let Some(dst) = builder.get_shared_texture_resource(1) else {
        errorf!(reporter, "shared resource at slot 1 is missing");
        return;
    };

    // Record the compute task
    let groups = vec![builder.finalize()];
    recorder.priv_().add(ComputeTask::make(groups));

    // Submit the work and wait for it to complete.
    let Some(_recording) = submit_recording(context, &mut recorder) else {
        errorf!(reporter, "Failed to make recording");
        return;
    };

    let image_info = ImageInfo::new(
        (K_DIM as i32, K_DIM as i32),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(&image_info, None);
    let mut pixels = bitmap.peek_pixels_mut().expect("the bitmap has pixels");
    let read_pixels_success = context.read_pixels_into(
        &mut pixels,
        &TextureProxyView::new(Some(dst), Swizzle::rgba()),
        &image_info,
        0,
        0,
    );
    reporter_assert!(reporter, read_pixels_success);

    for x in 0..K_DIM {
        for y in 0..K_DIM {
            let expected =
                color4f_from_bytes_rgba(color_set_argb(255, x * 256 / K_DIM, y * 256 / K_DIM, 0));
            let color = pixels.get_color_4f((x as i32, y as i32));
            reporter_assert!(
                reporter,
                expected == color,
                "At position {{{}, {}}}, expected {{{:.1}, {:.1}, {:.1}, {:.1}}}, found {{{:.1}, {:.1}, {:.1}, {:.1}}}",
                x,
                y,
                expected.r,
                expected.g,
                expected.b,
                expected.a,
                color.r,
                color.g,
                color.b,
                color.a
            );
        }
    }
});

/// `SkColorSetARGB(a, r, g, b)`.
// Port of: include/core/SkColor.h (chrome/m156)
fn color_set_argb(a: u32, r: u32, g: u32, b: u32) -> u32 {
    (a << 24) | (r << 16) | (g << 8) | b
}

/// `SkColor4f::FromBytes_RGBA(c)`: the bytes of `c` read as R, G, B, A.
// Port of: include/core/SkColor.h (chrome/m156)
fn color4f_from_bytes_rgba(c: u32) -> Color4f {
    let channel = |shift: u32| ((c >> shift) & 0xFF) as f32 * (1.0 / 255.0);
    Color4f::new(channel(0), channel(8), channel(16), channel(24))
}

// Port of: tests/graphite/ComputeTest.cpp#L1036-L1176 (chrome/m156)
def_graphite_adapter_test!(Compute_ReadOnlyStorageBuffer, |reporter, context| {
    const K_DIM: u32 = 8;

    #[derive(Debug)]
    struct TestComputeStep {
        base: ComputeStepBase,
    }

    impl ComputeStep for TestComputeStep {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        fn compute_sksl(&self) -> String {
            r"
                void main() {
                    uint ix = sk_LocalInvocationID.y * 8 + sk_LocalInvocationID.x;
                    uint value = in_data[ix];
                    half4 splat = half4(
                        half(value & 0xFF),
                        half((value >> 8) & 0xFF),
                        half((value >> 16) & 0xFF),
                        half((value >> 24) & 0xFF)
                    );
                    textureWrite(dst, sk_LocalInvocationID.xy, splat / 255.0);
                }
            "
            .to_owned()
        }

        fn calculate_buffer_size(&self, index: usize, _r: &ResourceDesc) -> usize {
            debug_assert_eq!(index, 0);
            K_DIM as usize * K_DIM as usize * std::mem::size_of::<u32>()
        }

        fn prepare_storage_buffer(
            &self,
            index: usize,
            _r: &ResourceDesc,
            mut writer: BufferWriter<'_>,
        ) {
            debug_assert_eq!(index, 0);
            for y in 0..K_DIM {
                for x in 0..K_DIM {
                    let value: u32 = ((x * 256 / K_DIM) & 0xFF)
                        | (((y * 256 / K_DIM) & 0xFF) << 8)
                        | (255 << 24);
                    writer.write(&value);
                }
            }
        }

        fn calculate_texture_parameters(
            &self,
            index: usize,
            _r: &ResourceDesc,
        ) -> (ISize, ColorType) {
            debug_assert_eq!(index, 1);
            (ISize::new(K_DIM as i32, K_DIM as i32), ColorType::RGBA8888)
        }

        fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
            WorkgroupSize::new(1, 1, 1)
        }
    }

    let resources = [
        ResourceDesc::with_slot_and_sksl(
            ResourceType::ReadOnlyStorageBuffer,
            DataFlow::Shared,
            ResourcePolicy::Mapped,
            0,
            "src { uint in_data[]; }",
        ),
        ResourceDesc::with_slot_and_sksl(
            ResourceType::WriteOnlyStorageTexture,
            DataFlow::Shared,
            ResourcePolicy::None,
            1,
            "dst",
        ),
    ];
    let step: Arc<dyn ComputeStep> = Arc::new(TestComputeStep {
        base: ComputeStepBase::new(
            "TestReadOnlyStorageBuffer",
            WorkgroupSize::new(K_DIM, K_DIM, 1),
            &resources,
            &[],
            false,
        ),
    });

    let mut recorder = context.make_recorder(None);

    let mut builder = Builder::new(&recorder);
    if !builder.append_step(&step, None) {
        errorf!(reporter, "Failed to add ComputeStep to DispatchGroup");
        return;
    }

    let Some(dst) = builder.get_shared_texture_resource(1) else {
        errorf!(reporter, "shared resource at slot 1 is missing");
        return;
    };

    let groups = vec![builder.finalize()];
    recorder.priv_().add(ComputeTask::make(groups));

    let Some(_recording) = submit_recording(context, &mut recorder) else {
        errorf!(reporter, "Failed to make recording");
        return;
    };

    let image_info = ImageInfo::new(
        (K_DIM as i32, K_DIM as i32),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(&image_info, None);
    let mut pixels = bitmap.peek_pixels_mut().expect("the bitmap has pixels");
    let read_pixels_success = context.read_pixels_into(
        &mut pixels,
        &TextureProxyView::new(Some(dst), Swizzle::rgba()),
        &image_info,
        0,
        0,
    );
    reporter_assert!(reporter, read_pixels_success);

    for x in 0..K_DIM {
        for y in 0..K_DIM {
            let expected =
                color4f_from_color(color_set_argb(255, x * 256 / K_DIM, y * 256 / K_DIM, 0));
            let color = pixels.get_color_4f((x as i32, y as i32));
            let pass = [color.r, color.g, color.b, color.a]
                == [expected.r, expected.g, expected.b, expected.a];
            reporter_assert!(
                reporter,
                pass,
                "At position {{{}, {}}}, expected {{{:.1}, {:.1}, {:.1}, {:.1}}}, found {{{:.1}, {:.1}, {:.1}, {:.1}}}",
                x,
                y,
                expected.r,
                expected.g,
                expected.b,
                expected.a,
                color.r,
                color.g,
                color.b,
                color.a
            );
        }
    }
});

/// `Pixmap::writable_addr32(x, y) = value`: writes the native-endian `u32` at `(x, y)`.
// Port of: include/core/SkPixmap.h (chrome/m156)
fn write_addr32(pixels: &mut Pixmap<'_>, x: u32, y: u32, value: u32) {
    let row_bytes = pixels.row_bytes();
    let offset = y as usize * row_bytes + x as usize * 4;
    let bytes = pixels.writable_addr().expect("the pixmap is writable");
    bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
}

/// `SkColor4f::FromColor(c)`: the ARGB components of `c`, as floats in [0, 1].
// Port of: include/core/SkColor.h (chrome/m156)
fn color4f_from_color(c: u32) -> Color4f {
    let channel = |shift: u32| ((c >> shift) & 0xFF) as f32 * (1.0 / 255.0);
    Color4f::new(channel(16), channel(8), channel(0), channel(24))
}

// Port of: tests/graphite/ComputeTest.cpp#L1179-L1324 (chrome/m156)
def_graphite_adapter_test!(
    Compute_StorageTextureMultipleComputeSteps,
    |reporter, context| {
        const K_DIM: u32 = 8;

        #[derive(Debug)]
        struct TestComputeStep1 {
            base: ComputeStepBase,
        }

        impl ComputeStep for TestComputeStep1 {
            fn base(&self) -> &ComputeStepBase {
                &self.base
            }

            fn compute_sksl(&self) -> String {
                r"
                void main() {
                    textureWrite(dst, sk_LocalInvocationID.xy, half4(0.0, 1.0, 0.0, 1.0));
                }
            "
                .to_owned()
            }

            fn calculate_texture_parameters(
                &self,
                index: usize,
                _r: &ResourceDesc,
            ) -> (ISize, ColorType) {
                debug_assert_eq!(index, 0);
                (ISize::new(K_DIM as i32, K_DIM as i32), ColorType::RGBA8888)
            }

            fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
                WorkgroupSize::new(1, 1, 1)
            }
        }

        #[derive(Debug)]
        struct TestComputeStep2 {
            base: ComputeStepBase,
        }

        impl ComputeStep for TestComputeStep2 {
            fn base(&self) -> &ComputeStepBase {
                &self.base
            }

            fn compute_sksl(&self) -> String {
                r"
                void main() {
                    half4 color = textureRead(src, sk_LocalInvocationID.xy);
                    textureWrite(dst, sk_LocalInvocationID.xy, color);
                }
            "
                .to_owned()
            }

            fn calculate_texture_parameters(
                &self,
                index: usize,
                _r: &ResourceDesc,
            ) -> (ISize, ColorType) {
                debug_assert_eq!(index, 1);
                (ISize::new(K_DIM as i32, K_DIM as i32), ColorType::RGBA8888)
            }

            fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
                WorkgroupSize::new(1, 1, 1)
            }
        }

        let resources1 = [ResourceDesc::with_slot_and_sksl(
            ResourceType::WriteOnlyStorageTexture,
            DataFlow::Shared,
            ResourcePolicy::None,
            0,
            "dst",
        )];
        let resources2 = [
            ResourceDesc::with_slot_and_sksl(
                ResourceType::ReadOnlyTexture,
                DataFlow::Shared,
                ResourcePolicy::None,
                0,
                "src",
            ),
            ResourceDesc::with_slot_and_sksl(
                ResourceType::WriteOnlyStorageTexture,
                DataFlow::Shared,
                ResourcePolicy::None,
                1,
                "dst",
            ),
        ];
        let step1: Arc<dyn ComputeStep> = Arc::new(TestComputeStep1 {
            base: ComputeStepBase::new(
                "TestStorageTexturesFirstPass",
                WorkgroupSize::new(K_DIM, K_DIM, 1),
                &resources1,
                &[],
                false,
            ),
        });
        let step2: Arc<dyn ComputeStep> = Arc::new(TestComputeStep2 {
            base: ComputeStepBase::new(
                "TestStorageTexturesSecondPass",
                WorkgroupSize::new(K_DIM, K_DIM, 1),
                &resources2,
                &[],
                false,
            ),
        });

        let mut recorder = context.make_recorder(None);

        let mut builder = Builder::new(&recorder);
        let _ = builder.append_step(&step1, None);
        let _ = builder.append_step(&step2, None);

        let Some(dst) = builder.get_shared_texture_resource(1) else {
            errorf!(reporter, "shared resource at slot 1 is missing");
            return;
        };

        let groups = vec![builder.finalize()];
        recorder.priv_().add(ComputeTask::make(groups));

        let Some(_recording) = submit_recording(context, &mut recorder) else {
            errorf!(reporter, "Failed to make recording");
            return;
        };

        let image_info = ImageInfo::new(
            (K_DIM as i32, K_DIM as i32),
            ColorType::RGBA8888,
            AlphaType::Unpremul,
            None,
        );
        let mut bitmap = Bitmap::new();
        bitmap.alloc_pixels_info(&image_info, None);
        let mut pixels = bitmap.peek_pixels_mut().expect("the bitmap has pixels");
        let read_pixels_success = context.read_pixels_into(
            &mut pixels,
            &TextureProxyView::new(Some(dst), Swizzle::rgba()),
            &image_info,
            0,
            0,
        );
        reporter_assert!(reporter, read_pixels_success);

        for x in 0..K_DIM {
            for y in 0..K_DIM {
                let expected = color4f_from_color(0xFF00_FF00);
                let color = pixels.get_color_4f((x as i32, y as i32));
                reporter_assert!(
                    reporter,
                    expected == color,
                    "At position {{{}, {}}}, expected {{{:.1}, {:.1}, {:.1}, {:.1}}}, found {{{:.1}, {:.1}, {:.1}, {:.1}}}",
                    x,
                    y,
                    expected.r,
                    expected.g,
                    expected.b,
                    expected.a,
                    color.r,
                    color.g,
                    color.b,
                    color.a
                );
            }
        }
    }
);

// Port of: tests/graphite/ComputeTest.cpp#L1329-L1491 (chrome/m156)
def_graphite_adapter_test!(Compute_SampledTexture, |reporter, context| {
    const K_SRC_DIM: u32 = 8;
    const K_DST_DIM: u32 = 4;

    #[derive(Debug)]
    struct TestComputeStep1 {
        base: ComputeStepBase,
    }

    impl ComputeStep for TestComputeStep1 {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        fn compute_sksl(&self) -> String {
            r"
                void main() {
                    uint2 c = sk_LocalInvocationID.xy;
                    uint checkerBoardColor = (c.x + (c.y % 2)) % 2;
                    textureWrite(dst, c, half4(checkerBoardColor, 0, 0, 1));
                }
            "
            .to_owned()
        }

        fn calculate_texture_parameters(
            &self,
            index: usize,
            _r: &ResourceDesc,
        ) -> (ISize, ColorType) {
            debug_assert_eq!(index, 0);
            (
                ISize::new(K_SRC_DIM as i32, K_SRC_DIM as i32),
                ColorType::RGBA8888,
            )
        }

        fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
            WorkgroupSize::new(1, 1, 1)
        }
    }

    #[derive(Debug)]
    struct TestComputeStep2 {
        base: ComputeStepBase,
    }

    impl ComputeStep for TestComputeStep2 {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        fn compute_sksl(&self) -> String {
            r"
                void main() {
                    uint2 dstCoord = sk_LocalInvocationID.xy;
                    const float2 dstSizeInv = float2(0.25, 0.25);
                    float2 unormCoord = float2(dstCoord) * dstSizeInv;
                    half4 color = sampleLod(src, unormCoord, 0);
                    textureWrite(dst, dstCoord, color);
                }
            "
            .to_owned()
        }

        fn calculate_texture_parameters(
            &self,
            index: usize,
            _r: &ResourceDesc,
        ) -> (ISize, ColorType) {
            debug_assert!(index == 0 || index == 1);
            (
                ISize::new(K_DST_DIM as i32, K_DST_DIM as i32),
                ColorType::RGBA8888,
            )
        }

        fn calculate_sampler_parameters(&self, index: usize, _r: &ResourceDesc) -> SamplerDesc {
            debug_assert_eq!(index, 1);
            SamplerDesc::new(&SamplingOptions::from(FilterMode::Linear), TileMode::Repeat)
        }

        fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
            WorkgroupSize::new(1, 1, 1)
        }
    }

    let resources1 = [ResourceDesc::with_slot_and_sksl(
        ResourceType::WriteOnlyStorageTexture,
        DataFlow::Shared,
        ResourcePolicy::None,
        0,
        "dst",
    )];
    let resources2 = [
        ResourceDesc::with_slot_and_sksl(
            ResourceType::WriteOnlyStorageTexture,
            DataFlow::Shared,
            ResourcePolicy::None,
            1,
            "dst",
        ),
        ResourceDesc::with_slot_and_sksl(
            ResourceType::SampledTexture,
            DataFlow::Shared,
            ResourcePolicy::None,
            0,
            "src",
        ),
    ];
    let step1: Arc<dyn ComputeStep> = Arc::new(TestComputeStep1 {
        base: ComputeStepBase::new(
            "Test_SampledTexture_Init",
            WorkgroupSize::new(K_SRC_DIM, K_SRC_DIM, 1),
            &resources1,
            &[],
            false,
        ),
    });
    let step2: Arc<dyn ComputeStep> = Arc::new(TestComputeStep2 {
        base: ComputeStepBase::new(
            "Test_SampledTexture_Sample",
            WorkgroupSize::new(K_DST_DIM, K_DST_DIM, 1),
            &resources2,
            &[],
            false,
        ),
    });

    let mut recorder = context.make_recorder(None);

    let mut builder = Builder::new(&recorder);
    let _ = builder.append_step(&step1, None);
    let _ = builder.append_step(&step2, None);

    let Some(dst) = builder.get_shared_texture_resource(1) else {
        errorf!(reporter, "shared resource at slot 1 is missing");
        return;
    };

    let groups = vec![builder.finalize()];
    recorder.priv_().add(ComputeTask::make(groups));

    let Some(_recording) = submit_recording(context, &mut recorder) else {
        errorf!(reporter, "Failed to make recording");
        return;
    };

    let image_info = ImageInfo::new(
        (K_DST_DIM as i32, K_DST_DIM as i32),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(&image_info, None);
    let mut pixels = bitmap.peek_pixels_mut().expect("the bitmap has pixels");
    let read_pixels_success = context.read_pixels_into(
        &mut pixels,
        &TextureProxyView::new(Some(dst), Swizzle::rgba()),
        &image_info,
        0,
        0,
    );
    reporter_assert!(reporter, read_pixels_success);

    for x in 0..K_DST_DIM {
        for y in 0..K_DST_DIM {
            let color = pixels.get_color_4f((x as i32, y as i32));
            reporter_assert!(
                reporter,
                color.r > 0.49 && color.r < 0.51,
                "At position {{{}, {}}}, expected red channel in range [0.49, 0.51], found {{{:.3}}}",
                x,
                y,
                color.r
            );
        }
    }
});

// Port of: tests/graphite/ComputeTest.cpp#L1498-L1628 (chrome/m156)
// The C++ test returns early on Dawn D3D11 (b/315834710); the wgpu backend has no such context.
def_graphite_adapter_test!(Compute_AtomicOperationsTest, |reporter, context| {
    const K_WORKGROUP_COUNT: u32 = 32;
    const K_WORKGROUP_SIZE: u32 = 128;

    #[derive(Debug)]
    struct TestComputeStep {
        base: ComputeStepBase,
    }

    impl ComputeStep for TestComputeStep {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        fn compute_sksl(&self) -> String {
            r"
                workgroup atomicUint localCounter;
                workgroup atomicUint minCounter;
                workgroup atomicUint maxCounter;
                void main() {
                    if (sk_LocalInvocationID.x == 0) {
                        atomicStore(localCounter, 0);
                        atomicStore(minCounter, 100u);
                        atomicStore(maxCounter, 100u);
                    }
                    workgroupBarrier();
                    atomicAdd(localCounter, 1);
                    atomicMin(minCounter, 50u);
                    atomicMax(maxCounter, 50u);
                    workgroupBarrier();
                    if (sk_LocalInvocationID.x == 0) {
                        if (atomicLoad(minCounter) == 50u && atomicLoad(maxCounter) == 100u) {
                            atomicAdd(globalCounter, atomicLoad(localCounter));
                        }
                    }
                }
            "
            .to_owned()
        }

        fn calculate_buffer_size(&self, index: usize, r: &ResourceDesc) -> usize {
            debug_assert_eq!(index, 0);
            debug_assert_eq!(r.slot, 0);
            debug_assert_eq!(r.flow, DataFlow::Shared);
            std::mem::size_of::<u32>()
        }

        fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
            WorkgroupSize::new(K_WORKGROUP_COUNT, 1, 1)
        }

        fn prepare_storage_buffer(
            &self,
            resource_index: usize,
            _r: &ResourceDesc,
            mut writer: BufferWriter<'_>,
        ) {
            debug_assert_eq!(resource_index, 0);
            writer.zero_bytes(std::mem::size_of::<u32>());
        }
    }

    let resources = [ResourceDesc::with_slot_and_sksl(
        ResourceType::StorageBuffer,
        DataFlow::Shared,
        ResourcePolicy::Mapped,
        0,
        "ssbo { atomicUint globalCounter; }",
    )];
    let step: Arc<dyn ComputeStep> = Arc::new(TestComputeStep {
        base: ComputeStepBase::new(
            "TestAtomicOperations",
            WorkgroupSize::new(K_WORKGROUP_SIZE, 1, 1),
            &resources,
            &[],
            false,
        ),
    });

    let mut recorder = context.make_recorder(None);

    let mut builder = Builder::new(&recorder);
    let _ = builder.append_step(&step, None);
    let info = builder.get_shared_buffer_resource(0);
    if !info.is_valid() {
        errorf!(reporter, "shared resource at slot 0 is missing");
        return;
    }

    let groups = vec![builder.finalize()];
    recorder.priv_().add(ComputeTask::make(groups));

    let buffer = sync_buffer_to_cpu(&mut recorder, &info.ref_buffer());

    let Some(_recording) = submit_recording(context, &mut recorder) else {
        errorf!(reporter, "Failed to make recording");
        return;
    };

    const K_EXPECTED_COUNT: u32 = K_WORKGROUP_COUNT * K_WORKGROUP_SIZE;
    let bytes = map_buffer(context, &buffer, info.offset as usize);
    let result = u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    reporter_assert!(
        reporter,
        result == K_EXPECTED_COUNT,
        "expected '{}', found '{}'",
        K_EXPECTED_COUNT,
        result
    );
});

// Port of: tests/graphite/ComputeTest.cpp#L1635-L1774 (chrome/m156)
// The C++ test returns early on Dawn D3D11 (b/315834710); the wgpu backend has no such context.
def_graphite_adapter_test!(
    Compute_AtomicOperationsOverArrayAndStructTest,
    |reporter, context| {
        const K_WORKGROUP_COUNT: u32 = 32;
        const K_WORKGROUP_SIZE: u32 = 128;

        #[derive(Debug)]
        struct TestComputeStep {
            base: ComputeStepBase,
        }

        impl ComputeStep for TestComputeStep {
            fn base(&self) -> &ComputeStepBase {
                &self.base
            }

            fn compute_sksl(&self) -> String {
                r"
                const uint WORKGROUP_SIZE = 128;
                workgroup atomicUint localCounts[2];
                void main() {
                    if (sk_LocalInvocationID.x == 0) {
                        atomicStore(localCounts[0], 0);
                        atomicStore(localCounts[1], 0);
                    }
                    workgroupBarrier();
                    uint idx = sk_LocalInvocationID.x < (WORKGROUP_SIZE / 2) ? 0 : 1;
                    atomicAdd(localCounts[idx], 1);
                    workgroupBarrier();
                    if (sk_LocalInvocationID.x == 0) {
                        atomicAdd(globalCountsFirstHalf, atomicLoad(localCounts[0]));
                        atomicAdd(globalCountsSecondHalf, atomicLoad(localCounts[1]));
                    }
                }
            "
                .to_owned()
            }

            fn calculate_buffer_size(&self, index: usize, r: &ResourceDesc) -> usize {
                debug_assert_eq!(index, 0);
                debug_assert_eq!(r.slot, 0);
                debug_assert_eq!(r.flow, DataFlow::Shared);
                2 * std::mem::size_of::<u32>()
            }

            fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
                WorkgroupSize::new(K_WORKGROUP_COUNT, 1, 1)
            }

            fn prepare_storage_buffer(
                &self,
                resource_index: usize,
                _r: &ResourceDesc,
                mut writer: BufferWriter<'_>,
            ) {
                debug_assert_eq!(resource_index, 0);
                writer.zero_bytes(2 * std::mem::size_of::<u32>());
            }
        }

        let resources = [ResourceDesc::with_slot_and_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Shared,
            ResourcePolicy::Mapped,
            0,
            "ssbo {\n   atomicUint globalCountsFirstHalf;\n   atomicUint globalCountsSecondHalf;\n}\n",
        )];
        let step: Arc<dyn ComputeStep> = Arc::new(TestComputeStep {
            base: ComputeStepBase::new(
                "TestAtomicOperationsOverArrayAndStruct",
                WorkgroupSize::new(K_WORKGROUP_SIZE, 1, 1),
                &resources,
                &[],
                false,
            ),
        });

        let mut recorder = context.make_recorder(None);

        let mut builder = Builder::new(&recorder);
        let _ = builder.append_step(&step, None);
        let info = builder.get_shared_buffer_resource(0);
        if !info.is_valid() {
            errorf!(reporter, "shared resource at slot 0 is missing");
            return;
        }

        let groups = vec![builder.finalize()];
        recorder.priv_().add(ComputeTask::make(groups));

        let buffer = sync_buffer_to_cpu(&mut recorder, &info.ref_buffer());

        let Some(_recording) = submit_recording(context, &mut recorder) else {
            errorf!(reporter, "Failed to make recording");
            return;
        };

        const K_EXPECTED_COUNT: u32 = K_WORKGROUP_COUNT * K_WORKGROUP_SIZE / 2;
        let bytes = map_buffer(context, &buffer, info.offset as usize);
        let first_half_count = u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let second_half_count = u32::from_ne_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        reporter_assert!(
            reporter,
            first_half_count == K_EXPECTED_COUNT,
            "expected '{}', found '{}'",
            K_EXPECTED_COUNT,
            first_half_count
        );
        reporter_assert!(
            reporter,
            second_half_count == K_EXPECTED_COUNT,
            "expected '{}', found '{}'",
            K_EXPECTED_COUNT,
            second_half_count
        );
    }
);

// Port of: tests/graphite/ComputeTest.cpp#L1776-L1881 (chrome/m156)
def_graphite_adapter_test!(Compute_ClearedBuffer, |reporter, context| {
    const K_PROBLEM_SIZE: u32 = 512;
    const K_WORKGROUP_SIZE: u32 = K_PROBLEM_SIZE / 4;

    #[derive(Debug)]
    struct TestComputeStep {
        base: ComputeStepBase,
    }

    impl ComputeStep for TestComputeStep {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        fn compute_sksl(&self) -> String {
            r"
                void main() {
                    out_data[sk_GlobalInvocationID.x] = in_data[sk_GlobalInvocationID.x];
                }
            "
            .to_owned()
        }

        fn calculate_buffer_size(&self, _index: usize, _r: &ResourceDesc) -> usize {
            std::mem::size_of::<u32>() * K_PROBLEM_SIZE as usize
        }

        fn prepare_storage_buffer(
            &self,
            resource_index: usize,
            _r: &ResourceDesc,
            _writer: BufferWriter<'_>,
        ) {
            debug_assert_eq!(resource_index, 1);
        }

        fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
            WorkgroupSize::new(1, 1, 1)
        }
    }

    let resources = [
        ResourceDesc::with_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Private,
            ResourcePolicy::Clear,
            "inputBlock { uint4 in_data[]; }\n",
        ),
        // shared to allow us to access it from the Builder
        // mappable for read-back
        ResourceDesc::with_slot_and_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Shared,
            ResourcePolicy::Mapped,
            0,
            "outputBlock { uint4 out_data[]; }\n",
        ),
    ];
    let step: Arc<dyn ComputeStep> = Arc::new(TestComputeStep {
        base: ComputeStepBase::new(
            "TestClearedBuffer",
            WorkgroupSize::new(K_WORKGROUP_SIZE, 1, 1),
            &resources,
            &[],
            false,
        ),
    });

    let mut recorder = context.make_recorder(None);

    let mut builder = Builder::new(&recorder);
    if !builder.append_step(&step, None) {
        errorf!(reporter, "Failed to add ComputeStep to DispatchGroup");
        return;
    }

    let output_info = builder.get_shared_buffer_resource(0);
    if !output_info.is_valid() {
        errorf!(reporter, "Failed to allocate an output buffer at slot 0");
        return;
    }

    let groups = vec![builder.finalize()];
    recorder.priv_().add(ComputeTask::make(groups));

    let output_buffer = sync_buffer_to_cpu(&mut recorder, &output_info.ref_buffer());

    let Some(_recording) = submit_recording(context, &mut recorder) else {
        errorf!(reporter, "Failed to make recording");
        return;
    };

    let out_bytes = map_buffer(context, &output_buffer, output_info.offset as usize);
    for i in 0..K_PROBLEM_SIZE as usize {
        let found = u32::from_ne_bytes(out_bytes[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
        reporter_assert!(reporter, found == 0, "expected '0u', found '{}'", found);
    }
});

// Port of: tests/graphite/ComputeTest.cpp#L1883-L1995 (chrome/m156)
def_graphite_adapter_test!(Compute_ClearOrdering, |reporter, context| {
    const K_WORKGROUP_SIZE: u32 = 64;

    #[derive(Debug)]
    struct FillWithGarbage {
        base: ComputeStepBase,
    }

    impl ComputeStep for FillWithGarbage {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        fn compute_sksl(&self) -> String {
            r"
                void main() {
                    out_data[sk_GlobalInvocationID.x] = uint4(0xFE);
                }
            "
            .to_owned()
        }
    }

    #[derive(Debug)]
    struct CopyBuffer {
        base: ComputeStepBase,
    }

    impl ComputeStep for CopyBuffer {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        fn compute_sksl(&self) -> String {
            r"
                void main() {
                    out_data[sk_GlobalInvocationID.x] = in_data[sk_GlobalInvocationID.x];
                }
            "
            .to_owned()
        }
    }

    let garbage_resources = [ResourceDesc::with_slot_and_sksl(
        ResourceType::StorageBuffer,
        DataFlow::Shared,
        ResourcePolicy::None,
        0,
        "outputBlock { uint4 out_data[]; }\n",
    )];
    let copy_resources = [
        ResourceDesc::with_slot_and_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Shared,
            ResourcePolicy::None,
            0,
            "inputBlock { uint4 in_data[]; }\n",
        ),
        ResourceDesc::with_slot_and_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Shared,
            ResourcePolicy::None,
            1,
            "outputBlock { uint4 out_data[]; }\n",
        ),
    ];
    let garbage_step: Arc<dyn ComputeStep> = Arc::new(FillWithGarbage {
        base: ComputeStepBase::new(
            "FillWithGarbage",
            WorkgroupSize::new(K_WORKGROUP_SIZE, 1, 1),
            &garbage_resources,
            &[],
            false,
        ),
    });
    let copy_step: Arc<dyn ComputeStep> = Arc::new(CopyBuffer {
        base: ComputeStepBase::new(
            "CopyBuffer",
            WorkgroupSize::new(K_WORKGROUP_SIZE, 1, 1),
            &copy_resources,
            &[],
            false,
        ),
    });

    let mut recorder = context.make_recorder(None);

    let mut builder = Builder::new(&recorder);
    const K_ELEMENT_COUNT: u32 = 4 * K_WORKGROUP_SIZE;
    const K_BUFFER_SIZE: u32 = std::mem::size_of::<u32>() as u32 * K_ELEMENT_COUNT;
    let input = recorder
        .priv_()
        .draw_buffer_manager()
        .get_storage(K_BUFFER_SIZE as usize, ClearBuffer::No);
    let mapped = recorder
        .priv_()
        .draw_buffer_manager()
        .get_mapped_storage_buffer(K_ELEMENT_COUNT as usize, std::mem::size_of::<u32>())
        .expect("a mapped output buffer");
    let output = mapped.binding.clone();
    drop(mapped);

    let mut groups = Vec::new();
    builder.assign_shared_buffer(input.clone(), 0, ClearBuffer::No);
    let _ = builder.append_step(&garbage_step, Some(WorkgroupSize::new(1, 1, 1)));
    groups.push(builder.finalize());
    builder.reset();
    builder.assign_shared_buffer(input, 0, ClearBuffer::Yes);
    builder.assign_shared_buffer(output.clone(), 1, ClearBuffer::No);
    let _ = builder.append_step(&copy_step, Some(WorkgroupSize::new(1, 1, 1)));
    groups.push(builder.finalize());
    recorder.priv_().add(ComputeTask::make(groups));

    let output_buffer = sync_buffer_to_cpu(&mut recorder, &output.ref_buffer());

    let Some(_recording) = submit_recording(context, &mut recorder) else {
        errorf!(reporter, "Failed to make recording");
        return;
    };

    let out_bytes = map_buffer(context, &output_buffer, output.offset as usize);
    for i in 0..K_ELEMENT_COUNT as usize {
        let found = u32::from_ne_bytes(out_bytes[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
        reporter_assert!(reporter, found == 0, "expected '0u', found '{}'", found);
    }
});

// Port of: tests/graphite/ComputeTest.cpp#L1997-L2117 (chrome/m156)
def_graphite_adapter_test!(Compute_ClearOrderingScratchBuffers, |reporter, context| {
    const K_WORKGROUP_SIZE: u32 = 64;

    #[derive(Debug)]
    struct FillWithGarbage {
        base: ComputeStepBase,
    }

    impl ComputeStep for FillWithGarbage {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        fn compute_sksl(&self) -> String {
            r"
                void main() {
                    out_data[sk_GlobalInvocationID.x] = uint4(0xFE);
                }
            "
            .to_owned()
        }
    }

    #[derive(Debug)]
    struct CopyBuffer {
        base: ComputeStepBase,
    }

    impl ComputeStep for CopyBuffer {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        fn compute_sksl(&self) -> String {
            r"
                void main() {
                    out_data[sk_GlobalInvocationID.x] = in_data[sk_GlobalInvocationID.x];
                }
            "
            .to_owned()
        }
    }

    let garbage_resources = [ResourceDesc::with_slot_and_sksl(
        ResourceType::StorageBuffer,
        DataFlow::Shared,
        ResourcePolicy::None,
        0,
        "outputBlock { uint4 out_data[]; }\n",
    )];
    let copy_resources = [
        ResourceDesc::with_slot_and_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Shared,
            ResourcePolicy::None,
            0,
            "inputBlock { uint4 in_data[]; }\n",
        ),
        ResourceDesc::with_slot_and_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Shared,
            ResourcePolicy::None,
            1,
            "outputBlock { uint4 out_data[]; }\n",
        ),
    ];
    let garbage_step: Arc<dyn ComputeStep> = Arc::new(FillWithGarbage {
        base: ComputeStepBase::new(
            "FillWithGarbage",
            WorkgroupSize::new(K_WORKGROUP_SIZE, 1, 1),
            &garbage_resources,
            &[],
            false,
        ),
    });
    let copy_step: Arc<dyn ComputeStep> = Arc::new(CopyBuffer {
        base: ComputeStepBase::new(
            "CopyBuffer",
            WorkgroupSize::new(K_WORKGROUP_SIZE, 1, 1),
            &copy_resources,
            &[],
            false,
        ),
    });

    let mut recorder = context.make_recorder(None);

    let mut builder = Builder::new(&recorder);
    const K_ELEMENT_COUNT: u32 = 4 * K_WORKGROUP_SIZE;
    const K_BUFFER_SIZE: u32 = std::mem::size_of::<u32>() as u32 * K_ELEMENT_COUNT;
    let mapped = recorder
        .priv_()
        .draw_buffer_manager()
        .get_mapped_storage_buffer(K_ELEMENT_COUNT as usize, std::mem::size_of::<u32>())
        .expect("a mapped output buffer");
    let output = mapped.binding.clone();
    drop(mapped);

    let mut groups = Vec::new();
    {
        let scratch = recorder
            .priv_()
            .draw_buffer_manager()
            .get_scratch_storage(K_BUFFER_SIZE as usize);
        let mut scratch = scratch;
        let input = scratch.get_subrange(K_ELEMENT_COUNT as usize, std::mem::size_of::<u32>(), 1);
        builder.assign_shared_buffer(input, 0, ClearBuffer::No);
    }
    let _ = builder.append_step(&garbage_step, Some(WorkgroupSize::new(1, 1, 1)));
    groups.push(builder.finalize());
    builder.reset();
    {
        let scratch = recorder
            .priv_()
            .draw_buffer_manager()
            .get_scratch_storage(K_BUFFER_SIZE as usize);
        let mut scratch = scratch;
        let input = scratch.get_subrange(K_ELEMENT_COUNT as usize, std::mem::size_of::<u32>(), 1);
        builder.assign_shared_buffer(input, 0, ClearBuffer::Yes);
    }
    builder.assign_shared_buffer(output.clone(), 1, ClearBuffer::No);
    let _ = builder.append_step(&copy_step, Some(WorkgroupSize::new(1, 1, 1)));
    groups.push(builder.finalize());
    recorder.priv_().add(ComputeTask::make(groups));

    let output_buffer = sync_buffer_to_cpu(&mut recorder, &output.ref_buffer());

    let Some(_recording) = submit_recording(context, &mut recorder) else {
        errorf!(reporter, "Failed to make recording");
        return;
    };

    let out_bytes = map_buffer(context, &output_buffer, output.offset as usize);
    for i in 0..K_ELEMENT_COUNT as usize {
        let found = u32::from_ne_bytes(out_bytes[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
        reporter_assert!(reporter, found == 0, "expected '0u', found '{}'", found);
    }
});

// Port of: tests/graphite/ComputeTest.cpp#L2119-L2289 (chrome/m156)
// The C++ test returns early on Dawn D3D11 (b/315834710); the wgpu backend has no such context.
def_graphite_adapter_test!(Compute_IndirectDispatch, |reporter, context| {
    const K_WORKGROUP_COUNT: u32 = 32;
    const K_WORKGROUP_SIZE: u32 = 64;

    #[derive(Debug)]
    struct IndirectStep {
        base: ComputeStepBase,
    }

    impl ComputeStep for IndirectStep {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        fn compute_sksl(&self) -> String {
            r"
                const uint kWorkgroupCount = 32;
                void main() {
                    if (sk_LocalInvocationID.x == 0) {
                        indirect[0] = kWorkgroupCount;
                        indirect[1] = 1;
                        indirect[2] = 1;
                    }
                }
            "
            .to_owned()
        }

        fn calculate_buffer_size(&self, index: usize, r: &ResourceDesc) -> usize {
            debug_assert_eq!(index, 0);
            debug_assert_eq!(r.slot, 0);
            debug_assert_eq!(r.flow, DataFlow::Shared);
            INDIRECT_DISPATCH_ARGUMENT_SIZE
        }

        fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
            WorkgroupSize::new(1, 1, 1)
        }
    }

    #[derive(Debug)]
    struct CountStep {
        base: ComputeStepBase,
    }

    impl ComputeStep for CountStep {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        fn compute_sksl(&self) -> String {
            r"
                workgroup atomicUint localCounter;
                void main() {
                    if (sk_LocalInvocationID.x == 0) {
                        atomicStore(localCounter, 0);
                    }
                    workgroupBarrier();
                    atomicAdd(localCounter, 1);
                    workgroupBarrier();
                    if (sk_LocalInvocationID.x == 0) {
                        atomicAdd(globalCounter, atomicLoad(localCounter));
                    }
                }
            "
            .to_owned()
        }

        fn calculate_buffer_size(&self, index: usize, r: &ResourceDesc) -> usize {
            debug_assert_eq!(index, 0);
            debug_assert_eq!(r.slot, 1);
            debug_assert_eq!(r.flow, DataFlow::Shared);
            std::mem::size_of::<u32>()
        }

        fn prepare_storage_buffer(
            &self,
            resource_index: usize,
            _r: &ResourceDesc,
            mut writer: BufferWriter<'_>,
        ) {
            debug_assert_eq!(resource_index, 0);
            writer.zero_bytes(std::mem::size_of::<u32>());
        }
    }

    let indirect_resources = [ResourceDesc::with_slot_and_sksl(
        ResourceType::IndirectBuffer,
        DataFlow::Shared,
        ResourcePolicy::Clear,
        0,
        "ssbo { uint indirect[]; }",
    )];
    let count_resources = [ResourceDesc::with_slot_and_sksl(
        ResourceType::StorageBuffer,
        DataFlow::Shared,
        ResourcePolicy::Mapped,
        1,
        "ssbo { atomicUint globalCounter; }",
    )];
    let indirect_step: Arc<dyn ComputeStep> = Arc::new(IndirectStep {
        base: ComputeStepBase::new(
            "TestIndirectDispatch_IndirectStep",
            WorkgroupSize::new(K_WORKGROUP_SIZE, 1, 1),
            &indirect_resources,
            &[],
            false,
        ),
    });
    let count_step: Arc<dyn ComputeStep> = Arc::new(CountStep {
        base: ComputeStepBase::new(
            "TestIndirectDispatch_CountStep",
            WorkgroupSize::new(K_WORKGROUP_SIZE, 1, 1),
            &count_resources,
            &[],
            false,
        ),
    });

    let mut recorder = context.make_recorder(None);

    let mut builder = Builder::new(&recorder);
    let _ = builder.append_step(&indirect_step, None);
    let indirect_buffer_info = builder.get_shared_buffer_resource(0);
    if !indirect_buffer_info.is_valid() {
        errorf!(reporter, "Shared resource at slot 0 is missing");
        return;
    }

    reporter_assert!(
        reporter,
        indirect_buffer_info.size as usize == INDIRECT_DISPATCH_ARGUMENT_SIZE
    );
    let _ = builder.append_step_indirect(&count_step, indirect_buffer_info);
    let info = builder.get_shared_buffer_resource(1);
    if !info.is_valid() {
        errorf!(reporter, "Shared resource at slot 1 is missing");
        return;
    }

    let groups = vec![builder.finalize()];
    recorder.priv_().add(ComputeTask::make(groups));

    let buffer = sync_buffer_to_cpu(&mut recorder, &info.ref_buffer());

    let Some(_recording) = submit_recording(context, &mut recorder) else {
        errorf!(reporter, "Failed to make recording");
        return;
    };

    const K_EXPECTED_COUNT: u32 = K_WORKGROUP_COUNT * K_WORKGROUP_SIZE;
    let bytes = map_buffer(context, &buffer, info.offset as usize);
    let result = u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    reporter_assert!(
        reporter,
        result == K_EXPECTED_COUNT,
        "expected '{}', found '{}'",
        K_EXPECTED_COUNT,
        result
    );
});

// Port of: tests/graphite/ComputeTest.cpp#L2542-L2663 (chrome/m156)
// The C++ test returns early on Dawn D3D11 (b/315834710); the wgpu backend has no such context.
def_graphite_adapter_test!(Compute_NativeShaderSourceWGSL, |reporter, context| {
    const K_WORKGROUP_COUNT: u32 = 32;
    const K_WORKGROUP_SIZE: u32 = 128;

    #[derive(Debug)]
    struct TestComputeStep {
        base: ComputeStepBase,
    }

    impl ComputeStep for TestComputeStep {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        fn native_shader_source(&self, format: NativeShaderFormat) -> NativeShaderSource<'_> {
            debug_assert_eq!(format, NativeShaderFormat::Wgsl);
            const K_SOURCE: &str = r"
                @group(0) @binding(0) var<storage, read_write> globalCounter: atomic<u32>;
                var<workgroup> localCounter: atomic<u32>;
                @compute @workgroup_size(128)
                fn atomicCount(@builtin(local_invocation_id) localId: vec3u) {
                    if localId.x == 0u {
                        atomicStore(&localCounter, 0u);
                    }
                    workgroupBarrier();
                    atomicAdd(&localCounter, 1u);
                    workgroupBarrier();
                    if localId.x == 0u {
                        let tally = atomicLoad(&localCounter);
                        atomicAdd(&globalCounter, tally);
                    }
                }
            ";
            NativeShaderSource {
                source: K_SOURCE,
                entry_point: "atomicCount".to_owned(),
            }
        }

        fn calculate_buffer_size(&self, index: usize, r: &ResourceDesc) -> usize {
            debug_assert_eq!(index, 0);
            debug_assert_eq!(r.slot, 0);
            debug_assert_eq!(r.flow, DataFlow::Shared);
            std::mem::size_of::<u32>()
        }

        fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
            WorkgroupSize::new(K_WORKGROUP_COUNT, 1, 1)
        }

        fn prepare_storage_buffer(
            &self,
            resource_index: usize,
            _r: &ResourceDesc,
            mut writer: BufferWriter<'_>,
        ) {
            debug_assert_eq!(resource_index, 0);
            writer.zero_bytes(std::mem::size_of::<u32>());
        }
    }

    let resources = [ResourceDesc::with_slot(
        ResourceType::StorageBuffer,
        DataFlow::Shared,
        ResourcePolicy::Mapped,
        0,
    )];
    let step: Arc<dyn ComputeStep> = Arc::new(TestComputeStep {
        base: ComputeStepBase::new(
            "TestAtomicOperationsWGSL",
            WorkgroupSize::new(K_WORKGROUP_SIZE, 1, 1),
            &resources,
            &[],
            true,
        ),
    });

    let mut recorder = context.make_recorder(None);

    let mut builder = Builder::new(&recorder);
    let _ = builder.append_step(&step, None);
    let info = builder.get_shared_buffer_resource(0);
    if !info.is_valid() {
        errorf!(reporter, "shared resource at slot 0 is missing");
        return;
    }

    let groups = vec![builder.finalize()];
    recorder.priv_().add(ComputeTask::make(groups));

    let buffer = sync_buffer_to_cpu(&mut recorder, &info.ref_buffer());

    let Some(_recording) = submit_recording(context, &mut recorder) else {
        errorf!(reporter, "Failed to make recording");
        return;
    };

    const K_EXPECTED_COUNT: u32 = K_WORKGROUP_COUNT * K_WORKGROUP_SIZE;
    let bytes = map_buffer(context, &buffer, info.offset as usize);
    let result = u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    reporter_assert!(
        reporter,
        result == K_EXPECTED_COUNT,
        "expected '{}', found '{}'",
        K_EXPECTED_COUNT,
        result
    );
});

// Port of: tests/graphite/ComputeTest.cpp#L2665-L2783 (chrome/m156)
// The C++ test returns early on Dawn D3D11 (b/315834710); the wgpu backend has no such context.
def_graphite_adapter_test!(Compute_WorkgroupUniformLoadTest, |reporter, context| {
    const K_PROBLEM_SIZE: u32 = 256;
    const K_WORKGROUP_SIZE: u32 = 256;

    #[derive(Debug)]
    struct TestComputeStep {
        base: ComputeStepBase,
    }

    impl ComputeStep for TestComputeStep {
        fn base(&self) -> &ComputeStepBase {
            &self.base
        }

        fn compute_sksl(&self) -> String {
            r"
                workgroup uint shared_uniform;
                void main() {
                    uint id = sk_GlobalInvocationID.x;
                    if (id == 0) {
                        shared_uniform = 0;
                    }
                    uint uni = workgroupUniformLoad(shared_uniform);
                    if (uni == 0) {
                        out_data[id] = in_data[id] * 2.0;
                    }
                }
            "
            .to_owned()
        }

        fn calculate_buffer_size(&self, index: usize, r: &ResourceDesc) -> usize {
            if index == 0 {
                debug_assert_eq!(r.flow, DataFlow::Private);
                return std::mem::size_of::<f32>() * K_PROBLEM_SIZE as usize;
            }
            debug_assert_eq!(index, 1);
            debug_assert_eq!(r.slot, 0);
            debug_assert_eq!(r.flow, DataFlow::Shared);
            std::mem::size_of::<f32>() * K_PROBLEM_SIZE as usize
        }

        fn prepare_storage_buffer(
            &self,
            resource_index: usize,
            r: &ResourceDesc,
            mut writer: BufferWriter<'_>,
        ) {
            if resource_index != 0 {
                return;
            }
            debug_assert_eq!(r.flow, DataFlow::Private);
            for i in 0..K_PROBLEM_SIZE {
                writer.write(&((i + 1) as f32));
            }
        }

        fn calculate_global_dispatch_size(&self) -> WorkgroupSize {
            WorkgroupSize::new(1, 1, 1)
        }
    }

    let resources = [
        ResourceDesc::with_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Private,
            ResourcePolicy::Mapped,
            "inputs { float in_data[]; }",
        ),
        ResourceDesc::with_slot_and_sksl(
            ResourceType::StorageBuffer,
            DataFlow::Shared,
            ResourcePolicy::Mapped,
            0,
            "outputs { float out_data[]; }",
        ),
    ];
    let step: Arc<dyn ComputeStep> = Arc::new(TestComputeStep {
        base: ComputeStepBase::new(
            "TestWorkgroupUniformLoad",
            WorkgroupSize::new(K_WORKGROUP_SIZE, 1, 1),
            &resources,
            &[],
            false,
        ),
    });

    let mut recorder = context.make_recorder(None);

    let mut builder = Builder::new(&recorder);
    if !builder.append_step(&step, None) {
        errorf!(reporter, "Failed to add ComputeStep to DispatchGroup");
        return;
    }

    let output_info = builder.get_shared_buffer_resource(0);
    if !output_info.is_valid() {
        errorf!(reporter, "Failed to allocate an output buffer at slot 0");
        return;
    }

    let groups = vec![builder.finalize()];
    recorder.priv_().add(ComputeTask::make(groups));

    let output_buffer = sync_buffer_to_cpu(&mut recorder, &output_info.ref_buffer());

    let Some(_recording) = submit_recording(context, &mut recorder) else {
        errorf!(reporter, "Failed to make recording");
        return;
    };

    let out_bytes = map_buffer(context, &output_buffer, output_info.offset as usize);
    let out_data = read_f32s(&out_bytes);
    for i in 0..K_PROBLEM_SIZE as usize {
        let expected = (i + 1) as f32 * 2.0;
        let found = out_data[i];
        reporter_assert!(
            reporter,
            expected == found,
            "expected '{}', found '{}'",
            expected,
            found
        );
    }
});
