// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnGraphiteUtils.h, DawnGraphiteUtils.cpp

//! `DawnGraphiteUtils`: the format tables of the wgpu back end.
//!
//! Also here: [`compile_wgsl_shader_module`] (`DawnCompileWGSLShaderModule`).
//!
//! Not ported here: `ContextFactory::MakeDawn` (it is [`super::make_context`]) and the
//! YCbCr-descriptor helpers (`DawnDescriptor*`): wgpu has no `YCbCrVkDescriptor`, multiplanar
//! formats or external formats, so those paths do not exist.

use std::fmt::Write as _;

use bitflags::bitflags;

use crate::gpu::shader_error_handler::ShaderErrorHandler;
use crate::graphite::texture_format::TextureFormat;
use crate::graphite::wgpu::async_wait::block_on;
use crate::graphite::wgpu::caps::DeviceFeatures;
use crate::graphite::wgpu::error_checker::{ErrorChecker, ErrorType};
use crate::graphite::wgpu::shared_context::WgpuSharedContext;

bitflags! {
    /// Helper bit mask for the columns of the "Texture Format Capabilities" tables in
    /// <https://gpuweb.github.io/gpuweb/#texture-format-caps>.
    // Port of: src/gpu/graphite/dawn/DawnGraphiteUtils.h#L79-L94 (chrome/m156)
    #[doc(alias = "DawnFormatFlag")]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct FormatFlag: u16 {
        /// Corresponds to "float" in the `GPUTextureSampleType` column; "unfilterable-float",
        /// "uint" and "sint" are readable but not filterable and can be inferred from the
        /// format's type.
        const FILTER = 0x001;
        /// Support for `TextureUsages::RENDER_ATTACHMENT`.
        const RENDER = 0x002;
        /// Corresponds to <https://gpuweb.github.io/gpuweb/#blendable>.
        const BLEND = 0x004;
        /// Supports MSAA (4x only).
        const MSAA = 0x008;
        /// Supports being a resolve target.
        const RESOLVE = 0x010;
        /// Support for `TextureUsages::STORAGE_BINDING` as "write-only".
        const WRITE_ONLY = 0x020;
        /// Support for `TextureUsages::STORAGE_BINDING` as "read-only".
        const READ_ONLY = 0x040;
        /// Support for `TextureUsages::STORAGE_BINDING` as "read-write".
        const READ_WRITE = 0x080;
    }
}

/// One row of the capability table: the flags `format` has when `feature` is available (an
/// empty `feature` is the baseline).
struct Row {
    format: wgpu::TextureFormat,
    feature: DeviceFeatures,
    flags: FormatFlag,
}

const fn row(format: wgpu::TextureFormat, feature: DeviceFeatures, flags: FormatFlag) -> Row {
    Row {
        format,
        feature,
        flags,
    }
}

/// `Filter | Render | ...` as a const expression.
macro_rules! flags {
    ($($flag:ident)|+) => {
        FormatFlag::empty()$(.union(FormatFlag::$flag))+
    };
}

/// All rows of `DawnFormatCapabilityMap::Get()`, flattened: a format listed several times in
/// the C++ table (a baseline and extensions) has one row per entry, and its flags are the union
/// of the rows whose feature is available.
///
/// To read this table with the HTML table in the link above, each format corresponds to a row
/// in the original document. For a format, the first entry is the "Required feature" column,
/// where blank is `COMPAT` (the baseline). Additional conditional capabilities in the row are
/// listed below the primary entry.
// Port of: src/gpu/graphite/dawn/DawnGraphiteUtils.cpp#L128-L300 (chrome/m156)
static FORMAT_CAPABILITIES: &[Row] = {
    use wgpu::TextureFormat as W;

    const COMPAT: DeviceFeatures = DeviceFeatures::empty();
    const CORE: DeviceFeatures = DeviceFeatures::CORE_FEATURES_AND_LIMITS;
    const TIER1: DeviceFeatures = DeviceFeatures::TEXTURE_FORMATS_TIER1;
    const TIER2: DeviceFeatures = DeviceFeatures::TEXTURE_FORMATS_TIER2;

    &[
        // Plain color formats: https://gpuweb.github.io/gpuweb/#plain-color-formats
        // 8 bits per component
        row(
            W::R8Unorm,
            COMPAT,
            flags!(FILTER | RENDER | BLEND | MSAA | RESOLVE),
        ),
        row(W::R8Unorm, TIER1, flags!(WRITE_ONLY | READ_ONLY)),
        row(W::R8Unorm, TIER2, flags!(READ_WRITE)),
        row(W::R8Snorm, COMPAT, flags!(FILTER)),
        row(
            W::R8Snorm,
            TIER1,
            flags!(RENDER | BLEND | MSAA | RESOLVE | WRITE_ONLY | READ_ONLY),
        ),
        row(W::R8Uint, COMPAT, flags!(RENDER)),
        row(W::R8Uint, CORE, flags!(MSAA)),
        row(W::R8Uint, TIER1, flags!(WRITE_ONLY | READ_ONLY)),
        row(W::R8Uint, TIER2, flags!(READ_WRITE)),
        row(W::R8Sint, COMPAT, flags!(RENDER)),
        row(W::R8Sint, CORE, flags!(MSAA)),
        row(W::R8Sint, TIER1, flags!(WRITE_ONLY | READ_ONLY)),
        row(W::R8Sint, TIER2, flags!(READ_WRITE)),
        row(
            W::Rg8Unorm,
            COMPAT,
            flags!(FILTER | RENDER | BLEND | MSAA | RESOLVE),
        ),
        row(W::Rg8Unorm, TIER1, flags!(WRITE_ONLY | READ_ONLY)),
        row(W::Rg8Snorm, COMPAT, flags!(FILTER)),
        row(
            W::Rg8Snorm,
            TIER1,
            flags!(RENDER | BLEND | MSAA | RESOLVE | WRITE_ONLY | READ_ONLY),
        ),
        row(W::Rg8Uint, COMPAT, flags!(RENDER)),
        row(W::Rg8Uint, CORE, flags!(MSAA)),
        row(W::Rg8Uint, TIER1, flags!(WRITE_ONLY | READ_ONLY)),
        row(W::Rg8Sint, COMPAT, flags!(RENDER)),
        row(W::Rg8Sint, CORE, flags!(MSAA)),
        row(W::Rg8Sint, TIER1, flags!(WRITE_ONLY | READ_ONLY)),
        row(
            W::Rgba8Unorm,
            COMPAT,
            flags!(FILTER | RENDER | BLEND | MSAA | RESOLVE | WRITE_ONLY | READ_ONLY),
        ),
        row(W::Rgba8Unorm, TIER2, flags!(READ_WRITE)),
        row(
            W::Rgba8UnormSrgb,
            COMPAT,
            flags!(FILTER | RENDER | BLEND | MSAA | RESOLVE),
        ),
        row(
            W::Rgba8Snorm,
            COMPAT,
            flags!(FILTER | WRITE_ONLY | READ_ONLY),
        ),
        row(
            W::Rgba8Snorm,
            TIER1,
            flags!(RENDER | BLEND | MSAA | RESOLVE),
        ),
        row(
            W::Rgba8Uint,
            COMPAT,
            flags!(RENDER | WRITE_ONLY | READ_ONLY),
        ),
        row(W::Rgba8Uint, CORE, flags!(MSAA)),
        row(W::Rgba8Uint, TIER2, flags!(READ_WRITE)),
        row(
            W::Rgba8Sint,
            COMPAT,
            flags!(RENDER | WRITE_ONLY | READ_ONLY),
        ),
        row(W::Rgba8Sint, CORE, flags!(MSAA)),
        row(W::Rgba8Sint, TIER2, flags!(READ_WRITE)),
        row(
            W::Bgra8Unorm,
            COMPAT,
            flags!(FILTER | RENDER | BLEND | MSAA | RESOLVE),
        ),
        row(
            W::Bgra8Unorm,
            DeviceFeatures::BGRA8UNORM_STORAGE,
            flags!(WRITE_ONLY),
        ),
        row(
            W::Bgra8UnormSrgb,
            CORE,
            flags!(FILTER | RENDER | BLEND | MSAA | RESOLVE),
        ),
        // 16 bits per component
        row(
            W::R16Unorm,
            TIER1,
            flags!(RENDER | BLEND | MSAA | WRITE_ONLY | READ_ONLY),
        ),
        row(
            W::R16Snorm,
            TIER1,
            flags!(RENDER | BLEND | MSAA | WRITE_ONLY | READ_ONLY),
        ),
        row(W::R16Uint, COMPAT, flags!(RENDER)),
        row(W::R16Uint, CORE, flags!(MSAA)),
        row(W::R16Uint, TIER1, flags!(WRITE_ONLY | READ_ONLY)),
        row(W::R16Uint, TIER2, flags!(READ_WRITE)),
        row(W::R16Sint, COMPAT, flags!(RENDER)),
        row(W::R16Sint, CORE, flags!(MSAA)),
        row(W::R16Sint, TIER1, flags!(WRITE_ONLY | READ_ONLY)),
        row(W::R16Sint, TIER2, flags!(READ_WRITE)),
        row(
            W::R16Float,
            COMPAT,
            flags!(FILTER | RENDER | BLEND | MSAA | RESOLVE),
        ),
        row(W::R16Float, TIER1, flags!(WRITE_ONLY | READ_ONLY)),
        row(W::R16Float, TIER2, flags!(READ_WRITE)),
        row(
            W::Rg16Unorm,
            TIER1,
            flags!(RENDER | BLEND | MSAA | WRITE_ONLY | READ_ONLY),
        ),
        row(
            W::Rg16Snorm,
            TIER1,
            flags!(RENDER | BLEND | MSAA | WRITE_ONLY | READ_ONLY),
        ),
        row(W::Rg16Uint, COMPAT, flags!(RENDER)),
        row(W::Rg16Uint, CORE, flags!(MSAA)),
        row(W::Rg16Uint, TIER1, flags!(WRITE_ONLY | READ_ONLY)),
        row(W::Rg16Sint, COMPAT, flags!(RENDER)),
        row(W::Rg16Sint, CORE, flags!(MSAA)),
        row(W::Rg16Sint, TIER1, flags!(WRITE_ONLY | READ_ONLY)),
        row(
            W::Rg16Float,
            COMPAT,
            flags!(FILTER | RENDER | BLEND | MSAA | RESOLVE),
        ),
        row(W::Rg16Float, TIER1, flags!(WRITE_ONLY | READ_ONLY)),
        row(
            W::Rgba16Unorm,
            TIER1,
            flags!(RENDER | BLEND | MSAA | WRITE_ONLY | READ_ONLY),
        ),
        row(
            W::Rgba16Snorm,
            TIER1,
            flags!(RENDER | BLEND | MSAA | WRITE_ONLY | READ_ONLY),
        ),
        row(
            W::Rgba16Uint,
            COMPAT,
            flags!(RENDER | WRITE_ONLY | READ_ONLY),
        ),
        row(W::Rgba16Uint, CORE, flags!(MSAA)),
        row(W::Rgba16Uint, TIER2, flags!(READ_WRITE)),
        row(
            W::Rgba16Sint,
            COMPAT,
            flags!(RENDER | WRITE_ONLY | READ_ONLY),
        ),
        row(W::Rgba16Sint, CORE, flags!(MSAA)),
        row(W::Rgba16Sint, TIER2, flags!(READ_WRITE)),
        row(
            W::Rgba16Float,
            COMPAT,
            flags!(FILTER | RENDER | BLEND | WRITE_ONLY | READ_ONLY),
        ),
        row(W::Rgba16Float, CORE, flags!(MSAA | RESOLVE)),
        row(W::Rgba16Float, TIER2, flags!(READ_WRITE)),
        // 32 bits per component
        row(
            W::R32Uint,
            COMPAT,
            flags!(RENDER | WRITE_ONLY | READ_ONLY | READ_WRITE),
        ),
        row(
            W::R32Sint,
            COMPAT,
            flags!(RENDER | WRITE_ONLY | READ_ONLY | READ_WRITE),
        ),
        row(
            W::R32Float,
            COMPAT,
            flags!(RENDER | WRITE_ONLY | READ_ONLY | READ_WRITE),
        ),
        row(W::R32Float, CORE, flags!(MSAA)),
        row(
            W::R32Float,
            DeviceFeatures::FLOAT32_FILTERABLE,
            flags!(FILTER),
        ),
        row(
            W::R32Float,
            DeviceFeatures::FLOAT32_BLENDABLE,
            flags!(BLEND),
        ),
        row(W::Rg32Uint, COMPAT, flags!(RENDER)),
        row(W::Rg32Uint, CORE, flags!(WRITE_ONLY | READ_ONLY)),
        row(W::Rg32Sint, COMPAT, flags!(RENDER)),
        row(W::Rg32Sint, CORE, flags!(WRITE_ONLY | READ_ONLY)),
        row(W::Rg32Float, COMPAT, flags!(RENDER)),
        row(W::Rg32Float, CORE, flags!(WRITE_ONLY | READ_ONLY)),
        row(
            W::Rg32Float,
            DeviceFeatures::FLOAT32_FILTERABLE,
            flags!(FILTER),
        ),
        row(
            W::Rg32Float,
            DeviceFeatures::FLOAT32_BLENDABLE,
            flags!(BLEND),
        ),
        row(
            W::Rgba32Uint,
            COMPAT,
            flags!(RENDER | WRITE_ONLY | READ_ONLY),
        ),
        row(W::Rgba32Uint, TIER2, flags!(READ_WRITE)),
        row(
            W::Rgba32Sint,
            COMPAT,
            flags!(RENDER | WRITE_ONLY | READ_ONLY),
        ),
        row(W::Rgba32Sint, TIER2, flags!(READ_WRITE)),
        row(
            W::Rgba32Float,
            COMPAT,
            flags!(RENDER | WRITE_ONLY | READ_ONLY),
        ),
        row(W::Rgba32Float, TIER2, flags!(READ_WRITE)),
        row(
            W::Rgba32Float,
            DeviceFeatures::FLOAT32_FILTERABLE,
            flags!(FILTER),
        ),
        row(
            W::Rgba32Float,
            DeviceFeatures::FLOAT32_BLENDABLE,
            flags!(BLEND),
        ),
        // mixed component width, 32 bits per texel
        row(W::Rgb10a2Uint, COMPAT, flags!(RENDER)),
        row(W::Rgb10a2Uint, CORE, flags!(MSAA)),
        row(W::Rgb10a2Uint, TIER1, flags!(WRITE_ONLY | READ_ONLY)),
        row(
            W::Rgb10a2Unorm,
            COMPAT,
            flags!(FILTER | RENDER | BLEND | MSAA | RESOLVE),
        ),
        row(W::Rgb10a2Unorm, TIER1, flags!(WRITE_ONLY | READ_ONLY)),
        row(W::Rg11b10Ufloat, COMPAT, flags!(FILTER)),
        row(
            W::Rg11b10Ufloat,
            DeviceFeatures::RG11B10UFLOAT_RENDERABLE,
            flags!(RENDER | BLEND | MSAA | RESOLVE),
        ),
        row(W::Rg11b10Ufloat, TIER1, flags!(WRITE_ONLY | READ_ONLY)),
        // Depth-stencil formats: https://gpuweb.github.io/gpuweb/#depth-formats
        row(W::Stencil8, COMPAT, flags!(RENDER | MSAA)),
        row(W::Depth16Unorm, COMPAT, flags!(RENDER | MSAA)),
        row(W::Depth24Plus, COMPAT, flags!(RENDER | MSAA)),
        row(W::Depth24PlusStencil8, COMPAT, flags!(RENDER | MSAA)),
        row(W::Depth32Float, COMPAT, flags!(RENDER | MSAA)),
        row(W::Depth32FloatStencil8, COMPAT, flags!(RENDER | MSAA)),
        // Packed formats: https://gpuweb.github.io/gpuweb/#packed-formats
        // (Only including compressed formats for BC1 and ETC2 for brevity, Skia doesn't use ASTC)
        row(W::Rgb9e5Ufloat, COMPAT, flags!(FILTER)),
        row(
            W::Bc1RgbaUnorm,
            DeviceFeatures::TEXTURE_COMPRESSION_BC,
            flags!(FILTER),
        ),
        row(
            W::Bc1RgbaUnormSrgb,
            DeviceFeatures::TEXTURE_COMPRESSION_BC,
            flags!(FILTER),
        ),
        row(
            W::Etc2Rgb8Unorm,
            DeviceFeatures::TEXTURE_COMPRESSION_ETC2,
            flags!(FILTER),
        ),
        row(
            W::Etc2Rgb8UnormSrgb,
            DeviceFeatures::TEXTURE_COMPRESSION_ETC2,
            flags!(FILTER),
        ),
        // Dawn-native only (either format OR feature), these are not included in the format
        // tables. NOTE: Unorm16TextureFormats is not an extension of TextureFormatsTier1 and is
        // deprecated. There will be a new Unorm16Filterable extension that augments the Tier1
        // capabilities.
        row(
            W::R16Unorm,
            DeviceFeatures::UNORM16_TEXTURE_FORMATS,
            flags!(FILTER | RENDER | BLEND | MSAA | RESOLVE),
        ),
        row(
            W::Rg16Unorm,
            DeviceFeatures::UNORM16_TEXTURE_FORMATS,
            flags!(FILTER | RENDER | BLEND | MSAA | RESOLVE),
        ),
        row(
            W::Rgba16Unorm,
            DeviceFeatures::UNORM16_TEXTURE_FORMATS,
            flags!(FILTER | RENDER | BLEND | MSAA | RESOLVE),
        ),
        // The multiplanar formats (`R8BG8Biplanar420Unorm`, ...) and `OpaqueYCbCrAndroid` have no
        // wgpu equivalent, so they have no rows.
    ]
};

/// `DawnTextureFormatSupport`: the capabilities of `format` on a device with `features`.
///
/// A format with no row has no capabilities (`DawnFormatFlag::None`).
// Port of: src/gpu/graphite/dawn/DawnGraphiteUtils.cpp#L82-L93, #L302-L305 (chrome/m156)
#[doc(alias = "DawnTextureFormatSupport")]
#[must_use]
pub fn texture_format_support(features: DeviceFeatures, format: wgpu::TextureFormat) -> FormatFlag {
    let mut supported = FormatFlag::empty();
    for row in FORMAT_CAPABILITIES
        .iter()
        .filter(|row| row.format == format)
    {
        if row.feature.is_empty() || features.contains(row.feature) {
            supported |= row.flags;
        }
    }
    supported
}

/// `DawnFormatToTextureFormat`: Graphite's format for a wgpu one, or
/// [`TextureFormat::Unsupported`] if Graphite does not use it.
// Port of: src/gpu/graphite/dawn/DawnGraphiteUtils.cpp#L307-L418 (chrome/m156)
#[doc(alias = "DawnFormatToTextureFormat")]
#[must_use]
pub fn wgpu_format_to_texture_format(format: wgpu::TextureFormat) -> TextureFormat {
    use wgpu::TextureFormat as W;
    match format {
        W::R8Unorm => TextureFormat::R8,
        W::R16Unorm => TextureFormat::R16,
        W::R16Float => TextureFormat::R16F,
        W::R32Float => TextureFormat::R32F,
        W::Rg8Unorm => TextureFormat::RG8,
        W::Rg16Unorm => TextureFormat::RG16,
        W::Rg16Float => TextureFormat::RG16F,
        W::Rg32Float => TextureFormat::RG32F,
        W::Rgba8Unorm => TextureFormat::RGBA8,
        W::Rgba16Unorm => TextureFormat::RGBA16,
        W::Rgba16Float => TextureFormat::RGBA16F,
        W::Rgba32Float => TextureFormat::RGBA32F,
        W::Rgb10a2Unorm => TextureFormat::RGB10_A2,
        W::Rgba8UnormSrgb => TextureFormat::RGBA8_sRGB,
        W::Bgra8Unorm => TextureFormat::BGRA8,
        W::Bgra8UnormSrgb => TextureFormat::BGRA8_sRGB,
        W::Etc2Rgb8Unorm => TextureFormat::RGB8_ETC2,
        W::Etc2Rgb8UnormSrgb => TextureFormat::RGB8_ETC2_sRGB,
        W::Bc1RgbaUnorm => TextureFormat::RGBA8_BC1,
        W::Bc1RgbaUnormSrgb => TextureFormat::RGBA8_BC1_sRGB,
        W::Stencil8 => TextureFormat::S8,
        W::Depth16Unorm => TextureFormat::D16,
        W::Depth32Float => TextureFormat::D32F,
        W::Depth24PlusStencil8 => TextureFormat::D24_S8,
        W::Depth32FloatStencil8 => TextureFormat::D32F_S8,
        _ => TextureFormat::Unsupported,
    }
}

/// `TextureFormatToDawnFormat`: the wgpu format for a Graphite one, or `None` (Dawn's
/// `wgpu::TextureFormat::Undefined`) if the back end does not support it.
// Port of: src/gpu/graphite/dawn/DawnGraphiteUtils.cpp#L307-L418 (chrome/m156)
#[doc(alias = "TextureFormatToDawnFormat")]
#[must_use]
pub fn texture_format_to_wgpu_format(format: TextureFormat) -> Option<wgpu::TextureFormat> {
    use wgpu::TextureFormat as W;
    Some(match format {
        TextureFormat::R8 => W::R8Unorm,
        TextureFormat::R16 => W::R16Unorm,
        TextureFormat::R16F => W::R16Float,
        TextureFormat::R32F => W::R32Float,
        TextureFormat::RG8 => W::Rg8Unorm,
        TextureFormat::RG16 => W::Rg16Unorm,
        TextureFormat::RG16F => W::Rg16Float,
        TextureFormat::RG32F => W::Rg32Float,
        TextureFormat::RGBA8 => W::Rgba8Unorm,
        TextureFormat::RGBA16 => W::Rgba16Unorm,
        TextureFormat::RGBA16F => W::Rgba16Float,
        TextureFormat::RGBA32F => W::Rgba32Float,
        TextureFormat::RGB10_A2 => W::Rgb10a2Unorm,
        TextureFormat::RGBA8_sRGB => W::Rgba8UnormSrgb,
        TextureFormat::BGRA8 => W::Bgra8Unorm,
        TextureFormat::BGRA8_sRGB => W::Bgra8UnormSrgb,
        TextureFormat::RGB8_ETC2 => W::Etc2Rgb8Unorm,
        TextureFormat::RGB8_ETC2_sRGB => W::Etc2Rgb8UnormSrgb,
        TextureFormat::RGBA8_BC1 => W::Bc1RgbaUnorm,
        TextureFormat::RGBA8_BC1_sRGB => W::Bc1RgbaUnormSrgb,
        TextureFormat::S8 => W::Stencil8,
        TextureFormat::D16 => W::Depth16Unorm,
        TextureFormat::D32F => W::Depth32Float,
        TextureFormat::D24_S8 => W::Depth24PlusStencil8,
        TextureFormat::D32F_S8 => W::Depth32FloatStencil8,
        _ => return None,
    })
}

/// `check_shader_module`: walks the module's compilation messages and, if there is a hard error,
/// reports all of them to `error_handler`. Returns whether the module compiled.
///
/// Dawn's `GetCompilationInfo` is a future; wgpu's resolves at once on native. In the browser
/// (`allow_scoped_error_checks` is off) a blocked thread cannot run the future, so the check is
/// skipped, like Skia's for an old Emscripten.
// Port of: src/gpu/graphite/dawn/DawnGraphiteUtils.cpp#L422-L495 (chrome/m156)
fn check_shader_module(
    module: &wgpu::ShaderModule,
    shader_text: &str,
    error_handler: &dyn ShaderErrorHandler,
) -> bool {
    let info = block_on(module.get_compilation_info());

    // Walk the message list and check for hard errors.
    let success = !info
        .messages
        .iter()
        .any(|entry| entry.message_type == wgpu::CompilationMessageType::Error);

    // If we found a hard error, report the compilation messages to the error handler.
    if !success {
        let mut errors = String::new();
        for entry in &info.messages {
            let (line, pos) = entry
                .location
                .map_or((0, 0), |l| (l.line_number, l.line_position));
            let _ = writeln!(errors, "line {line}:{pos} {}", entry.message);
        }
        error_handler.compile_error(shader_text, &errors, /* shader_was_cached= */ false);
    }
    success
}

/// `DawnCompileWGSLShaderModule(sharedContext, label, wgsl, &module, errorHandler)`: creates a
/// shader module from WGSL, which wgpu compiles with naga. Returns `None`, after reporting the
/// errors to `error_handler`, if it does not compile.
///
/// Dawn returns an invalid module and the compilation info says why. wgpu raises the validation
/// error through the device, so the creation runs in error scopes (when the caps allow them), and
/// an error the scopes caught, but the compilation info did not explain, is reported with the
/// text the scopes logged.
// Port of: src/gpu/graphite/dawn/DawnGraphiteUtils.cpp#L497-L518 (chrome/m156)
#[doc(alias = "DawnCompileWGSLShaderModule")]
#[must_use]
pub fn compile_wgsl_shader_module(
    shared_context: &WgpuSharedContext,
    label: &str,
    wgsl: &str,
    error_handler: &dyn ShaderErrorHandler,
) -> Option<wgpu::ShaderModule> {
    let caps = shared_context.caps();
    let device = shared_context.device();
    // naga does not implement WGSL's `unrestricted_pointer_parameters`, which Skia's generated
    // WGSL uses (docs/design/gpu.md 6.3, W4): such a module is rewritten, every other one is
    // handed over as the text. The browser compiles the text itself.
    #[cfg(not(target_arch = "wasm32"))]
    let source = match crate::graphite::wgpu::naga_pointer_args::shader_source(wgsl) {
        Ok(source) => source,
        Err(error) => {
            error_handler.compile_error(
                wgsl,
                &format!("{error}\n"),
                /* shader_was_cached= */ false,
            );
            return None;
        }
    };
    #[cfg(target_arch = "wasm32")]
    let source = wgpu::ShaderSource::Wgsl(wgsl.into());
    let descriptor = wgpu::ShaderModuleDescriptor {
        label: caps.set_backend_labels().then_some(label),
        source,
    };

    if !caps.allow_scoped_error_checks() {
        // The browser: nothing can wait for the errors, so the module is trusted.
        return Some(device.create_shader_module(descriptor));
    }

    let mut checker = ErrorChecker::new(device);
    let module = device.create_shader_module(descriptor);
    let scope_error = checker.pop_error_scopes();

    if !check_shader_module(&module, wgsl, error_handler) {
        return None;
    }
    if scope_error != ErrorType::NO_ERROR {
        error_handler.compile_error(
            wgsl,
            &format!("the shader module failed in an error scope: {scope_error:?}\n"),
            /* shader_was_cached= */ false,
        );
        return None;
    }
    Some(module)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphite::texture_format::TextureFormat;

    #[test]
    fn mapping_is_bidirectional() {
        let mut mapped = 0;
        for format in TextureFormat::ALL {
            if let Some(wgpu_format) = texture_format_to_wgpu_format(format) {
                assert_eq!(wgpu_format_to_texture_format(wgpu_format), format);
                mapped += 1;
            } else {
                assert_ne!(format, TextureFormat::RGBA8);
            }
        }
        assert_eq!(mapped, 25);
        assert_eq!(
            wgpu_format_to_texture_format(wgpu::TextureFormat::Rgba8Snorm),
            TextureFormat::Unsupported
        );
    }

    #[test]
    fn support_follows_features() {
        let none = DeviceFeatures::empty();
        let rgba8 = wgpu::TextureFormat::Rgba8Unorm;
        assert_eq!(
            texture_format_support(none, rgba8),
            flags!(FILTER | RENDER | BLEND | MSAA | RESOLVE | WRITE_ONLY | READ_ONLY)
        );
        assert!(
            texture_format_support(DeviceFeatures::TEXTURE_FORMATS_TIER2, rgba8)
                .contains(FormatFlag::READ_WRITE)
        );

        // BGRA8 storage needs its feature.
        let bgra8 = wgpu::TextureFormat::Bgra8Unorm;
        assert!(!texture_format_support(none, bgra8).contains(FormatFlag::WRITE_ONLY));
        assert!(
            texture_format_support(DeviceFeatures::BGRA8UNORM_STORAGE, bgra8)
                .contains(FormatFlag::WRITE_ONLY)
        );

        // A format listed under several features accumulates their flags.
        let r16 = wgpu::TextureFormat::R16Unorm;
        assert_eq!(texture_format_support(none, r16), FormatFlag::empty());
        assert_eq!(
            texture_format_support(DeviceFeatures::TEXTURE_FORMATS_TIER1, r16),
            flags!(RENDER | BLEND | MSAA | WRITE_ONLY | READ_ONLY)
        );
        assert_eq!(
            texture_format_support(
                DeviceFeatures::TEXTURE_FORMATS_TIER1 | DeviceFeatures::UNORM16_TEXTURE_FORMATS,
                r16
            ),
            flags!(FILTER | RENDER | BLEND | MSAA | RESOLVE | WRITE_ONLY | READ_ONLY)
        );

        // Compressed formats need their compression feature.
        let bc1 = wgpu::TextureFormat::Bc1RgbaUnorm;
        assert_eq!(texture_format_support(none, bc1), FormatFlag::empty());
        assert_eq!(
            texture_format_support(DeviceFeatures::TEXTURE_COMPRESSION_BC, bc1),
            FormatFlag::FILTER
        );
    }
}
