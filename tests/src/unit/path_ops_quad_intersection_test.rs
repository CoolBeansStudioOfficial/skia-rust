// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsQuadIntersectionTest.cpp (chrome/m156)

#![cfg(test)]

use crate::unit::path_ops_quad_intersection_test_data::QUADRATICTESTS;
use crate::unit::path_ops_test_common::{QuadPts, valid_quad};
use crate::{Reporter, def_test, reporter_assert};
use skia_rust_pathops::intersections::Intersections;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::quad::DQuad;
use skia_rust_pathops::reduce_order::ReduceOrder;

/// `SkPathOpsDebug::gVeryVerbose`, which is false unless the test is run with extensive checking.
/// `PathOpsQuadBinaryProfile` returns at once when it is false.
const VERY_VERBOSE: bool = false;

/// `static const auto testSet`. The literals are the C++ `float` constants (`f`) widened to
/// `double`, and the plain `double` constants, as `QuadPts` holds `SkDPoint`s.
// Port of: tests/PathOpsQuadIntersectionTest.cpp#L68-L425 (chrome/m156)
#[allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::approx_constant
)] // the table keeps the C++ literals verbatim
static TEST_SET: [QuadPts; 155] = [
    QuadPts::new([
        DPoint::new(123.637985_f32 as f64, 102.405312_f32 as f64),
        DPoint::new(125.172699_f32 as f64, 104.575714_f32 as f64),
        DPoint::new(123.387383_f32 as f64, 106.91227_f32 as f64),
    ]),
    QuadPts::new([
        DPoint::new(123.388428_f32 as f64, 106.910896_f32 as f64),
        DPoint::new(123.365623_f32 as f64, 106.94088_f32 as f64),
        DPoint::new(123.320007_f32 as f64, 107.000946_f32 as f64),
    ]),
    QuadPts::new([
        DPoint::new(-0.001019871095195412636, -0.008523519150912761688),
        DPoint::new(-0.005396408028900623322, -0.005396373569965362549),
        DPoint::new(-0.02855382487177848816, -0.02855364233255386353),
    ]),
    QuadPts::new([
        DPoint::new(-0.004567248281091451645, -0.01482933573424816132),
        DPoint::new(-0.01142475008964538574, -0.01140109263360500336),
        DPoint::new(-0.02852955088019371033, -0.02847047336399555206),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, 2.0),
        DPoint::new(3.0, 3.0),
    ]),
    QuadPts::new([
        DPoint::new(3.0, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 2.0),
    ]),
    QuadPts::new([
        DPoint::new(0.33333333333333326, 0.81481481481481488),
        DPoint::new(0.63395173631977997, 0.68744136726313931),
        DPoint::new(1.205684411948591, 0.81344322326274499),
    ]),
    QuadPts::new([
        DPoint::new(0.33333333333333326, 0.81481481481481488),
        DPoint::new(0.63396444791444551, 0.68743368362444768),
        DPoint::new(1.205732763658403, 0.81345617746834109),
    ]),
    QuadPts::new([
        DPoint::new(4981.9990234375, 1590.0),
        DPoint::new(4981.9990234375, 1617.7523193359375),
        DPoint::new(4962.375, 1637.3760986328125),
    ]),
    QuadPts::new([
        DPoint::new(4962.3759765625, 1637.3760986328125),
        DPoint::new(4982.0, 1617.7523193359375),
        DPoint::new(4982.0, 1590.0),
    ]),
    QuadPts::new([
        DPoint::new(48.7416_f32 as f64, 7.74160004_f32 as f64),
        DPoint::new(96.4831848_f32 as f64, -40.0),
        DPoint::new(164.0, -40.0),
    ]),
    QuadPts::new([
        DPoint::new(56.9671326_f32 as f64, 0.0),
        DPoint::new(52.7835083_f32 as f64, 3.69968891_f32 as f64),
        DPoint::new(48.7416_f32 as f64, 7.74160004_f32 as f64),
    ]),
    QuadPts::new([
        DPoint::new(138.0, 80.0),
        DPoint::new(147.15692138671875, 80.0),
        DPoint::new(155.12803649902344, 82.86279296875),
    ]),
    QuadPts::new([
        DPoint::new(155.12803649902344, 82.86279296875),
        DPoint::new(153.14971923828125, 82.152290344238281),
        DPoint::new(151.09841918945312, 81.618133544921875),
    ]),
    QuadPts::new([
        DPoint::new(88.0, 130.0),
        DPoint::new(88.0, 131.54483032226562),
        DPoint::new(88.081489562988281, 133.0560302734375),
    ]),
    QuadPts::new([
        DPoint::new(88.081489562988281, 133.0560302734375),
        DPoint::new(88.0, 131.54483032226562),
        DPoint::new(88.0, 130.0),
    ]),
    QuadPts::new([
        DPoint::new(0.59987992, 2.14448452),
        DPoint::new(0.775417507, 1.95606446),
        DPoint::new(1.00564098, 1.79310346),
    ]),
    QuadPts::new([
        DPoint::new(1.00564098, 1.79310346),
        DPoint::new(1.25936198, 1.615623),
        DPoint::new(1.35901463, 1.46834028),
    ]),
    QuadPts::new([
        DPoint::new(3.0, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(3.0, 2.0),
    ]),
    QuadPts::new([
        DPoint::new(2.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
    ]),
    QuadPts::new([
        DPoint::new(38.656852722167969, 38.656852722167969),
        DPoint::new(38.651023864746094, 38.662681579589844),
        DPoint::new(38.644744873046875, 38.668937683105469),
    ]),
    QuadPts::new([
        DPoint::new(38.656852722167969, 38.656852722167969),
        DPoint::new(36.313709259033203, 41.0),
        DPoint::new(33.0, 41.0),
    ]),
    QuadPts::new([
        DPoint::new(4914.9990234375, 1523.0),
        DPoint::new(4942.75146484375, 1523.0),
        DPoint::new(4962.375, 1542.6239013671875),
    ]),
    QuadPts::new([
        DPoint::new(4962.3759765625, 1542.6239013671875),
        DPoint::new(4942.75244140625, 1523.0),
        DPoint::new(4915.0, 1523.0),
    ]),
    QuadPts::new([
        DPoint::new(4867.623046875, 1637.3760986328125),
        DPoint::new(4847.9990234375, 1617.7523193359375),
        DPoint::new(4847.9990234375, 1590.0),
    ]),
    QuadPts::new([
        DPoint::new(4848.0, 1590.0),
        DPoint::new(4848.0, 1617.7523193359375),
        DPoint::new(4867.6240234375, 1637.3760986328125),
    ]),
    QuadPts::new([
        DPoint::new(102.64466094970703, 165.3553466796875),
        DPoint::new(110.79246520996094, 173.50314331054687),
        DPoint::new(120.81797790527344, 177.11778259277344),
    ]),
    QuadPts::new([
        DPoint::new(113.232177734375, 173.57899475097656),
        DPoint::new(116.88026428222656, 175.69805908203125),
        DPoint::new(120.81797790527344, 177.11778259277344),
    ]),
    QuadPts::new([
        DPoint::new(-37.3484879, 10.0192947),
        DPoint::new(-36.4966316, 13.2140198),
        DPoint::new(-38.1506348, 16.0788383),
    ]),
    QuadPts::new([
        DPoint::new(-38.1462746, 16.08918),
        DPoint::new(-36.4904327, 13.2193804),
        DPoint::new(-37.3484879, 10.0192947),
    ]),
    QuadPts::new([
        DPoint::new(-37.3513985, 10.0082998),
        DPoint::new(-36.4938011, 13.2090998),
        DPoint::new(-38.1506004, 16.0788002),
    ]),
    QuadPts::new([
        DPoint::new(-37.3508987, 10.0102997),
        DPoint::new(-36.4930992, 13.2110004),
        DPoint::new(-38.1497993, 16.0809002),
    ]),
    QuadPts::new([
        DPoint::new(-37.3508987, 10.0102997),
        DPoint::new(-37.3510017, 10.0098),
        DPoint::new(-37.3512001, 10.0093002),
    ]),
    QuadPts::new([
        DPoint::new(-49.0778008, 19.0097008),
        DPoint::new(-38.2086983, 6.80954981),
        DPoint::new(-37.3508987, 10.0102997),
    ]),
    QuadPts::new([
        DPoint::new(
            f32::from_bits(0xc22423b2) as f64,
            f32::from_bits(0x40afae2c) as f64,
        ),
        DPoint::new(
            f32::from_bits(0xc2189b24) as f64,
            f32::from_bits(0x40e3f058) as f64,
        ),
        DPoint::new(
            f32::from_bits(0xc21511d9) as f64,
            f32::from_bits(0x41251125) as f64,
        ),
    ]),
    QuadPts::new([
        DPoint::new(
            f32::from_bits(0xc2153d2f) as f64,
            f32::from_bits(0x412299db) as f64,
        ),
        DPoint::new(
            f32::from_bits(0xc2153265) as f64,
            f32::from_bits(0x41233845) as f64,
        ),
        DPoint::new(
            f32::from_bits(0xc21527fc) as f64,
            f32::from_bits(0x4123d684) as f64,
        ),
    ]),
    QuadPts::new([
        DPoint::new(-37.3097496, 10.1625624),
        DPoint::new(-37.2992134, 10.2012377),
        DPoint::new(-37.2890472, 10.239872),
    ]),
    QuadPts::new([
        DPoint::new(-41.0348587, 5.49001122),
        DPoint::new(-38.1515045, 7.12308884),
        DPoint::new(-37.2674294, 10.3166857),
    ]),
    QuadPts::new([
        DPoint::new(-52.8062439, 14.1493912),
        DPoint::new(-53.6638947, 10.948595),
        DPoint::new(-52.0070419, 8.07883835),
    ]),
    QuadPts::new([
        DPoint::new(-52.8054848, 14.1522331),
        DPoint::new(-53.6633072, 10.9514809),
        DPoint::new(-52.0066071, 8.08163643),
    ]),
    QuadPts::new([
        DPoint::new(441.853149, 308.209106),
        DPoint::new(434.672272, 315.389984),
        DPoint::new(424.516998, 315.389984),
    ]),
    QuadPts::new([
        DPoint::new(385.207275, 334.241272),
        DPoint::new(406.481598, 312.96698),
        DPoint::new(436.567993, 312.96698),
    ]),
    QuadPts::new([
        DPoint::new(-708.00779269310044, -154.36998607290101),
        DPoint::new(-707.90560262312511, -154.36998607290101),
        DPoint::new(-707.8333433370193, -154.44224536635932),
    ]),
    QuadPts::new([
        DPoint::new(-708.00779269310044, -154.61669472244046),
        DPoint::new(-701.04513225634582, -128.85970734043804),
        DPoint::new(505.58447265625, -504.9130859375),
    ]),
    QuadPts::new([
        DPoint::new(164.0, -40.0),
        DPoint::new(231.51681518554687, -40.0),
        DPoint::new(279.25839233398438, 7.7416000366210938),
    ]),
    QuadPts::new([
        DPoint::new(279.25839233398438, 7.7416000366210938),
        DPoint::new(275.2164306640625, 3.6996400356292725),
        DPoint::new(271.03286743164062, -5.3290705182007514e-015),
    ]),
    QuadPts::new([
        DPoint::new(2.9999997378517067, 1.9737872594345709),
        DPoint::new(2.9999997432230918, 1.9739647181863822),
        DPoint::new(1.2414155459263587e-163, 5.2957833941332142e-315),
    ]),
    QuadPts::new([
        DPoint::new(2.9999047485265304, 1.9739164225694723),
        DPoint::new(3.0000947268526112, 1.9738379076623633),
        DPoint::new(0.61149411077591886, 0.0028382324376270418),
    ]),
    QuadPts::new([
        DPoint::new(2.9999996843656502, 1.9721416019045801),
        DPoint::new(2.9999997725237835, 1.9749798343422071),
        DPoint::new(5.3039068214821359e-315, 8.9546185262775165e-307),
    ]),
    QuadPts::new([
        DPoint::new(2.9984791443874976, 1.974505741312242),
        DPoint::new(2.9999992702127476, 1.9738772171479178),
        DPoint::new(3.0015187977319759, 1.9732495027303418),
    ]),
    QuadPts::new([
        DPoint::new(0.647069409, 2.97691634),
        DPoint::new(0.946860918, 3.17625612),
        DPoint::new(1.46875407, 2.65105457),
    ]),
    QuadPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.723699095, 2.82756208),
        DPoint::new(1.08907197, 2.97497449),
    ]),
    QuadPts::new([
        DPoint::new(131.37418, 11414.9825),
        DPoint::new(130.28798, 11415.9328),
        DPoint::new(130.042755, 11417.4131),
    ]),
    QuadPts::new([
        DPoint::new(130.585787, 11418.4142),
        DPoint::new(130.021447, 11417.8498),
        DPoint::new(130.0, 11417.0),
    ]),
    QuadPts::new([
        DPoint::new(130.73167037963867, 11418.546386718750),
        DPoint::new(131.26360225677490, 11418.985778808592),
        DPoint::new(132.0, 11419.0),
    ]),
    QuadPts::new([
        DPoint::new(132.0, 11419.0),
        DPoint::new(131.15012693405151, 11418.978546142578),
        DPoint::new(130.58578681945801, 11418.414184570313),
    ]),
    QuadPts::new([
        DPoint::new(132.0, 11419.0),
        DPoint::new(130.73167037963867, 11418.546386718750),
        DPoint::new(131.26360225677490, 11418.985778808592),
    ]),
    QuadPts::new([
        DPoint::new(131.15012693405151, 11418.978546142578),
        DPoint::new(130.58578681945801, 11418.414184570313),
        DPoint::new(132.0, 11419.0),
    ]),
    QuadPts::new([
        DPoint::new(3.0774019473063863, 3.35198509346713),
        DPoint::new(3.0757503498668397, 3.327320623945933),
        DPoint::new(3.0744102085015879, 3.3025879417907196),
    ]),
    QuadPts::new([
        DPoint::new(3.053913680774329, 3.3310471586283938),
        DPoint::new(3.0758730889691694, 3.3273466070370152),
        DPoint::new(3.0975671980059394, 3.3235031316554351),
    ]),
    QuadPts::new([
        DPoint::new(3.39068129, 4.44939202),
        DPoint::new(3.03659239, 3.81843234),
        DPoint::new(3.06844529, 3.02100922),
    ]),
    QuadPts::new([
        DPoint::new(2.10714698, 3.44196686),
        DPoint::new(3.12180288, 3.38575704),
        DPoint::new(3.75968569, 3.1281838),
    ]),
    QuadPts::new([
        DPoint::new(2.74792918, 4.77711896),
        DPoint::new(2.82236867, 4.23882547),
        DPoint::new(2.82848144, 3.63729341),
    ]),
    QuadPts::new([
        DPoint::new(2.62772567, 3.64823958),
        DPoint::new(3.46652495, 3.64258364),
        DPoint::new(4.1425079, 3.48623815),
    ]),
    QuadPts::new([
        DPoint::new(1.34375, 2.03125),
        DPoint::new(2.2734375, 2.6640625),
        DPoint::new(3.25, 3.25),
    ]),
    QuadPts::new([
        DPoint::new(3.96875, 4.65625),
        DPoint::new(3.3359375, 3.7265625),
        DPoint::new(2.75, 2.75),
    ]),
    QuadPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.324417544, 2.27953848),
        DPoint::new(0.664376547, 2.58940267),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 2.0),
        DPoint::new(0.62109375, 2.70703125),
        DPoint::new(0.640625, 2.546875),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 2.0),
        DPoint::new(0.984375, 2.3359375),
        DPoint::new(1.0625, 2.15625),
    ]),
    QuadPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.983539095, 2.30041152),
        DPoint::new(1.47325103, 2.61316872),
    ]),
    QuadPts::new([
        DPoint::new(4.09011926, 2.20971038),
        DPoint::new(4.74608133, 1.9335932),
        DPoint::new(5.02469918, 2.00694987),
    ]),
    QuadPts::new([
        DPoint::new(2.79472921, 1.73568666),
        DPoint::new(3.36246373, 1.21251209),
        DPoint::new(5.0, 2.0),
    ]),
    QuadPts::new([
        DPoint::new(1.80814127, 2.41537795),
        DPoint::new(2.23475077, 2.05922313),
        DPoint::new(3.16529668, 1.98358763),
    ]),
    QuadPts::new([
        DPoint::new(2.16505631, 2.55782454),
        DPoint::new(2.40541285, 2.02193091),
        DPoint::new(2.99836023, 1.68247638),
    ]),
    QuadPts::new([
        DPoint::new(3.0, 1.875),
        DPoint::new(3.375, 1.54296875),
        DPoint::new(3.375, 1.421875),
    ]),
    QuadPts::new([
        DPoint::new(3.375, 1.421875),
        DPoint::new(3.3749999999999996, 1.3007812499999998),
        DPoint::new(3.0, 2.0),
    ]),
    QuadPts::new([
        DPoint::new(3.34, 8.98),
        DPoint::new(2.83363281, 9.4265625),
        DPoint::new(2.83796875, 9.363125),
    ]),
    QuadPts::new([
        DPoint::new(2.83796875, 9.363125),
        DPoint::new(2.84230469, 9.2996875),
        DPoint::new(3.17875, 9.1725),
    ]),
    QuadPts::new([
        DPoint::new(2.7279999999999998, 3.024),
        DPoint::new(2.5600000000000005, 2.5600000000000005),
        DPoint::new(2.1520000000000001, 1.8560000000000001),
    ]),
    QuadPts::new([
        DPoint::new(0.66666666666666652, 1.1481481481481481),
        DPoint::new(1.3333333333333326, 1.3333333333333335),
        DPoint::new(2.6666666666666665, 2.1851851851851851),
    ]),
    QuadPts::new([
        DPoint::new(2.728, 3.024),
        DPoint::new(2.56, 2.56),
        DPoint::new(2.152, 1.856),
    ]),
    QuadPts::new([
        DPoint::new(0.666666667, 1.14814815),
        DPoint::new(1.33333333, 1.33333333),
        DPoint::new(2.66666667, 2.18518519),
    ]),
    QuadPts::new([
        DPoint::new(0.875, 1.5),
        DPoint::new(1.03125, 1.11022302e-16),
        DPoint::new(1.0, 0.0),
    ]),
    QuadPts::new([
        DPoint::new(0.875, 0.859375),
        DPoint::new(1.6875, 0.73046875),
        DPoint::new(2.5, 0.625),
    ]),
    QuadPts::new([
        DPoint::new(1.64451042, 0.0942001592),
        DPoint::new(1.53635465, 0.00152863961),
        DPoint::new(1.0, 0.0),
    ]),
    QuadPts::new([
        DPoint::new(1.27672209, 0.15),
        DPoint::new(1.32143477, 9.25185854e-17),
        DPoint::new(1.0, 0.0),
    ]),
    QuadPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.51851851851851849, 1.0185185185185186),
        DPoint::new(1.2592592592592591, 1.9259259259259258),
    ]),
    QuadPts::new([
        DPoint::new(1.2592592592592593, 1.9259259259259265),
        DPoint::new(0.51851851851851893, 1.0185185185185195),
        DPoint::new(0.0, 0.0),
    ]),
    QuadPts::new([
        DPoint::new(1.93281168, 2.58856757),
        DPoint::new(2.38543691, 2.7096125),
        DPoint::new(2.51967352, 2.34531784),
    ]),
    QuadPts::new([
        DPoint::new(2.51967352, 2.34531784),
        DPoint::new(2.65263731, 2.00639194),
        DPoint::new(3.1212119, 1.98608967),
    ]),
    QuadPts::new([
        DPoint::new(2.09544533, 2.51981963),
        DPoint::new(2.33331524, 2.25252128),
        DPoint::new(2.92003302, 2.39442311),
    ]),
    QuadPts::new([
        DPoint::new(0.924337655, 1.94072717),
        DPoint::new(1.25185043, 1.52836494),
        DPoint::new(1.71793901, 1.06149951),
    ]),
    QuadPts::new([
        DPoint::new(0.940798831, 1.67439357),
        DPoint::new(1.25988251, 1.39778567),
        DPoint::new(1.71791672, 1.06650313),
    ]),
    QuadPts::new([
        DPoint::new(0.924337655, 1.94072717),
        DPoint::new(1.39158994, 1.32418496),
        DPoint::new(2.14967426, 0.687365435),
    ]),
    QuadPts::new([
        DPoint::new(0.940798831, 1.67439357),
        DPoint::new(1.48941875, 1.16280321),
        DPoint::new(2.47884711, 0.60465921),
    ]),
    QuadPts::new([
        DPoint::new(1.7465749139282332, 1.9930452039527999),
        DPoint::new(1.8320006564080331, 1.859481345189089),
        DPoint::new(1.8731035127758437, 1.6344055934266613),
    ]),
    QuadPts::new([
        DPoint::new(1.8731035127758437, 1.6344055934266613),
        DPoint::new(1.89928170345231, 1.5006405518943067),
        DPoint::new(1.9223833226085514, 1.3495796165215643),
    ]),
    QuadPts::new([
        DPoint::new(1.74657491, 1.9930452),
        DPoint::new(1.87407679, 1.76762853),
        DPoint::new(1.92238332, 1.34957962),
    ]),
    QuadPts::new([
        DPoint::new(0.60797907, 1.68776977),
        DPoint::new(1.0447864, 1.50810914),
        DPoint::new(1.87464474, 1.63655092),
    ]),
    QuadPts::new([
        DPoint::new(1.87464474, 1.63655092),
        DPoint::new(2.70450308, 1.76499271),
        DPoint::new(4.0, 3.0),
    ]),
    QuadPts::new([
        DPoint::new(1.2071879545809394, 0.82163474041730045),
        DPoint::new(1.1534203513372994, 0.52790870069930229),
        DPoint::new(1.0880000000000001, 0.29599999999999982),
    ]),
    QuadPts::new([
        DPoint::new(0.33333333333333326, 0.81481481481481488),
        DPoint::new(0.63395173631977997, 0.68744136726313931),
        DPoint::new(1.205684411948591, 0.81344322326274499),
    ]),
    QuadPts::new([
        DPoint::new(0.33333333333333326, 0.81481481481481488),
        DPoint::new(0.63396444791444551, 0.68743368362444768),
        DPoint::new(1.205732763658403, 0.81345617746834109),
    ]),
    QuadPts::new([
        DPoint::new(1.205684411948591, 0.81344322326274499),
        DPoint::new(1.2057085875611198, 0.81344969999329253),
        DPoint::new(1.205732763658403, 0.81345617746834109),
    ]),
    QuadPts::new([
        DPoint::new(1.20718795, 0.82163474),
        DPoint::new(1.15342035, 0.527908701),
        DPoint::new(1.088, 0.296),
    ]),
    QuadPts::new([
        DPoint::new(1.20568441, 0.813443223),
        DPoint::new(1.20570859, 0.8134497),
        DPoint::new(1.20573276, 0.813456177),
    ]),
    QuadPts::new([
        DPoint::new(41.5072916, 87.1234036),
        DPoint::new(28.2747836, 80.9545395),
        DPoint::new(23.5780771, 69.3344126),
    ]),
    QuadPts::new([
        DPoint::new(72.9633878, 95.6593007),
        DPoint::new(42.7738746, 88.4730382),
        DPoint::new(31.1932785, 80.2458029),
    ]),
    QuadPts::new([
        DPoint::new(31.1663962, 54.7302484),
        DPoint::new(31.1662882, 54.7301074),
        DPoint::new(31.1663969, 54.7302485),
    ]),
    QuadPts::new([
        DPoint::new(26.0404936, 45.4260361),
        DPoint::new(27.7887523, 33.1863051),
        DPoint::new(40.8833242, 26.0301855),
    ]),
    QuadPts::new([
        DPoint::new(29.9404074, 49.1672596),
        DPoint::new(44.3131071, 45.3915253),
        DPoint::new(58.1067559, 59.5061814),
    ]),
    QuadPts::new([
        DPoint::new(72.6510251, 64.2972928),
        DPoint::new(53.6989659, 60.1862397),
        DPoint::new(35.2053722, 44.8391126),
    ]),
    QuadPts::new([
        DPoint::new(52.14807018377202, 65.012420045148644),
        DPoint::new(44.778669050208237, 66.315562705604378),
        DPoint::new(51.619118408823567, 63.787827046262684),
    ]),
    QuadPts::new([
        DPoint::new(30.004993234763383, 93.921296668202288),
        DPoint::new(53.384822003076991, 60.732180341802753),
        DPoint::new(58.652998934338584, 43.111073088306185),
    ]),
    QuadPts::new([
        DPoint::new(80.897794748143198, 49.236332042718459),
        DPoint::new(81.082078218891212, 64.066749904488631),
        DPoint::new(69.972305057149981, 72.968595519850993),
    ]),
    QuadPts::new([
        DPoint::new(72.503745601281395, 32.952320736577882),
        DPoint::new(88.030880716061645, 38.137194847810164),
        DPoint::new(73.193774825517906, 67.773492479591397),
    ]),
    QuadPts::new([
        DPoint::new(67.426548091427676, 37.993772624988935),
        DPoint::new(51.129513170665035, 57.542281234563646),
        DPoint::new(44.594748190899189, 65.644267382683879),
    ]),
    QuadPts::new([
        DPoint::new(61.336508189019057, 82.693132843213675),
        DPoint::new(54.825078921449354, 71.663932799212432),
        DPoint::new(47.727444217558926, 61.4049645128392),
    ]),
    QuadPts::new([
        DPoint::new(67.4265481, 37.9937726),
        DPoint::new(51.1295132, 57.5422812),
        DPoint::new(44.5947482, 65.6442674),
    ]),
    QuadPts::new([
        DPoint::new(61.3365082, 82.6931328),
        DPoint::new(54.8250789, 71.6639328),
        DPoint::new(47.7274442, 61.4049645),
    ]),
    QuadPts::new([
        DPoint::new(53.774852327053594, 53.318060789841951),
        DPoint::new(45.787877803416805, 51.393492026284981),
        DPoint::new(46.703936967162392, 53.06860709822206),
    ]),
    QuadPts::new([
        DPoint::new(46.703936967162392, 53.06860709822206),
        DPoint::new(47.619996130907957, 54.74372217015916),
        DPoint::new(53.020051653535361, 48.633140968832024),
    ]),
    QuadPts::new([
        DPoint::new(50.934805397717923, 51.52391952648901),
        DPoint::new(56.803308902971423, 44.246234610627596),
        DPoint::new(69.776888596721406, 40.166645096692555),
    ]),
    QuadPts::new([
        DPoint::new(50.230212796400401, 38.386469101526998),
        DPoint::new(49.855620812184917, 38.818990392153609),
        DPoint::new(56.356567496227363, 47.229909093319407),
    ]),
    QuadPts::new([
        DPoint::new(36.148792695174222, 70.336952793070424),
        DPoint::new(36.141613037691357, 70.711654739870085),
        DPoint::new(36.154708826402597, 71.088492662905836),
    ]),
    QuadPts::new([
        DPoint::new(35.216235592661825, 70.580199617313212),
        DPoint::new(36.244476835123969, 71.010897787304074),
        DPoint::new(37.230244263238326, 71.423156953613102),
    ]),
    QuadPts::new([
        DPoint::new(369.848602, 145.680267),
        DPoint::new(382.360413, 121.298294),
        DPoint::new(406.207703, 121.298294),
    ]),
    QuadPts::new([
        DPoint::new(369.850525, 145.675964),
        DPoint::new(382.362915, 121.29287),
        DPoint::new(406.211273, 121.29287),
    ]),
    QuadPts::new([
        DPoint::new(33.567436351153468, 62.336347586395924),
        DPoint::new(35.200980274619084, 65.038561460144479),
        DPoint::new(36.479571811084995, 67.632178905412445),
    ]),
    QuadPts::new([
        DPoint::new(41.349524945572696, 67.886658677862641),
        DPoint::new(39.125562529359087, 67.429772735149214),
        DPoint::new(35.600314083992416, 66.705372160552685),
    ]),
    QuadPts::new([
        DPoint::new(67.25299631583178, 21.109080184767524),
        DPoint::new(43.617595267398613, 33.658034168577529),
        DPoint::new(33.38371819435676, 44.214192553988745),
    ]),
    QuadPts::new([
        DPoint::new(40.476838859398541, 39.543209911285999),
        DPoint::new(36.701186108431131, 34.8817994016458),
        DPoint::new(30.102144288878023, 26.739063172945315),
    ]),
    QuadPts::new([
        DPoint::new(25.367434474345036, 50.4712103169743),
        DPoint::new(17.865013304933097, 37.356741010559439),
        DPoint::new(16.818988838905465, 37.682915484123129),
    ]),
    QuadPts::new([
        DPoint::new(16.818988838905465, 37.682915484123129),
        DPoint::new(15.772964372877833, 38.009089957686811),
        DPoint::new(20.624104547604965, 41.825131596683121),
    ]),
    QuadPts::new([
        DPoint::new(26.440225044088567, 79.695009812848298),
        DPoint::new(26.085525979582247, 83.717928354134784),
        DPoint::new(27.075079976297072, 84.820633667838905),
    ]),
    QuadPts::new([
        DPoint::new(27.075079976297072, 84.820633667838905),
        DPoint::new(28.276546859574015, 85.988574184029034),
        DPoint::new(25.649263209500006, 87.166762066617025),
    ]),
    QuadPts::new([
        DPoint::new(34.879150914024962, 83.862726601601125),
        DPoint::new(35.095810134304429, 83.693473210169543),
        DPoint::new(35.359284111931586, 83.488069234177502),
    ]),
    QuadPts::new([
        DPoint::new(54.503204203015471, 76.094098492518242),
        DPoint::new(51.366889541918894, 71.609856061299155),
        DPoint::new(46.53086955445437, 69.949863036494207),
    ]),
    QuadPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, 3.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 1.0),
    ]),
    QuadPts::new([
        DPoint::new(369.961151, 137.980698),
        DPoint::new(383.970093, 121.298294),
        DPoint::new(406.213287, 121.298294),
    ]),
    QuadPts::new([
        DPoint::new(353.2948, 194.351074),
        DPoint::new(353.2948, 173.767563),
        DPoint::new(364.167572, 160.819855),
    ]),
    QuadPts::new([
        DPoint::new(360.416077, 166.795715),
        DPoint::new(370.126831, 147.872162),
        DPoint::new(388.635406, 147.872162),
    ]),
    QuadPts::new([
        DPoint::new(406.236359, 121.254936),
        DPoint::new(409.445679, 121.254936),
        DPoint::new(412.975952, 121.789818),
    ]),
    QuadPts::new([
        DPoint::new(406.235992, 121.254936),
        DPoint::new(425.705902, 121.254936),
        DPoint::new(439.71994, 137.087616),
    ]),
    QuadPts::new([
        DPoint::new(369.8543701171875, 145.66734313964844),
        DPoint::new(382.36788940429688, 121.28203582763672),
        DPoint::new(406.21844482421875, 121.28203582763672),
    ]),
    QuadPts::new([
        DPoint::new(369.96469116210938, 137.96672058105469),
        DPoint::new(383.97555541992188, 121.28203582763672),
        DPoint::new(406.2218017578125, 121.28203582763672),
    ]),
    QuadPts::new([
        DPoint::new(369.962311, 137.976044),
        DPoint::new(383.971893, 121.29287),
        DPoint::new(406.216125, 121.29287),
    ]),
    QuadPts::new([
        DPoint::new(400.121704, 149.468719),
        DPoint::new(391.949493, 161.037186),
        DPoint::new(391.949493, 181.202423),
    ]),
    QuadPts::new([
        DPoint::new(391.946747, 181.839218),
        DPoint::new(391.946747, 155.62442),
        DPoint::new(406.115479, 138.855438),
    ]),
    QuadPts::new([
        DPoint::new(360.048828125, 229.2578125),
        DPoint::new(360.048828125, 224.4140625),
        DPoint::new(362.607421875, 221.3671875),
    ]),
    QuadPts::new([
        DPoint::new(362.607421875, 221.3671875),
        DPoint::new(365.166015625, 218.3203125),
        DPoint::new(369.228515625, 218.3203125),
    ]),
    QuadPts::new([
        DPoint::new(8.0, 8.0),
        DPoint::new(10.0, 10.0),
        DPoint::new(8.0, -10.0),
    ]),
    QuadPts::new([
        DPoint::new(8.0, 8.0),
        DPoint::new(12.0, 12.0),
        DPoint::new(14.0, 4.0),
    ]),
    QuadPts::new([
        DPoint::new(8.0, 8.0),
        DPoint::new(9.0, 9.0),
        DPoint::new(10.0, 8.0),
    ]),
];

/// `testSetCount`.
// Port of: tests/PathOpsQuadIntersectionTest.cpp#L427 (chrome/m156)
const TEST_SET_COUNT: usize = TEST_SET.len();

/// `static const auto coincidentTestSet`.
// Port of: tests/PathOpsQuadIntersectionTest.cpp#L466-L479 (chrome/m156)
#[allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::approx_constant
)] // the table keeps the C++ literals verbatim
static COINCIDENT_TEST_SET: [QuadPts; 6] = [
    QuadPts::new([
        DPoint::new(4914.9990234375, 1523.0),
        DPoint::new(4942.75146484375, 1523.0),
        DPoint::new(4962.375, 1542.6239013671875),
    ]),
    QuadPts::new([
        DPoint::new(4962.3759765625, 1542.6239013671875),
        DPoint::new(4942.75244140625, 1523.0),
        DPoint::new(4915.0, 1523.0),
    ]),
    QuadPts::new([
        DPoint::new(369.850525, 145.675964),
        DPoint::new(382.362915, 121.29287),
        DPoint::new(406.211273, 121.29287),
    ]),
    QuadPts::new([
        DPoint::new(369.850525, 145.675964),
        DPoint::new(382.362915, 121.29287),
        DPoint::new(406.211273, 121.29287),
    ]),
    QuadPts::new([
        DPoint::new(8.0, 8.0),
        DPoint::new(10.0, 10.0),
        DPoint::new(8.0, -10.0),
    ]),
    QuadPts::new([
        DPoint::new(8.0, -10.0),
        DPoint::new(10.0, 10.0),
        DPoint::new(8.0, 8.0),
    ]),
];

/// Port of `standardTestCases`. The C++ reports the orders of the quads only under
/// `showSkipped`, through `SkDebugf`, which is not ported.
// Port of: tests/PathOpsQuadIntersectionTest.cpp#L19-L51 (chrome/m156)
fn standard_test_cases(reporter: &mut Reporter) {
    for test in &QUADRATICTESTS {
        let quad1 = DQuad::new(test[0].pts);
        debug_assert!(valid_quad(&quad1));
        let quad2 = DQuad::new(test[1].pts);
        debug_assert!(valid_quad(&quad2));
        let mut reduce1 = ReduceOrder::default();
        let mut reduce2 = ReduceOrder::default();
        let order1 = reduce1.reduce_quad(&quad1);
        let order2 = reduce2.reduce_quad(&quad2);
        if order1 == 3 && order2 == 3 {
            let mut intersections = Intersections::default();
            intersections.intersect_quad_quad(&quad1, &quad2);
            for pt in 0..intersections.used() {
                let tt1 = intersections.t(0, pt);
                let xy1 = quad1.pt_at_t(tt1);
                let tt2 = intersections.t(1, pt);
                let xy2 = quad2.pt_at_t(tt2);
                if !xy1.approximately_equal(xy2) {
                    // SkDebugf("%s [%d,%d] x!= ...") is not ported: the failure is the assert.
                    reporter_assert!(reporter, false);
                }
            }
        }
    }
}

/// Port of `oneOffTest1`.
// Port of: tests/PathOpsQuadIntersectionTest.cpp#L429-L456 (chrome/m156)
fn one_off_test1(reporter: &mut Reporter, outer: usize, inner: usize) {
    let quad1 = DQuad::new(TEST_SET[outer].pts);
    debug_assert!(valid_quad(&quad1));
    let quad2 = DQuad::new(TEST_SET[inner].pts);
    debug_assert!(valid_quad(&quad2));
    let mut intersections = Intersections::default();
    intersections.intersect_quad_quad(&quad1, &quad2);
    for pt in 0..intersections.used() {
        let tt1 = intersections.t(0, pt);
        let xy1 = quad1.pt_at_t(tt1);
        let tt2 = intersections.t(1, pt);
        let xy2 = quad2.pt_at_t(tt2);
        if !xy1.approximately_equal(xy2) {
            reporter_assert!(reporter, false);
        }
    }
}

/// Port of `oneOffTests`.
// Port of: tests/PathOpsQuadIntersectionTest.cpp#L458-L464 (chrome/m156)
fn one_off_tests(reporter: &mut Reporter) {
    for outer in 0..TEST_SET_COUNT - 1 {
        for inner in outer + 1..TEST_SET_COUNT {
            one_off_test1(reporter, outer, inner);
        }
    }
}

/// Port of `coincidentTestOne`.
// Port of: tests/PathOpsQuadIntersectionTest.cpp#L483-L503 (chrome/m156)
fn coincident_test_one(reporter: &mut Reporter, test1: usize, test2: usize) {
    let quad1 = DQuad::new(COINCIDENT_TEST_SET[test1].pts);
    debug_assert!(valid_quad(&quad1));
    let quad2 = DQuad::new(COINCIDENT_TEST_SET[test2].pts);
    debug_assert!(valid_quad(&quad2));
    let mut intersections2 = Intersections::default();
    intersections2.intersect_quad_quad(&quad1, &quad2);
    reporter_assert!(reporter, intersections2.debug_coincident_used() >= 2);
    reporter_assert!(reporter, intersections2.used() >= 2);
    let limit = intersections2.debug_coincident_used();
    let mut pt = 0;
    while pt < limit {
        let index = usize::try_from(pt).expect("coincident index is non-negative");
        let tt1 = intersections2.t(0, index);
        let tt2 = intersections2.t(1, index);
        let pt1 = quad1.pt_at_t(tt1);
        let pt2 = quad2.pt_at_t(tt2);
        reporter_assert!(reporter, pt1.approximately_equal(pt2));
        pt += 2;
    }
}

/// Port of `coincidentTest`.
// Port of: tests/PathOpsQuadIntersectionTest.cpp#L505-L509 (chrome/m156)
fn coincident_test(reporter: &mut Reporter) {
    let count = COINCIDENT_TEST_SET.len();
    let mut test_index = 0;
    while test_index + 1 < count {
        coincident_test_one(reporter, test_index, test_index + 1);
        test_index += 2;
    }
}

// `QuadraticIntersection_IntersectionFinder` (tests/PathOpsQuadIntersectionTest.cpp#L511-L609) is
// reached only from `if ((false))` in `PathOpsQuadIntersection`, and its body only prints with
// `SkDebugf`, so it is not ported.

/// Port of the body of `PathOpsQuadBinaryProfile`: run only with `gVeryVerbose`.
// Port of: tests/PathOpsQuadIntersectionTest.cpp#L626-L658 (chrome/m156)
// mirrors REPORTER_ASSERT(used() >= 0), an int comparison that always holds
#[allow(unused_comparisons, clippy::absurd_extreme_comparisons)]
fn binary_profile(reporter: &mut Reporter) {
    let mut intersections = Intersections::default();
    for _x in 0..100 {
        let mut outer = 0;
        let mut inner = outer + 1;
        loop {
            let quad1 = DQuad::new(TEST_SET[outer].pts);
            let quad2 = DQuad::new(TEST_SET[inner].pts);
            intersections.intersect_quad_quad(&quad1, &quad2);
            reporter_assert!(reporter, intersections.used() >= 0); // make sure code isn't tossed
            inner += 2;
            outer += 2;
            if outer >= TEST_SET_COUNT {
                break;
            }
        }
    }
    for _x in 0..100 {
        for test in &QUADRATICTESTS {
            let quad1 = DQuad::new(test[0].pts);
            let quad2 = DQuad::new(test[1].pts);
            intersections.intersect_quad_quad(&quad1, &quad2);
            reporter_assert!(reporter, intersections.used() >= 0); // make sure code isn't tossed
        }
    }
}

def_test!(PathOpsQuadIntersectionOneOff, |reporter| {
    one_off_test1(reporter, 0, 1);
});

def_test!(PathOpsQuadIntersectionCoincidenceOneOff, |reporter| {
    coincident_test_one(reporter, 0, 1);
});

def_test!(PathOpsQuadIntersection, |reporter| {
    one_off_tests(reporter);
    coincident_test(reporter);
    standard_test_cases(reporter);
});

def_test!(PathOpsQuadBinaryProfile, |reporter| {
    if VERY_VERBOSE {
        binary_profile(reporter);
    }
});
