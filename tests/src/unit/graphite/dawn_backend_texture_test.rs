// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/DawnBackendTextureTest.cpp (chrome/m156)
//
// `DawnTextureInfo` is the wgpu texture info (`WgpuTextureInfo`), and `DawnBackendTexture` the wgpu
// backend texture; the tests run on the Dawn profiles (`def_graphite_test_for_dawn_context!`).

#![cfg(test)]

use skia_rust_core::size::ISize;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::backend_texture::BackendTexture;
use skia_rust_gpu::graphite::graphite_types::SampleCount;
use skia_rust_gpu::graphite::wgpu::{WgpuTextureInfo, backend_textures, texture_infos};

use crate::{def_graphite_test_for_dawn_context, reporter_assert};

const K_SIZE: ISize = ISize {
    width: 16,
    height: 16,
};

def_graphite_test_for_dawn_context!(DawnBackendTextureSimpleCreationTest, |reporter, context| {
    // Port of: tests/graphite/DawnBackendTextureTest.cpp#L19-L37 (chrome/m156)
    let mut recorder = context.make_recorder(None);

    let mut texture_info = WgpuTextureInfo::new(
        SampleCount::One,
        Mipmapped::No,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::TEXTURE_BINDING,
        wgpu::TextureAspect::All,
    );

    let be_texture =
        recorder.create_backend_texture(K_SIZE, &texture_infos::make_wgpu(&texture_info));
    reporter_assert!(reporter, be_texture.is_valid());
    recorder.delete_backend_texture(&be_texture);

    // It should also pass if we set the usage to be a render target
    texture_info.usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    let be_texture =
        recorder.create_backend_texture(K_SIZE, &texture_infos::make_wgpu(&texture_info));
    reporter_assert!(reporter, be_texture.is_valid());
    recorder.delete_backend_texture(&be_texture);
});

// Test that copying BackendTexture variables works.
// Port of: tests/graphite/DawnBackendTextureTest.cpp#L40-L77 (chrome/m156)
def_graphite_test_for_dawn_context!(DawnBackendTextureCopyVariableTest, |reporter, context| {
    let mut recorder = context.make_recorder(None);

    let texture_info = WgpuTextureInfo::new(
        SampleCount::One,
        Mipmapped::No,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::TEXTURE_BINDING,
        wgpu::TextureAspect::All,
    );

    let be_texture: BackendTexture =
        recorder.create_backend_texture(K_SIZE, &texture_infos::make_wgpu(&texture_info));
    reporter_assert!(reporter, be_texture.is_valid());

    let mut be_texture2 = BackendTexture::new();
    reporter_assert!(reporter, be_texture2 != be_texture);
    reporter_assert!(
        reporter,
        backend_textures::get_wgpu_texture(&be_texture2).is_none()
    );
    reporter_assert!(
        reporter,
        backend_textures::get_wgpu_texture_view(&be_texture2).is_none()
    );

    be_texture2 = be_texture.clone();
    reporter_assert!(reporter, be_texture2 == be_texture);
    reporter_assert!(
        reporter,
        backend_textures::get_wgpu_texture(&be_texture2).is_some()
    );
    // `GetDawnTexturePtr(beTexture2) == GetDawnTexturePtr(beTexture)`: `wgpu::Texture` has no
    // identity comparison, so the pointer check is the backend texture equality asserted above
    // (`BackendTexture::equal` compares the shared texture object, as the C++ pointers do).

    recorder.delete_backend_texture(&be_texture);
});
