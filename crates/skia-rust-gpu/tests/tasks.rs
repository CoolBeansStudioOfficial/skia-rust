// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The tasks of `src/gpu/graphite/task` (and `TaskList`) against a mock command buffer. Skia
//! has no unit tests of its own for them: their behavior is exercised through the
//! `Context`-based tests, so these follow the C++ step by step.

mod support;

use std::sync::{Arc, Mutex};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ColorInfo;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, Mipmapped};
use skia_rust_gpu::gpu::swizzle::Swizzle;
use skia_rust_gpu::graphite::buffer::{BindBufferInfo, Buffer};
use skia_rust_gpu::graphite::caps::AttachmentSizePolicy;
use skia_rust_gpu::graphite::command_buffer::BufferTextureCopyData;
use skia_rust_gpu::graphite::graphics_pipeline::GraphicsPipeline;
use skia_rust_gpu::graphite::graphite_types::{DepthStencilFlags, SampleCount};
use skia_rust_gpu::graphite::render_pass_desc::{AttachmentDesc, RenderPassDesc};
use skia_rust_gpu::graphite::resource::ResourceRef;
use skia_rust_gpu::graphite::resource_provider::ResourceProvider;
use skia_rust_gpu::graphite::resource_types::{
    AccessPattern, BufferType, DstReadStrategy, LoadOp, StoreOp,
};
use skia_rust_gpu::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use skia_rust_gpu::graphite::scratch_resource_manager::{
    ProxyReadCountMap, ScratchResourceManager,
};
use skia_rust_gpu::graphite::task::clear_buffers_task::ClearBuffersTask;
use skia_rust_gpu::graphite::task::compute_task::{ComputeTask, DispatchGroup};
use skia_rust_gpu::graphite::task::copy_task::{
    CopyBufferToBufferTask, CopyTextureToBufferTask, CopyTextureToTextureTask,
};
use skia_rust_gpu::graphite::task::draw_task::DrawTask;
use skia_rust_gpu::graphite::task::render_pass_task::{DrawPass, RenderPassTask};
use skia_rust_gpu::graphite::task::synchronize_to_cpu_task::SynchronizeToCpuTask;
use skia_rust_gpu::graphite::task::task_list::TaskList;
use skia_rust_gpu::graphite::task::upload_task::{
    ConditionalUploadContext, ImageUploadContext, MipLevel, UploadInstance, UploadList,
    UploadSource, UploadTask,
};
use skia_rust_gpu::graphite::task::{ReplayTargetData, Status, TaskRef};
use skia_rust_gpu::graphite::texture::Texture;
use skia_rust_gpu::graphite::texture_format::TextureFormat;
use skia_rust_gpu::graphite::texture_info::TextureInfo;
use skia_rust_gpu::graphite::texture_proxy::TextureProxy;
use skia_rust_gpu::graphite::texture_proxy_view::TextureProxyView;
use skia_rust_gpu::graphite::upload_buffer_manager::UploadBufferManager;
use support::{
    Call, MockCaps, MockCommandBuffer, MockContext, provider, rgba_info, shared_provider,
    texture_info,
};

fn proxy(
    rp: &mut ResourceProvider,
    size: i32,
    info: &TextureInfo,
    name: &str,
) -> Arc<TextureProxy> {
    TextureProxy::make(
        &MockCaps::default(),
        rp,
        ISize::new(size, size),
        info,
        Budgeted::Yes,
        name,
    )
    .unwrap()
}

fn buffer(rp: &mut ResourceProvider, size: usize) -> ResourceRef<Buffer> {
    rp.find_or_create_non_shareable_buffer(size, BufferType::Storage, AccessPattern::GpuOnly, "b")
        .unwrap()
}

fn run_prepare(list: &mut TaskList, rp: &mut ResourceProvider) -> Status {
    let mut scratch = ScratchResourceManager::new(ProxyReadCountMap::new());
    list.prepare_resources(rp, &mut scratch, None)
}

fn run_commands(list: &mut TaskList, cb: &mut MockCommandBuffer) -> Status {
    let (mut context, _) = MockContext::new(MockCaps::default());
    list.add_commands(&mut context, cb, &ReplayTargetData::default())
}

fn color_info() -> ColorInfo {
    ColorInfo::new(ColorType::RGBA8888, AlphaType::Premul, None)
}

#[test]
fn empty_task_list_discards() {
    let (mut rp, _) = provider();
    let mut list = TaskList::new();
    assert!(!list.has_tasks());
    // Zero discarded children of zero children: everything was discarded.
    assert_eq!(run_prepare(&mut list, &mut rp), Status::Discard);
    let mut cb = MockCommandBuffer::default();
    assert_eq!(run_commands(&mut list, &mut cb), Status::Discard);
}

#[test]
fn clear_buffers_task_attempts_every_clear() {
    let (mut rp, _) = provider();
    let b = buffer(&mut rp, 256);
    let clears = vec![
        BindBufferInfo::new(&b, 0, 64),
        BindBufferInfo::new(&b, 64, 32),
    ];
    let mut list = TaskList::new();
    list.add(ClearBuffersTask::make(clears));
    assert_eq!(run_prepare(&mut list, &mut rp), Status::Success);

    let mut cb = MockCommandBuffer::default();
    assert_eq!(run_commands(&mut list, &mut cb), Status::Success);
    assert_eq!(
        cb.calls,
        [
            Call::ClearBuffer {
                offset: 0,
                size: 64
            },
            Call::ClearBuffer {
                offset: 64,
                size: 32
            }
        ]
    );

    // A failing clear does not stop the others, but fails the task and the list.
    let mut cb = MockCommandBuffer {
        fail: true,
        ..MockCommandBuffer::default()
    };
    assert_eq!(run_commands(&mut list, &mut cb), Status::Fail);
    assert_eq!(cb.calls.len(), 2);
}

#[test]
fn synchronize_to_cpu_task_gives_its_buffer_to_the_command_buffer_once() {
    let (mut rp, _) = provider();
    let b = buffer(&mut rp, 16);
    let mut list = TaskList::new();
    list.add(SynchronizeToCpuTask::make(b));
    let mut cb = MockCommandBuffer::default();
    assert_eq!(run_commands(&mut list, &mut cb), Status::Success);
    assert_eq!(cb.calls, [Call::SynchronizeBufferToCpu]);
    // The buffer was moved into the command buffer; a replay has nothing to give.
    let mut cb = MockCommandBuffer::default();
    assert_eq!(run_commands(&mut list, &mut cb), Status::Fail);
    assert_eq!(cb.calls.len(), 0);
}

#[test]
fn copy_buffer_to_buffer_task() {
    let (mut rp, _) = provider();
    let src = buffer(&mut rp, 128);
    let dst = buffer(&mut rp, 128);
    let task = CopyBufferToBufferTask::make(src.as_arc(), 16, dst, 32, 64);
    let mut list = TaskList::new();
    list.add(task);
    assert_eq!(run_prepare(&mut list, &mut rp), Status::Success);
    let mut cb = MockCommandBuffer::default();
    assert_eq!(run_commands(&mut list, &mut cb), Status::Success);
    assert_eq!(
        cb.calls,
        [Call::CopyBufferToBuffer {
            src_offset: 16,
            dst_offset: 32,
            size: 64
        }]
    );
    // Copies repeat on replay.
    let mut cb = MockCommandBuffer::default();
    assert_eq!(run_commands(&mut list, &mut cb), Status::Success);
    assert_eq!(cb.calls.len(), 1);
}

#[test]
fn copy_texture_tasks() {
    let (mut rp, _) = provider();
    let src = proxy(&mut rp, 4, &rgba_info(), "src");
    let dst = proxy(&mut rp, 4, &rgba_info(), "dst");
    assert!(src.instantiate(&mut rp));

    // Texture to texture: formats must match.
    let other_info = texture_info(TextureFormat::R8, SampleCount::One, Mipmapped::No);
    let r8 = proxy(&mut rp, 4, &other_info, "r8");
    assert!(
        CopyTextureToTextureTask::make(
            Some(src.clone()),
            IRect::from_wh(4, 4),
            Some(r8),
            IPoint::new(0, 0),
            0
        )
        .is_none()
    );
    assert!(
        CopyTextureToTextureTask::make(
            None,
            IRect::from_wh(4, 4),
            Some(dst.clone()),
            IPoint::new(0, 0),
            0
        )
        .is_none()
    );
    let task = CopyTextureToTextureTask::make(
        Some(src.clone()),
        IRect::from_wh(2, 2),
        Some(dst.clone()),
        IPoint::new(1, 1),
        0,
    )
    .unwrap();

    // Texture to buffer.
    let b = buffer(&mut rp, 1024);
    let to_buffer =
        CopyTextureToBufferTask::make(Some(src.clone()), IRect::from_wh(4, 4), b, 0, 16).unwrap();
    assert!(
        CopyTextureToBufferTask::make(None, IRect::from_wh(1, 1), buffer(&mut rp, 4), 0, 4)
            .is_none()
    );

    // The task graph visits the proxies: the source is always read, the destination is only
    // visited when not restricted to reads.
    let mut list = TaskList::new();
    list.add(task);
    list.add(to_buffer);
    let mut visited = Vec::new();
    assert!(list.visit_proxies(
        &mut |p| {
            visited.push(Arc::as_ptr(p));
            true
        },
        /*reads_only=*/ true
    ));
    assert_eq!(visited, [Arc::as_ptr(&src), Arc::as_ptr(&src)]);
    let mut visited = Vec::new();
    assert!(list.visit_proxies(
        &mut |p| {
            visited.push(Arc::as_ptr(p));
            true
        },
        false
    ));
    assert_eq!(
        visited,
        [Arc::as_ptr(&src), Arc::as_ptr(&dst), Arc::as_ptr(&src)]
    );
    // A visitor that stops early ends the visit.
    assert!(!list.visit_proxies(&mut |_| false, false));
    assert!(list.visit_pipelines(&mut |_| false));

    // prepareResources() instantiates the destination of the texture copy.
    assert!(!dst.is_instantiated());
    assert_eq!(run_prepare(&mut list, &mut rp), Status::Success);
    assert!(dst.is_instantiated());

    let mut cb = MockCommandBuffer::default();
    assert_eq!(run_commands(&mut list, &mut cb), Status::Success);
    assert_eq!(
        cb.calls,
        [
            Call::CopyTextureToTexture {
                src_rect: IRect::from_wh(2, 2),
                dst_point: IPoint::new(1, 1),
                dst_level: 0
            },
            Call::CopyTextureToBuffer {
                src_rect: IRect::from_wh(4, 4),
                buffer_offset: 0,
                buffer_row_bytes: 16
            }
        ]
    );
}

fn upload_instance_for(
    rp: &support::BackendCounts,
    um: &mut UploadBufferManager,
    caps: &MockCaps,
    proxy: &Arc<TextureProxy>,
    levels: &[MipLevel<'_>],
    dst_rect: IRect,
    context: Option<Box<dyn ConditionalUploadContext>>,
) -> UploadInstance {
    let _ = rp;
    let source = UploadSource::make(
        caps,
        &TextureProxyView::from_proxy(Some(proxy.clone())),
        &color_info(),
        &color_info(),
        levels,
        dst_rect,
    );
    assert!(source.is_valid());
    UploadInstance::make(caps, um, &source, context)
}

#[test]
fn upload_source_validation() {
    let (rp, counts) = shared_provider();
    let caps = MockCaps::default();
    let target = proxy(&mut rp.lock().unwrap(), 4, &rgba_info(), "t");
    let view = TextureProxyView::from_proxy(Some(target.clone()));
    let pixels = [1u8; 64];
    let level = MipLevel {
        pixels: Some(&pixels),
        row_bytes: 16,
    };
    let make = |levels: &[MipLevel<'_>], rect: IRect| {
        UploadSource::make(&caps, &view, &color_info(), &color_info(), levels, rect).is_valid()
    };
    let _ = counts;

    assert!(make(&[level], IRect::from_wh(4, 4)));
    assert!(make(&[level], IRect::from_xywh(1, 1, 2, 2)));
    // No data to upload.
    assert!(!make(&[level], IRect::new_empty()));
    // The data would not fit into the texture.
    assert!(!make(&[level], IRect::from_wh(5, 4)));
    // A texture without mipmaps takes one level.
    assert!(!make(&[level, level], IRect::from_wh(4, 4)));
    assert!(!make(&[], IRect::from_wh(4, 4)));
    // A missing level is a gap.
    assert!(!make(&[MipLevel::default()], IRect::from_wh(4, 4)));

    // Mipmapped textures take all levels or just the first.
    let mip_info = texture_info(TextureFormat::RGBA8, SampleCount::One, Mipmapped::Yes);
    let mip_target = proxy(&mut rp.lock().unwrap(), 4, &mip_info, "mip");
    let mip_view = TextureProxyView::from_proxy(Some(mip_target));
    let mip = |levels: &[MipLevel<'_>], rect: IRect| {
        UploadSource::make(&caps, &mip_view, &color_info(), &color_info(), levels, rect).is_valid()
    };
    assert!(!mip(&[level], IRect::from_wh(4, 4)));
    assert!(mip(&[level, level, level], IRect::from_wh(4, 4)));
    // With several levels the rect must cover the texture.
    assert!(!mip(&[level, level, level], IRect::from_wh(2, 2)));

    // Compressed data must be the size of the whole chain.
    let bc1 = texture_info(TextureFormat::RGB8_BC1, SampleCount::One, Mipmapped::No);
    let bc1_target = proxy(&mut rp.lock().unwrap(), 4, &bc1, "bc1");
    let data = [0u8; 8];
    let compressed = UploadSource::make_compressed(&caps, bc1_target.clone(), Some(&data));
    assert!(compressed.is_valid());
    assert_eq!(compressed.levels().len(), 1);
    assert_eq!(compressed.levels()[0].row_bytes, 8);
    assert_eq!(*compressed.dst_rect(), IRect::from_wh(4, 4));
    assert!(compressed.format_xfer_fn().is_identity());
    assert!(!UploadSource::make_compressed(&caps, bc1_target.clone(), Some(&data[..4])).is_valid());
    assert!(!UploadSource::make_compressed(&caps, bc1_target, None).is_valid());
    // Uncompressed textures are not compressed upload targets.
    assert!(!UploadSource::make_compressed(&caps, target, Some(&data)).is_valid());
}

#[test]
fn upload_task_describes_the_copy_of_every_mip_level() {
    let (rp, counts) = shared_provider();
    let caps = MockCaps::default();
    let mip_info = texture_info(TextureFormat::RGBA8, SampleCount::One, Mipmapped::Yes);
    let target = proxy(&mut rp.lock().unwrap(), 4, &mip_info, "mip");
    let mut um = UploadBufferManager::new(rp.clone(), &caps);

    let base = [1u8; 64];
    let mip1_data = [2u8; 16];
    let mip2_data = [3u8; 4];
    let levels = [
        MipLevel {
            pixels: Some(&base),
            row_bytes: 16,
        },
        MipLevel {
            pixels: Some(&mip1_data),
            row_bytes: 8,
        },
        MipLevel {
            pixels: Some(&mip2_data),
            row_bytes: 4,
        },
    ];
    let mut list = UploadList::new();
    let source = UploadSource::make(
        &caps,
        &TextureProxyView::from_proxy(Some(target.clone())),
        &color_info(),
        &color_info(),
        &levels,
        IRect::from_wh(4, 4),
    );
    assert!(list.record_upload(&caps, &mut um, &source, None));
    assert_eq!(list.size(), 1);
    let task = UploadTask::make(&mut list).unwrap();
    assert_eq!(list.size(), 0);
    assert!(UploadTask::make(&mut list).is_none());

    let mut tasks = TaskList::new();
    tasks.add(task);
    // The upload target is instantiated by prepareResources().
    assert!(!target.is_instantiated());
    assert_eq!(
        run_prepare(&mut tasks, &mut rp.lock().unwrap()),
        Status::Success
    );
    assert!(target.is_instantiated());

    let mut cb = MockCommandBuffer::default();
    assert_eq!(run_commands(&mut tasks, &mut cb), Status::Success);
    // The levels sit one after the other in the transfer buffer: 64, 16 and 4 bytes, each
    // aligned to the transfer alignment.
    assert_eq!(
        cb.calls,
        [Call::CopyBufferToTexture(vec![
            BufferTextureCopyData {
                buffer_offset: 0,
                buffer_row_bytes: 16,
                rect: IRect::from_wh(4, 4),
                mip_level: 0
            },
            BufferTextureCopyData {
                buffer_offset: 64,
                buffer_row_bytes: 8,
                rect: IRect::from_wh(2, 2),
                mip_level: 1
            },
            BufferTextureCopyData {
                buffer_offset: 80,
                buffer_row_bytes: 4,
                rect: IRect::from_wh(1, 1),
                mip_level: 2
            },
        ])]
    );

    // The pixels were converted into the transfer buffer.
    let mut um_commands = MockCommandBuffer::default();
    um.transfer_to_command_buffer(&mut um_commands);
    assert_eq!(um_commands.tracked.len(), 1);
    let _ = counts;

    // An upload to a mipmapped texture is replayed as it was recorded.
    let mut cb = MockCommandBuffer::default();
    assert_eq!(run_commands(&mut tasks, &mut cb), Status::Success);
    assert_eq!(cb.calls.len(), 1);

    // Uploads are textures written, never read.
    let mut visited = 0;
    assert!(tasks.visit_proxies(
        &mut |_| {
            visited += 1;
            true
        },
        true
    ));
    assert_eq!(visited, 0);
    assert!(tasks.visit_proxies(
        &mut |p| {
            visited += 1;
            Arc::ptr_eq(p, &target)
        },
        false
    ));
    assert_eq!(visited, 1);
}

#[test]
fn image_upload_context_discards_the_upload_after_the_first_replay() {
    let (rp, counts) = shared_provider();
    let caps = MockCaps::default();
    let target = proxy(&mut rp.lock().unwrap(), 2, &rgba_info(), "t");
    let mut um = UploadBufferManager::new(rp.clone(), &caps);
    let pixels = [9u8; 16];
    let levels = [MipLevel {
        pixels: Some(&pixels),
        row_bytes: 8,
    }];
    let upload = upload_instance_for(
        &counts,
        &mut um,
        &caps,
        &target,
        &levels,
        IRect::from_wh(2, 2),
        Some(Box::new(ImageUploadContext)),
    );
    assert!(upload.is_valid());
    let mut tasks = TaskList::new();
    tasks.add(UploadTask::make_instance(upload).unwrap());
    assert!(UploadTask::make_instance(UploadInstance::invalid()).is_none());
    assert_eq!(
        run_prepare(&mut tasks, &mut rp.lock().unwrap()),
        Status::Success
    );

    let mut cb = MockCommandBuffer::default();
    // The only upload is discarded after it ran, so the whole list is.
    assert_eq!(run_commands(&mut tasks, &mut cb), Status::Discard);
    assert_eq!(cb.calls.len(), 1);
    let mut cb = MockCommandBuffer::default();
    assert_eq!(run_commands(&mut tasks, &mut cb), Status::Discard);
    assert_eq!(cb.calls.len(), 0);
}

#[derive(Debug)]
struct Toggle(Arc<Mutex<bool>>);

impl ConditionalUploadContext for Toggle {
    fn needs_upload(
        &mut self,
        _context: &mut dyn skia_rust_gpu::graphite::context_priv::ContextPriv,
    ) -> bool {
        *self.0.lock().unwrap()
    }
}

#[test]
fn conditional_upload_context_skips_and_keeps_the_upload() {
    let (rp, counts) = shared_provider();
    let caps = MockCaps::default();
    let target = proxy(&mut rp.lock().unwrap(), 2, &rgba_info(), "t");
    let mut um = UploadBufferManager::new(rp.clone(), &caps);
    let pixels = [9u8; 16];
    let levels = [MipLevel {
        pixels: Some(&pixels),
        row_bytes: 8,
    }];
    let flag = Arc::new(Mutex::new(false));
    let upload = upload_instance_for(
        &counts,
        &mut um,
        &caps,
        &target,
        &levels,
        IRect::from_wh(2, 2),
        Some(Box::new(Toggle(flag.clone()))),
    );
    let mut tasks = TaskList::new();
    tasks.add(UploadTask::make_instance(upload).unwrap());
    assert_eq!(
        run_prepare(&mut tasks, &mut rp.lock().unwrap()),
        Status::Success
    );

    let mut cb = MockCommandBuffer::default();
    assert_eq!(run_commands(&mut tasks, &mut cb), Status::Success);
    assert_eq!(cb.calls.len(), 0);
    *flag.lock().unwrap() = true;
    assert_eq!(run_commands(&mut tasks, &mut cb), Status::Success);
    assert_eq!(cb.calls.len(), 1);
    // Still part of the replay.
    assert_eq!(run_commands(&mut tasks, &mut cb), Status::Success);
    assert_eq!(cb.calls.len(), 2);

    // A failing command buffer fails the step.
    cb.fail = true;
    assert_eq!(run_commands(&mut tasks, &mut cb), Status::Fail);
}

#[test]
fn upload_to_the_replay_target_is_translated_and_cropped() {
    let (rp, counts) = shared_provider();
    let caps = MockCaps::default();
    let target = proxy(&mut rp.lock().unwrap(), 4, &rgba_info(), "t");
    let mut um = UploadBufferManager::new(rp.clone(), &caps);
    let pixels = [9u8; 16];
    let levels = [MipLevel {
        pixels: Some(&pixels),
        row_bytes: 8,
    }];
    let upload = upload_instance_for(
        &counts,
        &mut um,
        &caps,
        &target,
        &levels,
        IRect::from_wh(2, 2),
        None,
    );
    let mut tasks = TaskList::new();
    tasks.add(UploadTask::make_instance(upload).unwrap());
    assert_eq!(
        run_prepare(&mut tasks, &mut rp.lock().unwrap()),
        Status::Success
    );

    let (mut context, _) = MockContext::new(MockCaps::default());
    let target_texture = target.ref_texture().unwrap();
    let replay = |translation: IPoint, clip: IRect| ReplayTargetData {
        target: Some(target_texture.as_arc().clone()),
        translation,
        clip,
    };

    // Translated by (-1, -1), the 2x2 copy covers (-1,-1)-(1,1) and is cropped to the texture:
    // one texel, taken from one row and one texel into the source.
    let mut cb = MockCommandBuffer::default();
    assert_eq!(
        tasks.add_commands(
            &mut context,
            &mut cb,
            &replay(IPoint::new(-1, -1), IRect::new_empty())
        ),
        Status::Success
    );
    let Call::CopyBufferToTexture(copies) = &cb.calls[0] else {
        panic!("expected a copy");
    };
    assert_eq!(copies.len(), 1);
    assert_eq!(copies[0].rect, IRect::from_wh(1, 1));
    assert_eq!(copies[0].buffer_offset, 8 + 4);
    assert_eq!(copies[0].buffer_row_bytes, 8);

    // A clip that misses the copy skips it.
    let mut cb = MockCommandBuffer::default();
    assert_eq!(
        tasks.add_commands(
            &mut context,
            &mut cb,
            &replay(IPoint::new(0, 0), IRect::from_xywh(3, 3, 1, 1))
        ),
        Status::Success
    );
    assert_eq!(cb.calls.len(), 0);

    // A translation that moves the copy off the texture skips it too.
    assert_eq!(
        tasks.add_commands(
            &mut context,
            &mut cb,
            &replay(IPoint::new(10, 10), IRect::new_empty())
        ),
        Status::Success
    );
    assert_eq!(cb.calls.len(), 0);

    // Other targets get the copy unchanged.
    assert_eq!(
        tasks.add_commands(&mut context, &mut cb, &ReplayTargetData::default()),
        Status::Success
    );
    let Call::CopyBufferToTexture(copies) = &cb.calls[0] else {
        panic!("expected a copy");
    };
    assert_eq!(copies[0].rect, IRect::from_wh(2, 2));
}

#[derive(Debug)]
struct MockGroup {
    child: Option<TaskRef>,
    prepare_ok: bool,
}

impl DispatchGroup for MockGroup {
    fn snap_child_task(&mut self) -> Option<TaskRef> {
        self.child.take()
    }

    fn prepare_resources(&mut self, _resource_provider: &mut ResourceProvider) -> bool {
        self.prepare_ok
    }
}

#[test]
fn compute_task_splits_compute_passes_at_dependent_tasks() {
    let (mut rp, _) = provider();
    let b = buffer(&mut rp, 64);
    let clear = |offset: u32| ClearBuffersTask::make(vec![BindBufferInfo::new(&b, offset, 4)]);
    let groups: Vec<Box<dyn DispatchGroup>> = vec![
        Box::new(MockGroup {
            child: None,
            prepare_ok: true,
        }),
        Box::new(MockGroup {
            child: Some(clear(8)),
            prepare_ok: true,
        }),
        Box::new(MockGroup {
            child: None,
            prepare_ok: true,
        }),
        Box::new(MockGroup {
            child: Some(clear(12)),
            prepare_ok: true,
        }),
    ];
    let mut list = TaskList::new();
    list.add(ComputeTask::make(groups));
    assert_eq!(run_prepare(&mut list, &mut rp), Status::Success);
    let mut cb = MockCommandBuffer::default();
    assert_eq!(run_commands(&mut list, &mut cb), Status::Success);
    assert_eq!(
        cb.calls,
        [
            Call::ComputePass(1),
            Call::ClearBuffer { offset: 8, size: 4 },
            Call::ComputePass(2),
            Call::ClearBuffer {
                offset: 12,
                size: 4
            },
            Call::ComputePass(1),
        ]
    );

    // A failing group preparation fails the task.
    let failing: Vec<Box<dyn DispatchGroup>> = vec![Box::new(MockGroup {
        child: None,
        prepare_ok: false,
    })];
    let mut list = TaskList::new();
    list.add(ComputeTask::make(failing));
    assert_eq!(run_prepare(&mut list, &mut rp), Status::Fail);

    // A failing compute pass fails the task.
    let groups: Vec<Box<dyn DispatchGroup>> = vec![Box::new(MockGroup {
        child: None,
        prepare_ok: true,
    })];
    let mut list = TaskList::new();
    list.add(ComputeTask::make(groups));
    let mut cb = MockCommandBuffer {
        fail: true,
        ..MockCommandBuffer::default()
    };
    assert_eq!(run_commands(&mut list, &mut cb), Status::Fail);

    // No groups: nothing to replay.
    let mut list = TaskList::new();
    list.add(ComputeTask::make(Vec::new()));
    let mut cb = MockCommandBuffer::default();
    assert_eq!(run_commands(&mut list, &mut cb), Status::Discard);
}

#[derive(Debug)]
struct MockPass {
    bounds: IRect,
    sampled: Vec<Arc<TextureProxy>>,
    prepared: Arc<Mutex<u32>>,
    prepare_ok: bool,
}

impl MockPass {
    fn boxed(sampled: Vec<Arc<TextureProxy>>) -> (Box<dyn DrawPass>, Arc<Mutex<u32>>) {
        let prepared = Arc::new(Mutex::new(0));
        (
            Box::new(MockPass {
                bounds: IRect::from_wh(8, 8),
                sampled,
                prepared: prepared.clone(),
                prepare_ok: true,
            }),
            prepared,
        )
    }
}

impl DrawPass for MockPass {
    fn bounds(&self) -> IRect {
        self.bounds
    }

    fn prepare_resources(
        &mut self,
        _resource_provider: &mut ResourceProvider,
        _runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
        _render_pass_desc: &RenderPassDesc,
    ) -> bool {
        *self.prepared.lock().unwrap() += 1;
        self.prepare_ok
    }

    fn pipelines(&self) -> Vec<Option<Arc<dyn GraphicsPipeline>>> {
        vec![None]
    }

    fn sampled_textures(&self) -> &[Arc<TextureProxy>] {
        &self.sampled
    }

    fn storage_fallback_texture(&self) -> Option<&Arc<TextureProxy>> {
        None
    }
}

fn simple_desc() -> RenderPassDesc {
    RenderPassDesc {
        color_attachment: AttachmentDesc {
            format: TextureFormat::RGBA8,
            load_op: LoadOp::Clear,
            store_op: StoreOp::Store,
            sample_count: SampleCount::One,
        },
        ..RenderPassDesc::default()
    }
}

#[test]
#[allow(clippy::too_many_lines)] // one long scenario, as in the C++ test
fn render_pass_task_records_a_render_pass() {
    let (mut rp, _) = provider();
    let target = proxy(&mut rp, 8, &rgba_info(), "target");
    let (pass, prepared) = MockPass::boxed(Vec::new());
    let task = RenderPassTask::make(
        vec![pass],
        &simple_desc(),
        Some(target.clone()),
        None,
        IRect::new_empty(),
    )
    .unwrap();
    assert!(
        RenderPassTask::make(
            vec![MockPass::boxed(Vec::new()).0],
            &simple_desc(),
            None,
            None,
            IRect::new_empty()
        )
        .is_none()
    );
    let mut list = TaskList::new();
    list.add(task);
    assert_eq!(run_prepare(&mut list, &mut rp), Status::Success);
    assert!(target.is_instantiated());
    assert_eq!(*prepared.lock().unwrap(), 1);

    let (mut context, _) = MockContext::new(MockCaps::default());
    let mut cb = MockCommandBuffer::default();
    assert_eq!(
        list.add_commands(&mut context, &mut cb, &ReplayTargetData::default()),
        Status::Success
    );
    assert_eq!(
        cb.calls,
        [
            Call::ReplayClip {
                translation: IPoint::new(0, 0),
                clip: IRect::new_empty(),
                bounds: IRect::from_wh(8, 8)
            },
            Call::RenderPass {
                has_resolve: false,
                has_depth_stencil: false,
                has_dst_copy: false,
                resolve_offset: IPoint::new(0, 0),
                viewport: ISize::new(8, 8),
                num_passes: 1
            },
        ]
    );

    // Drawing to the replay target applies the replay translation and clip.
    let replay = ReplayTargetData {
        target: Some(target.ref_texture().unwrap().as_arc().clone()),
        translation: IPoint::new(3, 4),
        clip: IRect::from_wh(2, 2),
    };
    let mut cb = MockCommandBuffer::default();
    assert_eq!(
        list.add_commands(&mut context, &mut cb, &replay),
        Status::Success
    );
    assert_eq!(
        cb.calls[0],
        Call::ReplayClip {
            translation: IPoint::new(3, 4),
            clip: IRect::from_wh(2, 2),
            bounds: IRect::from_wh(8, 8)
        }
    );

    // A clip that misses the target skips the pass without failing.
    let mut cb = MockCommandBuffer {
        reject_clip: true,
        ..MockCommandBuffer::default()
    };
    assert_eq!(
        list.add_commands(&mut context, &mut cb, &replay),
        Status::Success
    );
    assert_eq!(cb.calls.len(), 1);

    // A failing pass fails the task.
    let mut cb = MockCommandBuffer {
        fail: true,
        ..MockCommandBuffer::default()
    };
    assert_eq!(
        list.add_commands(&mut context, &mut cb, &replay),
        Status::Fail
    );

    // The visitors see the pass's pipelines and the target.
    let mut pipelines = 0;
    assert!(list.visit_pipelines(&mut |pipeline| {
        assert!(pipeline.is_none());
        pipelines += 1;
        true
    }));
    assert_eq!(pipelines, 1);
    let mut visited = 0;
    assert!(list.visit_proxies(
        &mut |_| {
            visited += 1;
            true
        },
        false
    ));
    assert_eq!(visited, 1);
    assert!(list.visit_proxies(&mut |_| unreachable!("reads only"), true));
}

#[test]
fn render_pass_task_fails_if_the_pass_cannot_be_prepared() {
    let (mut rp, _) = provider();
    let target = proxy(&mut rp, 8, &rgba_info(), "target");
    let pass: Box<dyn DrawPass> = Box::new(MockPass {
        bounds: IRect::from_wh(8, 8),
        sampled: Vec::new(),
        prepared: Arc::default(),
        prepare_ok: false,
    });
    let mut list = TaskList::new();
    list.add(
        RenderPassTask::make(
            vec![pass],
            &simple_desc(),
            Some(target),
            None,
            IRect::new_empty(),
        )
        .unwrap(),
    );
    assert_eq!(run_prepare(&mut list, &mut rp), Status::Fail);
}

#[test]
fn render_pass_task_makes_msaa_and_depth_stencil_attachments() {
    let caps = MockCaps::default();
    let (mut context, counts) = MockContext::new(caps.clone());
    let mut rp = context.resource_provider.lock().unwrap();
    let target = proxy(&mut rp, 8, &rgba_info(), "target");
    let desc = RenderPassDesc::make(
        &caps,
        target.texture_info(),
        LoadOp::Clear,
        StoreOp::Store,
        DepthStencilFlags::DepthStencil,
        [0.0; 4],
        /*requires_msaa=*/ true,
        Swizzle::rgba(),
        DstReadStrategy::NoneRequired,
    );
    assert_eq!(desc.sample_count, SampleCount::Four);
    assert_eq!(desc.color_attachment.sample_count, SampleCount::Four);
    assert_eq!(desc.color_resolve_attachment.format, TextureFormat::RGBA8);
    assert_eq!(desc.color_resolve_attachment.sample_count, SampleCount::One);
    assert_eq!(desc.color_resolve_attachment.load_op, LoadOp::Discard);
    assert_eq!(desc.depth_stencil_attachment.format, TextureFormat::D24_S8);

    let (pass, _) = MockPass::boxed(Vec::new());
    let mut list = TaskList::new();
    list.add(
        RenderPassTask::make(vec![pass], &desc, Some(target), None, IRect::new_empty()).unwrap(),
    );
    assert_eq!(run_prepare(&mut list, &mut rp), Status::Success);
    drop(rp);

    let textures_before = counts.textures.load(std::sync::atomic::Ordering::Relaxed);
    let mut cb = MockCommandBuffer::default();
    assert_eq!(
        list.add_commands(&mut context, &mut cb, &ReplayTargetData::default()),
        Status::Success
    );
    // The target, then the MSAA color and depth/stencil attachments were made on demand.
    assert_eq!(
        counts.textures.load(std::sync::atomic::Ordering::Relaxed),
        textures_before + 2
    );
    assert!(matches!(
        cb.calls[1],
        Call::RenderPass {
            has_resolve: true,
            has_depth_stencil: true,
            ..
        }
    ));

    // The attachments are shareable: another pass reuses them.
    let mut cb = MockCommandBuffer::default();
    assert_eq!(
        list.add_commands(&mut context, &mut cb, &ReplayTargetData::default()),
        Status::Success
    );
    assert_eq!(
        counts.textures.load(std::sync::atomic::Ordering::Relaxed),
        textures_before + 2
    );
}

#[test]
fn render_pass_task_shrinks_msaa_attachments_to_the_render_area() {
    let caps = MockCaps {
        attachment_size_policy: AttachmentSizePolicy::MsaaRenderArea,
        ..MockCaps::default()
    };
    let (mut context, _) = MockContext::new(caps.clone());
    let mut rp = context.resource_provider.lock().unwrap();
    let target = proxy(&mut rp, 8, &rgba_info(), "target");
    let desc = RenderPassDesc::make(
        &caps,
        target.texture_info(),
        LoadOp::Load,
        StoreOp::Store,
        DepthStencilFlags::None,
        [0.0; 4],
        true,
        Swizzle::rgba(),
        DstReadStrategy::NoneRequired,
    );
    assert_eq!(desc.color_resolve_attachment.load_op, LoadOp::Load);
    let pass: Box<dyn DrawPass> = Box::new(MockPass {
        bounds: IRect::from_xywh(2, 2, 4, 4),
        sampled: Vec::new(),
        prepared: Arc::default(),
        prepare_ok: true,
    });
    let mut list = TaskList::new();
    list.add(
        RenderPassTask::make(vec![pass], &desc, Some(target), None, IRect::new_empty()).unwrap(),
    );
    assert_eq!(run_prepare(&mut list, &mut rp), Status::Success);
    drop(rp);

    let mut cb = MockCommandBuffer::default();
    assert_eq!(
        list.add_commands(&mut context, &mut cb, &ReplayTargetData::default()),
        Status::Success
    );
    // The MSAA attachment only covers the draws (at least 16x16 once approximated), and the
    // resolve is offset to where the draws are.
    assert_eq!(
        cb.calls[0],
        Call::ReplayClip {
            translation: IPoint::new(-2, -2),
            clip: IRect::new_empty(),
            bounds: IRect::from_wh(16, 16)
        }
    );
    assert!(matches!(
        cb.calls[1],
        Call::RenderPass {
            has_resolve: true,
            resolve_offset: IPoint { x: 2, y: 2 },
            ..
        }
    ));
}

// A DrawTask whose target is read by a later pass of its parent returns the target's scratch
// texture to the scratch manager once that pass consumed it.
#[test]
fn draw_task_returns_scratch_textures_after_the_consuming_pass() {
    let (mut rp, counts) = provider();
    let info = rgba_info();
    let scratch_target = proxy(&mut rp, 8, &info, "scratch");
    let final_target = proxy(&mut rp, 8, &info, "final");

    // The scratch device's own draw.
    let (producer_pass, _) = MockPass::boxed(Vec::new());
    let producer = RenderPassTask::make(
        vec![producer_pass],
        &simple_desc(),
        Some(scratch_target.clone()),
        None,
        IRect::new_empty(),
    )
    .unwrap();
    let mut scratch_draw = DrawTask::new(scratch_target.clone());
    scratch_draw.add_task(producer);
    let scratch_draw = scratch_draw.into_ref();

    // The pass that samples it.
    let (consumer_pass, _) = MockPass::boxed(vec![scratch_target.clone()]);
    let consumer = RenderPassTask::make(
        vec![consumer_pass],
        &simple_desc(),
        Some(final_target.clone()),
        None,
        IRect::new_empty(),
    )
    .unwrap();
    let mut final_draw = DrawTask::new(final_target.clone());
    final_draw.add_task(scratch_draw.clone());
    final_draw.add_task(consumer);
    let mut list = TaskList::new();
    list.add(final_draw.into_ref());

    // The scratch device has a pending read, so its texture is a scratch texture.
    let mut read_counts = ProxyReadCountMap::new();
    read_counts.increment(&scratch_target);
    let mut scratch = ScratchResourceManager::new(read_counts);
    assert_eq!(scratch.pending_read_count(&scratch_target), 1);
    assert_eq!(
        list.prepare_resources(&mut rp, &mut scratch, None),
        Status::Success
    );
    assert!(scratch_target.is_instantiated());
    assert!(final_target.is_instantiated());
    // The consuming pass reclaimed the scratch texture, so it is available again.
    assert_eq!(scratch.pending_read_count(&scratch_target), 0);
    let scratch_texture = scratch_target.ref_texture().unwrap();
    let reused = scratch
        .get_scratch_texture(&mut rp, ISize::new(8, 8), &info, "again")
        .unwrap();
    assert!(ResourceRef::ptr_eq(&scratch_texture, &reused));
    assert_eq!(
        counts.textures.load(std::sync::atomic::Ordering::Relaxed),
        2
    );
    // The manager asserts that every scratch texture was returned when it goes away.
    scratch.return_texture(&reused);

    // Encountering the scratch device's task again discards it, leaving the first
    // encounter as the only one that records commands.
    let mut again = TaskList::new();
    again.add(scratch_draw);
    let mut scratch = ScratchResourceManager::new({
        let mut counts = ProxyReadCountMap::new();
        counts.increment(&scratch_target);
        counts
    });
    assert_eq!(
        again.prepare_resources(&mut rp, &mut scratch, None),
        Status::Discard
    );
    // The manager asserts that every pending read was consumed when it goes away.
    assert!(scratch.remove_pending_read(&scratch_target));
}

#[test]
fn replay_target_data_compares_textures_by_identity() {
    let (mut rp, _) = provider();
    let a = proxy(&mut rp, 2, &rgba_info(), "a");
    let b = proxy(&mut rp, 2, &rgba_info(), "b");
    assert!(a.instantiate(&mut rp));
    assert!(b.instantiate(&mut rp));
    let texture_a: ResourceRef<Texture> = a.ref_texture().unwrap();
    let texture_b: ResourceRef<Texture> = b.ref_texture().unwrap();
    let data = ReplayTargetData {
        target: Some(texture_a.as_arc().clone()),
        ..ReplayTargetData::default()
    };
    assert!(data.is_target(Some(texture_a.as_arc())));
    assert!(!data.is_target(Some(texture_b.as_arc())));
    assert!(!data.is_target(None));
    assert!(ReplayTargetData::default().is_target(None));
}
