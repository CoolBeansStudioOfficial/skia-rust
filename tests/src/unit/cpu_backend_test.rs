#![cfg(test)]

// Port of: tests/CPUBackendTest.cpp (chrome/m156)

use skia_rust_core::mask::{CreateMode, MaskBuilder};
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_enums::ResolveConvexity;
use skia_rust_core::path_priv;
use skia_rust_core::rect::IRect;
use skia_rust_core::stroke_rec::InitStyle;
use skia_rust_raster::draw::draw_to_mask;

use crate::{def_test, reporter_assert};

def_test!(
    #[allow(clippy::excessive_precision, clippy::unreadable_literal)]
    // the C++ float constants, verbatim
    DrawToMask_RejectsBigPath,
    |reporter| {
        let mut pb = PathBuilder::new();
        pb.move_to((2.449_059_32e-5_f32, 0.0625_f32));
        pb.quad_to(
            (2.449_059_32e-5_f32, -2.128_432_82e38_f32),
            (2.449_059_32e-5_f32, 0.0625_f32),
        );
        pb.line_to((2.449_059_32e-5_f32, 0.0625_f32));

        let raw = path_priv::raw_builder(&pb, ResolveConvexity::No);
        reporter_assert!(reporter, raw.is_some());

        let clip = IRect::from_ltrb(0, 0, 0, 0);
        let mut dst = MaskBuilder::default();
        let filter = MaskFilter::blur(skia_rust_core::blur_types::BlurStyle::Normal, 1.0, None)
            .expect("a blur mask filter");
        let matrix = Matrix::new_identity();

        // This crashes without the fix
        if let Some(raw) = raw {
            let _ = draw_to_mask(
                &raw,
                &clip,
                &filter,
                &matrix,
                &mut dst,
                CreateMode::ComputeBoundsAndRenderImage,
                InitStyle::Hairline,
            );
        }
    }
);
