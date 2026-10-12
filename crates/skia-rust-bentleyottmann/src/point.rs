//! Integer points ordered for the sweep. Port of `modules/bentleyottmann/include/Point.h` and
//! `src/Point.cpp` (chrome/m156).

use std::cmp::Ordering;

// Port of: modules/bentleyottmann/include/Point.h#L9-L38 (chrome/m156)

/// An integer point. Points are ordered by `y` first, then `x` (the sweep order).
#[doc(alias = "bentleyottmann::Point")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point {
    /// The x coordinate.
    pub x: i32,
    /// The y coordinate.
    pub y: i32,
}

impl Point {
    /// Creates a point from its coordinates.
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// `Point::Smallest()`: the point with both coordinates at `i32::MIN`.
    // Port of: modules/bentleyottmann/src/Point.cpp#L42-L45 (chrome/m156)
    #[must_use]
    pub const fn smallest() -> Self {
        Self {
            x: i32::MIN,
            y: i32::MIN,
        }
    }

    /// `Point::Largest()`: the point with both coordinates at `i32::MAX`.
    // Port of: modules/bentleyottmann/src/Point.cpp#L47-L50 (chrome/m156)
    #[must_use]
    pub const fn largest() -> Self {
        Self {
            x: i32::MAX,
            y: i32::MAX,
        }
    }

    /// `Point::DifferenceTooBig(p0, p1)`: true if `p1 - p0` in either coordinate would overflow
    /// `i32`.
    // Port of: modules/bentleyottmann/src/Point.cpp#L52-L60 (chrome/m156)
    #[must_use]
    pub fn difference_too_big(p0: Self, p1: Self) -> bool {
        let too_big = |a: i32, b: i32| (b > 0 && a < i32::MIN + b) || (b < 0 && a > i32::MAX + b);
        too_big(p0.x, p1.x) || too_big(p0.y, p1.y)
    }
}

impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Point {
    /// `operator<`: `std::tie(p0.y, p0.x) < std::tie(p1.y, p1.x)`.
    // Port of: modules/bentleyottmann/src/Point.cpp#L13-L15 (chrome/m156)
    fn cmp(&self, other: &Self) -> Ordering {
        (self.y, self.x).cmp(&(other.y, other.x))
    }
}

impl std::ops::Add for Point {
    type Output = Self;

    // Port of: modules/bentleyottmann/include/Point.h#L31-L33 (chrome/m156)
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x.wrapping_add(rhs.x),
            y: self.y.wrapping_add(rhs.y),
        }
    }
}

impl std::ops::Sub for Point {
    type Output = Self;

    // Port of: modules/bentleyottmann/include/Point.h#L34-L36 (chrome/m156)
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x.wrapping_sub(rhs.x),
            y: self.y.wrapping_sub(rhs.y),
        }
    }
}
