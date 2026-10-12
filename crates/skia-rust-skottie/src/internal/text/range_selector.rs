// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/text/RangeSelector.h, RangeSelector.cpp (chrome/m156)

use std::cell::RefCell;
use std::rc::Rc;

use skia_rust_core::cubic_map::CubicMap;
use skia_rust_core::floating_point::ieee_float_divide;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::t_pin::t_pin;

use crate::internal::animator::{Prop, PropertyContainer};
use crate::internal::skottie_priv::AnimationBuilder;
use crate::internal::text::text_animator::{DomainMap, DomainMaps, ModulatorBuffer};
use crate::json::{ObjectValue, Value};
use crate::skottie::LoggerLevel;
use crate::skottie_json::parse_default;

/// What the range values are relative to (`RangeSelector::Units`).
// Port of: modules/skottie/src/text/RangeSelector.h#L33-L36 (chrome/m156)
#[doc(alias = "skottie::internal::RangeSelector::Units")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Units {
    /// Values are percentages of domain size (`kPercentage`).
    Percentage,
    /// Values are direct domain indices (`kIndex`).
    Index,
}

/// What the selector indexes (`RangeSelector::Domain`).
// Port of: modules/skottie/src/text/RangeSelector.h#L38-L43 (chrome/m156)
#[doc(alias = "skottie::internal::RangeSelector::Domain")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    /// Domain indices map to glyph indices (`kChars`).
    Chars,
    /// Domain indices map to glyph indices, ignoring spaces (`kCharsExcludingSpaces`).
    CharsExcludingSpaces,
    /// Domain indices map to word indices (`kWords`).
    Words,
    /// Domain indices map to line indices (`kLines`).
    Lines,
}

/// How the coverage accumulates (`RangeSelector::Mode`).
// Port of: modules/skottie/src/text/RangeSelector.h#L45-L53 (chrome/m156)
#[doc(alias = "skottie::internal::RangeSelector::Mode")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Add (`kAdd`).
    Add,
}

/// The shape of the coverage (`RangeSelector::Shape`).
// Port of: modules/skottie/src/text/RangeSelector.h#L55-L62 (chrome/m156)
#[doc(alias = "skottie::internal::RangeSelector::Shape")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// Square.
    Square,
    /// Ramp up.
    RampUp,
    /// Ramp down.
    RampDown,
    /// Triangle.
    Triangle,
    /// Round.
    Round,
    /// Smooth.
    Smooth,
}

// Maps a 1-based JSON enum to one of the values in the array.
// Port of: modules/skottie/src/text/RangeSelector.cpp#L28-L48 (chrome/m156) (`ParseEnum`)
fn parse_enum<T: Copy>(
    arr: &[T],
    jenum: &Value,
    abuilder: &AnimationBuilder<'_>,
    warn_name: &str,
) -> T {
    let idx = parse_default::<i32>(jenum, 1);

    if idx > 0 && usize::try_from(idx).is_ok_and(|idx| idx <= arr.len()) {
        return arr[usize::try_from(idx).expect("positive") - 1];
    }

    // For animators without selectors, BM emits placeholder selector entries with 0 (inval)
    // props. Supress warnings for these as they are "normal".
    if idx != 0 {
        abuilder.log(
            LoggerLevel::Warning,
            &format!("Ignoring unknown range selector {warn_name} '{idx}'"),
        );
    }

    debug_assert!(!arr.is_empty());
    arr[0]
}

// `UnitTraits<Units>::Defaults`.
// Port of: modules/skottie/src/text/RangeSelector.cpp#L50-L77 (chrome/m156) (`UnitTraits`)
fn unit_defaults(units: Units) -> (f32, f32, f32) {
    match units {
        Units::Percentage => (0.0, 100.0, 0.0),
        // It's OK to default fEnd to FLOAT_MAX, as it gets clamped when resolved.
        Units::Index => (0.0, f32::MAX, 0.0),
    }
}

// `UnitTraits<Units>::Resolve`.
// Port of: modules/skottie/src/text/RangeSelector.cpp#L50-L77 (chrome/m156) (`UnitTraits`)
#[allow(clippy::cast_precision_loss)] // mirrors the size_t to float conversion
fn unit_resolve(units: Units, s: f32, e: f32, o: f32, domain_size: usize) -> (f32, f32) {
    match units {
        Units::Percentage => (
            domain_size as f32 * (s + o) / 100.0,
            domain_size as f32 * (e + o) / 100.0,
        ),
        Units::Index => (s + o, e + o),
    }
}

// Port of: modules/skottie/src/text/RangeSelector.cpp#L79-L134 (chrome/m156) (`CoverageProcessor`)
struct CoverageProcessor<'a> {
    dst: &'a mut ModulatorBuffer,
    map: Option<&'a DomainMap>,
    domain_size: usize,
}

impl<'a> CoverageProcessor<'a> {
    // Port of: modules/skottie/src/text/RangeSelector.cpp#L81-L98 (chrome/m156)
    fn new(
        map: Option<&'a DomainMap>,
        _domain: Domain,
        mode: Mode,
        dst: &'a mut ModulatorBuffer,
    ) -> Self {
        debug_assert_eq!(mode, Mode::Add);

        // When no domain map is active, the processor applies the mode proc directly.
        // Otherwise, we punt through a domain mapper proxy.
        let domain_size = match map {
            Some(map) => map.len(),
            None => dst.len(),
        };
        Self {
            dst,
            map,
            domain_size,
        }
    }

    // Port of: modules/skottie/src/text/RangeSelector.cpp#L100 (chrome/m156)
    fn size(&self) -> usize {
        self.domain_size
    }

    // Port of: modules/skottie/src/text/RangeSelector.cpp#L102-L104 (chrome/m156)
    fn call(&mut self, amount: f32, offset: usize, count: usize) {
        match self.map {
            None => self.add_proc(amount, offset, count),
            Some(map) => self.domain_map_proc(map, amount, offset, count),
        }
    }

    // mode: kAdd
    // Port of: modules/skottie/src/text/RangeSelector.cpp#L107-L114 (chrome/m156)
    #[allow(clippy::float_cmp)] // C++ `!amount`
    fn add_proc(&mut self, amount: f32, offset: usize, count: usize) {
        if amount == 0.0 || count == 0 {
            return;
        }

        for dst in &mut self.dst[offset..offset + count] {
            dst.coverage = t_pin(dst.coverage + amount, -1.0, 1.0);
        }
    }

    // A proxy for mapping domain indices to the target buffer.
    // Port of: modules/skottie/src/text/RangeSelector.cpp#L117-L125 (chrome/m156)
    fn domain_map_proc(&mut self, map: &DomainMap, amount: f32, offset: usize, count: usize) {
        for span in &map[offset..offset + count] {
            self.add_proc(amount, span.offset, span.count);
        }
    }
}

/*
  Selector shapes can be generalized as a signal generator with the following
  parameters/properties:


  1  +               -------------------------
     |              /.           .           .\
     |             / .           .           . \
     |            /  .           .           .  \
     |           /   .           .           .   \
     |          /    .           .           .    \
     |         /     .           .           .     \
     |        /      .           .           .      \
     |       /       .           .           .       \
  0  +----------------------------------------------------------
            ^ <----->            ^            <-----> ^
           e0   crs             sp              crs    e1


    * e0, e1: left/right edges
    * sp    : symmetry/reflection point (sp == (e0+e1)/2)
    * crs   : cubic ramp size (transitional portion mapped using a Bezier easing function)

  Based on these,

            |  0                  , t <= e0
            |
            |  Bez((t-e0)/crs)    , e0 < t < e0+crs
     F(t) = |
            |  1                  , e0 + crs <= t <= sp
            |
            |  F(reflect(t,sp))   , t > sp


   Tweaking this function's parameters, we can achieve all range selectors shapes:

     - square    -> e0:    0, e1:    1, crs: 0
     - ramp up   -> e0:    0, e1: +inf, crs: 1
     - ramp down -> e0: -inf, e1:    1, crs: 1
     - triangle  -> e0:    0, e1:    1, crs: 0.5
     - round     -> e0:    0, e1:    1, crs: 0.5   (nonlinear cubic mapper)
     - smooth    -> e0:    0, e1:    1, crs: 0.5   (nonlinear cubic mapper)

*/

// Port of: modules/skottie/src/text/RangeSelector.cpp#L174-L178 (chrome/m156) (`ShapeInfo`)
struct ShapeInfo {
    ctrl0: Vector,
    ctrl1: Vector,
    e0: f32,
    e1: f32,
    crs: f32,
}

// Port of: modules/skottie/src/text/RangeSelector.cpp#L180-L182 (chrome/m156) (`EaseVec`)
fn ease_vec(ease: f32) -> Vector {
    if ease < 0.0 {
        Vector::new(0.0, -ease)
    } else {
        Vector::new(ease, 0.0)
    }
}

// Port of: modules/skottie/src/text/RangeSelector.cpp#L184-L207 (chrome/m156) (`ShapeGenerator`)
struct ShapeGenerator {
    shape_mapper: CubicMap,
    ease_mapper: CubicMap,
    e0: f32,
    e1: f32,
    crs: f32,
}

impl ShapeGenerator {
    // Port of: modules/skottie/src/text/RangeSelector.cpp#L189-L195 (chrome/m156)
    fn new(sinfo: &ShapeInfo, ease_lo: f32, ease_hi: f32) -> Self {
        let ease_hi_vec = ease_vec(ease_hi);
        Self {
            shape_mapper: CubicMap::new(sinfo.ctrl0, sinfo.ctrl1),
            ease_mapper: CubicMap::new(
                ease_vec(ease_lo),
                Vector::new(1.0 - ease_hi_vec.x, 1.0 - ease_hi_vec.y),
            ),
            e0: sinfo.e0,
            e1: sinfo.e1,
            crs: sinfo.crs,
        }
    }

    // Port of: modules/skottie/src/text/RangeSelector.cpp#L197-L204 (chrome/m156)
    fn call(&self, t: f32) -> f32 {
        // SkCubicMap clamps its input, so we can let it all hang out.
        let a = t - self.e0;
        let b = self.e1 - t;
        // std::min(a, b)
        let t = if b < a { b } else { a };
        let t = ieee_float_divide(t, self.crs);

        self.ease_mapper
            .compute_y_from_x(self.shape_mapper.compute_y_from_x(t))
    }
}

// Port of: modules/skottie/src/text/RangeSelector.cpp#L209-L216 (chrome/m156) (`gShapeInfo`)
fn shape_info(shape: Shape) -> ShapeInfo {
    let (c0, c1, e0, e1, crs) = match shape {
        // kSquare
        Shape::Square => ((0.0, 0.0), (1.0, 1.0), 0.0, 1.0, 0.0),
        // kRampUp
        Shape::RampUp => ((0.0, 0.0), (1.0, 1.0), 0.0, f32::INFINITY, 1.0),
        // kRampDown
        Shape::RampDown => ((0.0, 0.0), (1.0, 1.0), f32::NEG_INFINITY, 1.0, 1.0),
        // kTriangle
        Shape::Triangle => ((0.0, 0.0), (1.0, 1.0), 0.0, 1.0, 0.5),
        // kRound
        Shape::Round => ((0.0, 0.5), (0.5, 1.0), 0.0, 1.0, 0.5),
        // kSmooth
        Shape::Smooth => ((0.5, 0.0), (0.5, 1.0), 0.0, 1.0, 0.5),
    };
    ShapeInfo {
        ctrl0: Point::new(c0.0, c0.1),
        ctrl1: Point::new(c1.0, c1.1),
        e0,
        e1,
        crs,
    }
}

// The state of the selector that follows the domain maps of its adapter.
// Port of: modules/skottie/src/text/RangeSelector.h#L83-L86 (chrome/m156) (`fLocalMap`, `fMap`)
#[derive(Default)]
struct MapState {
    /// Domain map adjusted for the current selector (for e.g. randomization).
    local_map: Option<DomainMap>,
    /// Adapter (layer) scoped domain map.
    map: Option<Rc<DomainMap>>,
}

/// Selects a range of the text for an animator to apply to.
// Port of: modules/skottie/src/text/RangeSelector.h#L28-L90 (chrome/m156) (`class RangeSelector`)
#[doc(alias = "skottie::internal::RangeSelector")]
pub struct RangeSelector {
    units: Units,
    domain: Domain,
    mode: Mode,
    shape: Shape,
    randomize_order: bool,

    maps: RefCell<MapState>,

    start: Prop<f32>,
    end: Prop<f32>,
    offset: Prop<f32>,
    amount: Prop<f32>,
    ease_lo: Prop<f32>,
    ease_hi: Prop<f32>,
    smoothness: Prop<f32>,
}

impl RangeSelector {
    // Port of: modules/skottie/src/text/RangeSelector.cpp#L237-L316 (chrome/m156)
    /// Makes a selector from `jrange`, binding its properties to `acontainer`.
    #[must_use]
    #[allow(clippy::items_after_statements)] // mirrors the function-local constants of C++
    pub fn make(
        jrange: Option<&ObjectValue>,
        abuilder: &AnimationBuilder<'_>,
        acontainer: &PropertyContainer,
    ) -> Option<Rc<Self>> {
        let jrange = jrange?;

        const RANGE_SELECTOR_TYPE: i32 = 0;
        {
            let type_ = parse_default::<i32>(jrange.get("t"), RANGE_SELECTOR_TYPE);
            if type_ != RANGE_SELECTOR_TYPE {
                abuilder.log(
                    LoggerLevel::Warning,
                    &format!("Ignoring unsupported selector type '{type_}'"),
                );
                return None;
            }
        }

        const UNIT_MAP: [Units; 2] = [
            Units::Percentage, // 'r': 1
            Units::Index,      // 'r': 2
        ];

        const DOMAIN_MAP: [Domain; 4] = [
            Domain::Chars,                // 'b': 1
            Domain::CharsExcludingSpaces, // 'b': 2
            Domain::Words,                // 'b': 3
            Domain::Lines,                // 'b': 4
        ];

        const MODE_MAP: [Mode; 1] = [
            Mode::Add, // 'm': 1
        ];

        const SHAPE_MAP: [Shape; 6] = [
            Shape::Square,   // 'sh': 1
            Shape::RampUp,   // 'sh': 2
            Shape::RampDown, // 'sh': 3
            Shape::Triangle, // 'sh': 4
            Shape::Round,    // 'sh': 5
            Shape::Smooth,   // 'sh': 6
        ];

        // 'rn': 0 | 1
        let randomize_order = parse_default::<i32>(jrange.get("rn"), 0) != 0;

        let selector = Rc::new(Self::new(
            parse_enum(&UNIT_MAP, jrange.get("r"), abuilder, "units"),
            parse_enum(&DOMAIN_MAP, jrange.get("b"), abuilder, "domain"),
            parse_enum(&MODE_MAP, jrange.get("m"), abuilder, "mode"),
            parse_enum(&SHAPE_MAP, jrange.get("sh"), abuilder, "shape"),
            randomize_order,
        ));

        acontainer.bind(abuilder, jrange.get("s"), &selector.start);
        acontainer.bind(abuilder, jrange.get("e"), &selector.end);
        acontainer.bind(abuilder, jrange.get("o"), &selector.offset);
        acontainer.bind(abuilder, jrange.get("a"), &selector.amount);
        acontainer.bind(abuilder, jrange.get("ne"), &selector.ease_lo);
        acontainer.bind(abuilder, jrange.get("xe"), &selector.ease_hi);

        // Optional square "smoothness" prop.
        if selector.shape == Shape::Square {
            acontainer.bind(abuilder, jrange.get("sm"), &selector.smoothness);
        }

        Some(selector)
    }

    // Port of: modules/skottie/src/text/RangeSelector.cpp#L318-L335 (chrome/m156)
    fn new(u: Units, d: Domain, m: Mode, sh: Shape, ro: bool) -> Self {
        // Range defaults are unit-specific.
        let (start, end, offset) = unit_defaults(u);
        Self {
            units: u,
            domain: d,
            mode: m,
            shape: sh,
            randomize_order: ro,
            maps: RefCell::new(MapState::default()),
            start: Prop::new(start),
            end: Prop::new(end),
            offset: Prop::new(offset),
            amount: Prop::new(100.0),
            ease_lo: Prop::new(0.0),
            ease_hi: Prop::new(0.0),
            smoothness: Prop::new(100.0),
        }
    }

    // Resolves this selector to a range in the coverage buffer index domain.
    // Port of: modules/skottie/src/text/RangeSelector.cpp#L337-L351 (chrome/m156)
    fn resolve(&self, len: usize) -> (f32, f32) {
        let (mut f_i0, mut f_i1) = unit_resolve(
            self.units,
            self.start.get(),
            self.end.get(),
            self.offset.get(),
            len,
        );
        if f_i0 > f_i1 {
            std::mem::swap(&mut f_i0, &mut f_i1);
        }

        (f_i0, f_i1)
    }

    /// Updates the domain map this selector follows (`updateDomainMap`).
    // Port of: modules/skottie/src/text/RangeSelector.cpp#L353-L395 (chrome/m156)
    #[doc(alias = "updateDomainMap")]
    #[allow(clippy::items_after_statements)] // mirrors the function-local constant of C++
    pub fn update_domain_map(&self, maps: &DomainMaps, fragment_count: usize) {
        let mut state = self.maps.borrow_mut();
        state.map = match self.domain {
            Domain::Chars => None,
            Domain::CharsExcludingSpaces => Some(Rc::clone(&maps.non_whitespace_map)),
            Domain::Words => Some(Rc::clone(&maps.words_map)),
            Domain::Lines => Some(Rc::clone(&maps.lines_map)),
        };

        if self.randomize_order {
            let mut local_map: DomainMap = if let Some(map) = &state.map {
                (**map).clone()
            } else {
                debug_assert_eq!(self.domain, Domain::Chars);
                // We don't normally generate a map for Domain::kChars, since it's trivial (1:1).
                // So we generate one on the fly here, for randomization.
                let mut local_map = state.local_map.take().unwrap_or_default();
                local_map.clear();
                local_map.reserve(fragment_count);
                for i in 0..fragment_count {
                    local_map.push(crate::internal::text::text_animator::DomainSpan {
                        offset: i,
                        count: 1,
                        advance: 0.0, // not used for range selectors
                        ascent: 0.0,  // not used for range selectors
                    });
                }
                local_map
            };

            const SEED: u32 = 42;
            let mut rng_engine = Mt19937::new(SEED);
            shuffle(&mut local_map, &mut rng_engine);
            state.local_map = Some(local_map);
        }
    }

    /*
     * General RangeSelector operation:
     *
     *   1) The range is resolved to a target domain (characters, words, etc) interval, based on
     *      |start|, |end|, |offset|, |units|.
     *
     *   2) A shape generator is mapped to this interval and applied across the whole domain,
     *      yielding coverage values in [0..1].
     *
     *   3) The coverage is then scaled by the |amount| parameter.
     *
     *   4) Finally, the resulting coverage is accumulated to existing fragment coverage based on
     *      the specified Mode (add, difference, etc).
     */
    /// Accumulates the coverage of this selector into `mbuf` (`modulateCoverage`).
    // Port of: modules/skottie/src/text/RangeSelector.cpp#L397-L437 (chrome/m156)
    #[doc(alias = "modulateCoverage")]
    pub fn modulate_coverage(&self, mbuf: &mut ModulatorBuffer) {
        let state = self.maps.borrow();
        let map: Option<&DomainMap> = match &state.local_map {
            Some(local_map) => Some(local_map),
            None => state.map.as_deref(),
        };
        let mut coverage_proc = CoverageProcessor::new(map, self.domain, self.mode, mbuf);
        if coverage_proc.size() == 0 {
            return;
        }

        // Amount, ease-low and ease-high are percentage-based [-100% .. 100%].
        let amount = t_pin(self.amount.get() / 100.0, -1.0, 1.0);
        let ease_lo = t_pin(self.ease_lo.get() / 100.0, -1.0, 1.0);
        let ease_hi = t_pin(self.ease_hi.get() / 100.0, -1.0, 1.0);

        // Resolve to a float range in the given domain.
        let range = self.resolve(coverage_proc.size());
        let mut r0 = range.0;
        // std::max(a, b)
        let len_a = range.1 - r0;
        let mut len = if len_a < f32::EPSILON {
            f32::EPSILON
        } else {
            len_a
        };

        let mut generator = ShapeGenerator::new(&shape_info(self.shape), ease_lo, ease_hi);

        if self.shape == Shape::Square {
            // Canonical square generators have collapsed ramps, but AE square selectors have
            // an additional "smoothness" property (0..1) which introduces a non-zero transition.
            // We achieve this by moving the range edges outward by |smoothness|/2, and adjusting
            // the generator cubic ramp size.

            // smoothness is percentage-based [0..100]
            let smoothness = t_pin(self.smoothness.get() / 100.0, 0.0, 1.0);

            r0 -= smoothness / 2.0;
            len += smoothness;

            generator.crs += smoothness / len;
        }

        debug_assert!(len > 0.0);
        let dt = 1.0 / len;
        let mut t = (0.5 - r0) / len; // sampling bias: mid-unit

        for i in 0..coverage_proc.size() {
            coverage_proc.call(amount * generator.call(t), i, 1);
            t += dt;
        }
    }
}

// `std::mt19937`: the 32-bit Mersenne Twister, seeded as `std::mersenne_twister_engine` does.
struct Mt19937 {
    state: [u32; 624],
    index: usize,
}

impl Mt19937 {
    fn new(seed: u32) -> Self {
        let mut state = [0_u32; 624];
        state[0] = seed;
        for i in 1..624 {
            let prev = state[i - 1];
            state[i] = 1_812_433_253_u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(u32::try_from(i).expect("small index"));
        }
        Self { state, index: 624 }
    }

    fn twist(&mut self) {
        const UPPER_MASK: u32 = 0x8000_0000;
        const LOWER_MASK: u32 = 0x7fff_ffff;
        for i in 0..624 {
            let x = (self.state[i] & UPPER_MASK) | (self.state[(i + 1) % 624] & LOWER_MASK);
            let mut x_a = x >> 1;
            if x & 1 != 0 {
                x_a ^= 0x9908_b0df;
            }
            self.state[i] = self.state[(i + 397) % 624] ^ x_a;
        }
        self.index = 0;
    }

    fn next_u32(&mut self) -> u32 {
        if self.index >= 624 {
            self.twist();
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }
}

// `std::shuffle(first, last, rng)` as the MSVC STL implements it (the standard leaves the
// algorithm unspecified): a Fisher-Yates shuffle whose random indices come from
// `_Rng_from_urng`, which draws 32-bit values from the engine into 64-bit words and rejects
// biased ones. skia-rust: the oracle's STL cannot be checked here, so this is the algorithm
// of that library as documented, not a verified match.
#[allow(clippy::items_after_statements)] // the constants of `_Rng_from_urng`
fn shuffle<T>(items: &mut [T], rng: &mut Mt19937) {
    if items.is_empty() {
        return;
    }

    // `_Rng_from_urng<ptrdiff_t, mt19937>`: `_Udiff` is 64 bits, the engine range is 32 bits.
    const BITS: u32 = 32;
    const BMASK: u64 = 0xffff_ffff;
    let mut index = |upper: u64| -> u64 {
        loop {
            let mut ret: u64 = 0;
            let mut mask: u64 = 0;
            while mask < upper - 1 {
                ret <<= BITS - 1;
                ret <<= 1;
                ret |= u64::from(rng.next_u32());
                mask <<= BITS - 1;
                mask <<= 1;
                mask |= BMASK;
            }
            if ret / upper < mask / upper || mask % upper == upper - 1 {
                return ret % upper;
            }
        }
    };

    for target in 1..items.len() {
        let off = usize::try_from(index(target as u64 + 1)).expect("in range");
        debug_assert!(off <= target);
        if off != target {
            items.swap(target, off);
        }
    }
}

opaque_debug!(RangeSelector);
