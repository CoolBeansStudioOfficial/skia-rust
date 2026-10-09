// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/RenderPassDesc.h, src/gpu/graphite/RenderPassDesc.cpp

//! `AttachmentDesc` and `RenderPassDesc`: the fixed description of a render pass.

use crate::gpu::swizzle::Swizzle;
use crate::graphite::caps::Caps;
use crate::graphite::graphite_types::{DepthStencilFlags, SampleCount};
use crate::graphite::resource_types::{DstReadStrategy, LoadOp, StoreOp};
use crate::graphite::texture_format::{TextureFormat, texture_format_name};
use crate::graphite::texture_info::{TextureInfo, texture_info_priv};

fn load_op_str(op: LoadOp) -> &'static str {
    match op {
        LoadOp::Load => "load",
        LoadOp::Clear => "clear",
        LoadOp::Discard => "discard",
    }
}

fn store_op_str(op: StoreOp) -> &'static str {
    match op {
        StoreOp::Store => "store",
        StoreOp::Discard => "discard",
    }
}

/// One attachment of a render pass.
// Port of: src/gpu/graphite/RenderPassDesc.h#L25-L48 (chrome/m156)
#[doc(alias = "skgpu::graphite::AttachmentDesc")]
#[derive(Clone, Copy, Debug)]
pub struct AttachmentDesc {
    /// `fFormat`.
    pub format: TextureFormat,
    /// `fLoadOp`.
    pub load_op: LoadOp,
    /// `fStoreOp`.
    pub store_op: StoreOp,
    /// `fSampleCount`.
    pub sample_count: SampleCount,
}

impl Default for AttachmentDesc {
    fn default() -> Self {
        Self {
            format: TextureFormat::Unsupported,
            load_op: LoadOp::Discard,
            store_op: StoreOp::Discard,
            sample_count: SampleCount::One,
        }
    }
}

impl PartialEq for AttachmentDesc {
    // Port of: src/gpu/graphite/RenderPassDesc.h#L30-L40 (chrome/m156)
    fn eq(&self, other: &Self) -> bool {
        if self.format == TextureFormat::Unsupported && other.format == TextureFormat::Unsupported {
            return true;
        }
        self.format == other.format
            && self.load_op == other.load_op
            && self.store_op == other.store_op
            && self.sample_count == other.sample_count
    }
}

impl AttachmentDesc {
    /// `isCompatible()`.
    // Port of: src/gpu/graphite/RenderPassDesc.cpp#L210-L212 (chrome/m156)
    #[doc(alias = "isCompatible")]
    #[must_use]
    pub fn is_compatible(&self, tex_info: &TextureInfo) -> bool {
        self.format == texture_info_priv::view_format(tex_info)
            && self.sample_count == tex_info.sample_count()
    }

    /// `toString()`.
    // Port of: src/gpu/graphite/RenderPassDesc.cpp#L198-L208 (chrome/m156)
    #[doc(alias = "toString")]
    #[must_use]
    pub fn to_string_desc(&self) -> String {
        if self.format == TextureFormat::Unsupported {
            "{}".to_owned()
        } else {
            format!(
                "{{f: {} x{}, ops: {}->{}}}",
                texture_format_name(self.format),
                self.sample_count as u32,
                load_op_str(self.load_op),
                store_op_str(self.store_op)
            )
        }
    }
}

/// The description of a render pass.
// Port of: src/gpu/graphite/RenderPassDesc.h#L50-L114 (chrome/m156)
#[doc(alias = "skgpu::graphite::RenderPassDesc")]
#[derive(Clone, Debug)]
pub struct RenderPassDesc {
    /// `fColorAttachment`.
    pub color_attachment: AttachmentDesc,
    /// `fColorResolveAttachment`.
    pub color_resolve_attachment: AttachmentDesc,
    /// `fDepthStencilAttachment`.
    pub depth_stencil_attachment: AttachmentDesc,
    /// The write swizzle is applied in shader, so affects SkSL code generation, but is
    /// determined by the desired color type semantics and target format combination of the
    /// render pass.
    pub write_swizzle: Swizzle,
    /// The overall sample count of the render pass.
    pub sample_count: SampleCount,
    /// Each renderpass determines what strategy to use for reading the dst texture.
    pub dst_read_strategy: DstReadStrategy,
    /// `fClearColor`.
    pub clear_color: [f32; 4],
    /// `fClearDepth`.
    pub clear_depth: f32,
    /// `fClearStencil`.
    pub clear_stencil: u32,
}

impl Default for RenderPassDesc {
    fn default() -> Self {
        Self {
            color_attachment: AttachmentDesc::default(),
            color_resolve_attachment: AttachmentDesc::default(),
            depth_stencil_attachment: AttachmentDesc::default(),
            write_swizzle: Swizzle::rgba(),
            sample_count: SampleCount::One,
            dst_read_strategy: DstReadStrategy::NoneRequired,
            clear_color: [0.0, 0.0, 0.0, 0.0],
            clear_depth: 0.0,
            clear_stencil: 0,
        }
    }
}

impl PartialEq for RenderPassDesc {
    // Port of: src/gpu/graphite/RenderPassDesc.h#L61-L69 (chrome/m156)
    fn eq(&self, other: &Self) -> bool {
        self.write_swizzle == other.write_swizzle
            && self.clear_depth == other.clear_depth
            && self.clear_color == other.clear_color
            && self.color_attachment == other.color_attachment
            && self.color_resolve_attachment == other.color_resolve_attachment
            && self.depth_stencil_attachment == other.depth_stencil_attachment
            && self.dst_read_strategy == other.dst_read_strategy
    }
}

impl RenderPassDesc {
    /// `Make()`.
    // Port of: src/gpu/graphite/RenderPassDesc.cpp#L40-L120 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    #[must_use]
    pub fn make(
        caps: &dyn Caps,
        target_info: &TextureInfo,
        load_op: LoadOp,
        store_op: StoreOp,
        mut depth_stencil_flags: DepthStencilFlags,
        clear_color: [f32; 4],
        requires_msaa: bool,
        write_swizzle: Swizzle,
        dst_read_strategy: DstReadStrategy,
    ) -> RenderPassDesc {
        // It doesn't make sense to have a storeOp for our main target not be store. Why are we
        // doing this DrawPass then
        debug_assert!(store_op == StoreOp::Store);

        let mut desc = RenderPassDesc {
            clear_color,
            // Depth and stencil is currently always cleared to 1.f or 0 if it's used. Depth is
            // 1.0 and counts down as painter's order increases due to HW preference for
            // historic OpenGL defaults of a fast hi-z clear value of 1.0 with depth test of
            // lesser.
            clear_depth: 1.0,
            clear_stencil: 0,
            write_swizzle,
            dst_read_strategy,
            ..RenderPassDesc::default()
        };

        let color_format = texture_info_priv::view_format(target_info);

        // The render pass's overall sample count will either be the target's sample count
        // (when single-sampling or already multisampled), or the default sample count (which
        // will then be either the implicit sample count for msaa-render-to-single-sample or
        // the explicit sample count of a separate color attachment).
        //
        // Higher-level logic should ensure the default MSAA sample count is supported if using
        // either msaa-render-to-single-sample or with separate attachments, and select non-MSAA
        // techniques if they weren't supported. getCompatibleMSAASampleCount() downgrades to
        // single-sampled if we got here and MSAA isn't supported.
        let msaa_render_to_single_sampled_support = caps.is_renderable_with_msrtss(target_info);
        desc.sample_count = if requires_msaa {
            caps.get_compatible_msaa_sample_count(target_info)
        } else {
            target_info.sample_count()
        };

        // We need to handle MSAA with an extra color attachment if:
        let needs_msaa_color_attachment = desc.sample_count > SampleCount::One // using MSAA for the render pass,
            && target_info.sample_count() == SampleCount::One // the target isn't already MSAA'ed,
            && !msaa_render_to_single_sampled_support; // can't use an MSAA->single extension.
        if needs_msaa_color_attachment {
            // We set the color and resolve attachments up the same regardless of if the backend
            // ends up using msaaRenderToSingleSampledSupport() to skip explicitly creating the
            // MSAA attachment. The color attachment (and any depth/stencil attachment) will use
            // `sampleCount` and the resolve attachment will be single-sampled.
            desc.color_attachment = AttachmentDesc {
                format: color_format,
                load_op: if load_op == LoadOp::Clear {
                    LoadOp::Clear
                } else {
                    LoadOp::Discard
                },
                store_op: StoreOp::Discard,
                sample_count: desc.sample_count,
            };
            desc.color_resolve_attachment = AttachmentDesc {
                format: color_format,
                load_op: if load_op == LoadOp::Load {
                    LoadOp::Load
                } else {
                    LoadOp::Discard
                },
                store_op,
                sample_count: SampleCount::One,
            };
        } else {
            // The target will be the color attachment and skip configuring the resolve
            // attachment.
            debug_assert!(desc.color_resolve_attachment.format == TextureFormat::Unsupported);
            desc.color_attachment = AttachmentDesc {
                format: color_format,
                load_op,
                store_op,
                sample_count: target_info.sample_count(),
            };
        }

        if depth_stencil_flags != DepthStencilFlags::None && !caps.avoid_depth_mode() {
            // To reduce pipeline compiles and attachment creations, if we need multisampling
            // and need a depth or stencil attachment, we always choose a depth-AND-stencil
            // format.
            if desc.color_attachment.sample_count > SampleCount::One {
                depth_stencil_flags = DepthStencilFlags::DepthStencil;
            }
            let ds_format = caps.get_depth_stencil_format(depth_stencil_flags);
            debug_assert!(ds_format != TextureFormat::Unsupported);

            // Depth and stencil values are currently always cleared and don't need to persist.
            // The sample count should always match render pass.
            desc.depth_stencil_attachment = AttachmentDesc {
                format: ds_format,
                load_op: LoadOp::Clear,
                store_op: StoreOp::Discard,
                sample_count: desc.sample_count,
            };
        } else {
            debug_assert!(desc.depth_stencil_attachment.format == TextureFormat::Unsupported);
        }

        desc
    }

    /// `toString()`.
    // Port of: src/gpu/graphite/RenderPassDesc.cpp#L122-L135 (chrome/m156)
    #[doc(alias = "toString")]
    #[must_use]
    pub fn to_string_desc(&self) -> String {
        format!(
            "RP(color: {}, resolve: {}, ds: {}, samples: {}, swizzle: {}, clear: c({:.6},{:.6},{:.6},{:.6}), d({:.6}), s(0x{:02x}), dst read: {})",
            self.color_attachment.to_string_desc(),
            self.color_resolve_attachment.to_string_desc(),
            self.depth_stencil_attachment.to_string_desc(),
            self.sample_count as u32,
            self.write_swizzle.as_string(),
            self.clear_color[0],
            self.clear_color[1],
            self.clear_color[2],
            self.clear_color[3],
            self.clear_depth,
            self.clear_stencil,
            self.dst_read_strategy as u32,
        )
    }

    /// `toPipelineLabel()`: only includes fixed state relevant to pipeline creation.
    // Port of: src/gpu/graphite/RenderPassDesc.cpp#L137-L196 (chrome/m156)
    #[doc(alias = "toPipelineLabel")]
    #[must_use]
    pub fn to_pipeline_label(&self) -> String {
        // Given current policies, these assumptions should hold and mean the conciseness in the
        // label is still unambiguous.
        debug_assert!(self.color_attachment.format != TextureFormat::Unsupported);
        debug_assert!(
            self.color_resolve_attachment.format == TextureFormat::Unsupported
                || self.color_resolve_attachment.format == self.color_attachment.format
        );
        debug_assert!(
            self.color_resolve_attachment.format == TextureFormat::Unsupported
                || self.color_resolve_attachment.sample_count == SampleCount::One
        );
        debug_assert!(
            self.color_attachment.sample_count == self.sample_count
                || (self.color_attachment.sample_count == SampleCount::One
                    && self.sample_count > SampleCount::One)
        );

        let color_format_str = texture_format_name(self.color_attachment.format);
        let ds_format_str = if self.depth_stencil_attachment.format == TextureFormat::Unsupported {
            "{}"
        } else {
            texture_format_name(self.depth_stencil_attachment.format)
        };

        // This intentionally only includes the fixed state that impacts pipeline compilation.
        // We include the load op of the color attachment when there is a resolve attachment
        // because the load may trigger a different renderpass description.
        let load_msaa_from_resolve = self.color_resolve_attachment.format
            != TextureFormat::Unsupported
            && self.color_resolve_attachment.load_op == LoadOp::Load;
        // This should, technically, check Caps::loadOpAffectsMSAAPipelines before adding the
        // extra string. Only the Metal backend doesn't set that flag, however, so we just
        // assume it is set to reduce plumbing.
        let color_load_str = if load_msaa_from_resolve {
            " w/ msaa load"
        } else {
            ""
        };

        // There are three supported ways of achieving MSAA rendering that we distinguish
        // compactly.
        // 1. Direct sampling w/ N samples (includes single sample)
        // 2. MSAA render to single-sampled extensions
        // 3. Explicit MSAA color attachment w/ resolve
        let sample_count_str = if self.color_resolve_attachment.format == TextureFormat::Unsupported
            && self.sample_count == self.color_attachment.sample_count
        {
            // Case 1: "xN"
            format!("x{}", self.sample_count as u32)
        } else {
            // Case 2 and 3: "xN->1"
            format!("x{}->1", self.sample_count as u32)
        };

        // NOTE: This label does not differentiate between explicitly resolved MSAA color
        // attachments and MSAA-render-to-single-sample renderpasses.
        format!(
            "RP(({}+{} {}).{}{})",
            color_format_str,
            ds_format_str,
            sample_count_str,
            self.write_swizzle.as_string(),
            color_load_str
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_attachments_compare_equal() {
        let a = AttachmentDesc::default();
        let b = AttachmentDesc {
            load_op: LoadOp::Clear,
            ..AttachmentDesc::default()
        };
        assert_eq!(a, b);
        let c = AttachmentDesc {
            format: TextureFormat::RGBA8,
            ..AttachmentDesc::default()
        };
        assert_ne!(a, c);
        assert_eq!(a.to_string_desc(), "{}");
        assert_eq!(c.to_string_desc(), "{f: RGBA8 x1, ops: discard->discard}");
    }

    #[test]
    fn pipeline_label() {
        let desc = RenderPassDesc {
            color_attachment: AttachmentDesc {
                format: TextureFormat::RGBA8,
                load_op: LoadOp::Clear,
                store_op: StoreOp::Store,
                sample_count: SampleCount::One,
            },
            ..RenderPassDesc::default()
        };
        assert_eq!(desc.to_pipeline_label(), "RP((RGBA8+{} x1).rgba)");
    }
}
