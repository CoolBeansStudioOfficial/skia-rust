// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/MatrixTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::m44::{M44, V3};
use skia_rust_core::matrix::{AffineMember, Matrix, Member, ScaleToFit, TypeMask};
use skia_rust_core::matrix_priv::{self, MAX_FLATTEN_SIZE};
use skia_rust_core::matrix_utils::decompose_upper_2x2;
use skia_rust_core::point::{Point, Vector, point_priv};
use skia_rust_core::point3::Point3;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{
    SCALAR_1, SCALAR_MAX, SCALAR_NAN, SCALAR_NEARLY_ZERO, Scalar, int_to_scalar, scalar, scalar_abs,
};

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/MatrixTest.cpp#L30-L33 (chrome/m156)
fn nearly_equal_scalar(a: scalar, b: scalar) -> bool {
    let tolerance: scalar = SCALAR_1 / 200_000.0;
    scalar_abs(a - b) <= tolerance
}

// Port of: tests/MatrixTest.cpp#L35-L43 (chrome/m156)
fn nearly_equal(a: &Matrix, b: &Matrix) -> bool {
    for i in 0..9usize {
        if !nearly_equal_scalar(a[i], b[i]) {
            eprintln!("matrices not equal [{}] {} {}", i, a[i], b[i]);
            return false;
        }
    }
    true
}

// Port of: tests/MatrixTest.cpp#L45-L49 (chrome/m156)
fn float_bits(f: f32) -> i32 {
    i32::from_ne_bytes(f.to_ne_bytes())
}

// Port of: tests/MatrixTest.cpp#L51-L88 (chrome/m156)
#[allow(clippy::float_cmp)] // the C++ compares floats with ==
fn are_equal(reporter: &mut Reporter, a: &Matrix, b: &Matrix) -> bool {
    let equal = a == b;
    let cheap_equal = matrix_priv::cheap_equal(a, b);
    if equal != cheap_equal {
        if equal {
            let mut found_zero_sign_diff = false;
            for i in 0..9usize {
                let a_val = a.get(i);
                let b_val = b.get(i);
                let a_val_i = float_bits(a_val);
                let b_val_i = float_bits(b_val);
                if 0.0 == a_val && 0.0 == b_val && a_val_i != b_val_i {
                    found_zero_sign_diff = true;
                } else {
                    reporter_assert!(reporter, a_val == b_val && a_val_i == b_val_i);
                }
            }
            reporter_assert!(reporter, found_zero_sign_diff);
        } else {
            let mut found_nan = false;
            for i in 0..9usize {
                let a_val = a.get(i);
                let b_val = b.get(i);
                let a_val_i = float_bits(a_val);
                let b_val_i = float_bits(b_val);
                if a_val.is_nan() && a_val_i == b_val_i {
                    found_nan = true;
                } else {
                    reporter_assert!(reporter, a_val == b_val && a_val_i == b_val_i);
                }
            }
            reporter_assert!(reporter, found_nan);
        }
    }
    equal
}

// Port of: tests/MatrixTest.cpp#L90-L94 (chrome/m156)
fn is_identity(m: &Matrix) -> bool {
    let mut identity = Matrix::default();
    identity.reset();
    nearly_equal(m, &identity)
}

// Port of: tests/MatrixTest.cpp#L96-L121 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ helper
#[allow(clippy::float_cmp, clippy::many_single_char_names)] // names follow the C++; the C++ compares floats with ==
fn assert9(
    reporter: &mut Reporter,
    m: &Matrix,
    a: scalar,
    b: scalar,
    c: scalar,
    d: scalar,
    e: scalar,
    f: scalar,
    g: scalar,
    h: scalar,
    i: scalar,
) {
    let mut buffer = [0.0; 9];
    m.get_9(&mut buffer);
    reporter_assert!(reporter, buffer[0] == a);
    reporter_assert!(reporter, buffer[1] == b);
    reporter_assert!(reporter, buffer[2] == c);
    reporter_assert!(reporter, buffer[3] == d);
    reporter_assert!(reporter, buffer[4] == e);
    reporter_assert!(reporter, buffer[5] == f);
    reporter_assert!(reporter, buffer[6] == g);
    reporter_assert!(reporter, buffer[7] == h);
    reporter_assert!(reporter, buffer[8] == i);

    reporter_assert!(reporter, m.rc(0, 0) == a);
    reporter_assert!(reporter, m.rc(0, 1) == b);
    reporter_assert!(reporter, m.rc(0, 2) == c);
    reporter_assert!(reporter, m.rc(1, 0) == d);
    reporter_assert!(reporter, m.rc(1, 1) == e);
    reporter_assert!(reporter, m.rc(1, 2) == f);
    reporter_assert!(reporter, m.rc(2, 0) == g);
    reporter_assert!(reporter, m.rc(2, 1) == h);
    reporter_assert!(reporter, m.rc(2, 2) == i);
}

// Port of: tests/MatrixTest.cpp#L123-L143 (chrome/m156)
fn test_set9(reporter: &mut Reporter) {
    let mut m = Matrix::default();
    m.reset();
    assert9(reporter, &m, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);

    m.set_scale((2.0, 3.0), None);
    assert9(reporter, &m, 2.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 1.0);

    m.post_translate((4.0, 5.0));
    assert9(reporter, &m, 2.0, 0.0, 4.0, 0.0, 3.0, 5.0, 0.0, 0.0, 1.0);

    let mut buffer = [0.0; 9];
    buffer[Member::ScaleX as usize] = 1.0;
    buffer[Member::ScaleY as usize] = 1.0;
    buffer[Member::Persp2 as usize] = 1.0;
    reporter_assert!(reporter, !m.is_identity());
    m.set_9(&buffer);
    reporter_assert!(reporter, m.is_identity());
}

// Port of: tests/MatrixTest.cpp#L145-L189 (chrome/m156)
fn test_matrix_recttorect(reporter: &mut Reporter) {
    let mut src = Rect::default();
    let mut dst;

    src.set_ltrb(0.0, 0.0, 10.0, 10.0);
    dst = src;
    let mut matrix = Matrix::rect_2_rect(src, dst, None);
    reporter_assert!(reporter, matrix.is_some());
    reporter_assert!(
        reporter,
        TypeMask::IDENTITY == matrix.as_ref().map_or(TypeMask::all(), Matrix::get_type)
    );
    reporter_assert!(
        reporter,
        matrix.as_ref().is_some_and(Matrix::rect_stays_rect)
    );

    dst.offset((1.0, 1.0));
    matrix = Matrix::rect_2_rect(src, dst, None);
    reporter_assert!(reporter, matrix.is_some());
    reporter_assert!(
        reporter,
        TypeMask::TRANSLATE == matrix.as_ref().map_or(TypeMask::all(), Matrix::get_type)
    );
    reporter_assert!(
        reporter,
        matrix.as_ref().is_some_and(Matrix::rect_stays_rect)
    );

    dst.right += 1.0;
    matrix = Matrix::rect_2_rect(src, dst, None);
    reporter_assert!(reporter, matrix.is_some());
    reporter_assert!(
        reporter,
        (TypeMask::TRANSLATE | TypeMask::SCALE)
            == matrix.as_ref().map_or(TypeMask::all(), Matrix::get_type)
    );
    reporter_assert!(
        reporter,
        matrix.as_ref().is_some_and(Matrix::rect_stays_rect)
    );

    dst = src;
    dst.right = src.right * 2.0;
    matrix = Matrix::rect_2_rect(src, dst, None);
    reporter_assert!(reporter, matrix.is_some());
    reporter_assert!(
        reporter,
        TypeMask::SCALE == matrix.as_ref().map_or(TypeMask::all(), Matrix::get_type)
    );
    reporter_assert!(
        reporter,
        matrix.as_ref().is_some_and(Matrix::rect_stays_rect)
    );

    // Now test handling failure (when src is empty)
    src = Rect::from_xywh(10.0, 20.0, 0.0, 0.0);
    matrix = Matrix::rect_2_rect(src, dst, None);
    reporter_assert!(reporter, matrix.is_none());
    let mx = Matrix::rect_to_rect_or_identity(src, dst, None);
    reporter_assert!(reporter, TypeMask::IDENTITY == mx.get_type());

    {
        let mut m = Matrix::translate((20.0, 20.0));
        reporter_assert!(
            reporter,
            !m.set_rect_to_rect(
                Rect::new_empty(),
                Rect::from_wh(10.0, 20.0),
                ScaleToFit::Center
            )
        );
        // setRectToRect failures are expected to reset the matrix.
        reporter_assert!(reporter, m.is_identity());
    }
}

// Port of: tests/MatrixTest.cpp#L191-L209 (chrome/m156)
fn test_flatten(reporter: &mut Reporter, m: &Matrix) {
    // add 100 in case we have a bug, I don't want to kill my stack in the test
    const K_BUFFER_SIZE: usize = MAX_FLATTEN_SIZE + 100;
    let mut buffer = [0u8; K_BUFFER_SIZE];
    let size1 = matrix_priv::write_to_memory(m, None);
    let size2 = matrix_priv::write_to_memory(m, Some(&mut buffer));
    reporter_assert!(reporter, size1 == size2);
    reporter_assert!(reporter, size1 <= MAX_FLATTEN_SIZE);

    let mut m2 = Matrix::default();
    let mut size3 = matrix_priv::read_from_memory(&mut m2, &buffer);
    reporter_assert!(reporter, size1 == size3);
    reporter_assert!(reporter, are_equal(reporter, m, &m2));

    let mut buffer2 = [0u8; K_BUFFER_SIZE];
    size3 = matrix_priv::write_to_memory(&m2, Some(&mut buffer2));
    reporter_assert!(reporter, size1 == size3);
    reporter_assert!(reporter, buffer[..size1] == buffer2[..size1]);
}

// `SkMatrix::getMinMaxScales(SkScalar scaleFactors[2])`: the C++ out-parameter form, which
// leaves `scale_factors` unchanged when it fails.
fn get_min_max_scales(m: &Matrix, scale_factors: &mut [scalar; 2]) -> bool {
    if let Some((min, max)) = m.min_max_scales() {
        *scale_factors = [min, max];
        true
    } else {
        false
    }
}

// `SkMatrix::invert(SkMatrix* inverse)`: the C++ out-parameter form, which writes `inverse` only
// when it succeeds.
fn invert_into(m: &Matrix, inverse: Option<&mut Matrix>) -> bool {
    if let Some(inv) = m.invert() {
        if let Some(inverse) = inverse {
            *inverse = inv;
        }
        return true;
    }
    false
}

// Port of: tests/MatrixTest.cpp#L211-L350 (chrome/m156)
#[allow(
    clippy::excessive_precision,
    clippy::float_cmp,
    clippy::items_after_statements,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::too_many_lines
)] // float literals are copied verbatim from the C++; mirrors the local constants of the C++; mirrors the long C++ test; the C++ asserts float comparisons directly; the C++ compares floats with ==
fn test_matrix_min_max_scale(reporter: &mut Reporter) {
    let mut scales = [0.0; 2];
    let mut success;

    let mut identity = Matrix::default();
    identity.reset();
    reporter_assert!(reporter, 1.0 == identity.min_scale());
    reporter_assert!(reporter, 1.0 == identity.max_scale());
    success = get_min_max_scales(&identity, &mut scales);
    reporter_assert!(reporter, success && 1.0 == scales[0] && 1.0 == scales[1]);

    let mut scale = Matrix::default();
    scale.set_scale((2.0, 4.0), None);
    reporter_assert!(reporter, 2.0 == scale.min_scale());
    reporter_assert!(reporter, 4.0 == scale.max_scale());
    success = get_min_max_scales(&scale, &mut scales);
    reporter_assert!(reporter, success && 2.0 == scales[0] && 4.0 == scales[1]);

    let mut rot90_scale = Matrix::default();
    rot90_scale
        .set_rotate(90.0, None)
        .post_scale((SCALAR_1 / 4.0, SCALAR_1 / 2.0), None);
    reporter_assert!(reporter, SCALAR_1 / 4.0 == rot90_scale.min_scale());
    reporter_assert!(reporter, SCALAR_1 / 2.0 == rot90_scale.max_scale());
    success = get_min_max_scales(&rot90_scale, &mut scales);
    reporter_assert!(
        reporter,
        success && SCALAR_1 / 4.0 == scales[0] && SCALAR_1 / 2.0 == scales[1]
    );

    let mut rotate = Matrix::default();
    rotate.set_rotate(128.0, None);
    reporter_assert!(
        reporter,
        scalar::nearly_equal(1.0, rotate.min_scale(), SCALAR_NEARLY_ZERO)
    );
    reporter_assert!(
        reporter,
        scalar::nearly_equal(1.0, rotate.max_scale(), SCALAR_NEARLY_ZERO)
    );
    success = get_min_max_scales(&rotate, &mut scales);
    reporter_assert!(reporter, success);
    reporter_assert!(
        reporter,
        scalar::nearly_equal(1.0, scales[0], SCALAR_NEARLY_ZERO)
    );
    reporter_assert!(
        reporter,
        scalar::nearly_equal(1.0, scales[1], SCALAR_NEARLY_ZERO)
    );

    let mut translate = Matrix::default();
    translate.set_translate((10.0, -5.0));
    reporter_assert!(reporter, 1.0 == translate.min_scale());
    reporter_assert!(reporter, 1.0 == translate.max_scale());
    success = get_min_max_scales(&translate, &mut scales);
    reporter_assert!(reporter, success && 1.0 == scales[0] && 1.0 == scales[1]);

    let mut persp_x = Matrix::default();
    persp_x.reset().set_persp_x(SCALAR_1 / 1000.0);
    reporter_assert!(reporter, -1.0 == persp_x.min_scale());
    reporter_assert!(reporter, -1.0 == persp_x.max_scale());
    success = get_min_max_scales(&persp_x, &mut scales);
    reporter_assert!(reporter, !success);

    // skbug.com/40035872
    let mut big = Matrix::default();
    big.set_all(
        2.393_940_89e+36,
        8.853_477_79e+36,
        9.265_262_04e+36,
        3.915_961_9e+36,
        1.448_234_53e+37,
        1.515_593_42e+37,
        0.0,
        0.0,
        1.0,
    );
    success = get_min_max_scales(&big, &mut scales);
    reporter_assert!(reporter, !success);

    // skbug.com/40035872
    let mut giving_negative_nearly_zeros = Matrix::default();
    giving_negative_nearly_zeros.set_all(
        0.004_365_34,
        0.114_138,
        0.371_41,
        0.003_588_57,
        0.093_622_8,
        -0.017_419_8,
        0.0,
        0.0,
        1.0,
    );
    success = get_min_max_scales(&giving_negative_nearly_zeros, &mut scales);
    reporter_assert!(reporter, success && 0.0 == scales[0]);

    let mut persp_y = Matrix::default();
    persp_y.reset().set_persp_y(-SCALAR_1 / 500.0);
    reporter_assert!(reporter, -1.0 == persp_y.min_scale());
    reporter_assert!(reporter, -1.0 == persp_y.max_scale());
    scales[0] = -5.0;
    scales[1] = -5.0;
    success = get_min_max_scales(&persp_y, &mut scales);
    reporter_assert!(reporter, !success && -5.0 == scales[0] && -5.0 == scales[1]);

    let base_mats = [
        scale.clone(),
        rot90_scale.clone(),
        rotate.clone(),
        translate.clone(),
        persp_x.clone(),
        persp_y.clone(),
    ];
    let mut mats: [Matrix; 12] = std::array::from_fn(|_| Matrix::default());
    for i in 0..base_mats.len() {
        mats[i] = base_mats[i].clone();
        let mut inverse = Matrix::default();
        let invertible = invert_into(&mats[i], Some(&mut inverse));
        mats[i + base_mats.len()] = inverse;
        reporter_assert!(reporter, invertible);
    }
    let mut rand = Random::default();
    let mut m: i32 = 0;
    while m < 1000 {
        let mut mat = Matrix::default();
        mat.reset();
        for _ in 0..4 {
            let x = (rand.next_u() as usize) % mats.len();
            mat.post_concat(&mats[x]);
        }

        let min_scale = mat.min_scale();
        let max_scale = mat.max_scale();
        reporter_assert!(reporter, (min_scale < 0.0) == (max_scale < 0.0));
        reporter_assert!(reporter, (max_scale < 0.0) == mat.has_perspective());

        success = get_min_max_scales(&mat, &mut scales);
        reporter_assert!(reporter, success != mat.has_perspective());
        reporter_assert!(
            reporter,
            !success || (scales[0] == min_scale && scales[1] == max_scale)
        );

        if mat.has_perspective() {
            // C++: `m -= 1; continue;` (undoing the for loop's `++m`) to try another non-persp matrix
            continue;
        }

        // test a bunch of vectors. All should be scaled by between minScale and maxScale
        // (modulo some error) and we should find a vector that is scaled by almost each.
        const G_VECTOR_SCALE_TOL: scalar = (105.0 * SCALAR_1) / 100.0;
        const G_CLOSE_SCALE_TOL: scalar = (97.0 * SCALAR_1) / 100.0;
        let mut max: scalar = 0.0;
        let mut min: scalar = SCALAR_MAX;
        let mut vectors = [Vector::default(); 1000];
        let mut i = 0;
        while i < vectors.len() {
            vectors[i].x = rand.next_s_scalar1();
            vectors[i].y = rand.next_s_scalar1();
            if !vectors[i].normalize() {
                // C++: `i -= 1; continue;` (undoing the for loop's `++i`)
                continue;
            }
            i += 1;
        }
        mat.map_vectors_inplace(&mut vectors);
        for v in &vectors {
            let d = v.length();
            reporter_assert!(reporter, d / max_scale < G_VECTOR_SCALE_TOL);
            reporter_assert!(reporter, min_scale / d < G_VECTOR_SCALE_TOL);
            if max < d {
                max = d;
            }
            if min > d {
                min = d;
            }
        }
        reporter_assert!(reporter, max / max_scale >= G_CLOSE_SCALE_TOL);
        reporter_assert!(reporter, min_scale / min >= G_CLOSE_SCALE_TOL);
        m += 1;
    }
}

// Port of: tests/MatrixTest.cpp#L352-L478 (chrome/m156)
fn test_matrix_preserve_shape(reporter: &mut Reporter) {
    let mut mat = Matrix::default();

    // identity
    mat.set_identity();
    reporter_assert!(reporter, mat.is_similarity());
    reporter_assert!(reporter, mat.preserves_right_angles());

    // translation only
    mat.set_translate((100.0, 100.0));
    reporter_assert!(reporter, mat.is_similarity());
    reporter_assert!(reporter, mat.preserves_right_angles());

    // scale with same size
    mat.set_scale((15.0, 15.0), None);
    reporter_assert!(reporter, mat.is_similarity());
    reporter_assert!(reporter, mat.preserves_right_angles());

    // scale with one negative
    mat.set_scale((-15.0, 15.0), None);
    reporter_assert!(reporter, mat.is_similarity());
    reporter_assert!(reporter, mat.preserves_right_angles());

    // scale with different size
    mat.set_scale((15.0, 20.0), None);
    reporter_assert!(reporter, !mat.is_similarity());
    reporter_assert!(reporter, mat.preserves_right_angles());

    // scale with same size at a pivot point
    mat.set_scale((15.0, 15.0), Point::new(2.0, 2.0));
    reporter_assert!(reporter, mat.is_similarity());
    reporter_assert!(reporter, mat.preserves_right_angles());

    // scale with different size at a pivot point
    mat.set_scale((15.0, 20.0), Point::new(2.0, 2.0));
    reporter_assert!(reporter, !mat.is_similarity());
    reporter_assert!(reporter, mat.preserves_right_angles());

    // skew with same size
    mat.set_skew((15.0, 15.0), None);
    reporter_assert!(reporter, !mat.is_similarity());
    reporter_assert!(reporter, !mat.preserves_right_angles());

    // skew with different size
    mat.set_skew((15.0, 20.0), None);
    reporter_assert!(reporter, !mat.is_similarity());
    reporter_assert!(reporter, !mat.preserves_right_angles());

    // skew with same size at a pivot point
    mat.set_skew((15.0, 15.0), Point::new(2.0, 2.0));
    reporter_assert!(reporter, !mat.is_similarity());
    reporter_assert!(reporter, !mat.preserves_right_angles());

    // skew with different size at a pivot point
    mat.set_skew((15.0, 20.0), Point::new(2.0, 2.0));
    reporter_assert!(reporter, !mat.is_similarity());
    reporter_assert!(reporter, !mat.preserves_right_angles());

    // perspective x
    mat.reset().set_persp_x(SCALAR_1 / 2.0);
    reporter_assert!(reporter, !mat.is_similarity());
    reporter_assert!(reporter, !mat.preserves_right_angles());

    // perspective y
    mat.reset().set_persp_y(SCALAR_1 / 2.0);
    reporter_assert!(reporter, !mat.is_similarity());
    reporter_assert!(reporter, !mat.preserves_right_angles());

    // rotate
    for angle in 0..360 {
        mat.set_rotate(int_to_scalar(angle), None);
        reporter_assert!(reporter, mat.is_similarity());
        reporter_assert!(reporter, mat.preserves_right_angles());
    }

    // see if there are any accumulated precision issues
    mat.reset();
    for _ in 1..360 {
        mat.post_rotate(1.0, None);
    }
    reporter_assert!(reporter, mat.is_similarity());
    reporter_assert!(reporter, mat.preserves_right_angles());

    // rotate + translate
    mat.set_rotate(30.0, None).post_translate((10.0, 20.0));
    reporter_assert!(reporter, mat.is_similarity());
    reporter_assert!(reporter, mat.preserves_right_angles());

    // rotate + uniform scale
    mat.set_rotate(30.0, None).post_scale((2.0, 2.0), None);
    reporter_assert!(reporter, mat.is_similarity());
    reporter_assert!(reporter, mat.preserves_right_angles());

    // rotate + non-uniform scale
    mat.set_rotate(30.0, None).post_scale((3.0, 2.0), None);
    reporter_assert!(reporter, !mat.is_similarity());
    reporter_assert!(reporter, !mat.preserves_right_angles());

    // non-uniform scale + rotate
    mat.set_scale((3.0, 2.0), None).post_rotate(30.0, None);
    reporter_assert!(reporter, !mat.is_similarity());
    reporter_assert!(reporter, mat.preserves_right_angles());

    // all zero
    mat.set_all(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    reporter_assert!(reporter, !mat.is_similarity());
    reporter_assert!(reporter, !mat.preserves_right_angles());

    // all zero except perspective
    mat.set_all(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
    reporter_assert!(reporter, !mat.is_similarity());
    reporter_assert!(reporter, !mat.preserves_right_angles());

    // scales zero, only skews (rotation)
    mat.set_all(0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0);
    reporter_assert!(reporter, mat.is_similarity());
    reporter_assert!(reporter, mat.preserves_right_angles());

    // scales zero, only skews (reflection)
    mat.set_all(0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0);
    reporter_assert!(reporter, mat.is_similarity());
    reporter_assert!(reporter, mat.preserves_right_angles());
}

// For test_matrix_decomposition, below.
// Port of: tests/MatrixTest.cpp#L480-L500 (chrome/m156)
fn scalar_nearly_equal_relative(a: scalar, b: scalar, tolerance: scalar) -> bool {
    // from Bruce Dawson
    // absolute check
    let diff = scalar_abs(a - b);
    if diff < tolerance {
        return true;
    }

    // relative check
    let a = scalar_abs(a);
    let b = scalar_abs(b);
    let largest = if b > a { b } else { a };

    if diff <= largest * tolerance {
        return true;
    }

    false
}

// Port of: tests/MatrixTest.cpp#L502-L523 (chrome/m156)
fn check_matrix_recomposition(
    mat: &Matrix,
    rotation1: Point,
    scale: Point,
    rotation2: Point,
) -> bool {
    let c1 = rotation1.x;
    let s1 = rotation1.y;
    let scale_x = scale.x;
    let scale_y = scale.y;
    let c2 = rotation2.x;
    let s2 = rotation2.y;

    // We do a relative check here because large scale factors cause problems with an absolute check
    scalar_nearly_equal_relative(
        mat[Member::ScaleX],
        scale_x * c1 * c2 - scale_y * s1 * s2,
        SCALAR_NEARLY_ZERO,
    ) && scalar_nearly_equal_relative(
        mat[Member::SkewX],
        -scale_x * s1 * c2 - scale_y * c1 * s2,
        SCALAR_NEARLY_ZERO,
    ) && scalar_nearly_equal_relative(
        mat[Member::SkewY],
        scale_x * c1 * s2 + scale_y * s1 * c2,
        SCALAR_NEARLY_ZERO,
    ) && scalar_nearly_equal_relative(
        mat[Member::ScaleY],
        -scale_x * s1 * s2 + scale_y * c1 * c2,
        SCALAR_NEARLY_ZERO,
    )
}

// Port of: tests/MatrixTest.cpp#L525-L657 (chrome/m156)
#[allow(clippy::items_after_statements, clippy::too_many_lines)] // mirrors the local constants of the C++; mirrors the long C++ test
fn test_matrix_decomposition(reporter: &mut Reporter) {
    let mut mat = Matrix::default();
    let mut rotation1 = Point::default();
    let mut scale = Point::default();
    let mut rotation2 = Point::default();

    const K_ROTATION0: f32 = 15.5;
    const K_ROTATION1: f32 = -50.0;
    const K_SCALE0: f32 = 5000.0;
    const K_SCALE1: f32 = 0.001;

    // Runs the decomposition of `mat` into the three points above.
    macro_rules! decompose {
        ($mat:expr) => {
            decompose_upper_2x2(
                &$mat,
                Some(&mut rotation1),
                Some(&mut scale),
                Some(&mut rotation2),
            )
        };
    }

    // identity
    mat.reset();
    reporter_assert!(reporter, decompose!(mat));
    reporter_assert!(
        reporter,
        check_matrix_recomposition(&mat, rotation1, scale, rotation2)
    );
    // make sure it doesn't crash if we pass in NULLs
    reporter_assert!(reporter, decompose_upper_2x2(&mat, None, None, None));

    // rotation only
    mat.set_rotate(K_ROTATION0, None);
    reporter_assert!(reporter, decompose!(mat));
    reporter_assert!(
        reporter,
        check_matrix_recomposition(&mat, rotation1, scale, rotation2)
    );

    // uniform scale only
    mat.set_scale((K_SCALE0, K_SCALE0), None);
    reporter_assert!(reporter, decompose!(mat));
    reporter_assert!(
        reporter,
        check_matrix_recomposition(&mat, rotation1, scale, rotation2)
    );

    // anisotropic scale only
    mat.set_scale((K_SCALE1, K_SCALE0), None);
    reporter_assert!(reporter, decompose!(mat));
    reporter_assert!(
        reporter,
        check_matrix_recomposition(&mat, rotation1, scale, rotation2)
    );

    // rotation then uniform scale
    mat.set_rotate(K_ROTATION1, None)
        .post_scale((K_SCALE0, K_SCALE0), None);
    reporter_assert!(reporter, decompose!(mat));
    reporter_assert!(
        reporter,
        check_matrix_recomposition(&mat, rotation1, scale, rotation2)
    );

    // uniform scale then rotation
    mat.set_scale((K_SCALE0, K_SCALE0), None)
        .post_rotate(K_ROTATION1, None);
    reporter_assert!(reporter, decompose!(mat));
    reporter_assert!(
        reporter,
        check_matrix_recomposition(&mat, rotation1, scale, rotation2)
    );

    // rotation then uniform scale+reflection
    mat.set_rotate(K_ROTATION0, None)
        .post_scale((K_SCALE1, -K_SCALE1), None);
    reporter_assert!(reporter, decompose!(mat));
    reporter_assert!(
        reporter,
        check_matrix_recomposition(&mat, rotation1, scale, rotation2)
    );

    // uniform scale+reflection, then rotate
    mat.set_scale((K_SCALE0, -K_SCALE0), None)
        .post_rotate(K_ROTATION1, None);
    reporter_assert!(reporter, decompose!(mat));
    reporter_assert!(
        reporter,
        check_matrix_recomposition(&mat, rotation1, scale, rotation2)
    );

    // rotation then anisotropic scale
    mat.set_rotate(K_ROTATION1, None)
        .post_scale((K_SCALE1, K_SCALE0), None);
    reporter_assert!(reporter, decompose!(mat));
    reporter_assert!(
        reporter,
        check_matrix_recomposition(&mat, rotation1, scale, rotation2)
    );

    // rotation then anisotropic scale
    mat.set_rotate(90.0, None)
        .post_scale((K_SCALE1, K_SCALE0), None);
    reporter_assert!(reporter, decompose!(mat));
    reporter_assert!(
        reporter,
        check_matrix_recomposition(&mat, rotation1, scale, rotation2)
    );

    // anisotropic scale then rotation
    mat.set_scale((K_SCALE1, K_SCALE0), None)
        .post_rotate(K_ROTATION0, None);
    reporter_assert!(reporter, decompose!(mat));
    reporter_assert!(
        reporter,
        check_matrix_recomposition(&mat, rotation1, scale, rotation2)
    );

    // anisotropic scale then rotation
    mat.set_scale((K_SCALE1, K_SCALE0), None)
        .post_rotate(90.0, None);
    reporter_assert!(reporter, decompose!(mat));
    reporter_assert!(
        reporter,
        check_matrix_recomposition(&mat, rotation1, scale, rotation2)
    );

    // rotation, uniform scale, then different rotation
    mat.set_rotate(K_ROTATION1, None)
        .post_scale((K_SCALE0, K_SCALE0), None)
        .post_rotate(K_ROTATION0, None);
    reporter_assert!(reporter, decompose!(mat));
    reporter_assert!(
        reporter,
        check_matrix_recomposition(&mat, rotation1, scale, rotation2)
    );

    // rotation, anisotropic scale, then different rotation
    mat.set_rotate(K_ROTATION0, None)
        .post_scale((K_SCALE1, K_SCALE0), None)
        .post_rotate(K_ROTATION1, None);
    reporter_assert!(reporter, decompose!(mat));
    reporter_assert!(
        reporter,
        check_matrix_recomposition(&mat, rotation1, scale, rotation2)
    );

    // rotation, anisotropic scale + reflection, then different rotation
    mat.set_rotate(K_ROTATION0, None)
        .post_scale((-K_SCALE1, K_SCALE0), None)
        .post_rotate(K_ROTATION1, None);
    reporter_assert!(reporter, decompose!(mat));
    reporter_assert!(
        reporter,
        check_matrix_recomposition(&mat, rotation1, scale, rotation2)
    );

    // try some random matrices
    let mut rand = Random::default();
    for _ in 0..1000 {
        let rot0 = rand.next_range_f(-180.0, 180.0);
        let sx = rand.next_range_f(-3000.0, 3000.0);
        let sy = rand.next_range_f(-3000.0, 3000.0);
        let rot1 = rand.next_range_f(-180.0, 180.0);
        mat.set_rotate(rot0, None)
            .post_scale((sx, sy), None)
            .post_rotate(rot1, None);

        if decompose!(mat) {
            reporter_assert!(
                reporter,
                check_matrix_recomposition(&mat, rotation1, scale, rotation2)
            );
        } else {
            // if the matrix is degenerate, the basis vectors should be near-parallel or near-zero
            let perpdot =
                mat[Member::ScaleX] * mat[Member::ScaleY] - mat[Member::SkewX] * mat[Member::SkewY];
            reporter_assert!(reporter, perpdot.nearly_zero(None));
        }
    }

    // translation shouldn't affect this
    mat.post_translate((-1000.0, 1000.0));
    reporter_assert!(reporter, decompose!(mat));
    reporter_assert!(
        reporter,
        check_matrix_recomposition(&mat, rotation1, scale, rotation2)
    );

    // perspective shouldn't affect this
    mat[Member::Persp0] = 12.0;
    mat[Member::Persp1] = 4.0;
    mat[Member::Persp2] = 1872.0;
    reporter_assert!(reporter, decompose!(mat));
    reporter_assert!(
        reporter,
        check_matrix_recomposition(&mat, rotation1, scale, rotation2)
    );

    // degenerate matrices
    // mostly zero entries
    mat.reset();
    mat[Member::ScaleX] = 0.0;
    reporter_assert!(reporter, !decompose!(mat));
    mat.reset();
    mat[Member::ScaleY] = 0.0;
    reporter_assert!(reporter, !decompose!(mat));
    mat.reset();
    // linearly dependent entries
    mat[Member::ScaleX] = 1.0;
    mat[Member::SkewX] = 2.0;
    mat[Member::SkewY] = 4.0;
    mat[Member::ScaleY] = 8.0;
    reporter_assert!(reporter, !decompose!(mat));
}

// For test_matrix_homogeneous, below.
// Port of: tests/MatrixTest.cpp#L659-L673 (chrome/m156)
fn point3_array_nearly_equal_relative(a: &[Point3], b: &[Point3], count: usize) -> bool {
    for i in 0..count {
        if !scalar_nearly_equal_relative(a[i].x, b[i].x, SCALAR_NEARLY_ZERO) {
            return false;
        }
        if !scalar_nearly_equal_relative(a[i].y, b[i].y, SCALAR_NEARLY_ZERO) {
            return false;
        }
        if !scalar_nearly_equal_relative(a[i].z, b[i].z, SCALAR_NEARLY_ZERO) {
            return false;
        }
    }
    true
}

// For test_matrix_homogeneous, below.
// Maps a single triple in src using m and compares results to those in dst
// Port of: tests/MatrixTest.cpp#L675-L687 (chrome/m156)
fn naive_homogeneous_mapping(m: &Matrix, src: &Point3, dst: &Point3) -> bool {
    let mut res = Point3::default();
    let ms = [m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8]];
    res.x = src.x * ms[0] + src.y * ms[1] + src.z * ms[2];
    res.y = src.x * ms[3] + src.y * ms[4] + src.z * ms[5];
    res.z = src.x * ms[6] + src.y * ms[7] + src.z * ms[8];
    point3_array_nearly_equal_relative(std::slice::from_ref(&res), std::slice::from_ref(dst), 1)
}

// Port of: tests/MatrixTest.cpp#L689-L795 (chrome/m156)
#[allow(clippy::items_after_statements, clippy::too_many_lines)] // mirrors the local constants of the C++; mirrors the long C++ test
fn test_matrix_homogeneous(reporter: &mut Reporter) {
    let mut mat = Matrix::default();

    const K_ROTATION0: f32 = 15.5;
    const K_ROTATION1: f32 = -50.0;
    const K_SCALE0: f32 = 5000.0;
    const K_TRIPLE_COUNT: usize = 1000;
    const K_MATRIX_COUNT: usize = 1000;
    let mut rand = Random::default();

    let mut rand_triples = [Point3::default(); K_TRIPLE_COUNT];
    for triple in &mut rand_triples {
        triple.x = rand.next_range_f(-3000.0, 3000.0);
        triple.y = rand.next_range_f(-3000.0, 3000.0);
        triple.z = rand.next_range_f(-3000.0, 3000.0);
    }

    let mut mats: [Matrix; K_MATRIX_COUNT] = std::array::from_fn(|_| Matrix::default());
    for m in &mut mats {
        for j in 0..9usize {
            m.set(j, rand.next_range_f(-3000.0, 3000.0));
        }
    }

    // identity
    {
        mat.reset();
        let mut dst = [Point3::default(); K_TRIPLE_COUNT];
        mat.map_homogeneous_points(&mut dst, &rand_triples);
        reporter_assert!(
            reporter,
            point3_array_nearly_equal_relative(&rand_triples, &dst, K_TRIPLE_COUNT)
        );
    }

    let zeros = Point3::new(0.0, 0.0, 0.0);
    // zero matrix
    {
        mat.set_all(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let mut dst = [Point3::default(); K_TRIPLE_COUNT];
        mat.map_homogeneous_points(&mut dst, &rand_triples);
        for d in &dst {
            reporter_assert!(
                reporter,
                point3_array_nearly_equal_relative(
                    std::slice::from_ref(d),
                    std::slice::from_ref(&zeros),
                    1
                )
            );
        }
    }

    // zero point
    {
        for m in &mats {
            let dst = m.map_homogeneous_point(zeros);
            reporter_assert!(
                reporter,
                point3_array_nearly_equal_relative(
                    std::slice::from_ref(&dst),
                    std::slice::from_ref(&zeros),
                    1
                )
            );
        }
    }

    // doesn't crash with empty spans
    {
        mats[0].map_homogeneous_points(&mut [], &[]);
    }

    // uniform scale of point
    {
        mat.set_scale((K_SCALE0, K_SCALE0), None);
        let src = Point3::new(rand_triples[0].x, rand_triples[0].y, 1.0);
        let mut pnt = Point::new(src.x, src.y);
        let dst = mat.map_homogeneous_point(src);
        pnt = mat.map_point(pnt);
        reporter_assert!(reporter, scalar::nearly_equal(dst.x, pnt.x, None));
        reporter_assert!(reporter, scalar::nearly_equal(dst.y, pnt.y, None));
        reporter_assert!(reporter, scalar::nearly_equal(dst.z, 1.0, None));
    }

    // rotation of point
    {
        mat.set_rotate(K_ROTATION0, None);
        let src = Point3::new(rand_triples[0].x, rand_triples[0].y, 1.0);
        let mut pnt = Point::default();
        pnt.set(src.x, src.y);
        let dst = mat.map_homogeneous_point(src);
        pnt = mat.map_point(pnt);
        reporter_assert!(reporter, scalar::nearly_equal(dst.x, pnt.x, None));
        reporter_assert!(reporter, scalar::nearly_equal(dst.y, pnt.y, None));
        reporter_assert!(reporter, scalar::nearly_equal(dst.z, 1.0, None));
    }

    // rotation, scale, rotation of point
    {
        mat.set_rotate(K_ROTATION1, None);
        mat.post_scale((K_SCALE0, K_SCALE0), None);
        mat.post_rotate(K_ROTATION0, None);
        let src = Point3::new(rand_triples[0].x, rand_triples[0].y, 1.0);
        let mut pnt = Point::default();
        pnt.set(src.x, src.y);
        let dst = mat.map_homogeneous_point(src);
        pnt = mat.map_point(pnt);
        reporter_assert!(reporter, scalar::nearly_equal(dst.x, pnt.x, None));
        reporter_assert!(reporter, scalar::nearly_equal(dst.y, pnt.y, None));
        reporter_assert!(reporter, scalar::nearly_equal(dst.z, 1.0, None));
    }

    // compare with naive approach
    {
        for m in &mats {
            for triple in &rand_triples {
                let dst = m.map_homogeneous_point(*triple);
                reporter_assert!(reporter, naive_homogeneous_mapping(m, triple, &dst));
            }
        }
    }
}

// Port of: tests/MatrixTest.cpp#L797-L846 (chrome/m156)
#[allow(clippy::items_after_statements)] // mirrors the local constants of the C++
fn check_decomp_scale(original: &Matrix) -> bool {
    let mut remaining = Matrix::default();

    let Some(scale) = original.decompose_scale(Some(&mut remaining)) else {
        return false;
    };
    if scale.width <= 0.0 || scale.height <= 0.0 {
        return false;
    }

    // First ensure that the decomposition reconstitutes back to the original
    {
        let mut reconstituted = remaining.clone();

        reconstituted.pre_scale((scale.width, scale.height), None);
        if !nearly_equal(original, &reconstituted) {
            return false;
        }
    }

    // Then push some points through both paths and make sure they are the same.
    const K_NUM_POINTS: usize = 5;
    let test_pts: [Point; K_NUM_POINTS] = [
        Point::new(0.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(1.0, 0.5),
        Point::new(-1.0, -0.5),
        Point::new(-1.0, 2.0),
    ];

    let mut v1 = [Point::default(); K_NUM_POINTS];
    original.map_points(&mut v1, &test_pts);

    let mut v2 = [Point::default(); K_NUM_POINTS];
    let scale_mat = Matrix::scale((scale.width, scale.height));

    // Note, we intend the decomposition to be applied in the order scale and then remainder but,
    // due to skbug.com/40038455, the order is reversed!
    scale_mat.map_points(&mut v2, &test_pts);
    remaining.map_points_inplace(&mut v2);

    for i in 0..K_NUM_POINTS {
        if !point_priv::equals_within_tolerance_tol(v1[i], v2[i], 0.00001) {
            return false;
        }
    }

    true
}

// Port of: tests/MatrixTest.cpp#L848-L866 (chrome/m156)
fn test_decomp_scale(reporter: &mut Reporter) {
    let mut m = Matrix::default();

    m.reset();
    reporter_assert!(reporter, check_decomp_scale(&m));
    m.set_scale((2.0, 3.0), None);
    reporter_assert!(reporter, check_decomp_scale(&m));
    m.set_rotate(35.0, Point::new(0.0, 0.0));
    reporter_assert!(reporter, check_decomp_scale(&m));

    m.set_scale((1.0, 0.0), None);
    reporter_assert!(reporter, !check_decomp_scale(&m));

    m.set_rotate(35.0, Point::new(0.0, 0.0))
        .pre_scale((2.0, 3.0), None);
    reporter_assert!(reporter, check_decomp_scale(&m));

    m.set_rotate(35.0, Point::new(0.0, 0.0))
        .post_scale((2.0, 3.0), None);
    reporter_assert!(reporter, check_decomp_scale(&m));
}

// Port of: tests/MatrixTest.cpp#L868-L1034 (chrome/m156)
// Allowed lints: float literals are copied verbatim from the C++; the C++ compares floats with ==
def_test!(
    #[allow(clippy::excessive_precision, clippy::float_cmp)]
    Matrix,
    |reporter| {
        let mut mat = Matrix::default();
        let mut inverse = Matrix::default();
        let mut iden1 = Matrix::default();
        let mut iden2 = Matrix::default();

        mat.reset();
        mat.set_translate((1.0, 1.0));
        reporter_assert!(reporter, invert_into(&mat, Some(&mut inverse)));
        iden1.set_concat(&mat, &inverse);
        reporter_assert!(reporter, is_identity(&iden1));

        mat.set_scale((2.0, 4.0), None);
        reporter_assert!(reporter, invert_into(&mat, Some(&mut inverse)));
        iden1.set_concat(&mat, &inverse);
        reporter_assert!(reporter, is_identity(&iden1));
        test_flatten(reporter, &mat);

        mat.set_scale((SCALAR_1 / 2.0, 2.0), None);
        reporter_assert!(reporter, invert_into(&mat, Some(&mut inverse)));
        iden1.set_concat(&mat, &inverse);
        reporter_assert!(reporter, is_identity(&iden1));
        test_flatten(reporter, &mat);

        mat.set_scale((3.0, 5.0), Point::new(20.0, 0.0))
            .post_rotate(25.0, None);
        reporter_assert!(reporter, invert_into(&mat, None));
        reporter_assert!(reporter, invert_into(&mat, Some(&mut inverse)));
        iden1.set_concat(&mat, &inverse);
        reporter_assert!(reporter, is_identity(&iden1));
        iden2.set_concat(&inverse, &mat);
        reporter_assert!(reporter, is_identity(&iden2));
        test_flatten(reporter, &mat);
        test_flatten(reporter, &iden2);

        mat.set_scale((0.0, 1.0), None);
        reporter_assert!(reporter, !invert_into(&mat, None));
        reporter_assert!(reporter, !invert_into(&mat, Some(&mut inverse)));
        mat.set_scale((1.0, 0.0), None);
        reporter_assert!(reporter, !invert_into(&mat, None));
        reporter_assert!(reporter, !invert_into(&mat, Some(&mut inverse)));

        // Inverting this matrix results in a non-finite matrix
        mat.set_all(
            0.0,
            1.0,
            2.0,
            0.0,
            1.0,
            -3.402_771_75e+38,
            1.000_030_40,
            1.0,
            0.0,
        );
        reporter_assert!(reporter, !invert_into(&mat, None));
        reporter_assert!(reporter, !invert_into(&mat, Some(&mut inverse)));

        // Inverting this matrix results in a non-finite matrix (1/scale overflows to infinity)
        // b/378231198: Previously this would pass invert() if a null inverse pointer was passed in.
        mat.set_all(
            f32::from_bits(1), // std::numeric_limits<float>::denorm_min()
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        reporter_assert!(reporter, mat.is_scale_translate());
        reporter_assert!(reporter, !invert_into(&mat, None));
        reporter_assert!(reporter, !invert_into(&mat, Some(&mut inverse)));

        // b/378231198: This matrix shouldn't be invertible, but previously the translation wasn't being
        // validated when taking the optimized scale+translate paths.
        mat.set_all(2.0, 0.0, f32::NAN, 0.0, 2.0, 0.0, 0.0, 0.0, 1.0);
        reporter_assert!(reporter, mat.is_scale_translate());
        reporter_assert!(reporter, !invert_into(&mat, None));
        reporter_assert!(reporter, !invert_into(&mat, Some(&mut inverse)));
        // Variant that tests the translate-only optimized invert()
        mat.set_all(1.0, 0.0, f32::NAN, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
        reporter_assert!(reporter, mat.is_translate());
        reporter_assert!(reporter, !invert_into(&mat, None));
        reporter_assert!(reporter, !invert_into(&mat, Some(&mut inverse)));

        // A finite scale+translate matrix whose inverse can't be calculated because trans/scale
        // becomes non-finite.
        mat.set_all(
            f32::MIN_POSITIVE, // std::numeric_limits<float>::min()
            0.0,
            f32::MAX, // std::numeric_limits<float>::max()
            0.0,
            1.0,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        reporter_assert!(reporter, mat.is_scale_translate());
        reporter_assert!(reporter, mat.is_finite());
        reporter_assert!(reporter, !invert_into(&mat, None));
        reporter_assert!(reporter, !invert_into(&mat, Some(&mut inverse)));

        // rectStaysRect test
        {
            struct RectStaysRectSamples {
                m00: scalar,
                m01: scalar,
                m10: scalar,
                m11: scalar,
                m_stays_rect: bool,
            }
            const fn sample(
                m00: scalar,
                m01: scalar,
                m10: scalar,
                m11: scalar,
                m_stays_rect: bool,
            ) -> RectStaysRectSamples {
                RectStaysRectSamples {
                    m00,
                    m01,
                    m10,
                    m11,
                    m_stays_rect,
                }
            }
            const G_RECT_STAYS_RECT_SAMPLES: [RectStaysRectSamples; 16] = [
                sample(0.0, 0.0, 0.0, 0.0, false),
                sample(0.0, 0.0, 0.0, 1.0, false),
                sample(0.0, 0.0, 1.0, 0.0, false),
                sample(0.0, 0.0, 1.0, 1.0, false),
                sample(0.0, 1.0, 0.0, 0.0, false),
                sample(0.0, 1.0, 0.0, 1.0, false),
                sample(0.0, 1.0, 1.0, 0.0, true),
                sample(0.0, 1.0, 1.0, 1.0, false),
                sample(1.0, 0.0, 0.0, 0.0, false),
                sample(1.0, 0.0, 0.0, 1.0, true),
                sample(1.0, 0.0, 1.0, 0.0, false),
                sample(1.0, 0.0, 1.0, 1.0, false),
                sample(1.0, 1.0, 0.0, 0.0, false),
                sample(1.0, 1.0, 0.0, 1.0, false),
                sample(1.0, 1.0, 1.0, 0.0, false),
                sample(1.0, 1.0, 1.0, 1.0, false),
            ];

            for sample in &G_RECT_STAYS_RECT_SAMPLES {
                let mut m = Matrix::default();

                m.reset();
                m.set(Member::ScaleX, sample.m00);
                m.set(Member::SkewX, sample.m01);
                m.set(Member::SkewY, sample.m10);
                m.set(Member::ScaleY, sample.m11);
                reporter_assert!(reporter, m.rect_stays_rect() == sample.m_stays_rect);
            }
        }

        mat.reset();
        mat.set(Member::ScaleX, 1.0)
            .set(Member::SkewX, 2.0)
            .set(Member::TransX, 3.0)
            .set(Member::SkewY, 4.0)
            .set(Member::ScaleY, 5.0)
            .set(Member::TransY, 6.0);
        let affine = mat.to_affine();
        reporter_assert!(reporter, affine.is_some());
        let affine = affine.unwrap_or_default();

        reporter_assert!(
            reporter,
            affine[AffineMember::ScaleX as usize] == mat.get(Member::ScaleX)
        );
        reporter_assert!(
            reporter,
            affine[AffineMember::SkewY as usize] == mat.get(Member::SkewY)
        );
        reporter_assert!(
            reporter,
            affine[AffineMember::SkewX as usize] == mat.get(Member::SkewX)
        );
        reporter_assert!(
            reporter,
            affine[AffineMember::ScaleY as usize] == mat.get(Member::ScaleY)
        );
        reporter_assert!(
            reporter,
            affine[AffineMember::TransX as usize] == mat.get(Member::TransX)
        );
        reporter_assert!(
            reporter,
            affine[AffineMember::TransY as usize] == mat.get(Member::TransY)
        );

        mat.set(Member::Persp1, SCALAR_1 / 2.0);
        reporter_assert!(reporter, mat.to_affine().is_none());

        let mut mat2 = Matrix::default();
        mat2.reset();
        mat.reset();
        let zero: scalar = 0.0;
        mat.set(Member::SkewX, -zero);
        reporter_assert!(reporter, are_equal(reporter, &mat, &mat2));

        mat2.reset();
        mat.reset();
        mat.set(Member::SkewX, SCALAR_NAN);
        mat2.set(Member::SkewX, SCALAR_NAN);
        reporter_assert!(reporter, !are_equal(reporter, &mat, &mat2));

        test_matrix_min_max_scale(reporter);
        test_matrix_preserve_shape(reporter);
        test_matrix_recttorect(reporter);
        test_matrix_decomposition(reporter);
        test_matrix_homogeneous(reporter);
        test_set9(reporter);

        test_decomp_scale(reporter);

        mat.set_scale_translate((2.0, 3.0), (1.0, 4.0));
        mat2.set_scale((2.0, 3.0), None).post_translate((1.0, 4.0));
        reporter_assert!(reporter, mat == mat2);
    }
);

// Port of: tests/MatrixTest.cpp#L1036-L1047 (chrome/m156)
def_test!(Matrix_Concat, |r| {
    let mut a = Matrix::default();
    a.set_translate((10.0, 20.0));

    let mut b = Matrix::default();
    b.set_scale((3.0, 5.0), None);

    let mut expected = Matrix::default();
    expected.set_concat(&a, &b);

    reporter_assert!(r, expected == Matrix::concat(&a, &b));
});

// Test that all variants of maprect are correct.
// Port of: tests/MatrixTest.cpp#L1049-L1088 (chrome/m156)
def_test!(Matrix_maprects, |r| {
    let scale: scalar = 1000.0;

    let mut mat = Matrix::default();
    mat.set_scale((2.0, 3.0), None).post_translate((1.0, 4.0));

    let mut rand = Random::default();
    for _ in 0..10000 {
        let src = Rect::new(
            rand.next_s_scalar1() * scale,
            rand.next_s_scalar1() * scale,
            rand.next_s_scalar1() * scale,
            rand.next_s_scalar1() * scale,
        );
        let mut dst = [Rect::default(); 4];

        // The C++ reinterprets the (left, top, right, bottom) of a rect as two points.
        let src_pts = [
            Point::new(src.left, src.top),
            Point::new(src.right, src.bottom),
        ];
        let mut dst_pts = [Point::default(); 2];
        mat.map_points(&mut dst_pts, &src_pts);
        dst[0] = Rect::new(dst_pts[0].x, dst_pts[0].y, dst_pts[1].x, dst_pts[1].y);
        dst[0].sort();
        dst[1] = mat.map_rect(src).0;
        dst[2] = mat.map_rect_scale_translate(src).unwrap_or_default();
        dst[3] = mat.map_rect(src).0;

        reporter_assert!(r, dst[0] == dst[1]);
        reporter_assert!(r, dst[0] == dst[2]);
        reporter_assert!(r, dst[0] == dst[3]);
    }

    // We should report nonfinite-ness after a mapping
    {
        // We have special-cases in mapRect for different matrix types
        let m0 = Matrix::scale((1e20, 1e20));
        let mut m1 = Matrix::default();
        m1.set_rotate(30.0, None);
        m1.post_scale((1e20, 1e20), None);

        for m in [m0, m1] {
            let mut rect = Rect::new(0.0, 0.0, 1e20, 1e20);
            reporter_assert!(r, rect.is_finite());
            rect = m.map_rect(rect).0;
            reporter_assert!(r, !rect.is_finite());
        }
    }
});

// Port of: tests/MatrixTest.cpp#L1090-L1102 (chrome/m156)
// Allowed lints: float literals are copied verbatim from the C++
def_test!(
    #[allow(clippy::excessive_precision)]
    Matrix_mapRect_skbug12335,
    |r| {
        // Stripped down test case from skbug.com/40043416. Essentially, the corners of this rect
        // would map to homogoneous coords with very small w's (below the old value of
        // kW0PlaneDistance) and so they would be clipped "behind" the plane, resulting in an
        // empty mapped rect. Coordinates with positive that wouldn't overflow when divided by w
        // should still be included in the mapped rectangle.
        let rect = Rect::from_ltrb(0.0, 0.0, 319.0, 620.0);
        let m = Matrix::new_all(
            0.000_152_695_269,
            0.000_000_00,
            -6.538_484_01e-05,
            -1.756_975_33e-05,
            0.000_157_153_074,
            -1.108_479_75e-06,
            -6.004_153_62e-08,
            0.000_000_00,
            0.000_169_880_834,
        );
        let out = m.map_rect(rect).0;
        reporter_assert!(r, !out.is_empty());
    }
);

// Port of: tests/MatrixTest.cpp#L1104-L1106 (chrome/m156)
def_test!(Matrix_Ctor, |r| {
    reporter_assert!(r, Matrix::default() == *Matrix::i());
});

// Port of: tests/MatrixTest.cpp#L1108-L1112 (chrome/m156)
def_test!(Matrix_LookAt, |r| {
    // Degenerate inputs should not trigger *SAN errors.
    let m = M44::look_at(
        &V3::new(0.0, 0.0, 0.0),
        &V3::new(0.0, 0.0, 0.0),
        &V3::new(0.0, 0.0, 0.0),
    );
    reporter_assert!(r, m == M44::default());
});

// Port of: tests/MatrixTest.cpp#L1114-L1131 (chrome/m156)
def_test!(Matrix_SetRotateSnap, |r| {
    let mut m = Matrix::default();

    // We need to snap sin & cos when we call setRotate, or rotations by multiples of 90 degrees
    // will end up with slight drift (and we won't consider them to satisfy rectStaysRect, which
    // is an important performance constraint). We test up to +-1080 degrees.
    let mut deg: f32 = 90.0;
    while deg <= 1080.0 {
        m.set_rotate(deg, None);
        reporter_assert!(r, m.rect_stays_rect());
        m.set_rotate(-deg, None);
        reporter_assert!(r, m.rect_stays_rect());
        deg += 90.0;
    }

    // But: we don't want to be too lenient with snapping. That prevents small rotations from being
    // registered at all. Ensure that .01 degrees produces an actual rotation. (crbug.com/1345038)
    m.set_rotate(0.01, None);
    reporter_assert!(r, !m.rect_stays_rect());
});

// Port of: tests/MatrixTest.cpp#L1133-L1200 (chrome/m156)
def_test!(Matrix_rectStaysRect_zeroScale, |r| {
    // rectStaysRect() returns true if the scale factors are non-zero, so preScale(0,0),
    // setScale(0,0), setScaleTranslate(0,0,...), ::Scale(), should not have the flag set.
    reporter_assert!(r, !Matrix::scale((0.0, 0.0)).rect_stays_rect());
    reporter_assert!(r, !Matrix::scale((0.0, 2.0)).rect_stays_rect());
    reporter_assert!(r, !Matrix::scale((2.0, 0.0)).rect_stays_rect());

    // RectToRect() is like scaling. It fails if the source rect is empty, but if the dst rect is
    // empty it's as if it had a zero scale factor, so it's type mask should reflect that.
    let src = Rect::new(0.0, 0.0, 10.0, 10.0);
    reporter_assert!(
        r,
        Matrix::rect_2_rect(src, Rect::new(0.0, 0.0, 0.0, 0.0), None)
            .is_some_and(|m| !m.rect_stays_rect())
    );
    reporter_assert!(
        r,
        Matrix::rect_2_rect(src, Rect::new(0.0, 0.0, 0.0, 20.0), None)
            .is_some_and(|m| !m.rect_stays_rect())
    );
    reporter_assert!(
        r,
        Matrix::rect_2_rect(src, Rect::new(0.0, 0.0, 20.0, 0.0), None)
            .is_some_and(|m| !m.rect_stays_rect())
    );

    {
        let rect_matrix = Matrix::i().clone(); // trivially
        reporter_assert!(r, rect_matrix.rect_stays_rect());

        let mut non_rect_matrix = rect_matrix.clone();
        non_rect_matrix.pre_scale((0.0, 0.0), None);
        reporter_assert!(r, !non_rect_matrix.rect_stays_rect());

        non_rect_matrix = rect_matrix.clone();
        non_rect_matrix.pre_scale((0.0, 2.0), None);
        reporter_assert!(r, !non_rect_matrix.rect_stays_rect());

        non_rect_matrix = rect_matrix.clone();
        non_rect_matrix.pre_scale((2.0, 0.0), None);
        reporter_assert!(r, !non_rect_matrix.rect_stays_rect());
    }

    {
        let mut m = Matrix::default();
        m.set_scale((0.0, 0.0), None);
        reporter_assert!(r, !m.rect_stays_rect());
    }

    {
        let mut m = Matrix::default();
        m.set_scale((0.0, 2.0), None);
        reporter_assert!(r, !m.rect_stays_rect());
    }

    {
        let mut m = Matrix::default();
        m.set_scale((2.0, 0.0), None);
        reporter_assert!(r, !m.rect_stays_rect());
    }

    {
        let mut m = Matrix::default();
        m.set_scale_translate((0.0, 0.0), (10.0, 10.0));
        reporter_assert!(r, !m.rect_stays_rect());
    }

    {
        let mut m = Matrix::default();
        m.set_scale_translate((0.0, 2.0), (10.0, 10.0));
        reporter_assert!(r, !m.rect_stays_rect());
    }

    {
        let mut m = Matrix::default();
        m.set_scale_translate((2.0, 0.0), (10.0, 10.0));
        reporter_assert!(r, !m.rect_stays_rect());
    }
});
