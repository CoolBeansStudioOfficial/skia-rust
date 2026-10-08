// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsCubicIntersectionTest.cpp (chrome/m156)

#![cfg(test)]

// Not ported: `CubicIntersection_RandTest` and `CubicIntersection_IntersectionFinder` run only
// under `if ((false))` in `PathOpsCubicIntersection`, and the `DEBUG_*`/`ONE_OFF_DEBUG` output
// (`SkDebugf`, the HTML dump) is compiled out.

use crate::unit::path_ops_cubic_intersection_test_data::TESTS;
use crate::unit::path_ops_test_common::{CubicPts, valid_cubic};
use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::geometry::{CubicType, chop_cubic_at, classify_cubic};
use skia_rust_core::point::Point;
use skia_rust_pathops::cubic::DCubic;
use skia_rust_pathops::intersections::Intersections;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::reduce_order::{Quadratics, ReduceOrder};

/// `kFirstCubicIntersectionTest`.
// Port of: tests/PathOpsCubicIntersectionTest.cpp#L28 (chrome/m156)
const FIRST_CUBIC_INTERSECTION_TEST: usize = 9;

/// `static constexpr auto testSet`.
// Port of: tests/PathOpsCubicIntersectionTest.cpp#L83-L217 (chrome/m156)
#[allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::approx_constant
)] // the table keeps the C++ literals verbatim
static TEST_SET: &[CubicPts] = &[
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(4.0, 5.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(5.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(3.0, 5.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(5.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 6.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(6.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(3.0, 4.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(5.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 5.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(4.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 2.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(6.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 6.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(2.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 5.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(4.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 4.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(5.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(3.0, 4.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(3.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 3.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(4.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 2.0),
        DPoint::new(3.0, 4.0),
        DPoint::new(4.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 2.0),
        DPoint::new(3.0, 4.0),
        DPoint::new(4.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 4.0),
        DPoint::new(1.0, 2.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(2.0, 3.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(3.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 2.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(2.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 2.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(2.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 6.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(2.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 2.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(6.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(5.0, 6.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(6.0, 5.0),
    ]),
    CubicPts::new([
        DPoint::new(95.837747722788592, 45.025976907939643),
        DPoint::new(16.564570095652982, 0.72959763963222402),
        DPoint::new(63.209855865319199, 68.047528419665767),
        DPoint::new(57.640240647662544, 59.524565264361243),
    ]),
    CubicPts::new([
        DPoint::new(51.593891741518817, 38.53849970667553),
        DPoint::new(62.34752929878772, 74.924924725166022),
        DPoint::new(74.810149322641152, 34.17966562983564),
        DPoint::new(29.368398119401373, 94.66719277886078),
    ]),
    CubicPts::new([
        DPoint::new(39.765160968417838, 33.060396198677083),
        DPoint::new(5.1922921581157908, 66.854301452103215),
        DPoint::new(31.619281802149157, 25.269248720849514),
        DPoint::new(81.541621071073038, 70.025341524754353),
    ]),
    CubicPts::new([
        DPoint::new(46.078911165743556, 48.259962651999651),
        DPoint::new(20.24450549867214, 49.403916182650214),
        DPoint::new(0.26325131778756683, 24.46489805563581),
        DPoint::new(15.915006546264051, 83.515023059917155),
    ]),
    CubicPts::new([
        DPoint::new(65.454505973241524, 93.881892270353575),
        DPoint::new(45.867360264932437, 92.723972719499827),
        DPoint::new(2.1464054482739447, 74.636369140183717),
        DPoint::new(33.774068594804994, 40.770872887582925),
    ]),
    CubicPts::new([
        DPoint::new(72.963387832494163, 95.659300729473728),
        DPoint::new(11.809496633619768, 82.209921247423594),
        DPoint::new(13.456139067865974, 57.329313623406605),
        DPoint::new(36.060621606214262, 70.867335643091849),
    ]),
    CubicPts::new([
        DPoint::new(32.484981432782945, 75.082940782924624),
        DPoint::new(42.467313093350882, 48.131159948246157),
        DPoint::new(3.5963115764764657, 43.208665839959245),
        DPoint::new(79.442476890721579, 89.709102357602262),
    ]),
    CubicPts::new([
        DPoint::new(18.98573861410177, 93.308887208490106),
        DPoint::new(40.405250173250792, 91.039661826118675),
        DPoint::new(8.0467721950480584, 42.100282172719147),
        DPoint::new(40.883324221187891, 26.030185504830527),
    ]),
    CubicPts::new([
        DPoint::new(7.5374809128872498, 82.441702896003477),
        DPoint::new(22.444346930107265, 22.138854312775123),
        DPoint::new(66.76091829629658, 50.753805856571446),
        DPoint::new(78.193478508942519, 97.7932997968948),
    ]),
    CubicPts::new([
        DPoint::new(97.700573130371311, 53.53260215070685),
        DPoint::new(87.72443481149358, 84.575876772671876),
        DPoint::new(19.215031396232092, 47.032676472809484),
        DPoint::new(11.989686410869325, 10.659507480757082),
    ]),
    CubicPts::new([
        DPoint::new(26.192053931854691, 9.8504326817814416),
        DPoint::new(10.174241480498686, 98.476562741434464),
        DPoint::new(21.177712558385782, 33.814968789841501),
        DPoint::new(75.329030899018534, 55.02231980442177),
    ]),
    CubicPts::new([
        DPoint::new(56.222082700683771, 24.54395039218662),
        DPoint::new(95.589995289030483, 81.050822735322086),
        DPoint::new(28.180450866082897, 28.837706255185282),
        DPoint::new(60.128952916771617, 87.311672180570511),
    ]),
    CubicPts::new([
        DPoint::new(42.449716172390481, 52.379709366885805),
        DPoint::new(27.896043159019225, 48.797373636065686),
        DPoint::new(92.770268299044233, 89.899302036454571),
        DPoint::new(12.102066544863426, 99.43241951960718),
    ]),
    CubicPts::new([
        DPoint::new(45.77532924980639, 45.958701495993274),
        DPoint::new(37.458701356062065, 68.393691335056758),
        DPoint::new(37.569326692060258, 27.673713456687381),
        DPoint::new(60.674866037757539, 62.47349659096146),
    ]),
    CubicPts::new([
        DPoint::new(67.426548091427676, 37.993772624988935),
        DPoint::new(23.483695892376684, 90.476863174921306),
        DPoint::new(35.597065061143162, 79.872482633158796),
        DPoint::new(75.38634169631932, 18.244890038969412),
    ]),
    CubicPts::new([
        DPoint::new(61.336508189019057, 82.693132843213675),
        DPoint::new(44.639380902349664, 54.074825790745592),
        DPoint::new(16.815615499771951, 20.049704667203923),
        DPoint::new(41.866884958868326, 56.735503699973002),
    ]),
    CubicPts::new([
        DPoint::new(18.1312339, 31.6473732),
        DPoint::new(95.5711034, 63.5350219),
        DPoint::new(92.3283165, 62.0158945),
        DPoint::new(18.5656052, 32.1268808),
    ]),
    CubicPts::new([
        DPoint::new(97.402018, 35.7169972),
        DPoint::new(33.1127443, 25.8935163),
        DPoint::new(1.13970027, 54.9424981),
        DPoint::new(56.4860195, 60.529264),
    ]),
];

/// `testSetCount`.
// Port of: tests/PathOpsCubicIntersectionTest.cpp#L219 (chrome/m156)
const TEST_SET_COUNT: usize = TEST_SET.len();

/// `static constexpr auto newTestSet`.
// Port of: tests/PathOpsCubicIntersectionTest.cpp#L221-L551 (chrome/m156)
#[allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::approx_constant
)] // the table keeps the C++ literals verbatim
static NEW_TEST_SET: &[CubicPts] = &[
    CubicPts::new([
        DPoint::new(130.0427549999999997, 11417.41309999999976),
        DPoint::new(130.2331240000000037, 11418.3192999999992),
        DPoint::new(131.0370790000000056, 11419.0),
        DPoint::new(132.0, 11419.0),
    ]),
    CubicPts::new([
        DPoint::new(132.0, 11419.0),
        DPoint::new(130.8954319999999996, 11419.0),
        DPoint::new(130.0, 11418.10449999999946),
        DPoint::new(130.0, 11417.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 3.0),
        DPoint::new(-1.0564518, 1.79032254),
        DPoint::new(1.45265341, 0.229448318),
        DPoint::new(1.45381773, 0.22913377),
    ]),
    CubicPts::new([
        DPoint::new(1.45381773, 0.22913377),
        DPoint::new(1.45425761, 0.229014933),
        DPoint::new(1.0967741, 0.451612949),
        DPoint::new(0.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(1.64551306_f32 as f64, 3.57876182_f32 as f64),
        DPoint::new(0.298127174_f32 as f64, 3.70454836_f32 as f64),
        DPoint::new(-0.809808373_f32 as f64, 6.39524937_f32 as f64),
        DPoint::new(-3.66666651_f32 as f64, 13.333334_f32 as f64),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 2.0),
        DPoint::new(1.0, 2.0),
        DPoint::new(-3.66666651_f32 as f64, 13.333334_f32 as f64),
        DPoint::new(5.0, 6.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0660428554, 1.65340209),
        DPoint::new(-0.251940489, 1.43560803),
        DPoint::new(-0.782382965, -0.196299091),
        DPoint::new(3.33333325, -0.666666627),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 3.0),
        DPoint::new(-1.22353387, 1.09411383),
        DPoint::new(0.319867611, 0.12996155),
        DPoint::new(0.886705518, 0.107543148),
    ]),
    CubicPts::new([
        DPoint::new(-0.13654758, 2.10514426),
        DPoint::new(-0.585797966, 1.89349782),
        DPoint::new(-0.807703257, -0.192306399),
        DPoint::new(6.0, -1.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 4.0),
        DPoint::new(-2.25000453, 1.42241001),
        DPoint::new(1.1314013, 0.0505309105),
        DPoint::new(1.87140274, 0.0363764353),
    ]),
    CubicPts::new([
        DPoint::new(1.3127951622009277, 2.0637707710266113),
        DPoint::new(1.8210518360137939, 1.9148571491241455),
        DPoint::new(1.6106204986572266, -0.68700540065765381),
        DPoint::new(8.5, -2.5),
    ]),
    CubicPts::new([
        DPoint::new(3.0, 4.0),
        DPoint::new(0.33333325386047363, 1.3333332538604736),
        DPoint::new(3.6666667461395264, -0.66666674613952637),
        DPoint::new(3.6666665077209473, -0.66666656732559204),
    ]),
    CubicPts::new([
        DPoint::new(980.026001, 1481.276),
        DPoint::new(980.026001, 1481.276),
        DPoint::new(980.02594, 1481.27576),
        DPoint::new(980.025879, 1481.27527),
    ]),
    CubicPts::new([
        DPoint::new(980.025879, 1481.27527),
        DPoint::new(980.025452, 1481.27222),
        DPoint::new(980.023743, 1481.26038),
        DPoint::new(980.02179, 1481.24072),
    ]),
    CubicPts::new([
        DPoint::new(1.80943513, 3.07782435),
        DPoint::new(1.66686702, 2.16806936),
        DPoint::new(1.68301272, 0.0),
        DPoint::new(3.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 3.0),
        DPoint::new(3.0, 2.0),
        DPoint::new(5.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(3.4386673, 2.66977954),
        DPoint::new(4.06668949, 2.17046738),
        DPoint::new(4.78887367, 1.59629118),
        DPoint::new(6.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.71985495, 3.49467373),
        DPoint::new(2.11620402, 2.7201426),
        DPoint::new(2.91897964, 1.15138781),
        DPoint::new(6.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.392703831, 1.78540766),
        DPoint::new(0.219947904, 2.05676103),
        DPoint::new(0.218561709, 2.05630541),
    ]),
    CubicPts::new([
        DPoint::new(0.218561709, 2.05630541),
        DPoint::new(0.216418028, 2.05560064),
        DPoint::new(0.624105453, 1.40486407),
        DPoint::new(4.16666651, 1.00000012),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(3.0, 5.0),
        DPoint::new(2.0, 1.0),
        DPoint::new(3.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(1.01366711_f32 as f64, 2.21379328_f32 as f64),
        DPoint::new(1.09074128_f32 as f64, 2.23241305_f32 as f64),
        DPoint::new(1.60246587_f32 as f64, 0.451849401_f32 as f64),
        DPoint::new(5.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.541499972_f32 as f64, 3.16599989_f32 as f64),
        DPoint::new(1.08299994_f32 as f64, 2.69299984_f32 as f64),
        DPoint::new(2.10083938_f32 as f64, 1.80391729_f32 as f64),
    ]),
    CubicPts::new([
        DPoint::new(0.806384504_f32 as f64, 2.85426903_f32 as f64),
        DPoint::new(1.52740121_f32 as f64, 1.99355423_f32 as f64),
        DPoint::new(2.81689167_f32 as f64, 0.454222918_f32 as f64),
        DPoint::new(5.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(1.90192389_f32 as f64, 2.90192389_f32 as f64),
        DPoint::new(2.59807634_f32 as f64, 2.79422879_f32 as f64),
        DPoint::new(3.1076951_f32 as f64, 2.71539044_f32 as f64),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 3.0),
        DPoint::new(2.36602545_f32 as f64, 3.36602545_f32 as f64),
        DPoint::new(2.330127_f32 as f64, 3.06217766_f32 as f64),
        DPoint::new(2.28460979_f32 as f64, 2.67691422_f32 as f64),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(1.90192389_f32 as f64, 2.90192389_f32 as f64),
        DPoint::new(2.59807634_f32 as f64, 2.79422879_f32 as f64),
        DPoint::new(3.1076951_f32 as f64, 2.71539044_f32 as f64),
    ]),
    CubicPts::new([
        DPoint::new(2.28460979_f32 as f64, 2.67691422_f32 as f64),
        DPoint::new(2.20577145_f32 as f64, 2.00961876_f32 as f64),
        DPoint::new(2.09807634_f32 as f64, 1.09807622_f32 as f64),
        DPoint::new(4.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.8211091160774231, 2.0948121547698975),
        DPoint::new(0.91805583238601685, 2.515404224395752),
        DPoint::new(0.91621249914169312, 2.5146586894989014),
    ]),
    CubicPts::new([
        DPoint::new(0.91621249914169312, 2.5146586894989014),
        DPoint::new(0.91132104396820068, 2.5126807689666748),
        DPoint::new(0.21079301834106445, -0.45617169141769409),
        DPoint::new(10.5, -1.6666665077209473),
    ]),
    CubicPts::new([
        DPoint::new(42.6237564, 68.9841232),
        DPoint::new(32.449646, 81.963089),
        DPoint::new(14.7713947, 103.565269),
        DPoint::new(12.6310005, 105.247002),
    ]),
    CubicPts::new([
        DPoint::new(37.2640038, 95.3540039),
        DPoint::new(37.2640038, 95.3540039),
        DPoint::new(11.3710003, 83.7339935),
        DPoint::new(-25.0779991, 124.912003),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(4.0, 5.0),
        DPoint::new(6.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 6.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(5.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(4.0, 6.0),
        DPoint::new(5.0, 1.0),
        DPoint::new(6.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 5.0),
        DPoint::new(2.0, 6.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(6.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(322.0, 896.04803466796875),
        DPoint::new(314.09201049804687, 833.4376220703125),
        DPoint::new(260.24713134765625, 785.0),
        DPoint::new(195.0, 785.0),
    ]),
    CubicPts::new([
        DPoint::new(195.0, 785.0),
        DPoint::new(265.14016723632812, 785.0),
        DPoint::new(322.0, 842.30755615234375),
        DPoint::new(322.0, 913.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 4.0),
        DPoint::new(4.0, 5.0),
        DPoint::new(3.0, 2.0),
        DPoint::new(6.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 3.0),
        DPoint::new(3.0, 6.0),
        DPoint::new(4.0, 1.0),
        DPoint::new(5.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(67.0, 913.0),
        DPoint::new(67.0, 917.388916015625),
        DPoint::new(67.224380493164063, 921.72576904296875),
        DPoint::new(67.662384033203125, 926.0),
    ]),
    CubicPts::new([
        DPoint::new(194.0, 1041.0),
        DPoint::new(123.85984039306641, 1041.0),
        DPoint::new(67.0, 983.69244384765625),
        DPoint::new(67.0, 913.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 4.0),
        DPoint::new(1.0, 5.0),
        DPoint::new(6.0, 0.0),
        DPoint::new(5.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 6.0),
        DPoint::new(1.0, 5.0),
        DPoint::new(4.0, 1.0),
        DPoint::new(5.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(4.0, 5.0),
        DPoint::new(6.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 6.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(5.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(4.0, 6.0),
        DPoint::new(2.0, 0.0),
        DPoint::new(2.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 2.0),
        DPoint::new(0.0, 2.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(6.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(980.9000244140625, 1474.3280029296875),
        DPoint::new(980.9000244140625, 1474.3280029296875),
        DPoint::new(978.89300537109375, 1471.95703125),
        DPoint::new(981.791015625, 1469.487060546875),
    ]),
    CubicPts::new([
        DPoint::new(981.791015625, 1469.487060546875),
        DPoint::new(981.791015625, 1469.4859619140625),
        DPoint::new(983.3580322265625, 1472.72900390625),
        DPoint::new(980.9000244140625, 1474.3280029296875),
    ]),
    CubicPts::new([
        DPoint::new(275.0, 532.0),
        DPoint::new(277.209137, 532.0),
        DPoint::new(279.0, 530.209106),
        DPoint::new(279.0, 528.0),
    ]),
    CubicPts::new([
        DPoint::new(278.0, 529.0),
        DPoint::new(278.0, 530.65686),
        DPoint::new(276.65686, 532.0),
        DPoint::new(275.0, 532.0),
    ]),
    CubicPts::new([
        DPoint::new(149.0, 710.001465),
        DPoint::new(149.000809, 712.209961),
        DPoint::new(150.791367, 714.0),
        DPoint::new(153.0, 714.0),
    ]),
    CubicPts::new([
        DPoint::new(154.0, 715.0),
        DPoint::new(151.238571, 715.0),
        DPoint::new(149.0, 712.761414),
        DPoint::new(149.0, 710.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 2.0),
        DPoint::new(1.0, 2.0),
        DPoint::new(2.0, 0.0),
        DPoint::new(6.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 2.0),
        DPoint::new(0.0, 6.0),
        DPoint::new(2.0, 1.0),
        DPoint::new(2.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(2.0, 3.0),
        DPoint::new(5.0, 1.0),
        DPoint::new(4.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 5.0),
        DPoint::new(3.0, 4.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(3.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(399.0, 657.0),
        DPoint::new(399.0, 661.970581),
        DPoint::new(403.029449, 666.0),
        DPoint::new(408.0, 666.0),
    ]),
    CubicPts::new([
        DPoint::new(406.0, 666.0),
        DPoint::new(402.686279, 666.0),
        DPoint::new(400.0, 663.313721),
        DPoint::new(400.0, 660.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 5.0),
        DPoint::new(3.0, 5.0),
        DPoint::new(3.0, 0.0),
        DPoint::new(3.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 3.0),
        DPoint::new(2.0, 3.0),
        DPoint::new(5.0, 0.0),
        DPoint::new(5.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(132.0, 11419.0),
        DPoint::new(130.89543151855469, 11419.0),
        DPoint::new(130.0, 11418.1044921875),
        DPoint::new(130.0, 11417.0),
    ]),
    CubicPts::new([
        DPoint::new(3.0, 4.0),
        DPoint::new(1.0, 5.0),
        DPoint::new(4.0, 3.0),
        DPoint::new(6.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(3.0, 4.0),
        DPoint::new(4.0, 6.0),
        DPoint::new(4.0, 3.0),
        DPoint::new(5.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(130.04275512695312, 11417.413085937500),
        DPoint::new(130.23312377929687, 11418.319335937500),
        DPoint::new(131.03707885742187, 11419.000000000000),
        DPoint::new(132.00000000000000, 11419.000000000000),
    ]),
    CubicPts::new([
        DPoint::new(132.00000000000000, 11419.000000000000),
        DPoint::new(130.89543151855469, 11419.000000000000),
        DPoint::new(130.00000000000000, 11418.104492187500),
        DPoint::new(130.00000000000000, 11417.000000000000),
    ]),
    CubicPts::new([
        DPoint::new(1.0516976506771041, 2.9684399028541346),
        DPoint::new(1.0604363140895228, 2.9633503074444141),
        DPoint::new(1.0692548215065762, 2.9580354426587459),
        DPoint::new(1.0781560339512140, 2.9525043684031349),
    ]),
    CubicPts::new([
        DPoint::new(1.0523038101345104, 2.9523755204833737),
        DPoint::new(1.0607035288264237, 2.9580853881628375),
        DPoint::new(1.0690530472271964, 2.9633896794787749),
        DPoint::new(1.0773566568712512, 2.9682969775000219),
    ]),
    CubicPts::new([
        DPoint::new(1.0386522625066592, 2.9759024812329078),
        DPoint::new(1.0559713690392631, 2.9661782500838885),
        DPoint::new(1.0736041309019990, 2.9555348259177858),
        DPoint::new(1.0915734362784633, 2.9440446879826569),
    ]),
    CubicPts::new([
        DPoint::new(1.0396670794879301, 2.9435062123457261),
        DPoint::new(1.0565690546812769, 2.9557413250983462),
        DPoint::new(1.0732616463413533, 2.9663369676594282),
        DPoint::new(1.0897791867435489, 2.9753618045797472),
    ]),
    CubicPts::new([
        DPoint::new(0.8685656183311091, 3.0409266475785208),
        DPoint::new(0.99189542936395292, 3.0212163698184424),
        DPoint::new(1.1302108367493320, 2.9265646471747306),
        DPoint::new(1.2952305904872474, 2.7940808546473788),
    ]),
    CubicPts::new([
        DPoint::new(0.85437872843682727, 2.7536036928549055),
        DPoint::new(1.0045584590592620, 2.9493041024831705),
        DPoint::new(1.1336998329885613, 3.0248027987251747),
        DPoint::new(1.2593809752247314, 3.0152560315809107),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 6.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(6.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(2.0, 6.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(6.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(134.0, 11414.0),
        DPoint::new(131.990234375, 11414.0),
        DPoint::new(130.32666015625, 11415.482421875),
        DPoint::new(130.04275512695312, 11417.4130859375),
    ]),
    CubicPts::new([
        DPoint::new(132.0, 11419.0),
        DPoint::new(130.89543151855469, 11419.0),
        DPoint::new(130.0, 11418.1044921875),
        DPoint::new(130.0, 11417.0),
    ]),
    CubicPts::new([
        DPoint::new(132.0, 11419.0),
        DPoint::new(130.89543151855469, 11419.0),
        DPoint::new(130.0, 11418.1044921875),
        DPoint::new(130.0, 11417.0),
    ]),
    CubicPts::new([
        DPoint::new(130.04275512695312, 11417.4130859375),
        DPoint::new(130.23312377929687, 11418.3193359375),
        DPoint::new(131.03707885742187, 11419.0),
        DPoint::new(132.0, 11419.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(2.0, 3.0),
        DPoint::new(5.0, 1.0),
        DPoint::new(4.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 5.0),
        DPoint::new(3.0, 4.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(3.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(3.0, 5.0),
        DPoint::new(1.0, 6.0),
        DPoint::new(5.0, 0.0),
        DPoint::new(3.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 5.0),
        DPoint::new(1.0, 3.0),
        DPoint::new(5.0, 3.0),
        DPoint::new(6.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 5.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(5.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 3.0),
        DPoint::new(5.0, 6.0),
        DPoint::new(5.0, 3.0),
        DPoint::new(5.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(3.0, 5.0),
        DPoint::new(4.0, 5.0),
        DPoint::new(3.0, 1.0),
        DPoint::new(6.0, 5.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 5.0),
        DPoint::new(0.0, 5.0),
        DPoint::new(5.0, 4.0),
        DPoint::new(6.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(4.0, 5.0),
        DPoint::new(4.0, 6.0),
        DPoint::new(5.0, 0.0),
        DPoint::new(5.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 4.0),
        DPoint::new(1.0, 3.0),
        DPoint::new(5.0, 4.0),
        DPoint::new(4.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(4.0, 5.0),
        DPoint::new(2.0, 4.0),
        DPoint::new(4.0, 0.0),
        DPoint::new(3.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 2.0),
        DPoint::new(1.0, 5.0),
        DPoint::new(3.0, 2.0),
        DPoint::new(4.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 3.0),
        DPoint::new(1.0, 4.0),
        DPoint::new(2.0, 0.0),
        DPoint::new(5.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 2.0),
        DPoint::new(2.0, 3.0),
        DPoint::new(5.0, 1.0),
        DPoint::new(3.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 5.0),
        DPoint::new(2.0, 3.0),
        DPoint::new(2.0, 0.0),
        DPoint::new(3.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 6.0),
        DPoint::new(4.0, 5.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(6.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 6.0),
        DPoint::new(6.0, 2.0),
        DPoint::new(5.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 2.0),
        DPoint::new(6.0, 5.0),
        DPoint::new(5.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(5.0, 6.0),
        DPoint::new(4.0, 5.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(2.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(2.5119999999999996, 1.5710000000000002),
        DPoint::new(2.6399999999999983, 1.6599999999999997),
        DPoint::new(2.8000000000000007, 1.8000000000000003),
        DPoint::new(3.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(2.4181876227114887, 1.9849772580462195),
        DPoint::new(2.8269904869227211, 2.009330650246834),
        DPoint::new(3.2004679292461624, 1.9942047174679169),
        DPoint::new(3.4986199496818058, 2.0035994597094731),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 3.0),
        DPoint::new(1.0, 4.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(6.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 6.0),
        DPoint::new(3.0, 2.0),
        DPoint::new(4.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 2.0),
        DPoint::new(1.0, 5.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(6.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 6.0),
        DPoint::new(2.0, 0.0),
        DPoint::new(5.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 5.0),
        DPoint::new(2.0, 1.0),
        DPoint::new(4.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 2.0),
        DPoint::new(0.0, 4.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(5.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(3.0, 5.0),
        DPoint::new(2.0, 1.0),
        DPoint::new(3.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 2.0),
        DPoint::new(1.0, 3.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(5.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(2.0, 5.0),
        DPoint::new(6.0, 0.0),
        DPoint::new(5.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 6.0),
        DPoint::new(3.0, 5.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(5.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(3.0, 6.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(5.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(2.0, 5.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(6.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 2.0),
        DPoint::new(5.0, 6.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(2.0, 1.0),
        DPoint::new(6.0, 5.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 6.0),
        DPoint::new(1.0, 2.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(6.0, 0.0),
        DPoint::new(2.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 2.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(3.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 3.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(2.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
];

/// `newTestSetCount`.
// Port of: tests/PathOpsCubicIntersectionTest.cpp#L553 (chrome/m156)
const NEW_TEST_SET_COUNT: usize = NEW_TEST_SET.len();

/// `static constexpr auto selfSet`.
// Port of: tests/PathOpsCubicIntersectionTest.cpp#L793-L802 (chrome/m156)
#[allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::approx_constant
)] // the table keeps the C++ literals verbatim
static SELF_SET: &[CubicPts] = &[
    CubicPts::new([
        DPoint::new(2.0, 3.0),
        DPoint::new(0.0, 4.0),
        DPoint::new(3.0, 2.0),
        DPoint::new(5.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(3.0, 6.0),
        DPoint::new(2.0, 3.0),
        DPoint::new(4.0, 0.0),
        DPoint::new(3.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 2.0),
        DPoint::new(2.0, 3.0),
        DPoint::new(5.0, 1.0),
        DPoint::new(3.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 2.0),
        DPoint::new(3.0, 5.0),
        DPoint::new(5.0, 0.0),
        DPoint::new(4.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(3.34, 8.98),
        DPoint::new(1.95, 10.27),
        DPoint::new(3.76, 7.65),
        DPoint::new(4.96, 10.64),
    ]),
    CubicPts::new([
        DPoint::new(3.13, 2.74),
        DPoint::new(1.08, 4.62),
        DPoint::new(3.71, 0.94),
        DPoint::new(2.01, 3.81),
    ]),
    CubicPts::new([
        DPoint::new(6.71, 3.14),
        DPoint::new(7.99, 2.75),
        DPoint::new(8.27, 1.96),
        DPoint::new(6.35, 3.57),
    ]),
    CubicPts::new([
        DPoint::new(12.81, 7.27),
        DPoint::new(7.22, 6.98),
        DPoint::new(12.49, 8.97),
        DPoint::new(11.42, 6.18),
    ]),
];

/// `static constexpr auto coinSet`.
// Port of: tests/PathOpsCubicIntersectionTest.cpp#L842-L869 (chrome/m156)
#[allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::approx_constant
)] // the table keeps the C++ literals verbatim
static COIN_SET: &[CubicPts] = &[
    CubicPts::new([
        DPoint::new(72.350448608398438, 27.966041564941406),
        DPoint::new(72.58441162109375, 27.861515045166016),
        DPoint::new(72.818222045898437, 27.756658554077148),
        DPoint::new(73.394996643066406, 27.49799919128418),
    ]),
    CubicPts::new([
        DPoint::new(73.394996643066406, 27.49799919128418),
        DPoint::new(72.818222045898437, 27.756658554077148),
        DPoint::new(72.58441162109375, 27.861515045166016),
        DPoint::new(72.350448608398438, 27.966041564941406),
    ]),
    CubicPts::new([
        DPoint::new(297.04998779296875, 43.928997039794922),
        DPoint::new(297.04998779296875, 43.928997039794922),
        DPoint::new(300.69699096679688, 45.391998291015625),
        DPoint::new(306.92498779296875, 43.08599853515625),
    ]),
    CubicPts::new([
        DPoint::new(297.04998779296875, 43.928997039794922),
        DPoint::new(297.04998779296875, 43.928997039794922),
        DPoint::new(300.69699096679688, 45.391998291015625),
        DPoint::new(306.92498779296875, 43.08599853515625),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 3.0),
        DPoint::new(0.0, 4.0),
        DPoint::new(3.0, 2.0),
        DPoint::new(5.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 3.0),
        DPoint::new(0.0, 4.0),
        DPoint::new(3.0, 2.0),
        DPoint::new(5.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(317.0, 711.0),
        DPoint::new(322.52285766601562, 711.0),
        DPoint::new(327.0, 715.4771728515625),
        DPoint::new(327.0, 721.0),
    ]),
    CubicPts::new([
        DPoint::new(324.07107543945312, 713.928955078125),
        DPoint::new(324.4051513671875, 714.26300048828125),
        DPoint::new(324.71566772460937, 714.62060546875),
        DPoint::new(325.0, 714.9990234375),
    ]),
];

/// Port of `standardTestCases`. Its `SkDebugf` lines are not ported (`showSkipped` is false).
// Port of: tests/PathOpsCubicIntersectionTest.cpp#L30-L81 (chrome/m156)
fn standard_test_cases(reporter: &mut Reporter) {
    for cubics in TESTS.iter().skip(FIRST_CUBIC_INTERSECTION_TEST) {
        let cubic1 = &cubics[0];
        let cubic2 = &cubics[1];
        let c1 = DCubic::new(cubic1.pts);
        let c2 = DCubic::new(cubic2.pts);
        let mut reduce1 = ReduceOrder::default();
        let mut reduce2 = ReduceOrder::default();
        let order1 = reduce1.reduce_cubic(&c1, Quadratics::No);
        let order2 = reduce2.reduce_cubic(&c2, Quadratics::No);
        if order1 < 4 {
            continue;
        }
        if order2 < 4 {
            continue;
        }
        let mut t_intersections = Intersections::default();
        t_intersections.intersect_cubic_cubic(&c1, &c2);
        if t_intersections.used() == 0 {
            continue;
        }
        if t_intersections.is_coincident(0) {
            continue;
        }
        for pt in 0..t_intersections.used() {
            let tt1 = t_intersections.t(0, pt);
            let xy1 = c1.pt_at_t(tt1);
            let tt2 = t_intersections.t(1, pt);
            let xy2 = c2.pt_at_t(tt2);
            reporter_assert!(reporter, xy1.approximately_equal(xy2));
        }
        reporter.bump_test_count();
    }
}

/// Port of `oneOff(reporter, cubic1, cubic2, coin)`.
// Port of: tests/PathOpsCubicIntersectionTest.cpp#L554-L599 (chrome/m156)
fn one_off(reporter: &mut Reporter, cubic1: &CubicPts, cubic2: &CubicPts, coin: bool) {
    let c1 = DCubic::new(cubic1.pts);
    let c2 = DCubic::new(cubic2.pts);
    debug_assert!(valid_cubic(&c1));
    debug_assert!(valid_cubic(&c2));
    let mut intersections = Intersections::default();
    intersections.intersect_cubic_cubic(&c1, &c2);
    reporter_assert!(reporter, !coin || intersections.used() >= 2);
    for pt3 in 0..intersections.used() {
        let tt1 = intersections.t(0, pt3);
        let xy1 = c1.pt_at_t(tt1);
        let tt2 = intersections.t(1, pt3);
        let xy2 = c2.pt_at_t(tt2);
        let i_pt = intersections.pt(pt3);
        reporter_assert!(reporter, xy1.approximately_equal(i_pt));
        reporter_assert!(reporter, xy2.approximately_equal(i_pt));
        reporter_assert!(reporter, xy1.approximately_equal(xy2));
    }
    reporter.bump_test_count();
}

/// Port of `oneOff(reporter, outer, inner)`.
// Port of: tests/PathOpsCubicIntersectionTest.cpp#L601-L605 (chrome/m156)
fn one_off_pair(reporter: &mut Reporter, outer: usize, inner: usize) {
    one_off(reporter, &TEST_SET[outer], &TEST_SET[inner], false);
}

/// Port of `newOneOff`.
// Port of: tests/PathOpsCubicIntersectionTest.cpp#L607-L611 (chrome/m156)
fn new_one_off(reporter: &mut Reporter, outer: usize, inner: usize) {
    one_off(reporter, &NEW_TEST_SET[outer], &NEW_TEST_SET[inner], false);
}

/// Port of `testsOneOff`.
// Port of: tests/PathOpsCubicIntersectionTest.cpp#L613-L617 (chrome/m156)
fn tests_one_off(reporter: &mut Reporter, index: usize) {
    one_off(reporter, &TESTS[index][0], &TESTS[index][1], false);
}

/// Port of `oneOffTests`.
// Port of: tests/PathOpsCubicIntersectionTest.cpp#L619-L630 (chrome/m156)
fn one_off_tests(reporter: &mut Reporter) {
    for outer in 0..TEST_SET_COUNT - 1 {
        for inner in outer + 1..TEST_SET_COUNT {
            one_off_pair(reporter, outer, inner);
        }
    }
    for outer in 0..NEW_TEST_SET_COUNT - 1 {
        for inner in outer + 1..NEW_TEST_SET_COUNT {
            new_one_off(reporter, outer, inner);
        }
    }
}

/// Port of `selfOneOff`.
// Port of: tests/PathOpsCubicIntersectionTest.cpp#L806-L833 (chrome/m156)
fn self_one_off(reporter: &mut Reporter, set_idx: usize) {
    let cubic = &SELF_SET[set_idx];
    let c: [Point; 4] = cubic.pts.map(DPoint::as_sk_point);
    let mut loop_t = [0.0_f32; 3];
    let cubic_type = classify_cubic(&c);
    let breaks = DCubic::complex_break(c, &mut loop_t);
    debug_assert!(breaks < 2);
    if breaks != 0 && cubic_type == CubicType::Loop {
        let mut i = Intersections::default();
        let mut two_cubics = [Point::default(); 7];
        chop_cubic_at(&c, &mut two_cubics, loop_t[0]);
        let mut chopped = [DCubic::default(); 2];
        chopped[0].set([two_cubics[0], two_cubics[1], two_cubics[2], two_cubics[3]]);
        chopped[1].set([two_cubics[3], two_cubics[4], two_cubics[5], two_cubics[6]]);
        let result = i.intersect_cubic_cubic(&chopped[0], &chopped[1]);
        reporter_assert!(reporter, result == 2);
        reporter_assert!(reporter, i.used() == 2);
        for index in 0..result {
            let pt1 = chopped[0].pt_at_t(i.t(0, index));
            let pt2 = chopped[1].pt_at_t(i.t(1, index));
            reporter_assert!(reporter, pt1.approximately_equal(pt2));
            reporter.bump_test_count();
        }
    }
}

/// Port of `cubicIntersectionSelfTest`.
// Port of: tests/PathOpsCubicIntersectionTest.cpp#L835-L840 (chrome/m156)
fn cubic_intersection_self_test(reporter: &mut Reporter) {
    let first_fail = 0;
    for index in first_fail..SELF_SET.len() {
        self_one_off(reporter, index);
    }
}

/// Port of `coinOneOff`.
// Port of: tests/PathOpsCubicIntersectionTest.cpp#L873-L877 (chrome/m156)
fn coin_one_off(reporter: &mut Reporter, index: usize) {
    one_off(reporter, &COIN_SET[index], &COIN_SET[index + 1], true);
}

/// Port of `cubicIntersectionCoinTest`.
// Port of: tests/PathOpsCubicIntersectionTest.cpp#L879-L884 (chrome/m156)
fn cubic_intersection_coin_test(reporter: &mut Reporter) {
    let first_fail = 0;
    let mut index = first_fail;
    while index < COIN_SET.len() {
        coin_one_off(reporter, index);
        index += 2;
    }
}

// Port of: tests/PathOpsCubicIntersectionTest.cpp#L886-L888 (chrome/m156)
def_test!(PathOpsCubicCoinOneOff, |reporter| {
    coin_one_off(reporter, 0);
});

// Port of: tests/PathOpsCubicIntersectionTest.cpp#L890-L892 (chrome/m156)
def_test!(PathOpsCubicIntersectionOneOff, |reporter| {
    new_one_off(reporter, 0, 1);
});

// Port of: tests/PathOpsCubicIntersectionTest.cpp#L894-L896 (chrome/m156)
def_test!(PathOpsCubicIntersectionTestsOneOff, |reporter| {
    tests_one_off(reporter, 10);
});

// Port of: tests/PathOpsCubicIntersectionTest.cpp#L898-L900 (chrome/m156)
def_test!(PathOpsCubicSelfOneOff, |reporter| {
    self_one_off(reporter, 0);
});

// Port of: tests/PathOpsCubicIntersectionTest.cpp#L902-L909 (chrome/m156)
def_test!(PathOpsCubicIntersection, |reporter| {
    one_off_tests(reporter);
    cubic_intersection_self_test(reporter);
    cubic_intersection_coin_test(reporter);
    standard_test_cases(reporter);
});
