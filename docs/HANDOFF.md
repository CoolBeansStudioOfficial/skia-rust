# Handoff: orchestration state and working rules

Written 2026-10-07 when the original Windows server (the oracle host) was decommissioned and work
moved to a cloud session. It records everything that previously lived only on that machine: the
maintainer's standing instructions, the orchestration scripts, and the current state.

## Standing instructions from the maintainer

- **Autonomy.** Build from `docs/PLAN.md` with minimal check-ins. Still ask before anything outside
  the categories below (publishing crates, deleting data, spending money, credentials).
- **Git.** One branch + PR per task; self-merge (squash) once *all* CI jobs are green on the PR's
  current head commit. Never merge on stale or partial checks.
- **Installs** allowed without asking: rustup toolchains/components/targets, OS dev packages
  (apt in Linux environments, winget on Windows).
- **Agents.** Fan out widely in parallel. Model ladder: **every agent starts on Sonnet** (set the
  model explicitly; omitting it inherits the orchestrator's model). **Opus is emergency-only** —
  only after a Sonnet agent has delivered an invalid result on that same task. Fix-ups and rebases
  go to fresh Sonnet agents. **Never use Haiku.**
- **No polling.** Never use wake-up/poll loops or chains of jobs that each notify. Put a whole
  landing sequence in ONE background job that reports once, on final success or first failure.
  Agents wait for CI with a single blocking `gh pr checks <n> --watch` / `gh run watch`.
- **API.** The public API mirrors rust-skia's `skia-safe` (names, module paths, signatures, enums,
  bitflags) minus FFI plumbing. Copy API *shape* only; implementations are ported from Skia C++.
  Reference checkout: `third_party/rust-skia` (pinned in `inventory/api-reference.toml`, fetched by
  `cargo xtask skia fetch`). Rules in `docs/PORTING.md` §3.
- **Goldens** may be generated and published to GitHub Releases without asking.
- **Credentials.** Never handle tokens or passwords. crates.io names are reserved; publishing
  needs the maintainer's own `cargo login`.

## Orchestration scripts (`tools/orch/`)

- `queue.sh "branch:pr[:agent-id]" ...` lands PRs strictly in order, stopping at the first failure.
- `land.sh <branch> <pr> [agent-id]` (one at a time, lock in `$TMPDIR`): checks out the branch in
  a dedicated integration worktree (`$INTEGRATE`, default `../skia-rust-integrate` next to the
  repo — create it with `git worktree add --detach ../skia-rust-integrate origin/main`), rebases
  onto `origin/main`, auto-resolves mechanical conflicts (module lists via `resolve_mods.py`,
  docs/Cargo.toml via `resolve_union.py`, `inventory/manifest.toml` via `resolve_manifest.py`,
  test files via `resolve_rust_union.py`; anything else exits 3 "MANUAL CONFLICT" → send back to
  an agent), runs fmt / clippy `-D warnings` / `cargo test --workspace` / `cargo xtask inventory
  verify`, force-pushes with lease, then `merge-when-green.sh <pr> 12 <head-sha>`, and finally
  removes `$REPO/.claude/worktrees/agent-<id>`.
- `merge-when-green.sh <pr> <n> <sha>` waits (up to 3 h) until the PR head is `<sha>` and has ≥ n
  checks, all complete; squash-merges with `--match-head-commit` only if every check succeeded.
- CI currently has **12 jobs**: lint, miri, wasm, manifest sync, `test` × 4 platforms
  (windows, ubuntu x64, macos arm64, ubuntu arm64), `test-release` × 4.
- Generated oracle files (`oracle/rp-diff/expected/*.txt`, `*/skia_dump*.txt`) must never be
  hand-merged: take main's version and regenerate.

## The oracle and what was lost with the server

The oracle builds of real Skia (`out/oracle/<tier>/` — clang-cl, Windows, x64 tiers sse2 / sse41 /
ml3 / ml4 / scalar / RGBA variants, Dawn GPU tiers) lived only on the server (AMD Zen4,
Windows 11). They are **not** recoverable from git. What survives:

- Goldens: GitHub Release `goldens-m156` (28 tiers; `inventory/goldens.lock`).
- Every oracle dump that tests compare against is committed (`oracle/*/…`, `crates/*/src/**/skia_*dump*.txt`,
  `oracle/rp-diff/expected/`). All CI tests run without the oracle.
- The build recipe: `oracle/tiers.toml`, `oracle/patches/skia-oracle.patch`, `oracle/README.md`,
  `cargo xtask oracle …`. Rebuilding needs Windows + clang-cl (LLVM 23.1.2) + VS Build Tools for
  the Windows (BGRA) tiers; Linux builds would give the RGBA-order variants.
- **Host estimates:** the oracle host is AMD Zen4. Tests pick tiers with
  `skia_rust_simd::testing::oracle_selection(tier)`: native only if the host's rcp/rsqrt estimate
  fingerprint matches Zen4, otherwise the `Model(AmdZen4)` twin. Any new oracle-compared test must
  use it (PR #71 failed release CI on Intel runners without it).

Until an oracle host exists again, new C++ comparison harnesses (the pattern used by D2–D5:
`oracle/rp-builder`, `oracle/scan-aaa`, `oracle/aaclip`, `oracle/draw`) cannot be run. Prefer
GM/golden comparisons (goldens are published) and the already-committed dumps; record anything
that needs a fresh oracle run as an issue labelled `needs-oracle`.

## State at handoff (main after PR #71)

- Pass rate: 382 / 6,221 in-scope (6.1%); unit 381 / 2,464; GM 1 / 1,095.
- Phase 2 Waves A, B, C (C1–C7) and D1–D5, D8 are merged. Design notes: "As implemented in …"
  sections of `docs/design/raster-pipeline.md`; API deviations in `docs/API_MAPPING.md`.
- **D6 (Canvas, ClipStack, raster Surface)** was in progress; work in progress pushed to branch
  `port/canvas-wip` (see the PR/commit message there for done / half-done / not-started). D6
  should: port raster Canvas + ClipStack + Surface (skia-safe API shape), plumb SurfaceProps into
  StageRec and the blitter create fns, switch the GM harness (`tests/gm`) from its Surface stub to
  the real Surface, un-ignore every test `#[ignore]`d for needing Canvas/Surface, port
  Canvas/ClipStack/Surface tests, and report the GM pass count. The D5 design note lists what D6
  must wire up.
- Next after D6: D7 (pictures), Wave E (GM sweep + benches), images/bitmap drawing, text.
- Open issues: #22-era items closed; #34 exactness is release-only; #36 interpreter perf;
  #63 x86 models give Arm default NaN on non-x86 hosts; #65 no sse41 rp-builder oracle output;
  #68 `RasterClip::op_shader` needs SkBlendShader.
