// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of the dynamic-sampler part of: src/gpu/graphite/GlobalCache.cpp#L110-L154 (chrome/m156)
//
// `Context::finishInitialization` runs `initializeDynamicSamplers` first. The rest of
// `finishInitialization` (the static buffer copies) needs copy recording on the wgpu command
// buffer (G11c), so this test only checks the samplers, and does not assert the return value.

use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::resource_types::{ImmutableSamplerInfo, SamplerDesc};
use skia_rust_gpu::graphite::wgpu::{make_context, noop_backend_context};

fn dynamic_desc(
    filter: FilterMode,
    mipmap: MipmapMode,
    tiles: (TileMode, TileMode),
) -> SamplerDesc {
    SamplerDesc::new_with_tile_modes(
        &SamplingOptions::new(filter, mipmap),
        tiles,
        ImmutableSamplerInfo::default(),
    )
}

#[test]
fn dynamic_samplers_cover_every_mode_and_skip_decal_without_clamp_to_border() {
    let context = make_context(&noop_backend_context(), &ContextOptions::default())
        .expect("a context on the noop device");

    let global_cache = context.shared_context().base().global_cache();
    let clamp_to_border = context.shared_context().caps().clamp_to_border_support();
    let filters = [
        (FilterMode::Nearest, MipmapMode::None),
        (FilterMode::Linear, MipmapMode::None),
        (FilterMode::Nearest, MipmapMode::Nearest),
        (FilterMode::Linear, MipmapMode::Nearest),
        (FilterMode::Nearest, MipmapMode::Linear),
        (FilterMode::Linear, MipmapMode::Linear),
    ];
    let tiles = [
        TileMode::Clamp,
        TileMode::Repeat,
        TileMode::Mirror,
        TileMode::Decal,
    ];
    for (filter, mipmap) in filters {
        for tile_x in tiles {
            for tile_y in tiles {
                let desc = dynamic_desc(filter, mipmap, (tile_x, tile_y));
                let has_decal = tile_x == TileMode::Decal || tile_y == TileMode::Decal;
                let expected = !has_decal || clamp_to_border;
                assert_eq!(
                    global_cache.dynamic_sampler(desc).is_some(),
                    expected,
                    "{filter:?}/{mipmap:?} tiles ({tile_x:?}, {tile_y:?})"
                );
            }
        }
    }
}

#[test]
fn immutable_descriptions_have_no_dynamic_sampler() {
    let context = make_context(&noop_backend_context(), &ContextOptions::default())
        .expect("a context on the noop device");
    let global_cache = context.shared_context().base().global_cache();
    let immutable = SamplerDesc::new_with_tile_modes(
        &SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
        (TileMode::Clamp, TileMode::Clamp),
        ImmutableSamplerInfo {
            non_format_ycbcr_conversion_info: 1,
            ..ImmutableSamplerInfo::default()
        },
    );
    assert!(immutable.is_immutable());
    assert!(global_cache.dynamic_sampler(immutable).is_none());
}
