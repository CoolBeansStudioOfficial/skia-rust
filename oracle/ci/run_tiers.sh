#!/usr/bin/env bash
# Renders one CI oracle build at its CPU tiers (see oracle/README.md, "CI-hosted oracle").
#
#   run_tiers.sh <build> <bin-dir> <goldens-dir> <out-dir>
#
# <bin-dir> holds `dm`, `toolchain.txt` and `resources/` (the build cache). <goldens-dir> holds
# the published `hashes-m156.json` (+ `objects-m156.tar` for the mismatch analysis). For every
# tier of the build it renders `--src gm` (8888) and compares with the golden tier of the same
# name; with EXTRAS=1 it also renders `--src image` (8888 565 f16) and `--src lottie`.
# Results land in <out-dir>/<tier>/: hashes-gm.json, equivalence.{json,md}, hashes-image.json,
# hashes-lottie.json and the DM logs.
set -uo pipefail

build=$1
bin=$(realpath "$2")
goldens=$(realpath "$3")
out=$(realpath -m "$4")
here=$(dirname "$(realpath "$0")")
ci="python3 -I $here/oracle_ci.py"
mkdir -p "$out"

# Tiers of each build: `<tier>:<SKIA_ORACLE_CPU_CAP>` (derived from oracle/tiers.toml the way
# `cargo xtask oracle tiers` does it).
case $build in
  x64-sse2-rgba)   tiers="cpu-x64-sse2-rgba:baseline cpu-x64-sse2-rgba-rt-ml3:ml3 cpu-x64-sse2-rgba-rt-ml4:ml4" ;;
  x64-sse41-rgba)  tiers="cpu-x64-sse41-rgba:ssse3" ;;
  x64-scalar-rgba) tiers="cpu-x64-scalar-rgba:baseline" ;;
  *) echo "unknown build $build"; exit 1 ;;
esac

# What the runner CPU can run. ml3 needs AVX2 (+BMI/FMA/F16C), ml4 AVX-512 F/DQ/CD/BW/VL.
flags=$(grep -m1 '^flags' /proc/cpuinfo)
has() { for f in "$@"; do [[ " $flags " == *" $f "* ]] || return 1; done; }
{
  grep -m1 'model name' /proc/cpuinfo
  echo "ml3-capable: $(has avx2 bmi1 bmi2 fma f16c && echo yes || echo no)"
  echo "ml4-capable: $(has avx512f avx512dq avx512cd avx512bw avx512vl && echo yes || echo no)"
  echo "flags: $flags"
} | tee "$out/cpu.txt"

cd "$bin"
dm_common=(--resourcePath resources --nativeFonts false --cpu true --gpu false --graphite false)

status=0
for spec in $tiers; do
  tier=${spec%%:*}
  cap=${spec##*:}
  tdir=$out/$tier
  mkdir -p "$tdir"
  case $cap in
    ml3) has avx2 bmi1 bmi2 fma f16c || { echo "SKIPPED: this runner lacks AVX2/FMA, cannot run $tier" | tee "$tdir/skipped.txt"; continue; } ;;
    ml4) has avx512f avx512dq avx512cd avx512bw avx512vl || { echo "SKIPPED: this runner lacks AVX-512, cannot run $tier" | tee "$tdir/skipped.txt"; continue; } ;;
  esac
  # The CPU tier the DM metadata reports for this cap.
  case $cap in baseline|ssse3|ml3|ml4) cpu_tier=$cap ;; esac
  echo "::group::$tier (cap $cap): --src gm"
  rm -rf "$tdir/raw-gm"
  SKIA_ORACLE_CPU_CAP=$cap ./dm "${dm_common[@]}" --oracleRawPath "$tdir/raw-gm" \
      --config 8888 --src gm > "$tdir/dm-gm.log" 2>&1
  echo "dm exit: $?" | tee -a "$tdir/dm-gm.log"
  tail -n 20 "$tdir/dm-gm.log"
  echo "::endgroup::"
  $ci hash "$tdir/raw-gm" "$tier" "$cpu_tier" "$bin/toolchain.txt" "$tdir/hashes-gm.json" || status=1
  $ci compare "$tdir/hashes-gm.json" "$goldens/hashes-m156.json" "$tier" "$tdir/raw-gm" \
      "$goldens/objects-m156.tar" "$tdir/equivalence" || status=1
  cat "$tdir/equivalence.md" >> "${GITHUB_STEP_SUMMARY:-/dev/null}" 2>/dev/null || true
  rm -rf "$tdir/raw-gm"   # keep only hashes (each GM tier is ~1 GB raw)

  if [[ ${EXTRAS:-0} == 1 ]]; then
    # 565 and f16 do not depend on the N32 byte order, so their hashes are filed under the BGRA
    # tier name (same file layout as goldens-m156); 8888 stays under the RGBA tier.
    free_tier=${tier/-rgba/}
    for src in image lottie; do
      echo "::group::$tier (cap $cap): --src $src"
      rm -rf "$tdir/raw-$src"
      if [[ $src == image ]]; then
        configs=(8888 565 f16); extra=(--images resources/images)
      else
        configs=(${LOTTIE_CONFIGS:-8888}); extra=(--lotties resources/skottie)
      fi
      SKIA_ORACLE_CPU_CAP=$cap ./dm "${dm_common[@]}" --oracleRawPath "$tdir/raw-$src" \
          "${extra[@]}" --config "${configs[@]}" --src "$src" > "$tdir/dm-$src.log" 2>&1
      echo "dm exit: $?" | tee -a "$tdir/dm-$src.log"
      tail -n 20 "$tdir/dm-$src.log"
      echo "::endgroup::"
      if [[ -d $tdir/raw-$src ]]; then
        $ci hash "$tdir/raw-$src" "$tier" "$cpu_tier" "$bin/toolchain.txt" "$tdir/hashes-$src.json" "$free_tier" || status=1
      fi
      rm -rf "$tdir/raw-$src"
    done
  fi
done
exit $status
