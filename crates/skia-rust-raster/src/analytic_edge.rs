// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkAnalyticEdge.h, src/core/SkAnalyticEdge.cpp

//! Edges for the analytic antialiasing scan converter (`SkAnalyticEdge.h`).
//!
//! Unlike [`Edge`](crate::edge::Edge), an analytic edge keeps its y range in 16.16 fixed point
//! (snapped to quarter pixels), so the scan converter can compute exact trapezoid coverage
//! between arbitrary scan lines. Curves are walked as a sequence of line segments by forward
//! differencing. All arithmetic is fixed point and mirrors Skia exactly, including its reliance on
//! two's complement wrapping (`wrapping_*` here).
//!
//! `SK_RASTERIZE_EVEN_ROUNDING` is not defined in Skia's builds, so the `SkScalarToFDot6` /
//! `int(x * scale)` branch of each `set*` function is the one ported.

use std::ops::{Deref, DerefMut};

use skia_rust_core::fdot6::{
    Fdot6, fdot6_div, fdot6_round, fdot6_to_fixed, fixed_to_fdot6, float_to_fdot6,
};
use skia_rust_core::fixed::{FIXED_1, Fixed, fixed_mul, fixed_round_to_fixed};
use skia_rust_core::math::{MAX_S32, left_shift};
use skia_rust_core::point::Point;
use skia_rust_core::safe32::abs32;

pub use crate::edge::{EdgeType, Winding};

pub use crate::scan_priv::NIL;
use crate::scan_priv::{LinkedEdge, float_to_int};

// Port of: src/core/SkAnalyticEdge.cpp#L21 (chrome/m156)
const INVERSE_TABLE_SIZE: i32 = 1024; // SK_FDot6One * 16

// Port of: src/core/SkAnalyticEdge.cpp#L24-L123 (chrome/m156)
#[rustfmt::skip]
#[allow(clippy::unreadable_literal)] // copied verbatim from the C++ table
static INVERSE_TABLE: [i32; 1025] = [
    -4096, -4100, -4104, -4108, -4112, -4116, -4120, -4124, -4128, -4132,
    -4136, -4140, -4144, -4148, -4152, -4156, -4161, -4165, -4169, -4173,
    -4177, -4181, -4185, -4190, -4194, -4198, -4202, -4206, -4211, -4215,
    -4219, -4223, -4228, -4232, -4236, -4240, -4245, -4249, -4253, -4258,
    -4262, -4266, -4271, -4275, -4279, -4284, -4288, -4293, -4297, -4301,
    -4306, -4310, -4315, -4319, -4324, -4328, -4332, -4337, -4341, -4346,
    -4350, -4355, -4359, -4364, -4369, -4373, -4378, -4382, -4387, -4391,
    -4396, -4401, -4405, -4410, -4415, -4419, -4424, -4429, -4433, -4438,
    -4443, -4447, -4452, -4457, -4462, -4466, -4471, -4476, -4481, -4485,
    -4490, -4495, -4500, -4505, -4510, -4514, -4519, -4524, -4529, -4534,
    -4539, -4544, -4549, -4554, -4559, -4563, -4568, -4573, -4578, -4583,
    -4588, -4593, -4599, -4604, -4609, -4614, -4619, -4624, -4629, -4634,
    -4639, -4644, -4650, -4655, -4660, -4665, -4670, -4675, -4681, -4686,
    -4691, -4696, -4702, -4707, -4712, -4718, -4723, -4728, -4733, -4739,
    -4744, -4750, -4755, -4760, -4766, -4771, -4777, -4782, -4788, -4793,
    -4798, -4804, -4809, -4815, -4821, -4826, -4832, -4837, -4843, -4848,
    -4854, -4860, -4865, -4871, -4877, -4882, -4888, -4894, -4899, -4905,
    -4911, -4917, -4922, -4928, -4934, -4940, -4946, -4951, -4957, -4963,
    -4969, -4975, -4981, -4987, -4993, -4999, -5005, -5011, -5017, -5023,
    -5029, -5035, -5041, -5047, -5053, -5059, -5065, -5071, -5077, -5084,
    -5090, -5096, -5102, -5108, -5115, -5121, -5127, -5133, -5140, -5146,
    -5152, -5159, -5165, -5171, -5178, -5184, -5190, -5197, -5203, -5210,
    -5216, -5223, -5229, -5236, -5242, -5249, -5256, -5262, -5269, -5275,
    -5282, -5289, -5295, -5302, -5309, -5315, -5322, -5329, -5336, -5343,
    -5349, -5356, -5363, -5370, -5377, -5384, -5391, -5398, -5405, -5412,
    -5418, -5426, -5433, -5440, -5447, -5454, -5461, -5468, -5475, -5482,
    -5489, -5497, -5504, -5511, -5518, -5526, -5533, -5540, -5548, -5555,
    -5562, -5570, -5577, -5584, -5592, -5599, -5607, -5614, -5622, -5629,
    -5637, -5645, -5652, -5660, -5667, -5675, -5683, -5691, -5698, -5706,
    -5714, -5722, -5729, -5737, -5745, -5753, -5761, -5769, -5777, -5785,
    -5793, -5801, -5809, -5817, -5825, -5833, -5841, -5849, -5857, -5866,
    -5874, -5882, -5890, -5899, -5907, -5915, -5924, -5932, -5940, -5949,
    -5957, -5966, -5974, -5983, -5991, -6000, -6009, -6017, -6026, -6034,
    -6043, -6052, -6061, -6069, -6078, -6087, -6096, -6105, -6114, -6123,
    -6132, -6141, -6150, -6159, -6168, -6177, -6186, -6195, -6204, -6213,
    -6223, -6232, -6241, -6250, -6260, -6269, -6278, -6288, -6297, -6307,
    -6316, -6326, -6335, -6345, -6355, -6364, -6374, -6384, -6393, -6403,
    -6413, -6423, -6432, -6442, -6452, -6462, -6472, -6482, -6492, -6502,
    -6512, -6523, -6533, -6543, -6553, -6563, -6574, -6584, -6594, -6605,
    -6615, -6626, -6636, -6647, -6657, -6668, -6678, -6689, -6700, -6710,
    -6721, -6732, -6743, -6754, -6765, -6775, -6786, -6797, -6808, -6820,
    -6831, -6842, -6853, -6864, -6875, -6887, -6898, -6909, -6921, -6932,
    -6944, -6955, -6967, -6978, -6990, -7002, -7013, -7025, -7037, -7049,
    -7061, -7073, -7084, -7096, -7108, -7121, -7133, -7145, -7157, -7169,
    -7182, -7194, -7206, -7219, -7231, -7244, -7256, -7269, -7281, -7294,
    -7307, -7319, -7332, -7345, -7358, -7371, -7384, -7397, -7410, -7423,
    -7436, -7449, -7463, -7476, -7489, -7503, -7516, -7530, -7543, -7557,
    -7570, -7584, -7598, -7612, -7626, -7639, -7653, -7667, -7681, -7695,
    -7710, -7724, -7738, -7752, -7767, -7781, -7796, -7810, -7825, -7839,
    -7854, -7869, -7884, -7898, -7913, -7928, -7943, -7958, -7973, -7989,
    -8004, -8019, -8035, -8050, -8065, -8081, -8097, -8112, -8128, -8144,
    -8160, -8176, -8192, -8208, -8224, -8240, -8256, -8272, -8289, -8305,
    -8322, -8338, -8355, -8371, -8388, -8405, -8422, -8439, -8456, -8473,
    -8490, -8507, -8525, -8542, -8559, -8577, -8594, -8612, -8630, -8648,
    -8665, -8683, -8701, -8719, -8738, -8756, -8774, -8793, -8811, -8830,
    -8848, -8867, -8886, -8905, -8924, -8943, -8962, -8981, -9000, -9020,
    -9039, -9058, -9078, -9098, -9118, -9137, -9157, -9177, -9198, -9218,
    -9238, -9258, -9279, -9300, -9320, -9341, -9362, -9383, -9404, -9425,
    -9446, -9467, -9489, -9510, -9532, -9554, -9576, -9597, -9619, -9642,
    -9664, -9686, -9709, -9731, -9754, -9776, -9799, -9822, -9845, -9868,
    -9892, -9915, -9939, -9962, -9986, -10010, -10034, -10058, -10082, -10106,
    -10131, -10155, -10180, -10205, -10230, -10255, -10280, -10305, -10330, -10356,
    -10381, -10407, -10433, -10459, -10485, -10512, -10538, -10564, -10591, -10618,
    -10645, -10672, -10699, -10727, -10754, -10782, -10810, -10837, -10866, -10894,
    -10922, -10951, -10979, -11008, -11037, -11066, -11096, -11125, -11155, -11184,
    -11214, -11244, -11275, -11305, -11335, -11366, -11397, -11428, -11459, -11491,
    -11522, -11554, -11586, -11618, -11650, -11683, -11715, -11748, -11781, -11814,
    -11848, -11881, -11915, -11949, -11983, -12018, -12052, -12087, -12122, -12157,
    -12192, -12228, -12264, -12300, -12336, -12372, -12409, -12446, -12483, -12520,
    -12557, -12595, -12633, -12671, -12710, -12748, -12787, -12826, -12865, -12905,
    -12945, -12985, -13025, -13066, -13107, -13148, -13189, -13231, -13273, -13315,
    -13357, -13400, -13443, -13486, -13530, -13573, -13617, -13662, -13706, -13751,
    -13797, -13842, -13888, -13934, -13981, -14027, -14074, -14122, -14169, -14217,
    -14266, -14315, -14364, -14413, -14463, -14513, -14563, -14614, -14665, -14716,
    -14768, -14820, -14873, -14926, -14979, -15033, -15087, -15141, -15196, -15252,
    -15307, -15363, -15420, -15477, -15534, -15592, -15650, -15709, -15768, -15827,
    -15887, -15947, -16008, -16070, -16131, -16194, -16256, -16320, -16384, -16448,
    -16513, -16578, -16644, -16710, -16777, -16844, -16912, -16980, -17050, -17119,
    -17189, -17260, -17331, -17403, -17476, -17549, -17623, -17697, -17772, -17848,
    -17924, -18001, -18078, -18157, -18236, -18315, -18396, -18477, -18558, -18641,
    -18724, -18808, -18893, -18978, -19065, -19152, -19239, -19328, -19418, -19508,
    -19599, -19691, -19784, -19878, -19972, -20068, -20164, -20262, -20360, -20460,
    -20560, -20661, -20763, -20867, -20971, -21076, -21183, -21290, -21399, -21509,
    -21620, -21732, -21845, -21959, -22075, -22192, -22310, -22429, -22550, -22671,
    -22795, -22919, -23045, -23172, -23301, -23431, -23563, -23696, -23831, -23967,
    -24105, -24244, -24385, -24528, -24672, -24818, -24966, -25115, -25266, -25420,
    -25575, -25731, -25890, -26051, -26214, -26379, -26546, -26715, -26886, -27060,
    -27235, -27413, -27594, -27776, -27962, -28149, -28339, -28532, -28728, -28926,
    -29127, -29330, -29537, -29746, -29959, -30174, -30393, -30615, -30840, -31068,
    -31300, -31536, -31775, -32017, -32263, -32513, -32768, -33026, -33288, -33554,
    -33825, -34100, -34379, -34663, -34952, -35246, -35544, -35848, -36157, -36472,
    -36792, -37117, -37449, -37786, -38130, -38479, -38836, -39199, -39568, -39945,
    -40329, -40721, -41120, -41527, -41943, -42366, -42799, -43240, -43690, -44150,
    -44620, -45100, -45590, -46091, -46603, -47127, -47662, -48210, -48770, -49344,
    -49932, -50533, -51150, -51781, -52428, -53092, -53773, -54471, -55188, -55924,
    -56679, -57456, -58254, -59074, -59918, -60787, -61680, -62601, -63550, -64527,
    -65536, -66576, -67650, -68759, -69905, -71089, -72315, -73584, -74898, -76260,
    -77672, -79137, -80659, -82241, -83886, -85598, -87381, -89240, -91180, -93206,
    -95325, -97541, -99864, -102300, -104857, -107546, -110376, -113359, -116508, -119837,
    -123361, -127100, -131072, -135300, -139810, -144631, -149796, -155344, -161319, -167772,
    -174762, -182361, -190650, -199728, -209715, -220752, -233016, -246723, -262144, -279620,
    -299593, -322638, -349525, -381300, -419430, -466033, -524288, -599186, -699050, -838860,
    -1048576, -1398101, -2097152, -4194304, 0,
];

// Port of: src/core/SkAnalyticEdge.cpp#L23-L134 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // |x| <= kLastEntry, so the indices are in range
fn quick_inverse(x: Fdot6) -> Fixed {
    const LAST_ENTRY: i32 = 1024;
    const _: () = assert!(LAST_ENTRY == INVERSE_TABLE_SIZE);
    debug_assert!(abs32(x) <= LAST_ENTRY);

    if x > 0 {
        INVERSE_TABLE[(LAST_ENTRY - x) as usize].wrapping_neg()
    } else {
        INVERSE_TABLE[(LAST_ENTRY + x) as usize]
    }
}

// Port of: src/core/SkAnalyticEdge.cpp#L136-L155 (chrome/m156)
#[allow(clippy::manual_range_contains)] // mirrors the C++ condition
fn quick_div(a: Fdot6, b: Fdot6) -> Fixed {
    const MIN_BITS: i32 = 3; // abs(b) should be at least (1 << kMinBits) for quick division
    const MAX_BITS: i32 = 31; // Number of bits available in signed int
    // Given abs(b) <= (1 << kMinBits), the inverse of abs(b) is at most 1 << (22 - kMinBits) in
    // SkFixed format. Hence abs(a) should be less than kMaxAbsA
    const MAX_ABS_A: i32 = 1 << (MAX_BITS - (22 - MIN_BITS));
    let abs_a = abs32(a);
    let abs_b = abs32(b);
    if abs_b >= (1 << MIN_BITS) && abs_b < INVERSE_TABLE_SIZE && abs_a < MAX_ABS_A {
        return a.wrapping_mul(quick_inverse(b)) >> 6;
    }
    fdot6_div(a, b)
}

/// An edge of the analytic AA scan converter: the line segment from `(upper_x, upper_y)` to the
/// x at `lower_y`, with slope `dx` (run over rise).
///
/// skia-rust: `fNext`/`fPrev` are indices into the array that holds the edges (the C++ links raw
/// pointers), with [`NIL`] for null.
// Port of: src/core/SkAnalyticEdge.h#L20-L108 (chrome/m156)
#[doc(alias = "SkAnalyticEdge")]
#[derive(Copy, Clone, Debug)]
pub struct AnalyticEdge {
    /// `fNext`.
    pub next: usize,
    /// `fPrev`.
    pub prev: usize,
    /// `fX`.
    pub x: Fixed,
    /// `fDX`.
    pub dx: Fixed,
    /// `fUpperX`: the x value when y = `upper_y`.
    pub upper_x: Fixed,
    /// `fY`: the current y.
    pub y: Fixed,
    /// `fUpperY`: the upper bound of y (our edge is from y = `upper_y` to y = `lower_y`).
    pub upper_y: Fixed,
    /// `fLowerY`: the lower bound of y.
    pub lower_y: Fixed,
    /// `fDY`: `abs(1/dx)`; may be `SK_MaxS32` when `dx` is close to 0. Only used for blitting
    /// trapezoids.
    pub dy: Fixed,
    /// `fEdgeType`: remembers the *initial* edge type.
    pub edge_type: EdgeType,
    /// `fCurveCount`: only used by quads (+) and cubics (-).
    pub curve_count: i8,
    /// `fCurveShift`: the parametric subdivision shift (stepExponent) used to scale derivatives
    /// during forward-differencing. In quadratics, it scales the first derivative during position
    /// updates. In cubics, it scales the second derivative, while the first derivative uses
    /// `to_fixed_shift` instead.
    pub curve_shift: u8,
    /// `fWinding`.
    pub winding: Winding,
}

impl Default for AnalyticEdge {
    fn default() -> Self {
        AnalyticEdge {
            next: NIL,
            prev: NIL,
            x: 0,
            dx: 0,
            upper_x: 0,
            y: 0,
            upper_y: 0,
            lower_y: 0,
            dy: 0,
            edge_type: EdgeType::Line,
            curve_count: 0,
            curve_shift: 0,
            winding: Winding::CW,
        }
    }
}

/// `SkAnalyticEdge::kDefaultAccuracy`: the default accuracy for snapping (quarter pixels).
// Port of: src/core/SkAnalyticEdge.h#L54 (chrome/m156)
pub const DEFAULT_ACCURACY: i32 = 2;

/// `SkAnalyticEdge::SnapY`: rounds `y` to a multiple of `1 / (1 << DEFAULT_ACCURACY)`.
// Port of: src/core/SkAnalyticEdge.h#L56-L60 (chrome/m156)
#[doc(alias = "SnapY")]
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // mirrors the (unsigned) arithmetic
#[must_use]
pub fn snap_y(y: Fixed) -> Fixed {
    const ACCURACY: i32 = DEFAULT_ACCURACY;
    // This approach is safer than left shift, round, then right shift
    (((y as u32).wrapping_add((FIXED_1 >> (ACCURACY + 1)) as u32) >> (16 - ACCURACY))
        << (16 - ACCURACY)) as Fixed
}

// Port of: src/core/SkAnalyticEdge.cpp#L209-L211 (chrome/m156)
fn swap_winding(w: Winding) -> Winding {
    match w {
        Winding::CW => Winding::CCW,
        Winding::CCW => Winding::CW,
    }
}

impl AnalyticEdge {
    /// Updates `x`, `y` of this edge so `y` is the given value.
    // Port of: src/core/SkAnalyticEdge.h#L62-L73 (chrome/m156)
    #[doc(alias = "goY")]
    pub fn go_y(&mut self, y: Fixed) {
        if y == self.y.wrapping_add(FIXED_1) {
            self.x = self.x.wrapping_add(self.dx);
            self.y = y;
        } else if y != self.y {
            // Drop lower digits as our alpha only has 8 bits
            // (fDX and y - fUpperY may be greater than SK_Fixed1)
            self.x = self
                .upper_x
                .wrapping_add(fixed_mul(self.dx, y.wrapping_sub(self.upper_y)));
            self.y = y;
        }
    }

    /// Steps to `y`, which is `1 >> y_shift` below the current `y`.
    // Port of: src/core/SkAnalyticEdge.h#L75-L80 (chrome/m156)
    #[doc(alias = "goY")]
    pub fn go_y_shift(&mut self, y: Fixed, y_shift: i32) {
        debug_assert!((0..=DEFAULT_ACCURACY).contains(&y_shift));
        debug_assert!(self.dx == 0 || y.wrapping_sub(self.y) == FIXED_1 >> y_shift);
        self.y = y;
        self.x = self.x.wrapping_add(self.dx >> y_shift);
    }

    /// Represents the straight line `p0`-`p1`. Returns false if it has height 0 (after snapping).
    // Port of: src/core/SkAnalyticEdge.cpp#L157-L207 (chrome/m156)
    #[doc(alias = "setLine")]
    pub fn set_line(&mut self, p0: Point, p1: Point) -> bool {
        // We must set X/Y using the same way (e.g., times 4, to FDot6, then to Fixed) as
        // Quads/Cubics. Otherwise the order of the edge might be wrong due to precision limit.
        const ACCURACY: i32 = DEFAULT_ACCURACY;
        #[allow(clippy::cast_precision_loss)] // 4 is exact in a float
        const MULTIPLIER: f32 = (1 << DEFAULT_ACCURACY) as f32;
        let mut x0 = fdot6_to_fixed(float_to_fdot6(p0.x * MULTIPLIER)) >> ACCURACY;
        let mut y0 = snap_y(fdot6_to_fixed(float_to_fdot6(p0.y * MULTIPLIER)) >> ACCURACY);
        let mut x1 = fdot6_to_fixed(float_to_fdot6(p1.x * MULTIPLIER)) >> ACCURACY;
        let mut y1 = snap_y(fdot6_to_fixed(float_to_fdot6(p1.y * MULTIPLIER)) >> ACCURACY);

        let mut winding = Winding::CW;

        if y0 > y1 {
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
            winding = Winding::CCW;
        }

        // are we a zero-height line?
        let dy = fixed_to_fdot6(y1.wrapping_sub(y0));
        if dy == 0 {
            return false;
        }
        let dx = fixed_to_fdot6(x1.wrapping_sub(x0));
        let slope = quick_div(dx, dy);
        let abs_slope = abs32(slope);

        self.x = x0;
        self.dx = slope;
        self.upper_x = x0;
        self.y = y0;
        self.upper_y = y0;
        self.lower_y = y1;
        self.dy = if dx == 0 || slope == 0 {
            MAX_S32
        } else if abs_slope < INVERSE_TABLE_SIZE {
            quick_inverse(abs_slope)
        } else {
            abs32(quick_div(dy, dx))
        };
        self.edge_type = EdgeType::Line;
        self.curve_count = 0;
        self.winding = winding;
        self.curve_shift = 0;

        true
    }

    // This will become a bottleneck for small ovals rendering if we call SkFixedDiv twice here.
    // Therefore, we'll let the outter function compute the slope once and send in the value.
    // Moreover, we'll compute fDY by quickly lookup the inverse table (if possible).
    // Port of: src/core/SkAnalyticEdge.cpp#L213-L258 (chrome/m156)
    #[doc(alias = "updateLine")]
    fn update_line(
        &mut self,
        mut x0: Fixed,
        mut y0: Fixed,
        mut x1: Fixed,
        mut y1: Fixed,
        slope: Fixed,
    ) -> bool {
        // Since we send in the slope, we can no longer snap y inside this function.
        // If we don't send in the slope, or we do some more sophisticated snapping, this function
        // could be a performance bottleneck.
        debug_assert!(self.curve_count != 0);

        // We don't chop at y extrema for cubics so the y is not guaranteed to be increasing for
        // them. In that case, we have to swap x/y and negate the winding.
        if y0 > y1 {
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
            self.winding = swap_winding(self.winding);
        }

        debug_assert!(y0 <= y1);

        let dx = fixed_to_fdot6(x1.wrapping_sub(x0));
        let dy = fixed_to_fdot6(y1.wrapping_sub(y0));

        // are we a zero-height line?
        if dy == 0 {
            return false;
        }

        debug_assert!(slope < MAX_S32);

        let abs_slope = abs32(fixed_to_fdot6(slope));
        self.x = x0;
        self.dx = slope;
        self.upper_x = x0;
        self.y = y0;
        self.upper_y = y0;
        self.lower_y = y1;
        self.dy = if dx == 0 || slope == 0 {
            MAX_S32
        } else if abs_slope < INVERSE_TABLE_SIZE {
            quick_inverse(abs_slope)
        } else {
            abs32(quick_div(dy, dx))
        };

        true
    }
}

/// We store `1 << shift` in a (signed) byte, so its maximum value is `1 << 6 == 64`. This limits
/// the number of lines we use to approximate a curve.
// Port of: src/core/SkAnalyticEdge.cpp#L270-L275 (chrome/m156)
const MAX_COEFF_SHIFT: i32 = 6;

// Port of: src/core/SkAnalyticEdge.cpp#L277-L288 (chrome/m156)
fn cheap_distance(dx: Fdot6, dy: Fdot6) -> Fdot6 {
    let mut dx = abs32(dx);
    let dy = abs32(dy);
    // return max + min/2
    if dx > dy {
        dx = dx.wrapping_add(dy >> 1);
    } else {
        dx = dy.wrapping_add(dx >> 1);
    }
    dx
}

// Port of: src/core/SkAnalyticEdge.cpp#L290-L304 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // dist is never negative (a sum of absolute values)
#[allow(clippy::cast_possible_wrap)] // 32 - clz is at most 32
fn diff_to_shift(dx: Fdot6, dy: Fdot6, shift_aa: i32) -> i32 {
    // cheap calc of distance from center of p0-p2 to the center of the curve
    let mut dist = cheap_distance(dx, dy);

    // shift down dist (it is currently in dot6)
    // down by 3 should give us 1/8 pixel accuracy (assuming our dist is accurate...)
    // this is chosen by heuristic: make it as big as possible (to minimize segments)
    // ... but small enough so that our curves still look smooth
    // When shift > 0, we're using AA and everything is scaled up so we can
    // lower the accuracy.
    dist = dist.wrapping_add(1 << (2 + shift_aa)) >> (3 + shift_aa);

    // each subdivision (shift value) cuts this dist (error) by 1/4
    (32 - (dist as u32).leading_zeros() as i32) >> 1
}

// Port of: src/core/SkAnalyticEdge.cpp#L318-L322 (chrome/m156)
fn fdot6_to_fixed_div2(value: Fdot6) -> Fixed {
    // we want to return SkFDot6ToFixed(value >> 1), but we don't want to throw
    // away data in value, so just perform a modify up-shift
    left_shift(value, 16 - 6 - 1)
}

/// A quadratic edge: walks a (monotonic in y) quad as up to 64 line segments.
// Port of: src/core/SkAnalyticEdge.h#L110-L130 (chrome/m156)
#[doc(alias = "SkAnalyticQuadraticEdge")]
#[derive(Copy, Clone, Debug, Default)]
pub struct AnalyticQuadraticEdge {
    base: AnalyticEdge,
    pub(crate) qx: Fixed,
    pub(crate) qy: Fixed,
    pub(crate) qdx: Fixed,
    pub(crate) qdy: Fixed,
    pub(crate) qddx: Fixed,
    pub(crate) qddy: Fixed,
    pub(crate) q_last_x: Fixed,
    pub(crate) q_last_y: Fixed,
    // snap y to integer points in the middle of the curve to accelerate AAA path filling
    pub(crate) snapped_x: Fixed,
    pub(crate) snapped_y: Fixed,
}

impl Deref for AnalyticQuadraticEdge {
    type Target = AnalyticEdge;
    fn deref(&self) -> &AnalyticEdge {
        &self.base
    }
}

impl DerefMut for AnalyticQuadraticEdge {
    fn deref_mut(&mut self) -> &mut AnalyticEdge {
        &mut self.base
    }
}

impl AnalyticQuadraticEdge {
    // Port of: src/core/SkAnalyticEdge.cpp#L324-L424 (chrome/m156)
    #[allow(clippy::many_single_char_names)] // mirrors the C++ names
    #[allow(clippy::cast_possible_truncation)] // mirrors SkToS8/SkToU8 (values are small)
    #[allow(clippy::cast_sign_loss)] // mirrors SkToU8 (shift >= 1)
    #[allow(clippy::cast_precision_loss)] // 1 << (shift + 6) is exact in a float
    fn set_quadratic_without_update(&mut self, pts: &[Point], shift: i32) -> bool {
        let mut shift = shift;
        let scale = (1 << (shift + 6)) as f32;
        let mut x0 = float_to_int(pts[0].x * scale);
        let mut y0 = float_to_int(pts[0].y * scale);
        let x1 = float_to_int(pts[1].x * scale);
        let y1 = float_to_int(pts[1].y * scale);
        let mut x2 = float_to_int(pts[2].x * scale);
        let mut y2 = float_to_int(pts[2].y * scale);

        let mut winding = Winding::CW;
        if y0 > y2 {
            std::mem::swap(&mut x0, &mut x2);
            std::mem::swap(&mut y0, &mut y2);
            winding = Winding::CCW;
        }
        debug_assert!(y0 <= y1 && y1 <= y2);

        let top = fdot6_round(y0);
        let bot = fdot6_round(y2);

        // are we a zero-height quad (line)?
        if top == bot {
            return false;
        }

        // compute number of steps needed (1 << shift)
        {
            let dx = left_shift(x1, 1).wrapping_sub(x0).wrapping_sub(x2) >> 2;
            let dy = left_shift(y1, 1).wrapping_sub(y0).wrapping_sub(y2) >> 2;
            // This is a little confusing:
            // before this line, shift is the scale up factor for AA;
            // after this line, shift is the fCurveShift.
            shift = diff_to_shift(dx, dy, shift);
            debug_assert!(shift >= 0);
        }
        // need at least 1 subdivision for our bias trick
        if shift == 0 {
            shift = 1;
        } else if shift > MAX_COEFF_SHIFT {
            shift = MAX_COEFF_SHIFT;
        }

        self.base.winding = winding;
        //fToFixedShift only set for cubics
        self.base.edge_type = EdgeType::Quad;
        self.base.curve_count = (1 << shift) as i8;

        // We want to reformulate into polynomial form, to make it clear how we should
        // forward-difference.
        //
        // p0 (1 - t)^2 + p1 t(1 - t) + p2 t^2 ==> At^2 + Bt + C
        //
        // A = p0 - 2p1 + p2
        // B = 2(p1 - p0)
        // C = p0
        //
        // Our caller must have constrained our inputs (p0..p2) to all fit into 16.16. However, as
        // seen above, we sometimes compute values that can be larger (e.g. B = 2*(p1 - p0)). To
        // guard against overflow, we will store A and B at 1/2 of their actual value, and just
        // apply a 2x scale during application in updateQuadratic(). Hence we store (shift - 1) in
        // fCurveShift.

        self.base.curve_shift = (shift - 1) as u8;

        let mut a = fdot6_to_fixed_div2(x0.wrapping_sub(x1).wrapping_sub(x1).wrapping_add(x2)); // 1/2 the real value
        let mut b = fdot6_to_fixed(x1.wrapping_sub(x0)); // 1/2 the real value

        self.qx = fdot6_to_fixed(x0);
        self.qdx = b.wrapping_add(a >> shift); // biased by shift
        self.qddx = a >> (shift - 1); // biased by shift

        a = fdot6_to_fixed_div2(y0.wrapping_sub(y1).wrapping_sub(y1).wrapping_add(y2)); // 1/2 the real value
        b = fdot6_to_fixed(y1.wrapping_sub(y0)); // 1/2 the real value

        self.qy = fdot6_to_fixed(y0);
        self.qdy = b.wrapping_add(a >> shift); // biased by shift
        self.qddy = a >> (shift - 1); // biased by shift

        self.q_last_x = fdot6_to_fixed(x2);
        self.q_last_y = fdot6_to_fixed(y2);

        true
    }

    /// Sets up the line segments of the (monotonic in y) quad `pts[0..3]`. Returns false if the
    /// quad has height 0.
    // Port of: src/core/SkAnalyticEdge.cpp#L426-L447 (chrome/m156)
    #[doc(alias = "setQuadratic")]
    pub fn set_quadratic(&mut self, pts: &[Point]) -> bool {
        if !self.set_quadratic_without_update(pts, DEFAULT_ACCURACY) {
            return false;
        }
        self.qx >>= DEFAULT_ACCURACY;
        self.qy >>= DEFAULT_ACCURACY;
        self.qdx >>= DEFAULT_ACCURACY;
        self.qdy >>= DEFAULT_ACCURACY;
        self.qddx >>= DEFAULT_ACCURACY;
        self.qddy >>= DEFAULT_ACCURACY;
        self.q_last_x >>= DEFAULT_ACCURACY;
        self.q_last_y >>= DEFAULT_ACCURACY;
        self.qy = snap_y(self.qy);
        self.q_last_y = snap_y(self.q_last_y);

        self.base.edge_type = EdgeType::Quad;

        self.snapped_x = self.qx;
        self.snapped_y = self.qy;

        self.update_quadratic()
    }

    /// Steps to the next line segment that is not of height 0. Returns false when the quad is
    /// done.
    // Port of: src/core/SkAnalyticEdge.cpp#L449-L511 (chrome/m156)
    #[doc(alias = "updateQuadratic")]
    #[allow(clippy::similar_names)] // mirrors the C++ names
    #[allow(clippy::nonminimal_bool)] // mirrors the C++ do-while condition
    #[allow(clippy::cast_possible_truncation)] // mirrors SkToS8 (count only decreases)
    pub fn update_quadratic(&mut self) -> bool {
        let mut success = false; // initialize to fail!
        let mut count = i32::from(self.base.curve_count);
        let mut oldx = self.qx;
        let mut oldy = self.qy;
        let mut dx = self.qdx;
        let mut dy = self.qdy;
        let mut newx;
        let mut newy;
        let mut new_snapped_x;
        let mut new_snapped_y;
        let shift = i32::from(self.base.curve_shift);

        debug_assert!(count > 0);

        loop {
            let slope;
            count -= 1;
            if count > 0 {
                newx = oldx.wrapping_add(dx >> shift);
                newy = oldy.wrapping_add(dy >> shift);
                // only snap when dy is large enough and dx/dy isn't too large
                if abs32(dy >> shift) >= FIXED_1 * 2
                    && (i64::from(abs32(dy)) << 6) > i64::from(abs32(dx))
                {
                    let diff_y = fixed_to_fdot6(newy.wrapping_sub(self.snapped_y));
                    slope = if diff_y != 0 {
                        quick_div(fixed_to_fdot6(newx.wrapping_sub(self.snapped_x)), diff_y)
                    } else {
                        MAX_S32
                    };
                    new_snapped_y = self.q_last_y.min(fixed_round_to_fixed(newy));
                    new_snapped_x =
                        newx.wrapping_sub(fixed_mul(slope, newy.wrapping_sub(new_snapped_y)));
                } else {
                    new_snapped_y = self.q_last_y.min(snap_y(newy));
                    new_snapped_x = newx;
                    let diff_y = fixed_to_fdot6(new_snapped_y.wrapping_sub(self.snapped_y));
                    slope = if diff_y != 0 {
                        quick_div(fixed_to_fdot6(newx.wrapping_sub(self.snapped_x)), diff_y)
                    } else {
                        MAX_S32
                    };
                }
                dx = dx.wrapping_add(self.qddx);
                dy = dy.wrapping_add(self.qddy);
            } else {
                // last segment
                newx = self.q_last_x;
                newy = self.q_last_y;
                new_snapped_y = newy;
                new_snapped_x = newx;
                let diff_y = fixed_to_fdot6(newy.wrapping_sub(self.snapped_y));
                slope = if diff_y != 0 {
                    quick_div(fixed_to_fdot6(newx.wrapping_sub(self.snapped_x)), diff_y)
                } else {
                    MAX_S32
                };
            }
            if slope < MAX_S32 {
                success = self.base.update_line(
                    self.snapped_x,
                    self.snapped_y,
                    new_snapped_x,
                    new_snapped_y,
                    slope,
                );
            }
            oldx = newx;
            oldy = newy;
            if !(count > 0 && !success) {
                break;
            }
        }

        debug_assert!(new_snapped_y <= self.q_last_y);

        self.qx = newx;
        self.qy = newy;
        self.qdx = dx;
        self.qdy = dy;
        self.snapped_x = new_snapped_x;
        self.snapped_y = new_snapped_y;
        self.base.curve_count = count as i8;
        success
    }

    /// `keepContinuous`: uses `x` as the starting x to ensure continuity. Without it, we may
    /// break the sorted edge list.
    // Port of: src/core/SkAnalyticEdge.h#L122-L129 (chrome/m156)
    #[doc(alias = "keepContinuous")]
    pub fn keep_continuous(&mut self) {
        self.snapped_x = self.base.x;
        self.snapped_y = self.base.y;
    }
}

// Port of: src/core/SkAnalyticEdge.cpp#L537-L540 (chrome/m156)
fn fdot6_up_shift(x: Fdot6, up_shift: i32) -> i32 {
    debug_assert_eq!(left_shift(x, up_shift) >> up_shift, x);
    left_shift(x, up_shift)
}

// f(1/3) = (8a + 12b + 6c + d) / 27
// f(2/3) = (a + 6b + 12c + 8d) / 27
//
// f(1/3)-b = (8a - 15b + 6c + d) / 27
// f(2/3)-c = (a + 6b - 15c + 8d) / 27
//
// use 16/512 to approximate 1/27
// Port of: src/core/SkAnalyticEdge.cpp#L542-L557 (chrome/m156)
fn cubic_delta_from_line(a: Fdot6, b: Fdot6, c: Fdot6, d: Fdot6) -> Fdot6 {
    // since our parameters may be negative, we don't use << to avoid ASAN warnings
    let one_third = a
        .wrapping_mul(8)
        .wrapping_sub(b.wrapping_mul(15))
        .wrapping_add(6_i32.wrapping_mul(c))
        .wrapping_add(d)
        .wrapping_mul(19)
        >> 9;
    let two_third = a
        .wrapping_add(6_i32.wrapping_mul(b))
        .wrapping_sub(c.wrapping_mul(15))
        .wrapping_add(d.wrapping_mul(8))
        .wrapping_mul(19)
        >> 9;

    abs32(one_third).max(abs32(two_third))
}

/// A cubic edge: walks a cubic as up to 64 line segments (not necessarily monotonic in y).
// Port of: src/core/SkAnalyticEdge.h#L132-L151 (chrome/m156)
#[doc(alias = "SkAnalyticCubicEdge")]
#[derive(Copy, Clone, Debug, Default)]
pub struct AnalyticCubicEdge {
    base: AnalyticEdge,
    pub(crate) cx: Fixed,
    pub(crate) cy: Fixed,
    pub(crate) cdx: Fixed,
    pub(crate) cdy: Fixed,
    pub(crate) cddx: Fixed,
    pub(crate) cddy: Fixed,
    pub(crate) cdddx: Fixed,
    pub(crate) cdddy: Fixed,
    pub(crate) c_last_x: Fixed,
    pub(crate) c_last_y: Fixed,
    // to make sure that y is increasing with smooth jump and snapping
    pub(crate) snapped_y: Fixed,
    // applied to fCDx and fCDy
    pub(crate) to_fixed_shift: u8,
}

impl Deref for AnalyticCubicEdge {
    type Target = AnalyticEdge;
    fn deref(&self) -> &AnalyticEdge {
        &self.base
    }
}

impl DerefMut for AnalyticCubicEdge {
    fn deref_mut(&mut self) -> &mut AnalyticEdge {
        &mut self.base
    }
}

impl AnalyticCubicEdge {
    /// Sets up the line segments of the cubic `pts[0..4]`. Returns false if the cubic has height
    /// 0.
    // Port of: src/core/SkAnalyticEdge.cpp#L513-L535 (chrome/m156)
    #[doc(alias = "setCubic")]
    pub fn set_cubic(&mut self, pts: &[Point]) -> bool {
        if !self.set_cubic_without_update(pts, DEFAULT_ACCURACY) {
            return false;
        }

        self.cx >>= DEFAULT_ACCURACY;
        self.cy >>= DEFAULT_ACCURACY;
        self.cdx >>= DEFAULT_ACCURACY;
        self.cdy >>= DEFAULT_ACCURACY;
        self.cddx >>= DEFAULT_ACCURACY;
        self.cddy >>= DEFAULT_ACCURACY;
        self.cdddx >>= DEFAULT_ACCURACY;
        self.cdddy >>= DEFAULT_ACCURACY;
        self.c_last_x >>= DEFAULT_ACCURACY;
        self.c_last_y >>= DEFAULT_ACCURACY;
        self.cy = snap_y(self.cy);
        self.snapped_y = self.cy;
        self.c_last_y = snap_y(self.c_last_y);

        self.base.edge_type = EdgeType::Cubic;

        self.update_cubic()
    }

    // Port of: src/core/SkAnalyticEdge.cpp#L559-L659 (chrome/m156)
    #[allow(clippy::many_single_char_names)] // mirrors the C++ names
    #[allow(clippy::similar_names)] // mirrors the C++ names
    #[allow(clippy::cast_possible_truncation)] // mirrors SkToS8/SkToU8 and the (int32_t) casts
    #[allow(clippy::cast_sign_loss)] // mirrors SkToU8 (values are non-negative)
    #[allow(clippy::cast_precision_loss)] // 1 << (shift + 6) is exact in a float
    fn set_cubic_without_update(&mut self, pts: &[Point], shift: i32) -> bool {
        let scale = (1 << (shift + 6)) as f32;
        let mut x0 = float_to_int(pts[0].x * scale);
        let mut y0 = float_to_int(pts[0].y * scale);
        let mut x1 = float_to_int(pts[1].x * scale);
        let mut y1 = float_to_int(pts[1].y * scale);
        let mut x2 = float_to_int(pts[2].x * scale);
        let mut y2 = float_to_int(pts[2].y * scale);
        let mut x3 = float_to_int(pts[3].x * scale);
        let mut y3 = float_to_int(pts[3].y * scale);

        let mut winding = Winding::CW;
        if y0 > y3 {
            std::mem::swap(&mut x0, &mut x3);
            std::mem::swap(&mut x1, &mut x2);
            std::mem::swap(&mut y0, &mut y3);
            std::mem::swap(&mut y1, &mut y2);
            winding = Winding::CCW;
        }

        let top = fdot6_round(y0);
        let bot = fdot6_round(y3);

        // are we a zero-height cubic (line)?
        if top == bot {
            return false;
        }

        // compute number of steps needed (1 << shift)
        let mut shift;
        {
            // Can't use (center of curve - center of baseline), since center-of-curve
            // need not be the max delta from the baseline (it could even be coincident)
            // so we try just looking at the two off-curve points
            let dx = cubic_delta_from_line(x0, x1, x2, x3);
            let dy = cubic_delta_from_line(y0, y1, y2, y3);
            // add 1 (by observation)
            shift = diff_to_shift(dx, dy, 2) + 1;
        }
        // need at least 1 subdivision for our bias trick
        debug_assert!(shift > 0);
        if shift > MAX_COEFF_SHIFT {
            shift = MAX_COEFF_SHIFT;
        }

        // Since our in coming data is initially shifted down by 10 (or 8 in antialias). That
        // means the most we can shift up is 8. However, we compute coefficients with a 3*, so the
        // safest upshift is really 6
        let mut up_shift = 6; // largest safe value
        let mut down_shift = shift + up_shift - 10;
        if down_shift < 0 {
            down_shift = 0;
            up_shift = 10 - shift;
        }

        self.base.winding = winding;
        self.base.edge_type = EdgeType::Cubic;
        self.base.curve_count = left_shift(-1, shift) as i8;
        self.base.curve_shift = shift as u8;
        self.to_fixed_shift = down_shift as u8;

        let mut b = fdot6_up_shift(3_i32.wrapping_mul(x1.wrapping_sub(x0)), up_shift);
        let mut c = fdot6_up_shift(
            3_i32.wrapping_mul(x0.wrapping_sub(x1).wrapping_sub(x1).wrapping_add(x2)),
            up_shift,
        );
        let mut d = fdot6_up_shift(
            x3.wrapping_add(3_i32.wrapping_mul(x1.wrapping_sub(x2)))
                .wrapping_sub(x0),
            up_shift,
        );

        self.cx = fdot6_to_fixed(x0);
        self.cdx = b.wrapping_add(c >> shift).wrapping_add(d >> (2 * shift)); // biased by shift
        // Use 64bit math for multiplying by D to avoid overflow
        self.cddx = 2_i64
            .wrapping_mul(i64::from(c))
            .wrapping_add(3_i64.wrapping_mul(i64::from(d)) >> (shift - 1))
            as i32; // biased by 2*shift
        self.cdddx = (3_i64.wrapping_mul(i64::from(d)) >> (shift - 1)) as i32; // biased by 2*shift

        b = fdot6_up_shift(3_i32.wrapping_mul(y1.wrapping_sub(y0)), up_shift);
        c = fdot6_up_shift(
            3_i32.wrapping_mul(y0.wrapping_sub(y1).wrapping_sub(y1).wrapping_add(y2)),
            up_shift,
        );
        d = fdot6_up_shift(
            y3.wrapping_add(3_i32.wrapping_mul(y1.wrapping_sub(y2)))
                .wrapping_sub(y0),
            up_shift,
        );

        self.cy = fdot6_to_fixed(y0);
        self.cdy = b.wrapping_add(c >> shift).wrapping_add(d >> (2 * shift)); // biased by shift
        self.cddy = 2_i64
            .wrapping_mul(i64::from(c))
            .wrapping_add(3_i64.wrapping_mul(i64::from(d)) >> (shift - 1))
            as i32; // biased by 2*shift
        self.cdddy = (3_i64.wrapping_mul(i64::from(d)) >> (shift - 1)) as i32; // biased by 2*shift

        self.c_last_x = fdot6_to_fixed(x3);
        self.c_last_y = fdot6_to_fixed(y3);

        true
    }

    /// Steps to the next line segment that is not of height 0. Returns false when the cubic is
    /// done.
    // Port of: src/core/SkAnalyticEdge.cpp#L661-L717 (chrome/m156)
    #[doc(alias = "updateCubic")]
    #[allow(clippy::similar_names)] // mirrors the C++ names
    #[allow(clippy::nonminimal_bool)] // mirrors the C++ do-while condition
    #[allow(clippy::cast_possible_truncation)] // mirrors SkToS8 (count only increases to 0)
    pub fn update_cubic(&mut self) -> bool {
        let mut success;
        let mut count = i32::from(self.base.curve_count);
        let mut oldx = self.cx;
        let mut oldy = self.cy;
        let mut newx;
        let mut newy;
        let ddshift = i32::from(self.base.curve_shift);
        let dshift = i32::from(self.to_fixed_shift);

        debug_assert!(count < 0);

        loop {
            count += 1;
            if count < 0 {
                newx = oldx.wrapping_add(self.cdx >> dshift);
                self.cdx = self.cdx.wrapping_add(self.cddx >> ddshift);
                self.cddx = self.cddx.wrapping_add(self.cdddx);

                newy = oldy.wrapping_add(self.cdy >> dshift);
                self.cdy = self.cdy.wrapping_add(self.cddy >> ddshift);
                self.cddy = self.cddy.wrapping_add(self.cdddy);
            } else {
                // last segment
                newx = self.c_last_x;
                newy = self.c_last_y;
            }

            // we want to say SkASSERT(oldy <= newy), but our finite fixedpoint
            // doesn't always achieve that, so we have to explicitly pin it here.
            if newy < oldy {
                newy = oldy;
            }

            let mut new_snapped_y = snap_y(newy);
            // we want to SkASSERT(snappedNewY <= fCLastY), but our finite fixedpoint
            // doesn't always achieve that, so we have to explicitly pin it here.
            if self.c_last_y < new_snapped_y {
                new_snapped_y = self.c_last_y;
                count = 0;
            }

            let slope = if fixed_to_fdot6(new_snapped_y.wrapping_sub(self.snapped_y)) == 0 {
                MAX_S32
            } else {
                fdot6_div(
                    fixed_to_fdot6(newx.wrapping_sub(oldx)),
                    fixed_to_fdot6(new_snapped_y.wrapping_sub(self.snapped_y)),
                )
            };

            success = self
                .base
                .update_line(oldx, self.snapped_y, newx, new_snapped_y, slope);

            oldx = newx;
            oldy = newy;
            self.snapped_y = new_snapped_y;
            if !(count < 0 && !success) {
                break;
            }
        }

        self.cx = newx;
        self.cy = newy;
        self.base.curve_count = count as i8;
        success
    }

    /// `keepContinuous`: uses `x` as the starting x to keep the edge list sorted.
    // Port of: src/core/SkAnalyticEdge.h#L146-L150 (chrome/m156)
    #[doc(alias = "keepContinuous")]
    pub fn keep_continuous(&mut self) {
        self.cx = self.base.x;
        self.snapped_y = self.base.y;
    }
}

/// Any of the analytic edge kinds, as stored by the edge builder (the C++ stores
/// `SkAnalyticEdge*` and dispatches on `fCurveCount`). Dereferences to the shared
/// [`AnalyticEdge`] state.
#[doc(alias = "SkAnalyticEdge")]
#[derive(Copy, Clone, Debug)]
pub enum AnyAnalyticEdge {
    Line(AnalyticEdge),
    Quad(AnalyticQuadraticEdge),
    Cubic(AnalyticCubicEdge),
}

impl AnyAnalyticEdge {
    /// Steps a curve edge to its next segment. Returns true if we're NOT done with this edge.
    // Port of: src/core/SkAnalyticEdge.cpp#L260-L268 (chrome/m156)
    pub fn update(&mut self, last_y: Fixed) -> bool {
        debug_assert!(last_y >= self.lower_y); // we shouldn't update edge if last_y < fLowerY
        if self.curve_count < 0 {
            return self.as_cubic().update_cubic();
        } else if self.curve_count > 0 {
            return self.as_quad().update_quadratic();
        }
        false
    }

    /// The cubic edge (`static_cast<SkAnalyticCubicEdge*>`); only valid when `curve_count < 0`.
    pub(crate) fn as_cubic(&mut self) -> &mut AnalyticCubicEdge {
        match self {
            Self::Cubic(e) => e,
            _ => unreachable!("not a cubic edge"),
        }
    }

    /// The quadratic edge (`static_cast<SkAnalyticQuadraticEdge*>`); only valid when
    /// `curve_count > 0`.
    pub(crate) fn as_quad(&mut self) -> &mut AnalyticQuadraticEdge {
        match self {
            Self::Quad(e) => e,
            _ => unreachable!("not a quadratic edge"),
        }
    }
}

impl Deref for AnyAnalyticEdge {
    type Target = AnalyticEdge;
    fn deref(&self) -> &AnalyticEdge {
        match self {
            Self::Line(e) => e,
            Self::Quad(e) => &e.base,
            Self::Cubic(e) => &e.base,
        }
    }
}

impl DerefMut for AnyAnalyticEdge {
    fn deref_mut(&mut self) -> &mut AnalyticEdge {
        match self {
            Self::Line(e) => e,
            Self::Quad(e) => &mut e.base,
            Self::Cubic(e) => &mut e.base,
        }
    }
}

impl LinkedEdge for AnyAnalyticEdge {
    fn next(&self) -> usize {
        let e: &AnalyticEdge = self;
        e.next
    }
    fn prev(&self) -> usize {
        let e: &AnalyticEdge = self;
        e.prev
    }
    fn set_next(&mut self, next: usize) {
        let e: &mut AnalyticEdge = self;
        e.next = next;
    }
    fn set_prev(&mut self, prev: usize) {
        let e: &mut AnalyticEdge = self;
        e.prev = prev;
    }
    fn x(&self) -> Fixed {
        let e: &AnalyticEdge = self;
        e.x
    }
}
