# dm/lottie/skottie-text-animator-randomize-{1,2}.json

Status: `todo`. 8888 differs from the CI oracle on sse2, sse41, ml3, ml4 (the oracle has no scalar tier).

## What was tried
- Text layers render now (`SkottieSrc` passes `skia_rust_shaper::best_available()` and `test_font_mgr()`); 61 of the 65 text-only entries match on every checkable tier, but these two do not.
- `RangeSelector::updateDomainMap` shuffles with `std::shuffle(map, std::mt19937(42))`. The port's `Mt19937` is checked against the reference values of `init_genrand(42)` (1608637542, 3421126067, 4083286876; unit test `mt19937_seed_42`).
- Shuffle variants run against the goldens (hash of `randomize-2`, sse2):
  1. first reconstruction (MSVC `_Rng_from_urng` v1, rejection over 64-bit words): ours `9b3728...` vs golden `4dd2ed...`.
  2. MSVC `_Rng_from_urng_v2` (Lemire), taken from microsoft/STL `stl/inc/algorithm` (`_Random_shuffle1`, `_Rng_from_urng_v2::operator()`; `_Bits = 32`, `_Udiff = uint64_t`, one engine word per index): ours `129c43...`. This is what the port now has: it is what current MSVC STL does.
  3. libc++-style downward Fisher-Yates and upward Fisher-Yates with masked rejection, and no shuffle at all (temporary env switch, since removed): all mismatch.
- No variant matches, including "no shuffle", so the divergence may not be the shuffle order at all.

## Hypotheses
- The oracle's STL may be an older MSVC with a different `_Rng_from_urng`, or clang-cl with libc++/libstdc++ (libstdc++ batches two indices per engine word when the bound product fits in 32 bits: not tried).
- Something else in the randomize path differs (the `local_map` for `Domain::Chars` is built from `fragment_count`; word/line domains copy `maps`). Compare per-fragment coverage against a debug dump of the oracle's `fLocalMap` once an oracle build is available.
