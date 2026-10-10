// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Which devices count as `kN32_SkColorType` when a draw has to behave like the oracle's.
//!
//! The default oracle goldens come from a Windows build, where `kN32_SkColorType` is
//! `kBGRA_8888_SkColorType`, so a surface that Skia makes with an explicit `kRGBA_8888_SkColorType`
//! (the tile of an `SkPictureShader`, for one) is *not* N32 there: `SkBlitter::Choose` draws it with
//! the raster pipeline blitter, never with the legacy `SkARGB32_*_Blitter`s, and the other
//! N32-only fast paths (`SkBlitMask`, the `kN32` sprite blitters, LCD text) do not apply either.
//! On a host whose N32 is RGBA (Linux, macOS) such a surface *is* N32 and would take those paths,
//! whose rounding differs from the raster pipeline's in a few anti-aliased edge pixels.
//!
//! skia-rust: [`testing::with_oracle_n32_bgra`] tells the code that makes such surfaces to behave
//! as if N32 were BGRA, for the duration of a draw; the GM harness uses it for the configs whose
//! goldens come from the default (BGRA) oracle tiers. Nothing else changes.

use std::cell::Cell;

use skia_rust_core::color_type::ColorType;

thread_local! {
    /// The oracle being compared with has `kN32_SkColorType == kBGRA_8888_SkColorType`.
    static ORACLE_N32_IS_BGRA: Cell<bool> = const { Cell::new(false) };
    /// Inside a surface that Skia makes with an explicit `kRGBA_8888_SkColorType`, in an oracle
    /// whose N32 is BGRA: `kRGBA_8888` is not N32.
    static RGBA8888_IS_NOT_N32: Cell<bool> = const { Cell::new(false) };
}

/// Whether `ct` is `kN32_SkColorType` for the blitter fast paths.
// skia-rust: `ColorType::N32`, except inside a [`NotN32Scope`].
pub(crate) fn is_n32(ct: ColorType) -> bool {
    ct == ColorType::N32 && !(ct == ColorType::RGBA8888 && RGBA8888_IS_NOT_N32.get())
}

/// While alive, `kRGBA_8888` devices are not N32 (see the module docs). Does nothing when the
/// oracle's N32 is the host's, which is the case outside the GM harness.
pub(crate) struct NotN32Scope {
    previous: bool,
}

impl NotN32Scope {
    /// The scope for drawing into a surface that Skia makes with an explicit `ct`.
    pub(crate) fn explicit_surface(ct: ColorType) -> Self {
        let previous = RGBA8888_IS_NOT_N32.get();
        if ct == ColorType::RGBA8888 && ORACLE_N32_IS_BGRA.get() {
            RGBA8888_IS_NOT_N32.set(true);
        }
        Self { previous }
    }
}

impl Drop for NotN32Scope {
    fn drop(&mut self) {
        RGBA8888_IS_NOT_N32.set(self.previous);
    }
}

/// Test support for comparing with the oracle's goldens.
pub mod testing {
    use super::ORACLE_N32_IS_BGRA;

    /// Runs `f` with the oracle's N32 being BGRA (`bgra`) or the host's. Restores the previous
    /// setting afterwards, also if `f` panics.
    pub fn with_oracle_n32_bgra<R>(bgra: bool, f: impl FnOnce() -> R) -> R {
        struct Restore(bool);
        impl Drop for Restore {
            fn drop(&mut self) {
                ORACLE_N32_IS_BGRA.set(self.0);
            }
        }
        let _restore = Restore(ORACLE_N32_IS_BGRA.replace(bgra));
        f()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn native(ct: ColorType) -> bool {
        ct == ColorType::N32
    }

    #[test]
    fn the_scope_only_applies_when_the_oracle_is_bgra() {
        {
            let _scope = NotN32Scope::explicit_surface(ColorType::RGBA8888);
            assert_eq!(is_n32(ColorType::RGBA8888), native(ColorType::RGBA8888));
        }
        testing::with_oracle_n32_bgra(true, || {
            {
                let _scope = NotN32Scope::explicit_surface(ColorType::RGBA8888);
                assert!(!is_n32(ColorType::RGBA8888));
                let _inner = NotN32Scope::explicit_surface(ColorType::BGRA8888);
                assert!(!is_n32(ColorType::RGBA8888));
            }
            assert_eq!(is_n32(ColorType::RGBA8888), native(ColorType::RGBA8888));
        });
        assert!(is_n32(ColorType::N32));
    }
}
