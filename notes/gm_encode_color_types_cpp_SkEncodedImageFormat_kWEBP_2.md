# gm/encode_color_types.cpp::SkEncodedImageFormat::kWEBP#2

Status: `failing`. The lossy (quality 80) set of `DEF_VARIANTS`. The same mismatch as the
lossless set: only the `f16` normal variant (`f16/encode-color-types-webp-lossy`) differs, on the
decoded premul pair. Full analysis in `notes/gm_encode_color_types_cpp_SkEncodedImageFormat_kWEBP.md`.
