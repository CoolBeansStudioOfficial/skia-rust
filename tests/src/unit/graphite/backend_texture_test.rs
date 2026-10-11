// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/BackendTextureTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::size::ISize;
use skia_rust_core::swizzle::Swizzle;
use skia_rust_gpu::gpu::gpu_types::{Mipmapped, Origin, Protected, Renderable};
use skia_rust_gpu::graphite::backend_texture::BackendTexture;
use skia_rust_gpu::graphite::image_factories::wrap_texture_for_alpha_type;
use skia_rust_gpu::graphite::image_graphite;
use skia_rust_gpu::graphite::recorder::Recorder;
use skia_rust_gpu::graphite::texture_format::TextureFormat;
use skia_rust_gpu::graphite::texture_info::texture_info_priv;

use crate::{Reporter, def_graphite_test_for_all_contexts, reporter_assert};

const K_SIZE: ISize = ISize {
    width: 16,
    height: 16,
};

// The `SurfaceBackendTextureTest` of this file wraps the textures in a `Surface` (G10d); it stays
// `todo`.

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

/// `SkImages::WrapTexture(recorder, texture, alphaType, nullptr)`: the colour type comes from the
/// texture's format, the origin is top-left and the image has no release proc.
fn wrap_texture(recorder: &Recorder, texture: &BackendTexture, at: AlphaType) -> Option<Image> {
    wrap_texture_for_alpha_type(recorder, texture, at, None, Origin::TopLeft, None)
}

/// The `REPORTER_ASSERT`s on a wrapped image: that it exists, has the mipmaps, colour type and
/// alpha type expected.
fn check_wrapped(
    reporter: &mut Reporter,
    image: Option<&Image>,
    with_mips: bool,
    ct: ColorType,
    at: AlphaType,
) {
    reporter_assert!(reporter, image.is_some());
    let Some(image) = image else {
        return;
    };
    reporter_assert!(reporter, image.has_mipmaps() == with_mips);
    reporter_assert!(reporter, image.color_type() == ct);
    reporter_assert!(reporter, image.alpha_type() == at);
}

/// `static_cast<Image*>(image)->textureProxyView().swizzle()`.
fn read_swizzle(image: &Image) -> Swizzle {
    image_graphite::Image::from_core(image)
        .expect("a Graphite image")
        .texture_proxy_view()
        .swizzle()
}

// Tests the wrapping of a BackendTexture in an SkImage
// Port of: tests/graphite/BackendTextureTest.cpp#L136-L316 (chrome/m156)
def_graphite_test_for_all_contexts!(ImageBackendTextureTest, |reporter, context| {
    // TODO: Right now this just tests very basic combinations of images. This should be expanded
    // to cover a much broader set of things once we add more support in Graphite for different
    // formats, color types, etc.
    let caps = context.wgpu_caps();
    let mut recorder = context.make_recorder(None);
    for mipmapped in [Mipmapped::Yes, Mipmapped::No] {
        let with_mips = mipmapped == Mipmapped::Yes;
        for renderable in [Renderable::Yes, Renderable::No] {
            let mut info = caps.get_default_sampled_texture_info(
                ColorType::RGBA8888,
                mipmapped,
                Protected::No,
                renderable,
            );
            let mut texture = recorder.create_backend_texture(K_SIZE, &info);
            reporter_assert!(reporter, texture.is_valid());
            let image = wrap_texture(&recorder, &texture, AlphaType::Premul);
            check_wrapped(
                reporter,
                image.as_ref(),
                with_mips,
                ColorType::RGBA8888,
                AlphaType::Premul,
            );
            drop(image);
            // We should switch to k888x when possible for forceOpaque
            let image = wrap_texture(&recorder, &texture, AlphaType::Unknown);
            check_wrapped(
                reporter,
                image.as_ref(),
                with_mips,
                ColorType::RGB888x,
                AlphaType::Opaque,
            );
            drop(image);
            recorder.delete_backend_texture(&texture);

            // We should still be flagged as opaque and have a A=1 swizzle for color types that
            // don't have an A->X variant
            info = caps.get_default_sampled_texture_info(
                ColorType::ARGB4444,
                mipmapped,
                Protected::No,
                renderable,
            );
            texture = recorder.create_backend_texture(K_SIZE, &info);
            // 4444 is not universally supported however
            if texture.is_valid() {
                let image = wrap_texture(&recorder, &texture, AlphaType::Unknown);
                check_wrapped(
                    reporter,
                    image.as_ref(),
                    with_mips,
                    ColorType::ARGB4444,
                    AlphaType::Opaque,
                );
                if let Some(image) = &image {
                    reporter_assert!(reporter, read_swizzle(image).as_string().chars().nth(3) == Some('1'));
                }
                drop(image);
                recorder.delete_backend_texture(&texture);
            }

            // Now test handling of red format ambiguity
            info = caps.get_default_sampled_texture_info(
                ColorType::R8UNorm,
                mipmapped,
                Protected::No,
                renderable,
            );
            texture = recorder.create_backend_texture(K_SIZE, &info);
            // Opaque/unknown becomes red + opaque
            let image = wrap_texture(&recorder, &texture, AlphaType::Opaque);
            check_wrapped(
                reporter,
                image.as_ref(),
                with_mips,
                ColorType::R8UNorm,
                AlphaType::Opaque,
            );
            let image = wrap_texture(&recorder, &texture, AlphaType::Unknown);
            check_wrapped(
                reporter,
                image.as_ref(),
                with_mips,
                ColorType::R8UNorm,
                AlphaType::Opaque,
            );
            // Premul/unpremul becomes alpha-only
            let image = wrap_texture(&recorder, &texture, AlphaType::Premul);
            check_wrapped(
                reporter,
                image.as_ref(),
                with_mips,
                ColorType::Alpha8,
                AlphaType::Premul,
            );
            let image = wrap_texture(&recorder, &texture, AlphaType::Unpremul);
            check_wrapped(
                reporter,
                image.as_ref(),
                with_mips,
                ColorType::Alpha8,
                AlphaType::Unpremul,
            );
            drop(image);
            recorder.delete_backend_texture(&texture);

            // Test how an A8 texture format behaves, which does not have the red/alpha ambiguity
            // and stays as an alpha-only colortype regardless of SkAlphaType. A8 is not always
            // available, so skip it if the TextureInfo selects an R8 texture again.
            info = caps.get_default_sampled_texture_info(
                ColorType::Alpha8,
                mipmapped,
                Protected::No,
                renderable,
            );
            if texture_info_priv::view_format(&info) != TextureFormat::A8 {
                // A8 isn't always available and if we picked R8, we'd really just be re-testing
                // the above scenarios.
                continue;
            }
            texture = recorder.create_backend_texture(K_SIZE, &info);
            // Opaque/unknown becomes opaque alpha (not super useful)
            let image = wrap_texture(&recorder, &texture, AlphaType::Opaque);
            check_wrapped(
                reporter,
                image.as_ref(),
                with_mips,
                ColorType::Alpha8,
                AlphaType::Opaque,
            );
            let image = wrap_texture(&recorder, &texture, AlphaType::Unknown);
            check_wrapped(
                reporter,
                image.as_ref(),
                with_mips,
                ColorType::Alpha8,
                AlphaType::Opaque,
            );
            if let Some(image) = &image {
                reporter_assert!(reporter, read_swizzle(image).as_string().chars().nth(3) == Some('1'));
            }
            // Premul/unpremul stay alpha-only
            let image = wrap_texture(&recorder, &texture, AlphaType::Premul);
            check_wrapped(
                reporter,
                image.as_ref(),
                with_mips,
                ColorType::Alpha8,
                AlphaType::Premul,
            );
            let image = wrap_texture(&recorder, &texture, AlphaType::Unpremul);
            check_wrapped(
                reporter,
                image.as_ref(),
                with_mips,
                ColorType::Alpha8,
                AlphaType::Unpremul,
            );
            drop(image);
            recorder.delete_backend_texture(&texture);
        }
    }
});
