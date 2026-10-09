// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnSampler.h, DawnSampler.cpp

//! `DawnSampler` on wgpu: the backend half of a Graphite [`Sampler`].
//!
//! wgpu has no YCbCr conversion samplers, so a [`SamplerDesc`] with an immutable-sampler info
//! (`isImmutable()`) cannot be created (`make` returns `None`) and the YCbCr label suffix of
//! Dawn's sampler labels does not exist.

use std::any::Any;
use std::sync::{Mutex, PoisonError};

use skia_rust_core::sampling_options::{FilterMode, MipmapMode};
use skia_rust_core::tile_mode::TileMode;

use crate::graphite::resource::ResourceRef;
use crate::graphite::resource_types::SamplerDesc;
use crate::graphite::sampler::{Sampler, SamplerBackend};
use crate::graphite::wgpu::async_wait::create_checked;
use crate::graphite::wgpu::shared_context::WgpuSharedContext;

// Port of: src/gpu/graphite/dawn/DawnSampler.cpp#L20-L28 (chrome/m156)
fn filter_mode_to_wgpu_filter_mode(mode: FilterMode) -> wgpu::FilterMode {
    match mode {
        FilterMode::Nearest => wgpu::FilterMode::Nearest,
        FilterMode::Linear => wgpu::FilterMode::Linear,
    }
}

// Port of: src/gpu/graphite/dawn/DawnSampler.cpp#L30-L42 (chrome/m156)
fn mipmap_mode_to_wgpu_filter_mode(mode: MipmapMode) -> wgpu::MipmapFilterMode {
    match mode {
        // Dawn doesn't have none filter mode.
        MipmapMode::None | MipmapMode::Nearest => wgpu::MipmapFilterMode::Nearest,
        MipmapMode::Linear => wgpu::MipmapFilterMode::Linear,
    }
}

// Port of: src/gpu/graphite/dawn/DawnSampler.cpp#L51-L71 (chrome/m156)
fn tile_modes_to_wgpu_address_modes(
    sampler_desc: &SamplerDesc,
) -> (wgpu::AddressMode, wgpu::AddressMode) {
    let to_wgpu_mode = |tm: TileMode| match tm {
        TileMode::Clamp => wgpu::AddressMode::ClampToEdge,
        TileMode::Repeat => wgpu::AddressMode::Repeat,
        TileMode::Mirror => wgpu::AddressMode::MirrorRepeat,
        TileMode::Decal => {
            // Dawn doesn't support kDecal; considered an error if we reach this point.
            debug_assert!(false);
            wgpu::AddressMode::ClampToEdge
        }
    };

    (
        to_wgpu_mode(sampler_desc.tile_mode_x()),
        to_wgpu_mode(sampler_desc.tile_mode_y()),
    )
}

/// The wgpu half of a [`Sampler`].
// Port of: src/gpu/graphite/dawn/DawnSampler.h#L21-L38 (chrome/m156)
#[doc(alias = "DawnSampler")]
#[derive(Debug)]
pub struct WgpuSampler {
    // `None` after `freeGpuData()`.
    sampler: Mutex<Option<wgpu::Sampler>>,
    sampler_desc: SamplerDesc,
}

impl WgpuSampler {
    /// `DawnSampler::Make()`.
    // Port of: src/gpu/graphite/dawn/DawnSampler.cpp#L73-L141 (chrome/m156)
    #[must_use]
    pub fn make(
        shared_context: &WgpuSharedContext,
        sampler_desc: SamplerDesc,
    ) -> Option<ResourceRef<Sampler>> {
        if sampler_desc.is_immutable() {
            // wgpu has no YCbCr conversion samplers.
            return None;
        }

        let sampling_options = sampler_desc.sampling_options();
        let (address_mode_u, address_mode_v) = tile_modes_to_wgpu_address_modes(&sampler_desc);
        let mag_filter = filter_mode_to_wgpu_filter_mode(sampling_options.filter);

        let mut label = String::new();
        if shared_context.caps().set_backend_labels() {
            const TILE_MODE_LABELS: [&str; 4] = ["Clamp", "Repeat", "Mirror", "Decal"];
            const MIN_MAG_FILTER_LABELS: [&str; 2] = ["Nearest", "Linear"];
            const MIP_FILTER_LABELS: [&str; 3] = ["MipNone", "MipNearest", "MipLinear"];
            label.push('X');
            label.push_str(TILE_MODE_LABELS[sampler_desc.tile_mode_x() as usize]);
            label.push('Y');
            label.push_str(TILE_MODE_LABELS[sampler_desc.tile_mode_y() as usize]);
            label.push_str(MIN_MAG_FILTER_LABELS[sampling_options.filter as usize]);
            label.push_str(MIP_FILTER_LABELS[sampling_options.mipmap as usize]);
        }

        let desc = wgpu::SamplerDescriptor {
            label: (!label.is_empty()).then_some(label.as_str()),
            address_mode_u,
            address_mode_v,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter,
            min_filter: mag_filter,
            mipmap_filter: mipmap_mode_to_wgpu_filter_mode(sampling_options.mipmap),
            lod_min_clamp: 0.0,
            // Disabling mipmap by clamping max lod to first level only.
            lod_max_clamp: if sampling_options.mipmap == MipmapMode::None {
                0.0
            } else {
                f32::MAX
            },
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        };

        let sampler = create_checked(
            shared_context.device(),
            shared_context.caps().allow_scoped_error_checks(),
            || shared_context.device().create_sampler(&desc),
        )?;
        Some(Sampler::make(Box::new(Self {
            sampler: Mutex::new(Some(sampler)),
            sampler_desc,
        })))
    }

    /// `samplerDesc()`.
    #[doc(alias = "samplerDesc")]
    #[must_use]
    pub fn sampler_desc(&self) -> SamplerDesc {
        self.sampler_desc
    }

    /// `dawnSampler()`: absent once freed.
    #[doc(alias = "dawnSampler")]
    #[must_use]
    pub fn wgpu_sampler(&self) -> Option<wgpu::Sampler> {
        self.sampler
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl SamplerBackend for WgpuSampler {
    // Port of: src/gpu/graphite/dawn/DawnSampler.cpp#L143-L145 (chrome/m156)
    fn free_gpu_data(&self) {
        *self.sampler.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The wgpu half of `sampler`, if it is a wgpu sampler.
#[must_use]
pub fn as_wgpu_sampler(sampler: &Sampler) -> Option<&WgpuSampler> {
    sampler.backend().as_any().downcast_ref::<WgpuSampler>()
}
