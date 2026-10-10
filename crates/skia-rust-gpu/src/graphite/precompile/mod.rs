//! Ports of `src/gpu/graphite/precompile/*`: the precompile option types and their combination
//! iteration (`PaintOptions`, `Precompile{Shader,ColorFilter,Blender,...}`).

pub mod base;
pub mod blender;
pub mod color_filter;
pub mod image_filter;
pub mod mask_filter;
pub mod paint_option;
pub mod paint_options;
pub mod runtime_effect;
pub mod shader;
pub mod shader_effects;
pub mod shader_image;
