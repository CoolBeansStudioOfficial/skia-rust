//! GPU support of skia-rust, ported from Skia's `src/gpu`.
//!
//! So far: the CPU-only shared code of `src/gpu` (swizzles and the other helpers below).
//! No GPU API is linked yet: Graphite on wgpu comes after this layer.

pub mod gpu;
pub mod tessellate;
