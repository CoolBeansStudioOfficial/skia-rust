// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/GraphitePromiseImageTest.cpp (chrome/m156)

#![cfg(test)]

use std::sync::{Arc, Mutex, PoisonError};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_filters::linear_to_srgb_gamma;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image as CoreImage;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::size::ISize;
use skia_rust_gpu::gpu::gpu_types::{Mipmapped, Origin, Protected, Renderable};
use skia_rust_gpu::graphite::backend_texture::BackendTexture;
use skia_rust_gpu::graphite::context_priv::{ContextPriv, SharedResourceProvider};
use skia_rust_gpu::graphite::graphite_types::{
    InsertRecordingInfo, InsertStatus, SubmitInfo, SyncToCpu, Volatile,
};
use skia_rust_gpu::graphite::image_factories::promise_texture_from;
use skia_rust_gpu::graphite::recorder::Recorder;
use skia_rust_gpu::graphite::recording::Recording;
use skia_rust_gpu::graphite::surface_graphite::Surface;
use skia_rust_gpu::graphite::texture_utils::PromiseTextureFulfillProc;
use skia_rust_gpu::graphite::wgpu::WgpuContext;

use crate::{Reporter, def_graphite_adapter_test, reporter_assert};

/// `PromiseTextureChecker`: counts the fulfill, image release and texture release calls of the
/// promise image under test.
// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L17-L58 (chrome/m156)
#[derive(Default)]
struct PromiseTextureChecker {
    has_two_backend_textures: bool,
    backend_textures: [BackendTexture; 2],
    fulfill_count: i32,
    image_release_count: i32,
    texture_release_counts: [i32; 2],
}

type Checker = Arc<Mutex<PromiseTextureChecker>>;

impl PromiseTextureChecker {
    // Port of: tests/graphite/GraphitePromiseImageTest.cpp#L30-L40 (chrome/m156)
    fn total_release_count(&self) -> i32 {
        self.texture_release_counts[0] + self.texture_release_counts[1]
    }
}

fn lock(checker: &Checker) -> std::sync::MutexGuard<'_, PromiseTextureChecker> {
    checker.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `PromiseTextureChecker::Fulfill`: returns the backend texture to use for this fulfill, and
/// the texture release callback that counts its release.
// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L43-L56 (chrome/m156)
fn fulfill_proc(checker: &Checker) -> PromiseTextureFulfillProc {
    let checker = Arc::clone(checker);
    Arc::new(move || {
        let (backend_texture, which) = {
            let mut c = lock(&checker);
            c.fulfill_count += 1;
            let which = if c.has_two_backend_textures {
                usize::try_from(c.fulfill_count % 2).unwrap_or(0)
            } else {
                0
            };
            (c.backend_textures[which].clone(), which)
        };
        let release_checker = Arc::clone(&checker);
        let texture_release: Box<dyn FnOnce() + Send> = Box::new(move || {
            lock(&release_checker).texture_release_counts[which] += 1;
        });
        (backend_texture, Some(texture_release))
    })
}

/// `PromiseTextureChecker::ImageRelease`.
// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L59-L63 (chrome/m156)
fn image_release(checker: &Checker) -> Box<dyn FnOnce() + Send> {
    let checker = Arc::clone(checker);
    Box::new(move || {
        lock(&checker).image_release_count += 1;
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReleaseBalanceExpectation {
    Balanced,
    /// fulfill calls ahead of release calls by 1
    OffByOne,
    /// fulfill calls ahead of release calls by 2
    OffByTwo,
    /// 'n' fulfill calls, 0 release calls
    FulfillsOnly,
}

// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L93-L125 (chrome/m156)
fn check_fulfill_and_release_cnts(
    reporter: &mut Reporter,
    checker: &Checker,
    expected_fulfill_cnt: i32,
    release_balance_expectation: ReleaseBalanceExpectation,
) {
    let c = lock(checker);
    reporter_assert!(reporter, c.fulfill_count == expected_fulfill_cnt);
    if expected_fulfill_cnt == 0 {
        // Release should only ever be called after Fulfill.
        reporter_assert!(reporter, c.image_release_count == 0);
        reporter_assert!(reporter, c.total_release_count() == 0);
        return;
    }

    let release_diff = c.fulfill_count - c.total_release_count();
    match release_balance_expectation {
        ReleaseBalanceExpectation::Balanced => {
            reporter_assert!(reporter, release_diff == 0);
        }
        ReleaseBalanceExpectation::OffByOne => {
            reporter_assert!(reporter, release_diff == 1);
        }
        ReleaseBalanceExpectation::OffByTwo => {
            reporter_assert!(reporter, release_diff == 2);
        }
        ReleaseBalanceExpectation::FulfillsOnly => {
            reporter_assert!(reporter, c.total_release_count() == 0);
        }
    }
}

// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L127-L131 (chrome/m156)
fn check_unfulfilled(reporter: &mut Reporter, checker: &Checker) {
    check_fulfill_and_release_cnts(reporter, checker, 0, ReleaseBalanceExpectation::Balanced);
}

// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L133-L138 (chrome/m156)
fn check_fulfilled_ahead_by_one(reporter: &mut Reporter, checker: &Checker, expected: i32) {
    check_fulfill_and_release_cnts(
        reporter,
        checker,
        expected,
        ReleaseBalanceExpectation::OffByOne,
    );
}

// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L140-L145 (chrome/m156)
fn check_fulfilled_ahead_by_two(reporter: &mut Reporter, checker: &Checker, expected: i32) {
    check_fulfill_and_release_cnts(
        reporter,
        checker,
        expected,
        ReleaseBalanceExpectation::OffByTwo,
    );
}

// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L147-L152 (chrome/m156)
fn check_all_done(reporter: &mut Reporter, checker: &Checker, expected: i32) {
    check_fulfill_and_release_cnts(
        reporter,
        checker,
        expected,
        ReleaseBalanceExpectation::Balanced,
    );
}

// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L154-L158 (chrome/m156)
fn check_fulfills_only(reporter: &mut Reporter, checker: &Checker, expected: i32) {
    check_fulfill_and_release_cnts(
        reporter,
        checker,
        expected,
        ReleaseBalanceExpectation::FulfillsOnly,
    );
}

/// `TestCtx`: the recorder, the promise image, its surface and the backend textures it is
/// fulfilled with. The backend textures are deleted on drop.
// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L160-L176 (chrome/m156)
struct TestCtx {
    provider: SharedResourceProvider,
    recorder: Option<Recorder>,
    backend_textures: [BackendTexture; 2],
    checker: Checker,
    img: Option<CoreImage>,
    surface: Option<Surface>,
}

impl Drop for TestCtx {
    // Port of: tests/graphite/GraphitePromiseImageTest.cpp#L163-L169 (chrome/m156)
    fn drop(&mut self) {
        for texture in &self.backend_textures {
            if texture.is_valid() {
                self.provider
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .delete_backend_texture(texture);
            }
        }
    }
}

// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L178-L240 (chrome/m156)
fn setup_test_context(
    context: &WgpuContext,
    reporter: &mut Reporter,
    dimensions: ISize,
    is_volatile: Volatile,
    invalid_backend_tex: bool,
) -> TestCtx {
    let caps = ContextPriv::caps(context);
    let mut recorder = context.make_recorder(None);
    let is_protected = if caps.protected_support() {
        Protected::Yes
    } else {
        Protected::No
    };
    let texture_info = caps.get_default_sampled_texture_info(
        ColorType::RGBA8888,
        Mipmapped::No,
        is_protected,
        Renderable::Yes,
    );

    let mut backend_textures = [BackendTexture::new(), BackendTexture::new()];
    if invalid_backend_tex {
        // Having invalid backend textures will invalidate all the fulfill calls
        reporter_assert!(reporter, !backend_textures[0].is_valid());
        reporter_assert!(reporter, !backend_textures[1].is_valid());
    } else {
        backend_textures[0] = recorder.create_backend_texture(dimensions, &texture_info);
        reporter_assert!(reporter, backend_textures[0].is_valid());
        if is_volatile == Volatile::Yes {
            backend_textures[1] = recorder.create_backend_texture(dimensions, &texture_info);
            reporter_assert!(reporter, backend_textures[1].is_valid());
        }
    }

    let checker: Checker = Arc::new(Mutex::new(PromiseTextureChecker {
        has_two_backend_textures: is_volatile == Volatile::Yes,
        backend_textures: backend_textures.clone(),
        ..Default::default()
    }));

    let ii = ImageInfo::new(dimensions, ColorType::RGBA8888, AlphaType::Premul, None);
    let img = promise_texture_from(
        &recorder,
        dimensions,
        &texture_info,
        ii.color_info(),
        Origin::TopLeft,
        is_volatile,
        fulfill_proc(&checker),
        Some(image_release(&checker)),
    );
    let surface = Surface::render_target(&recorder, &ii, Mipmapped::No, None, "");

    TestCtx {
        provider: ContextPriv::resource_provider(context).clone(),
        recorder: Some(recorder),
        backend_textures,
        checker,
        img,
        surface,
    }
}

/// `testContext.fRecorder->snap()`.
fn snap(test_ctx: &mut TestCtx) -> Recording {
    test_ctx
        .recorder
        .as_mut()
        .expect("the test context has a recorder")
        .snap()
        .expect("snap makes a recording")
}

/// `testContext.fSurface->getCanvas()`.
fn canvas_of(test_ctx: &TestCtx) -> &skia_rust_core::canvas::Canvas {
    test_ctx
        .surface
        .as_ref()
        .expect("the test context has a surface")
        .canvas()
}

/// `context->insertRecording({ recording.get() })`.
fn insert_recording(context: &mut WgpuContext, recording: &mut Recording) -> bool {
    context.insert_recording(InsertRecordingInfo {
        recording: Some(recording),
        ..Default::default()
    }) == InsertStatus::Success
}

/// `GraphiteTestContext::syncedSubmit(context)`.
fn synced_submit(context: &mut WgpuContext) {
    let sync = if ContextPriv::caps(context).allow_cpu_sync() {
        SyncToCpu::Yes
    } else {
        SyncToCpu::No
    };
    // The submit result is ignored, as in the C++ helper.
    let _ = context.submit(SubmitInfo {
        sync,
        ..Default::default()
    });
    if sync == SyncToCpu::No {
        while context.has_unfinished_gpu_work() {
            context.shared_context().tick();
            context.check_async_work_completion();
        }
    }
}

// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L242-L326 (chrome/m156)
def_graphite_adapter_test!(NonVolatileGraphitePromiseImageTest, |reporter, context| {
    let dimensions = ISize::new(16, 16);
    let mut test_ctx = setup_test_context(context, reporter, dimensions, Volatile::No, false);
    let checker = Arc::clone(&test_ctx.checker);
    {
        let img = test_ctx.img.clone().expect("the image");
        canvas_of(&test_ctx).draw_image(&img, (0.0, 0.0), None);
        check_unfulfilled(reporter, &checker);
        let mut recording = snap(&mut test_ctx);
        check_unfulfilled(reporter, &checker); // NVPIs not fulfilled at snap
        reporter_assert!(reporter, insert_recording(context, &mut recording));
        check_fulfilled_ahead_by_one(reporter, &checker, 1); // NVPIs fulfilled at insert
    }
    let _ = context.submit(SubmitInfo::default());
    // testContext.fImg still has a ref so we should not have called TextureRelease.
    check_fulfilled_ahead_by_one(reporter, &checker, 1);
    synced_submit(context);
    check_fulfilled_ahead_by_one(reporter, &checker, 1);
    // Test that more draws and insertions don't refulfill the NVPI
    {
        let img = test_ctx.img.clone().expect("the image");
        canvas_of(&test_ctx).draw_image(&img, (0.0, 0.0), None);
        canvas_of(&test_ctx).draw_image(&img, (0.0, 0.0), None);
        let mut recording = snap(&mut test_ctx);
        check_fulfilled_ahead_by_one(reporter, &checker, 1); // No new fulfill
        reporter_assert!(reporter, insert_recording(context, &mut recording));
        // testContext.fImg should still be fulfilled from the first time we inserted a Recording.
        check_fulfilled_ahead_by_one(reporter, &checker, 1);
    }
    synced_submit(context);
    check_fulfilled_ahead_by_one(reporter, &checker, 1);
    // Test that dropping the SkImage's ref doesn't change anything
    {
        let img = test_ctx.img.take().expect("the image");
        canvas_of(&test_ctx).draw_image(&img, (0.0, 0.0), None);
        drop(img);
        let mut recording = snap(&mut test_ctx);
        check_fulfilled_ahead_by_one(reporter, &checker, 1);
        reporter_assert!(reporter, insert_recording(context, &mut recording));
        check_fulfilled_ahead_by_one(reporter, &checker, 1);
    }
    // fImg's proxy is reffed by the recording so, despite fImg being reset earlier,
    // the imageRelease callback doesn't occur until the recording is deleted.
    reporter_assert!(reporter, lock(&checker).image_release_count == 1);
    // testContext.fImg no longer holds a ref but the last recording is still not submitted.
    check_fulfilled_ahead_by_one(reporter, &checker, 1);
    synced_submit(context);
    // Now TextureRelease should definitely have been called.
    check_all_done(reporter, &checker, 1);
});

// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L328-L400 (chrome/m156)
def_graphite_adapter_test!(
    NonVolatileGraphitePromiseImageFulfillFailureTest,
    |reporter, context| {
        let dimensions = ISize::new(16, 16);
        let mut test_ctx = setup_test_context(context, reporter, dimensions, Volatile::No, true);
        let checker = Arc::clone(&test_ctx.checker);
        // Draw the image a few different ways.
        {
            let img = test_ctx.img.clone().expect("the image");
            canvas_of(&test_ctx).draw_image(&img, (0.0, 0.0), None);
            check_unfulfilled(reporter, &checker);
            let mut recording = snap(&mut test_ctx);
            check_unfulfilled(reporter, &checker);
            reporter_assert!(reporter, !insert_recording(context, &mut recording));
            check_fulfilled_ahead_by_one(reporter, &checker, 1);
            // Test that reinserting gives uninstantiated PromiseImages a second chance
            reporter_assert!(reporter, !insert_recording(context, &mut recording));
            check_fulfills_only(reporter, &checker, 2);
        }
        {
            let img = test_ctx.img.clone().expect("the image");
            let mut paint = Paint::default();
            paint.set_color_filter(linear_to_srgb_gamma());
            canvas_of(&test_ctx).draw_image_with_sampling_options(
                &img,
                (0.0, 0.0),
                SamplingOptions::default(),
                Some(&paint),
            );
            let mut recording = snap(&mut test_ctx);
            check_fulfills_only(reporter, &checker, 2);
            reporter_assert!(reporter, !insert_recording(context, &mut recording));
            check_fulfills_only(reporter, &checker, 3);
        }
        {
            let img = test_ctx.img.clone().expect("the image");
            let shader = img.to_shader(None, SamplingOptions::default(), None);
            reporter_assert!(reporter, shader.is_some());
            let mut paint = Paint::default();
            paint.set_shader(shader);
            canvas_of(&test_ctx).draw_rect(Rect::from_wh(1.0, 1.0), &paint);
            let mut recording = snap(&mut test_ctx);
            check_fulfills_only(reporter, &checker, 3);
            reporter_assert!(reporter, !insert_recording(context, &mut recording));
            check_fulfills_only(reporter, &checker, 4);
        }
        test_ctx.surface = None;
        test_ctx.img = None;
        // Despite fulfill failing 4x, the imageRelease callback still fires
        reporter_assert!(reporter, lock(&checker).image_release_count == 1);
        synced_submit(context);
        // fulfill should've been called 4x while release should never have been called
        check_fulfills_only(reporter, &checker, 4);
    }
);

// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L402-L422 (chrome/m156)
def_graphite_adapter_test!(
    NonVolatileGraphitePromiseImageCreationFailureTest,
    |reporter, context| {
        // Note: these dimensions are invalid and will cause MakeGraphitePromiseTexture to fail
        let dimensions = ISize::new(0, 0);
        let test_ctx = setup_test_context(context, reporter, dimensions, Volatile::No, true);
        let checker = Arc::clone(&test_ctx.checker);
        reporter_assert!(reporter, test_ctx.img.is_none());
        // Despite MakeGraphitePromiseTexture failing, ImageRelease is called
        reporter_assert!(reporter, lock(&checker).fulfill_count == 0);
        reporter_assert!(reporter, lock(&checker).image_release_count == 1);
        reporter_assert!(reporter, lock(&checker).total_release_count() == 0);
    }
);

// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L424-L517 (chrome/m156)
def_graphite_adapter_test!(VolatileGraphitePromiseImageTest, |reporter, context| {
    let dimensions = ISize::new(16, 16);
    let mut test_ctx = setup_test_context(context, reporter, dimensions, Volatile::Yes, false);
    let checker = Arc::clone(&test_ctx.checker);
    {
        let img = test_ctx.img.clone().expect("the image");
        canvas_of(&test_ctx).draw_image(&img, (0.0, 0.0), None);
        check_unfulfilled(reporter, &checker);
        let mut recording = snap(&mut test_ctx);
        // Nothing happens at snap time for VPIs
        check_unfulfilled(reporter, &checker);
        reporter_assert!(reporter, insert_recording(context, &mut recording));
        check_fulfilled_ahead_by_one(reporter, &checker, 1); // VPIs fulfilled on insert
        // Test that multiple insertions will clobber prior fulfills
        reporter_assert!(reporter, insert_recording(context, &mut recording));
        check_fulfilled_ahead_by_two(reporter, &checker, 2);
    }
    synced_submit(context);
    check_all_done(reporter, &checker, 2);
    {
        let img = test_ctx.img.clone().expect("the image");
        canvas_of(&test_ctx).draw_image(&img, (0.0, 0.0), None);
        canvas_of(&test_ctx).draw_image(&img, (0.0, 0.0), None);
        let mut recording = snap(&mut test_ctx);
        // Nothing happens at snap time for volatile images
        check_all_done(reporter, &checker, 2);
        reporter_assert!(reporter, insert_recording(context, &mut recording));
        check_fulfilled_ahead_by_one(reporter, &checker, 3);
        reporter_assert!(reporter, insert_recording(context, &mut recording));
        check_fulfilled_ahead_by_two(reporter, &checker, 4);
    }
    synced_submit(context);
    check_all_done(reporter, &checker, 4);
    {
        let img = test_ctx.img.take().expect("the image");
        canvas_of(&test_ctx).draw_image(&img, (0.0, 0.0), None);
        drop(img);
        let mut recording = snap(&mut test_ctx);
        // Nothing happens at snap time for volatile images
        check_all_done(reporter, &checker, 4);
        reporter_assert!(reporter, insert_recording(context, &mut recording));
        check_fulfilled_ahead_by_one(reporter, &checker, 5);
        reporter_assert!(reporter, insert_recording(context, &mut recording));
        check_fulfilled_ahead_by_two(reporter, &checker, 6);
    }
    // testContext.fImg no longer holds a ref but the last recordings are still not submitted.
    check_fulfilled_ahead_by_two(reporter, &checker, 6);
    synced_submit(context);
    // Now all Releases should definitely have been called.
    check_all_done(reporter, &checker, 6);
});

// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L519-L590 (chrome/m156)
def_graphite_adapter_test!(
    VolatileGraphitePromiseImageFulfillFailureTest,
    |reporter, context| {
        let dimensions = ISize::new(16, 16);
        let mut test_ctx = setup_test_context(context, reporter, dimensions, Volatile::Yes, true);
        let checker = Arc::clone(&test_ctx.checker);
        {
            let img = test_ctx.img.clone().expect("the image");
            canvas_of(&test_ctx).draw_image(&img, (0.0, 0.0), None);
            check_unfulfilled(reporter, &checker);
            let mut recording = snap(&mut test_ctx);
            check_unfulfilled(reporter, &checker);
            reporter_assert!(reporter, !insert_recording(context, &mut recording));
            check_fulfills_only(reporter, &checker, 1);
            reporter_assert!(reporter, !insert_recording(context, &mut recording));
            check_fulfills_only(reporter, &checker, 2);
        }
        {
            let img = test_ctx.img.clone().expect("the image");
            let mut paint = Paint::default();
            paint.set_color_filter(linear_to_srgb_gamma());
            canvas_of(&test_ctx).draw_image_with_sampling_options(
                &img,
                (0.0, 0.0),
                SamplingOptions::default(),
                Some(&paint),
            );
            let mut recording = snap(&mut test_ctx);
            check_fulfills_only(reporter, &checker, 2);
            reporter_assert!(reporter, !insert_recording(context, &mut recording));
            check_fulfills_only(reporter, &checker, 3);
            reporter_assert!(reporter, !insert_recording(context, &mut recording));
            check_fulfills_only(reporter, &checker, 4);
        }
        {
            let img = test_ctx.img.clone().expect("the image");
            let shader = img.to_shader(None, SamplingOptions::default(), None);
            reporter_assert!(reporter, shader.is_some());
            let mut paint = Paint::default();
            paint.set_shader(shader);
            canvas_of(&test_ctx).draw_rect(Rect::from_wh(1.0, 1.0), &paint);
            let mut recording = snap(&mut test_ctx);
            check_fulfills_only(reporter, &checker, 4);
            reporter_assert!(reporter, !insert_recording(context, &mut recording));
            check_fulfills_only(reporter, &checker, 5);
            reporter_assert!(reporter, !insert_recording(context, &mut recording));
            check_fulfills_only(reporter, &checker, 6);
        }
        test_ctx.surface = None;
        test_ctx.img = None;
        synced_submit(context);
        check_fulfills_only(reporter, &checker, 6);
    }
);

// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L593-L627 (chrome/m156)
def_graphite_adapter_test!(GraphitePromiseImageRecorderLoss, |reporter, context| {
    let dimensions = ISize::new(16, 16);
    for is_volatile in [Volatile::No, Volatile::Yes] {
        let mut test_ctx = setup_test_context(context, reporter, dimensions, is_volatile, false);
        let checker = Arc::clone(&test_ctx.checker);
        let img = test_ctx.img.clone().expect("the image");
        canvas_of(&test_ctx).draw_image(&img, (0.0, 0.0), None);
        check_unfulfilled(reporter, &checker);
        let mut recording = snap(&mut test_ctx);
        check_unfulfilled(reporter, &checker);
        test_ctx.recorder = None; // Recorder drop
        reporter_assert!(reporter, insert_recording(context, &mut recording));
        check_fulfills_only(reporter, &checker, 1);
        synced_submit(context);
        test_ctx.surface = None;
        test_ctx.img = None;
        drop(img);
        drop(recording);
        check_all_done(reporter, &checker, 1);
    }
});

// Port of: tests/graphite/GraphitePromiseImageTest.cpp#L631-L701 (chrome/m156)
def_graphite_adapter_test!(GraphitePromiseImageMultipleImgUses, |reporter, context| {
    const NUM_RECORDINGS: usize = 3;
    let dimensions = ISize::new(16, 16);
    for is_volatile in [Volatile::No, Volatile::Yes] {
        let expected_volatile = i32::from(is_volatile == Volatile::Yes);
        let expected_non_volatile = 1 - expected_volatile;
        let mut test_ctx = setup_test_context(context, reporter, dimensions, is_volatile, false);
        let checker = Arc::clone(&test_ctx.checker);
        let mut recordings: Vec<Recording> = Vec::with_capacity(NUM_RECORDINGS);
        let img = test_ctx.img.clone().expect("the image");
        for i in 0..NUM_RECORDINGS {
            let i_count = i32::try_from(i).unwrap_or(0);
            canvas_of(&test_ctx).draw_image(&img, (0.0, 0.0), None);
            recordings.push(snap(&mut test_ctx));
            if is_volatile == Volatile::Yes {
                check_fulfills_only(reporter, &checker, i_count);
            } else {
                check_fulfills_only(reporter, &checker, i32::from(i > 0));
            }
            let recording = &mut recordings[i];
            let num_volatile = i32::try_from(recording.priv_().num_volatile_promise_images());
            let num_non_volatile =
                i32::try_from(recording.priv_().num_non_volatile_promise_images());
            reporter_assert!(reporter, num_volatile == Ok(expected_volatile));
            reporter_assert!(reporter, num_non_volatile == Ok(expected_non_volatile));
            reporter_assert!(reporter, insert_recording(context, recording));
            if is_volatile == Volatile::Yes {
                check_fulfills_only(reporter, &checker, i_count + 1);
            } else {
                check_fulfills_only(reporter, &checker, 1);
            }
            reporter_assert!(
                reporter,
                recordings[i].priv_().num_non_volatile_promise_images() == 0
            );
        }
        synced_submit(context);
        test_ctx.surface = None;
        drop(img);
        test_ctx.img = None;
        recordings.clear();
        if is_volatile == Volatile::Yes {
            check_all_done(
                reporter,
                &checker,
                i32::try_from(NUM_RECORDINGS).unwrap_or(0),
            );
        } else {
            check_all_done(reporter, &checker, 1);
        }
    }
});
