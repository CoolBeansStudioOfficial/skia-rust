//! skcms, Skia's color management module, ported to Rust: ICC profile parsing, transfer
//! functions, gamut matrices and pixel format / color profile transforms.
//!
//! This is the portable scalar port: the transform pipeline runs the baseline stages of
//! `Transform_inl.h` one pixel at a time. Per-CPU-tier kernels will come later through
//! `skia-rust-simd`.
//!
//! Names follow the mechanical mapping of `docs/PORTING.md` (`skcms_ICCProfile` is
//! [`IccProfile`], `skcms_Parse` is [`parse`], ...); each public item carries a
//! `#[doc(alias = "skcms_...")]`. See `docs/API_MAPPING.md`.

mod curve;
mod math;
mod parse;
mod pipeline;
mod profiles;
mod public;
mod transform;

pub use curve::{approximate_curve, are_approximate_inverses, max_roundtrip_error};
pub use math::{matrix3x3_concat, matrix3x3_invert, powf_};
pub use parse::{
    IccTag, get_chad, get_input_channel_count, get_tag_by_index, get_tag_by_signature, get_wtpt,
    parse, parse_with_a2b_priority,
};
pub use profiles::{
    RANDOM_BYTES_252, adapt_to_xyzd50, approximately_equal_profiles, disable_runtime_cpu_detection,
    identity_transfer_function, make_usable_as_destination,
    make_usable_as_destination_with_single_curve, primaries_to_xyzd50,
    srgb_inverse_transfer_function, srgb_profile, srgb_transfer_function,
    trcs_are_approximate_inverse, xyzd50_profile,
};
pub use public::{
    A2B, AlphaFormat, B2A, ByteView, Cicp, Curve, Hagc, IccProfile, Matrix3x3, Matrix3x4,
    PixelFormat, TfType, TransferFunction, signature,
};
pub use transform::{transform, transform_in_place};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_inverse_maps_one_to_one() {
        let tf = srgb_transfer_function();
        let inv = tf.invert().expect("sRGB is invertible");
        assert_eq!(inv.eval(tf.eval(1.0)).to_bits(), 1.0f32.to_bits());
        assert_eq!(tf.tf_type(), TfType::SRGBish);
    }

    #[test]
    fn matrix_invert() {
        let identity = Matrix3x3 {
            vals: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        };
        assert_eq!(identity.invert().expect("invertible"), identity);
        assert!(Matrix3x3::default().invert().is_none());
    }

    #[test]
    fn transform_8888_roundtrip_is_identity() {
        let src: Vec<u8> = (0..=255u8).flat_map(|v| [v, 255 - v, v / 2, 255]).collect();
        let mut dst = vec![0u8; src.len()];
        assert!(transform(
            &src,
            PixelFormat::Rgba8888,
            AlphaFormat::Unpremul,
            None,
            &mut dst,
            PixelFormat::Rgba8888,
            AlphaFormat::Unpremul,
            None,
            256,
        ));
        assert_eq!(src, dst);
    }

    #[test]
    fn transform_rejects_short_buffers() {
        let src = [0u8; 4];
        let mut dst = [0u8; 4];
        assert!(!transform(
            &src,
            PixelFormat::Rgba8888,
            AlphaFormat::Unpremul,
            None,
            &mut dst,
            PixelFormat::Rgba8888,
            AlphaFormat::Unpremul,
            None,
            2,
        ));
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(parse(&[]).is_none());
        assert!(parse(&[0u8; 200]).is_none());
    }
}
