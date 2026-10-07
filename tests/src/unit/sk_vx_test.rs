// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tests/SkVxTest.cpp

// Port of: tests/SkVxTest.cpp (chrome/m156)
//
// Mapping notes (see `skia_rust_simd::vx`):
// - `a == b`, `a < b`, ... on vectors are `a.eq_mask(b)`, `a.lt_mask(b)`, ... (they return masks).
// - `shuffle<2,1,0,3>(v)` is `shuffle(v, [2, 1, 0, 3])`.
// - `Vec{a, b}` is `Vec::from_list(&[a, b])` (missing lanes are zero); `Vec(s)` is `Vec::splat(s)`.
// - `SkPoint`, `SkRandom` and `SkScalarNearlyEqual` are not ported to `skia_rust_core` yet, so
//   minimal exact ports of just what this file uses live at the bottom of this file.

use skia_rust_simd::vx::{
    self, Byte2, Byte4, Byte8, Byte16, CastFrom, Double2, Double4, Float2, Float4, Float8, Int2,
    Int4, Lane, ScaledDividerU32, UInt4, all, any, approx_scale, cross, div255, dot, floor,
    from_half, if_then_else, isfinite, join, length, mull, naive_if_then_else, normalize,
    reduce_max, reduce_min, saturated_add, shuffle, sqrt, strided_load2, strided_load4, to_half,
};

use crate::{Reporter, def_test, reporter_assert};

def_test!(
    #[allow(clippy::float_cmp)] // the C++ compares floats with ==
    SkVx,
    |r| {
        const _: () = assert!(size_of::<Float2>() == 8);
        const _: () = assert!(size_of::<Float4>() == 16);
        const _: () = assert!(size_of::<Float8>() == 32);

        const _: () = assert!(size_of::<Byte2>() == 2);
        const _: () = assert!(size_of::<Byte4>() == 4);
        const _: () = assert!(size_of::<Byte8>() == 8);

        {
            let mask: Int4 =
                Float4::new(1.0, 2.0, 3.0, 4.0).lt_mask(Float4::new(1.0, 2.0, 4.0, 8.0));
            reporter_assert!(r, mask[0] == 0_i32);
            reporter_assert!(r, mask[1] == 0_i32);
            reporter_assert!(r, mask[2] == -1_i32);
            reporter_assert!(r, mask[3] == -1_i32);

            reporter_assert!(r, any(mask));
            reporter_assert!(r, !all(mask));
        }

        {
            let mask: vx::Long4 =
                Double4::new(1.0, 2.0, 3.0, 4.0).lt_mask(Double4::new(1.0, 2.0, 4.0, 8.0));
            reporter_assert!(r, mask[0] == 0_i64);
            reporter_assert!(r, mask[1] == 0_i64);
            reporter_assert!(r, mask[2] == -1_i64);
            reporter_assert!(r, mask[3] == -1_i64);

            reporter_assert!(r, any(mask));
            reporter_assert!(r, !all(mask));
        }

        {
            // Tests that any/all work with non-zero values, not just full bit lanes.
            reporter_assert!(r, all(Int4::new(1, 2, 3, 4)));
            reporter_assert!(r, !all(Int4::from_list(&[1, 2, 3])));
            reporter_assert!(r, any(Int4::from_list(&[1, 2])));
            reporter_assert!(r, !any(Int4::from_list(&[])));
        }

        reporter_assert!(r, reduce_min(Float4::new(1.0, 2.0, 3.0, 4.0)) == 1.0);
        reporter_assert!(r, reduce_max(Float4::new(1.0, 2.0, 3.0, 4.0)) == 4.0);

        reporter_assert!(r, all(Int4::new(1, 2, 3, 4).eq_mask(Int4::new(1, 2, 3, 4))));
        reporter_assert!(
            r,
            all(Int4::from_list(&[1, 2, 3]).eq_mask(Int4::new(1, 2, 3, 0)))
        );
        reporter_assert!(
            r,
            all(Int4::from_list(&[1, 2]).eq_mask(Int4::new(1, 2, 0, 0)))
        );
        reporter_assert!(r, all(Int4::from_list(&[1]).eq_mask(Int4::new(1, 0, 0, 0))));
        reporter_assert!(r, all(Int4::splat(1).eq_mask(Int4::new(1, 1, 1, 1))));
        reporter_assert!(r, all(Int4::from_list(&[]).eq_mask(Int4::new(0, 0, 0, 0))));
        reporter_assert!(r, all(Int4::default().eq_mask(Int4::new(0, 0, 0, 0))));

        reporter_assert!(
            r,
            all(Int4::new(1, 2, 2, 1).eq_mask(Int4::new(1, 2, 3, 4).min(Int4::new(4, 3, 2, 1))))
        );
        reporter_assert!(
            r,
            all(Int4::new(4, 3, 3, 4).eq_mask(Int4::new(1, 2, 3, 4).max(Int4::new(4, 3, 2, 1))))
        );

        reporter_assert!(
            r,
            all(if_then_else(
                Float4::new(1.0, 2.0, 3.0, 2.0).le_mask(Float4::new(2.0, 2.0, 2.0, 2.0)),
                Float4::splat(42.0),
                Float4::splat(47.0)
            )
            .eq_mask(Float4::new(42.0, 42.0, 47.0, 42.0)))
        );

        reporter_assert!(
            r,
            all(floor(Float4::new(-1.5, 1.5, 1.0, -1.0)).eq_mask(Float4::new(-2.0, 1.0, 1.0, -1.0)))
        );
        reporter_assert!(
            r,
            all(vx::ceil(Float4::new(-1.5, 1.5, 1.0, -1.0))
                .eq_mask(Float4::new(-1.0, 2.0, 1.0, -1.0)))
        );
        reporter_assert!(
            r,
            all(vx::trunc(Float4::new(-1.5, 1.5, 1.0, -1.0))
                .eq_mask(Float4::new(-1.0, 1.0, 1.0, -1.0)))
        );
        reporter_assert!(
            r,
            all(vx::round(Float4::new(-1.5, 1.5, 1.0, -1.0))
                .eq_mask(Float4::new(-2.0, 2.0, 1.0, -1.0)))
        );

        reporter_assert!(
            r,
            all(vx::abs(Float4::new(-2.0, -1.0, 0.0, 1.0)).eq_mask(Float4::new(2.0, 1.0, 0.0, 1.0)))
        );

        // TODO(mtklein): these tests could be made less loose.
        reporter_assert!(
            r,
            all(sqrt(Float4::new(2.0, 3.0, 4.0, 5.0)).lt_mask(Float4::new(2.0, 2.0, 3.0, 3.0)))
        );
        reporter_assert!(
            r,
            all(sqrt(Float2::new(2.0, 3.0)).lt_mask(Float2::new(2.0, 2.0)))
        );

        reporter_assert!(
            r,
            all(Float4::new(-1.5, 0.5, 1.0, 1.5)
                .cast::<i32>()
                .eq_mask(Int4::new(-1, 0, 1, 1)))
        );

        let mut buf = [1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0];
        reporter_assert!(
            r,
            all(Float4::load(&buf).eq_mask(Float4::new(1.0, 2.0, 3.0, 4.0)))
        );
        Float4::new(2.0, 3.0, 4.0, 5.0).store(&mut buf);
        reporter_assert!(
            r,
            buf[0] == 2.0
                && buf[1] == 3.0
                && buf[2] == 4.0
                && buf[3] == 5.0
                && buf[4] == 5.0
                && buf[5] == 6.0
        );
        reporter_assert!(
            r,
            all(Float4::load(&buf[0..]).eq_mask(Float4::new(2.0, 3.0, 4.0, 5.0)))
        );
        reporter_assert!(
            r,
            all(Float4::load(&buf[2..]).eq_mask(Float4::new(4.0, 5.0, 5.0, 6.0)))
        );

        reporter_assert!(
            r,
            all(shuffle(Float4::new(1.0, 2.0, 3.0, 4.0), [2, 1, 0, 3])
                .eq_mask(Float4::new(3.0, 2.0, 1.0, 4.0)))
        );
        reporter_assert!(
            r,
            all(shuffle(Float4::new(1.0, 2.0, 3.0, 4.0), [2, 1]).eq_mask(Float2::new(3.0, 2.0)))
        );
        reporter_assert!(
            r,
            all(shuffle(Float4::new(1.0, 2.0, 3.0, 4.0), [3, 3, 3, 3])
                .eq_mask(Float4::new(4.0, 4.0, 4.0, 4.0)))
        );
        reporter_assert!(
            r,
            all(
                shuffle(Float4::new(1.0, 2.0, 3.0, 4.0), [2, 1, 2, 1, 2, 1, 2, 1])
                    .eq_mask(Float8::from_list(&[3.0, 2.0, 3.0, 2.0, 3.0, 2.0, 3.0, 2.0]))
            )
        );

        // Test that mixed types can be used where they make sense.  Mostly about ergonomics.
        // (A scalar converts to the lane type, as C++'s `is_convertible<U,T>` allows.)
        reporter_assert!(r, all(Float4::new(1.0, 2.0, 3.0, 4.0).lt_mask(5.0)));
        reporter_assert!(r, all(Byte4::new(1, 2, 3, 4).lt_mask(5)));
        reporter_assert!(
            r,
            all(Int4::new(1, 2, 3, 4).lt_mask(<i32 as CastFrom<f32>>::cast_from(5.0_f32)))
        );
        let five = Float4::from(5.0);
        reporter_assert!(r, all(five.eq_mask(5.0_f32)));
        reporter_assert!(r, all(five.eq_mask(<f32 as CastFrom<i32>>::cast_from(5))));

        reporter_assert!(
            r,
            all(Float4::from(2.0)
                .max(Float4::new(1.0, 2.0, 3.0, 4.0).min(3.0))
                .eq_mask(Float4::new(2.0, 2.0, 3.0, 3.0)))
        );

        for x in 0..256_i32 {
            for y in 0..256_i32 {
                test_div255_approx_scale(r, x, y);
            }
        }

        for x in 0..256_u16 {
            for y in 0..256_u16 {
                let xy: u16 = x * y;
                let (bx, by) = (u8::try_from(x).unwrap(), u8::try_from(y).unwrap());

                // Make sure to cover implementation cases N=8, N<8, and N>8.
                reporter_assert!(r, all(mull(Byte2::splat(bx), Byte2::splat(by)).eq_mask(xy)));
                reporter_assert!(r, all(mull(Byte4::splat(bx), Byte4::splat(by)).eq_mask(xy)));
                reporter_assert!(r, all(mull(Byte8::splat(bx), Byte8::splat(by)).eq_mask(xy)));
                reporter_assert!(
                    r,
                    all(mull(Byte16::splat(bx), Byte16::splat(by)).eq_mask(xy))
                );
            }
        }

        {
            // Intentionally not testing -0, as we don't care if it's 0x0000 or 0x8000.
            let fs = Float8::from_list(&[0.0, 0.5, 1.0, 2.0, -4.0, -0.5, -1.0, -2.0]);
            let hs = vx::Vec::<8, u16>::from_list(&[
                0x0000, 0x3800, 0x3c00, 0x4000, 0xc400, 0xb800, 0xbc00, 0xc000,
            ]);
            reporter_assert!(r, all(to_half(fs).eq_mask(hs)));
            reporter_assert!(r, all(from_half(hs).eq_mask(fs)));
        }
    }
);

// The body of the `for x ... for y ...` loop of DEF_TEST(SkVx).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // (uint8_t)(... + 0.5), in 0..=255
fn test_div255_approx_scale(r: &mut Reporter, x: i32, y: i32) {
    let want: u8 = (255.0 * (f64::from(x) / 255.0 * f64::from(y) / 255.0) + 0.5) as u8;

    {
        let got: u8 = div255(
            vx::Vec::<8, u16>::splat(u16::try_from(x).unwrap())
                * vx::Vec::<8, u16>::splat(u16::try_from(y).unwrap()),
        )[0];
        reporter_assert!(r, got == want);
    }

    {
        let got: u8 = approx_scale(
            vx::Vec::<8, u8>::splat(u8::try_from(x).unwrap()),
            vx::Vec::<8, u8>::splat(u8::try_from(y).unwrap()),
        )[0];

        reporter_assert!(
            r,
            i32::from(got) == i32::from(want) - 1
                || got == want
                || i32::from(got) == i32::from(want) + 1
        );
        if x == 0 || y == 0 || x == 255 || y == 255 {
            reporter_assert!(r, got == want);
        }
    }
}

def_test!(
    #[allow(clippy::float_cmp)] // the C++ compares floats with ==
    SkVx_xy,
    |r| {
        let mut f = Float2::new(1.0, 2.0);
        reporter_assert!(r, all(f.eq_mask(Float2::new(1.0, 2.0))));
        reporter_assert!(r, f.x() == 1.0);
        reporter_assert!(r, f.y() == 2.0);
        *f.y_mut() = 9.0;
        reporter_assert!(r, all(f.eq_mask(Float2::new(1.0, 9.0))));
        *f.x_mut() = 0.0;
        reporter_assert!(r, all(f.eq_mask(Float2::new(0.0, 9.0))));
        f[0] = 8.0;
        reporter_assert!(r, f.x() == 8.0);
        f[1] = 6.0;
        reporter_assert!(r, f.y() == 6.0);
        reporter_assert!(r, all(f.eq_mask(Float2::new(8.0, 6.0))));
        f = f.yx();
        reporter_assert!(r, all(f.eq_mask(Float2::new(6.0, 8.0))));
        // skia-rust: SkPoint is not ported yet; `[f32; 2]` has SkPoint's layout (x, y).
        let point_bits: [f32; 2] = f.0;
        reporter_assert!(r, point_bits == [6.0, 8.0]);
        let mut p = [0.0_f32; 2];
        f.store(&mut p);
        reporter_assert!(r, p == [6.0, 8.0]);
        f.yx().store(&mut p);
        reporter_assert!(r, p == [8.0, 6.0]);
        reporter_assert!(r, all(f.xyxy().eq_mask(Float4::new(6.0, 8.0, 6.0, 8.0))));
        reporter_assert!(r, all(f.xyxy().eq_mask(Float4::from_xy_zw(f, f))));
        reporter_assert!(r, all(join(f, f).eq_mask(f.xyxy())));
        reporter_assert!(
            r,
            all(join(f.yx(), f).eq_mask(Float4::from_x_y_zw(f.y(), f.x(), f)))
        );
        reporter_assert!(
            r,
            all(join(f.yx(), f).eq_mask(Float4::from_xy_z_w(f.yx(), f.x(), f.y())))
        );
        reporter_assert!(
            r,
            all(join(f, f.yx()).eq_mask(Float4::from_x_y_zw(f.x(), f.y(), f.yx())))
        );
        reporter_assert!(
            r,
            all(join(f.yx(), f.yx()).eq_mask(Float4::from_xy_zw(f.yx(), f.yx())))
        );
    }
);

def_test!(
    #[allow(clippy::float_cmp)] // the C++ compares floats with ==
    SkVx_xyzw,
    |r| {
        let mut f = Float4::new(1.0, 2.0, 3.0, 4.0);
        reporter_assert!(r, all(f.eq_mask(Float4::new(1.0, 2.0, 3.0, 4.0))));
        reporter_assert!(
            r,
            all(f.eq_mask(Float4::from_x_y_zw(1.0, 2.0, Float2::new(3.0, 4.0))))
        );
        reporter_assert!(
            r,
            all(f.eq_mask(Float4::from_xy_z_w(Float2::new(1.0, 2.0), 3.0, 4.0)))
        );
        reporter_assert!(
            r,
            all(f.eq_mask(Float4::from_xy_zw(
                Float2::new(1.0, 2.0),
                Float2::new(3.0, 4.0)
            )))
        );
        f.set_xy(Float2::new(9.0, 8.0));
        reporter_assert!(r, all(f.eq_mask(Float4::new(9.0, 8.0, 3.0, 4.0))));
        // `f.zw().x() = 7; f.zw().y() = 6;` write through the `zw()` reference.
        f.zw_mut()[0] = 7.0;
        f.zw_mut()[1] = 6.0;
        reporter_assert!(r, all(f.eq_mask(Float4::new(9.0, 8.0, 7.0, 6.0))));
        *f.x_mut() = 5.0;
        *f.y_mut() = 4.0;
        *f.z_mut() = 3.0;
        *f.w_mut() = 2.0;
        reporter_assert!(r, all(f.eq_mask(Float4::new(5.0, 4.0, 3.0, 2.0))));
        f[0] = 0.0;
        reporter_assert!(r, f.x() == 0.0);
        f[1] = 1.0;
        reporter_assert!(r, f.y() == 1.0);
        f[2] = 2.0;
        reporter_assert!(r, f.z() == 2.0);
        f[3] = 3.0;
        reporter_assert!(r, f.w() == 3.0);
        reporter_assert!(r, all(f.xy().eq_mask(Float2::new(0.0, 1.0))));
        reporter_assert!(r, all(f.zw().eq_mask(Float2::new(2.0, 3.0))));
        reporter_assert!(r, all(f.eq_mask(Float4::new(0.0, 1.0, 2.0, 3.0))));
        reporter_assert!(r, all(f.yxwz().lo().eq_mask(shuffle(f, [1, 0]))));
        reporter_assert!(r, all(f.yxwz().hi().eq_mask(shuffle(f, [3, 2]))));
        reporter_assert!(r, all(f.zwxy().lo().lo().eq_mask(f.z())));
        reporter_assert!(r, all(f.zwxy().lo().hi().eq_mask(f.w())));
        reporter_assert!(r, all(f.zwxy().hi().lo().eq_mask(f.x())));
        reporter_assert!(r, all(f.zwxy().hi().hi().eq_mask(f.y())));
        reporter_assert!(r, f.yxwz().lo().lo().val() == f.y());
        reporter_assert!(r, f.yxwz().lo().hi().val() == f.x());
        reporter_assert!(r, f.yxwz().hi().lo().val() == f.w());
        reporter_assert!(r, f.yxwz().hi().hi().val() == f.z());

        reporter_assert!(
            r,
            all(naive_if_then_else(
                Int2::new(0, !0),
                shuffle(Float4::new(0.0, 1.0, 2.0, 3.0), [3, 2]),
                Float4::new(4.0, 5.0, 6.0, 7.0).xy()
            )
            .eq_mask(Float2::new(4.0, 2.0)))
        );
        reporter_assert!(
            r,
            all(if_then_else(
                Int2::new(0, !0),
                shuffle(Float4::new(0.0, 1.0, 2.0, 3.0), [3, 2]),
                Float4::new(4.0, 5.0, 6.0, 7.0).xy()
            )
            .eq_mask(Float2::new(4.0, 2.0)))
        );
        reporter_assert!(
            r,
            all(naive_if_then_else(
                Int2::new(0, !0).xyxy(),
                Float4::new(0.0, 1.0, 2.0, 3.0).zwxy(),
                Float4::new(4.0, 5.0, 6.0, 7.0)
            )
            .eq_mask(Float4::new(4.0, 3.0, 6.0, 1.0)))
        );
        reporter_assert!(
            r,
            all(if_then_else(
                Int2::new(0, !0).xyxy(),
                Float4::new(0.0, 1.0, 2.0, 3.0).zwxy(),
                Float4::new(4.0, 5.0, 6.0, 7.0)
            )
            .eq_mask(Float4::new(4.0, 3.0, 6.0, 1.0)))
        );

        reporter_assert!(
            r,
            all(Float4::new(0.0, 1.0, 2.0, 3.0)
                .yxwz()
                .pin(Float2::splat(1.0).xyxy(), Float2::splat(2.0).xyxy())
                .eq_mask(Float4::new(1.0, 1.0, 2.0, 2.0)))
        );
    }
);

def_test!(
    #[allow(clippy::unreadable_literal)] // literals copied verbatim from the C++
    SkVx_cross_dot,
    |r| {
        const K_TOLERANCE: f32 = 1.0 / 1_048_576.0; // 1.f / (1 << 20)
        reporter_assert!(r, cross(Int2::new(0, 1), Int2::new(0, 1)) == 0);
        reporter_assert!(r, cross(Int2::new(1, 0), Int2::new(1, 0)) == 0);
        reporter_assert!(r, cross(Int2::new(1, 1), Int2::new(1, 1)) == 0);
        reporter_assert!(r, cross(Int2::new(1, 1), Int2::new(1, -1)) == -2);
        reporter_assert!(r, cross(Int2::new(1, 1), Int2::new(-1, 1)) == 2);

        reporter_assert!(r, dot(Int2::new(0, 1), Int2::new(1, 0)) == 0);
        reporter_assert!(r, dot(Int2::new(1, 0), Int2::new(0, 1)) == 0);
        reporter_assert!(r, dot(Int2::new(1, 1), Int2::new(1, -1)) == 0);
        reporter_assert!(r, dot(Int2::new(1, 1), Int2::new(1, 1)) == 2);
        reporter_assert!(r, dot(Int2::new(1, 1), Int2::new(-1, -1)) == -2);

        let mut rand = SkRandom::new();
        for _ in 0..100 {
            let a = rand.next_range_f(-1.0, 1.0);
            let b = rand.next_range_f(-1.0, 1.0);
            let c = rand.next_range_f(-1.0, 1.0);
            let d = rand.next_range_f(-1.0, 1.0);
            reporter_assert!(
                r,
                sk_scalar_nearly_equal(
                    cross(Float2::new(a, b), Float2::new(c, d)),
                    sk_point_cross_product((a, b), (c, d)),
                    K_TOLERANCE
                )
            );
            reporter_assert!(
                r,
                sk_scalar_nearly_equal(
                    dot(Float2::new(a, b), Float2::new(c, d)),
                    sk_point_dot_product((a, b), (c, d)),
                    K_TOLERANCE
                )
            );
        }

        assert_doubles_equal(
            r,
            cross(Double2::new(1.2, 3.4), Double2::new(3.4, -1.2)),
            -13.000000,
        );
        assert_doubles_equal(
            r,
            cross(Double2::new(12.34, 5.6), Double2::new(7.8, -9.0)),
            -154.740000,
        );
        assert_doubles_equal(
            r,
            cross(Double2::new(12.34, 5.6), Double2::new(7.8, 9.012345678)),
            67.532346,
        );
    }
);

// Port of: tests/SkVxTest.cpp#L278-L295 (chrome/m156)
#[allow(clippy::many_single_char_names)] // the C++ names the vectors a, b, c, d
fn check_strided_loads_n<const N: usize, T: Lane + CastFrom<i32>>(r: &mut Reporter) {
    // std::iota(values, values + N*4, 0)
    let values: Vec<T> = (0..N * 4)
        .map(|i| T::cast_from(i32::try_from(i).unwrap()))
        .collect();
    let (a, b) = strided_load2::<N, T>(&values);
    for i in 0..N {
        reporter_assert!(r, a[i] == values[i * 2]);
        reporter_assert!(r, b[i] == values[i * 2 + 1]);
    }
    let (a, b, c, d) = strided_load4::<N, T>(&values);
    for i in 0..N {
        reporter_assert!(r, a[i] == values[i * 4]);
        reporter_assert!(r, b[i] == values[i * 4 + 1]);
        reporter_assert!(r, c[i] == values[i * 4 + 2]);
        reporter_assert!(r, d[i] == values[i * 4 + 3]);
    }
}

// Port of: tests/SkVxTest.cpp#L297-L304 (chrome/m156)
fn check_strided_loads<T: Lane + CastFrom<i32>>(r: &mut Reporter) {
    check_strided_loads_n::<1, T>(r);
    check_strided_loads_n::<2, T>(r);
    check_strided_loads_n::<4, T>(r);
    check_strided_loads_n::<8, T>(r);
    check_strided_loads_n::<16, T>(r);
    check_strided_loads_n::<32, T>(r);
}

def_test!(SkVx_strided_loads, |r| {
    check_strided_loads::<u32>(r);
    check_strided_loads::<u16>(r);
    check_strided_loads::<u8>(r);
    check_strided_loads::<i32>(r);
    check_strided_loads::<i16>(r);
    check_strided_loads::<i8>(r);
    check_strided_loads::<f32>(r);
});

def_test!(SkVx_ScaledDividerU32, |r| {
    const K_MAX: u32 = u32::MAX;

    let error_bounds = |actual: u32, expected: u32| {
        let lower_limit = if expected == 0 { 0 } else { expected - 1 };
        let upper_limit = if expected == K_MAX {
            K_MAX
        } else {
            expected + 1
        };
        lower_limit <= actual && actual <= upper_limit
    };

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // static_cast<uint32_t>(floor(..))
    let mut test = |denom: u32| {
        // half == 1 so, the max to check is kMax-1
        let d = ScaledDividerU32::new(denom);
        let max_check = ((f64::from(K_MAX - d.half()) / f64::from(denom) + 0.5).floor()) as u32;
        reporter_assert!(r, error_bounds(d.divide(UInt4::splat(K_MAX))[0], max_check));
        let mut i: u32 = 0;
        while i < K_MAX - d.half() {
            let expected = ((f64::from(i) / f64::from(denom) + 0.5).floor()) as u32;
            let actual = d.divide(UInt4::splat(i + d.half()));
            if !error_bounds(actual[0], expected) {
                eprintln!("i: {} expected: {} actual: {}", i, expected, actual[0]);
            }
            // Make sure all the lanes are the same.
            for e in 1..4 {
                debug_assert_eq!(actual[0], actual[e]);
            }
            i += 65535;
        }
    };

    test(2);
    test(3);
    test(5);
    test(7);
    test(27);
    test(65_535);
    test(15_485_863);
    test(512_927_377);
});

def_test!(SkVx_saturated_add, |r| {
    for a in 0..(1 << 8) {
        for b in 0..(1 << 8) {
            let exact = (a + b).clamp(0, 255);

            reporter_assert!(
                r,
                i32::from(
                    saturated_add(
                        Byte16::splat(u8::try_from(a).unwrap()),
                        Byte16::splat(u8::try_from(b).unwrap())
                    )[0]
                ) == exact
            );
        }
    }
});

// Port of: tests/SkVxTest.cpp#L366-L383 (chrome/m156)
fn assert_floats_equal(r: &mut Reporter, left: f32, right: f32) {
    reporter_assert!(
        r,
        sk_scalar_nearly_equal(left, right, SK_SCALAR_NEARLY_ZERO),
        "{left:.6} != {right:.6}"
    );
}

// Port of: tests/SkVxTest.cpp#L366-L383 (chrome/m156)
// The C++ lambda takes doubles and passes them to SkScalarNearlyEqual(SkScalar, SkScalar).
#[allow(clippy::cast_possible_truncation)] // double -> SkScalar (float) argument conversion
fn assert_doubles_equal(r: &mut Reporter, left: f64, right: f64) {
    reporter_assert!(
        r,
        sk_scalar_nearly_equal(left as f32, right as f32, SK_SCALAR_NEARLY_ZERO),
        "{left:.6} != {right:.6}"
    );
}

def_test!(
    #[allow(clippy::unreadable_literal, clippy::approx_constant)]
    // literals copied verbatim from the C++
    SkVx_length,
    |r| {
        assert_floats_equal(r, length(Float2::new(0.0, 1.0)), 1.000000);
        assert_floats_equal(r, length(Float2::new(2.0, 0.0)), 2.000000);
        assert_floats_equal(r, length(Float2::new(3.0, 4.0)), 5.000000);
        assert_floats_equal(r, length(Float2::new(1.0, 1.0)), 1.414214);
        assert_floats_equal(r, length(Float2::new(2.5, 2.5)), 3.535534);
        assert_floats_equal(r, length(Float4::new(1.0, 2.0, 3.0, 4.0)), 5.477226);

        assert_doubles_equal(r, length(Double2::new(2.5, 2.5)), 3.535534);
        assert_doubles_equal(r, length(Double4::new(1.5, 2.5, 3.5, 4.5)), 6.403124);
    }
);

def_test!(
    #[allow(clippy::unreadable_literal)] // literals copied verbatim from the C++
    SkVx_normalize,
    |r| {
        let two_floats: Float2 = normalize(Float2::new(1.2, 3.4));
        assert_floats_equal(r, two_floats[0], 0.332820);
        assert_floats_equal(r, two_floats[1], 0.942990);

        let two_doubles: Double2 = normalize(Double2::new(2.3, -4.5));
        assert_doubles_equal(r, two_doubles[0], 0.455111);
        assert_doubles_equal(r, two_doubles[1], -0.890435);

        let four_doubles: Double4 = normalize(Double4::new(1.2, 3.4, 5.6, 7.8));
        assert_doubles_equal(r, four_doubles[0], 0.116997);
        assert_doubles_equal(r, four_doubles[1], 0.331490);
        assert_doubles_equal(r, four_doubles[2], 0.545984);
        assert_doubles_equal(r, four_doubles[3], 0.760478);
    }
);

def_test!(
    #[allow(clippy::float_cmp)] // the C++ compares floats with ==
    SkVx_normalize_infinity_and_nan,
    |r| {
        let zero_len_vec = normalize(Float2::new(0.0, 0.0));
        reporter_assert!(
            r,
            zero_len_vec[0].is_nan(),
            "{:.6} is not nan",
            zero_len_vec[0]
        );
        reporter_assert!(
            r,
            zero_len_vec[1].is_nan(),
            "{:.6} is not nan",
            zero_len_vec[1]
        );
        reporter_assert!(r, !isfinite(zero_len_vec));

        let too_big_vec = normalize(Float2::new(f32::MAX, f32::MAX));
        reporter_assert!(r, too_big_vec[0] == 0.0, "{:.6} != 0", too_big_vec[0]);
        reporter_assert!(r, too_big_vec[1] == 0.0, "{:.6} != 0", too_big_vec[1]);

        let too_big_vec_d = normalize(Double2::new(f64::MAX, f64::MAX));
        reporter_assert!(r, too_big_vec_d[0] == 0.0, "{:.6} != 0", too_big_vec_d[0]);
        reporter_assert!(r, too_big_vec_d[1] == 0.0, "{:.6} != 0", too_big_vec_d[1]);
    }
);

def_test!(SkVx_isfinite, |r| {
    reporter_assert!(r, isfinite(Float2::new(0.0, 0.0)));
    reporter_assert!(r, isfinite(Double4::new(1.2, 3.4, 5.6, 7.8)));
    reporter_assert!(
        r,
        isfinite(Float8::from_list(&[8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0]))
    );

    reporter_assert!(r, !isfinite(Float2::new(0.0, f32::NAN)));
    reporter_assert!(r, !isfinite(Float2::new(f32::INFINITY, 10.0)));
    reporter_assert!(r, !isfinite(Float2::new(f32::NAN, f32::INFINITY)));

    for i in 0..4 {
        let mut v = Double4::new(4.0, 3.0, 2.0, 1.0);
        v[i] = f64::INFINITY;
        reporter_assert!(r, !isfinite(v), "index {} INFINITY", i);
        v[i] = f64::NAN;
        reporter_assert!(r, !isfinite(v), "index {} NAN", i);
    }

    for i in 0..8 {
        let mut v = Float8::from_list(&[8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0]);
        v[i] = f32::INFINITY;
        reporter_assert!(r, !isfinite(v), "index {} INFINITY", i);
        v[i] = f32::NAN;
        reporter_assert!(r, !isfinite(v), "index {} NAN", i);
    }
});

// ---- Minimal ports of what this file needs from not-yet-ported Skia code. ----

// Port of: include/core/SkScalar.h#L98-L110 (chrome/m156)
const SK_SCALAR_NEARLY_ZERO: f32 = 1.0 / 4096.0; // SK_Scalar1 / (1 << 12)

// Port of: include/core/SkScalar.h#L106-L110 (chrome/m156)
fn sk_scalar_nearly_equal(x: f32, y: f32, tolerance: f32) -> bool {
    debug_assert!(tolerance >= 0.0);
    (x - y).abs() <= tolerance
}

// Port of: include/core/SkPoint.h#L532-L534 (chrome/m156)
fn sk_point_cross_product(a: (f32, f32), b: (f32, f32)) -> f32 {
    a.0 * b.1 - a.1 * b.0
}

// Port of: include/core/SkPoint.h#L518-L520 (chrome/m156)
fn sk_point_dot_product(a: (f32, f32), b: (f32, f32)) -> f32 {
    a.0 * b.0 + a.1 * b.1
}

// Port of: src/core/SkRandom.h#L26-L171 (chrome/m156), the parts used here.
struct SkRandom {
    k: u32,
    j: u32,
}

impl SkRandom {
    const K_MUL: u32 = 1_664_525;
    const K_ADD: u32 = 1_013_904_223;
    const K_K_MUL: u32 = 30345;
    const K_J_MUL: u32 = 18000;

    fn next_lcg(seed: u32) -> u32 {
        Self::K_MUL.wrapping_mul(seed).wrapping_add(Self::K_ADD)
    }

    /// `SkRandom()`: seed 0.
    fn new() -> Self {
        let mut k = Self::next_lcg(0);
        if k == 0 {
            k = Self::next_lcg(k);
        }
        let mut j = Self::next_lcg(k);
        if j == 0 {
            j = Self::next_lcg(j);
        }
        debug_assert!(k != 0 && j != 0);
        Self { k, j }
    }

    fn next_u(&mut self) -> u32 {
        self.k = Self::K_K_MUL
            .wrapping_mul(self.k & 0xffff)
            .wrapping_add(self.k >> 16);
        self.j = Self::K_J_MUL
            .wrapping_mul(self.j & 0xffff)
            .wrapping_add(self.j >> 16);
        self.k.rotate_left(16).wrapping_add(self.j) // (fK << 16) | (fK >> 16)
    }

    /// Returns value [0...1) as an IEEE float.
    fn next_f(&mut self) -> f32 {
        let floatint = 0x3f80_0000 | (self.next_u() >> 9);
        f32::from_bits(floatint) - 1.0
    }

    /// Returns value [min...max) as a float.
    fn next_range_f(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f() * (max - min)
    }
}
