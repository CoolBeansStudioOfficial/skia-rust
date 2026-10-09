//! Ports of Skia's `src/gpu/tessellate/*`: the CPU side of path tessellation (Wang's formula,
//! patch writers, stroke iteration, culling and the fixed-count buffer helpers).

// Port of: src/gpu/tessellate/WangsFormula.h (chrome/m156)
pub mod wangs_formula;
