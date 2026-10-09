// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/BackendTextureTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::color_type::ColorType;
use skia_rust_core::size::ISize;
use skia_rust_gpu::gpu::gpu_types::{Mipmapped, Protected, Renderable};
use skia_rust_gpu::graphite::backend_texture::BackendTexture;

use crate::{def_graphite_test_for_all_contexts, reporter_assert};

const K_SIZE: ISize = ISize {
    width: 16,
    height: 16,
};

// The `SurfaceBackendTextureTest` and `ImageBackendTextureTest` of this file wrap the textures in
// a `Surface` and an `Image` (G10d); they stay `todo`.

def_graphite_test_for_all_contexts!(BackendTextureTest, |reporter, context| {
    // Port of: tests/graphite/BackendTextureTest.cpp#L34-L86 (chrome/m156)
    let caps = context.wgpu_caps();
    let mut recorder = context.make_recorder(None);

    let is_protected = if context.supports_protected_content() {
        Protected::Yes
    } else {
        Protected::No
    };

    let info = caps.get_default_sampled_texture_info(
        ColorType::RGBA8888,
        /* mipmapped= */ Mipmapped::No,
        is_protected,
        Renderable::No,
    );
    reporter_assert!(reporter, info.is_valid());

    let mut texture1 = recorder.create_backend_texture(K_SIZE, &info);
    reporter_assert!(reporter, texture1.is_valid());

    // We make a copy to do the remaining tests so we still have texture1 to safely delete the
    // backend object.
    let mut texture1_copy = texture1.clone();
    reporter_assert!(reporter, texture1_copy.is_valid());
    reporter_assert!(reporter, texture1 == texture1_copy);

    let texture2 = recorder.create_backend_texture(K_SIZE, &info);
    reporter_assert!(reporter, texture2.is_valid());

    reporter_assert!(reporter, texture1_copy != texture2);

    // Test state after assignment
    texture1_copy = texture2.clone();
    reporter_assert!(reporter, texture1_copy.is_valid());
    reporter_assert!(reporter, texture1_copy == texture2);

    let invalid_texture = BackendTexture::new();
    reporter_assert!(reporter, !invalid_texture.is_valid());

    texture1_copy = invalid_texture;
    reporter_assert!(reporter, !texture1_copy.is_valid());

    texture1_copy = texture1.clone();
    reporter_assert!(reporter, texture1_copy.is_valid());
    reporter_assert!(reporter, texture1 == texture1_copy);

    recorder.delete_backend_texture(&texture1);
    recorder.delete_backend_texture(&texture2);

    // Test that deleting is safe from the Context or a different Recorder.
    texture1 = recorder.create_backend_texture(K_SIZE, &info);
    context.delete_backend_texture(&texture1);

    let mut recorder2 = context.make_recorder(None);
    texture1 = recorder.create_backend_texture(K_SIZE, &info);
    recorder2.delete_backend_texture(&texture1);
});
