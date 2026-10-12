# dm/lottie/skottie-brightnesscontrast{,-legacy}.json

Status: `todo`. 8888 differs on sse2, sse41, ml3, ml4 (all four tiers).

- Both files have 18 text layers (font "ArialMT"/Arial, which is not in the test font set) plus shape layers inside precomps, with a Brightness & Contrast effect on two precomp layers.
- `-legacy` uses the matrix colour filter path of `BrightnessContrastEffect.cpp` (`makeLegacyCF`, no `pow`, no runtime effect) and still mismatches, so the effect's `std::pow(2.0f, ...)` is not the (only) cause. The first suspect is the text (Arial fallback in the test font manager, glyph positions) or the shapes inside the precomps, since every other text file in the batch (61) matches.
- Hashes (sse2): brightnesscontrast ours `2877fa78...` golden `42bf8616...`; legacy ours `34a1f888...` golden `298ca997...`.
- Next step: render `brightnesscontrast-legacy` with the effect stripped (needs oracle goldens of that variant, not available) or bisect by layer against an oracle build.
