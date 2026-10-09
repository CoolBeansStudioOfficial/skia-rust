//! Ports of Skia's `src/gpu/tessellate/*`: the CPU side of path tessellation (Wang's formula,
//! patch writers, stroke iteration, culling and the fixed-count buffer helpers).

// Port of: src/gpu/tessellate/AffineMatrix.h (chrome/m156)
pub mod affine_matrix;
// Port of: src/gpu/tessellate/CullTest.h (chrome/m156)
pub mod cull_test;
// Port of: src/gpu/tessellate/FixedCountBufferUtils.{h,cpp} (chrome/m156)
pub mod fixed_count_buffer_utils;
// Port of: src/gpu/tessellate/LinearTolerances.h (chrome/m156)
pub mod linear_tolerances;
// Port of: src/gpu/tessellate/MiddleOutPolygonTriangulator.h (chrome/m156)
pub mod middle_out_polygon_triangulator;
// Port of: src/gpu/tessellate/MidpointContourParser.h (chrome/m156)
pub mod midpoint_contour_parser;
// Port of: src/gpu/tessellate/PatchWriter.h (chrome/m156)
pub mod patch_writer;
// Port of: src/gpu/tessellate/StrokeIterator.h (chrome/m156)
pub mod stroke_iterator;
// Port of: src/gpu/tessellate/Tessellation.h and Tessellation.cpp (chrome/m156)
pub mod tessellation;
// Port of: src/gpu/tessellate/WangsFormula.h (chrome/m156)
pub mod wangs_formula;
