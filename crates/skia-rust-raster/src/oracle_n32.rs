// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Which color type the blitter fast paths treat as `kN32_SkColorType`.
//!
//! In Skia, `kN32_SkColorType` is a build constant: an alias of `kBGRA_8888_SkColorType` on
//! Windows and of `kRGBA_8888_SkColorType` elsewhere. `SkBlitter::Choose` takes the legacy
//! `SkARGB32_*` blitters only for an N32 destination, and the other N32-only fast paths
//! (`SkBlitMask::BlitColor`, the `kN32` sprite blitters, LCD text in `SkGlyphRunListPainter`)
//! test the same thing. On a real host this module answers exactly that: [`is_n32`] is
//! `ct == ColorType::N32`.
//!
//! skia-rust (oracle emulation, `docs/PORTING.md` §11 "N32 byte order"): the default goldens
//! come from a build whose N32 is BGRA. A surface that Skia makes with an *explicit*
//! `kRGBA_8888_SkColorType` (the tile of an `SkPictureShader`) is not N32 there and is drawn by
//! the raster pipeline blitter, whose anti-aliased coverage rounds differently from the legacy
//! blitters'. On a host whose N32 is RGBA the same surface is N32, and the difference reaches
//! the `565`/`f16` results. Two things are needed to reproduce the oracle:
//!
//! - *which oracle is being reproduced*: its N32, forced for a whole render by the test harness
//!   with [`testing::force_oracle_n32`], next to (and like) the forced CPU tier
//!   (`skia_rust_simd::testing::force_tier`). Both model a build or process setting of real
//!   Skia (`kN32_SkColorType`, `SkOpts`), and both exist only with the `testing` feature;
//! - *which surfaces were made with an explicit 8888 color type*: on this host
//!   `ColorType::RGBA8888 == ColorType::N32`, so no device state can tell such a surface from one
//!   made with `kN32_SkColorType`; only the code that names the color type knows. It draws
//!   inside an [`ExplicitColorType`] scope. Devices made while drawing into it (layers, image
//!   filter surfaces) copy its color type, so they are explicit too, as in the oracle.
//!
//! Without the `testing` feature none of this state exists: [`ExplicitColorType`] is empty and
//! [`is_n32`] is the plain comparison, so library behaviour is the host's Skia, always.
//!
//! Only the narrowing direction is emulated (an explicit `kRGBA_8888` surface is not N32 under
//! a BGRA oracle). The opposite one (an explicit `kBGRA_8888` surface that *is* N32 under a BGRA
//! oracle) would need the legacy blitters to run in the other byte order; it is not emulated and
//! no oracle-compared code reaches it.

use skia_rust_core::color_type::ColorType;

/// Whether `ct` is `kN32_SkColorType` for the blitter fast paths (`device.colorType() ==
/// kN32_SkColorType` in Skia).
///
/// skia-rust: also `false` for an explicit 8888 color type that is not the forced oracle's N32
/// (see the module docs; test builds only).
#[must_use]
pub(crate) fn is_n32(ct: ColorType) -> bool {
    #[cfg(any(test, feature = "testing"))]
    if emulation::explicit_and_not_oracle_n32(ct) {
        return false;
    }
    ct == ColorType::N32
}

/// While alive, devices of the color type it names were made with that color type explicitly
/// (not as `kN32_SkColorType`). Scopes nest; the innermost one is in effect.
///
/// Without the `testing` feature this is an empty value with no effect.
#[must_use = "the color type is only explicit while the scope is alive"]
#[derive(Debug)]
pub struct ExplicitColorType {
    #[cfg(any(test, feature = "testing"))]
    previous: Option<ColorType>,
    _not_send: std::marker::PhantomData<*const ()>,
}

impl ExplicitColorType {
    /// The scope for drawing into a surface that Skia makes with the explicit color type `ct`.
    pub fn enter(ct: ColorType) -> Self {
        #[cfg(not(any(test, feature = "testing")))]
        let _ = ct;
        Self {
            #[cfg(any(test, feature = "testing"))]
            previous: emulation::EXPLICIT.replace(Some(ct)),
            _not_send: std::marker::PhantomData,
        }
    }
}

impl Drop for ExplicitColorType {
    fn drop(&mut self) {
        #[cfg(any(test, feature = "testing"))]
        emulation::EXPLICIT.set(self.previous);
    }
}

#[cfg(any(test, feature = "testing"))]
mod emulation {
    use std::cell::Cell;

    use skia_rust_core::color_type::ColorType;

    thread_local! {
        /// The forced oracle's `kN32_SkColorType` (`testing::force_oracle_n32`); `None`: the
        /// host's.
        pub(super) static ORACLE_N32: Cell<Option<ColorType>> = const { Cell::new(None) };
        /// The color type of the innermost [`super::ExplicitColorType`] scope.
        pub(super) static EXPLICIT: Cell<Option<ColorType>> = const { Cell::new(None) };
    }

    /// `ct` is the host's N32 but was named explicitly, and the forced oracle's N32 is another
    /// color type.
    pub(super) fn explicit_and_not_oracle_n32(ct: ColorType) -> bool {
        ct == ColorType::N32
            && EXPLICIT.get() == Some(ct)
            && ORACLE_N32.get().is_some_and(|n32| n32 != ct)
    }
}

/// Test support for comparing with the oracle's goldens (feature `testing`).
#[cfg(any(test, feature = "testing"))]
pub mod testing {
    use skia_rust_core::color_type::ColorType;

    use super::emulation::ORACLE_N32;

    /// Draws as Skia built with `kN32_SkColorType == n32` would, on the current thread, until
    /// the returned guard drops. The test harness forces it for every render it compares with
    /// goldens, with the N32 of the oracle build those goldens come from.
    pub fn force_oracle_n32(n32: ColorType) -> OracleN32Guard {
        OracleN32Guard {
            previous: ORACLE_N32.replace(Some(n32)),
            _not_send: std::marker::PhantomData,
        }
    }

    /// The oracle N32 forced on this thread, or the host's `kN32_SkColorType` when none is forced.
    /// GMs that read N32 pixels as raw `SkColor`s use it to read them in the oracle's byte order.
    #[must_use]
    pub fn oracle_n32() -> ColorType {
        ORACLE_N32.get().unwrap_or(ColorType::N32)
    }

    /// Restores the previous oracle N32 of this thread when dropped. Not `Send`: it must drop on
    /// the thread that created it.
    #[must_use = "the oracle's N32 is only forced while the guard is alive"]
    #[derive(Debug)]
    pub struct OracleN32Guard {
        previous: Option<ColorType>,
        _not_send: std::marker::PhantomData<*const ()>,
    }

    impl Drop for OracleN32Guard {
        fn drop(&mut self) {
            ORACLE_N32.set(self.previous);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The color type that is N32 on the other kind of host.
    const OTHER_8888: ColorType = if matches!(ColorType::N32, ColorType::RGBA8888) {
        ColorType::BGRA8888
    } else {
        ColorType::RGBA8888
    };

    #[test]
    fn host_behaviour_without_a_forced_oracle() {
        let _scope = ExplicitColorType::enter(ColorType::N32);
        assert!(is_n32(ColorType::N32));
        assert!(!is_n32(OTHER_8888));
    }

    #[test]
    fn oracle_of_the_host_byte_order_changes_nothing() {
        let _oracle = testing::force_oracle_n32(ColorType::N32);
        let _scope = ExplicitColorType::enter(ColorType::N32);
        assert!(is_n32(ColorType::N32));
    }

    #[test]
    fn explicit_host_n32_is_not_n32_for_the_other_oracle() {
        let _oracle = testing::force_oracle_n32(OTHER_8888);
        assert!(is_n32(ColorType::N32), "not explicit: the oracle's N32 too");
        {
            let _scope = ExplicitColorType::enter(ColorType::N32);
            assert!(!is_n32(ColorType::N32));
            {
                // A scope for another color type (the F16 tile of a picture shader).
                let _inner = ExplicitColorType::enter(ColorType::RGBAF16Norm);
                assert!(is_n32(ColorType::N32));
            }
            assert!(!is_n32(ColorType::N32));
            // Widening is not emulated.
            assert!(!is_n32(OTHER_8888));
        }
        assert!(is_n32(ColorType::N32));
    }
}
