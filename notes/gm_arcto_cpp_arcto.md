# gm/arcto.cpp::arcto — host-dependent libm

Ported 1:1 in `tests/gm/src/gm/arcto.rs`. Matches the goldens on every tier on Linux x64 (and
Windows), but PR #78's macOS arm64 CI (debug and release) mismatches on every tier and config with
one hash per config (8888 ours `e58a5397…` vs golden `f8003ff0…`; 565 ours `217bec56…` vs golden
`5d478f0d…`). A tier-independent mismatch on one OS only points at the platform math library:
`SkPath::arcTo` and the conic/arc code call `sk_float_sin`/`cos`/`tan`/`atan2` (`sinf`, … from the
C library), and Apple's libm rounds some inputs differently from glibc and the oracle host's UCRT.

Same root cause as `gm_arcto_cpp_parsedpaths.md` and `gm_dashing_cpp_longpathdash.md`. Fix belongs in
a project-wide deterministic libm (see the libm determinism task), not in this GM.
