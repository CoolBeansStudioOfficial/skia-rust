#!/usr/bin/env bash
# Renders one CI oracle build at its CPU tiers (see oracle/README.md, "CI-hosted oracle").
#
#   run_tiers.sh <build> <bin-dir> <goldens-dir> <out-dir> [<tier>]
#
# <bin-dir> holds `dm`, `toolchain.txt` and `resources/` (the build cache). <goldens-dir> holds
# the published `hashes-m156.json` (+ `objects-m156.tar` for the mismatch analysis). For every
# tier of the build it renders `--src gm` (8888) and compares with the golden tier of the same
# name; with EXTRAS=1 it also renders `--src image` (8888 565 f16) and `--src lottie`.
# Results land in <out-dir>/<tier>/: hashes-gm.json, equivalence.{json,md}, hashes-image.json,
# hashes-lottie.json and the DM logs. With <tier>, only that tier runs. REQUIRE_AMD=1 (default)
# makes the script fail on a non-AMD CPU (except for ml4): the goldens come from a Zen 4 host, and Intel's
# `rcpps`/`rsqrtps` estimates differ from AMD's, which changes the baseline and ml3 code paths
# (design raster-pipeline.md §4.6). Re-run the failed job to get another runner.
set -uo pipefail

build=$1
bin=$(realpath "$2")
goldens=$(realpath "$3")
out=$(realpath -m "$4")
here=$(dirname "$(realpath "$0")")
ci="python3 -I $here/oracle_ci.py"
only=${5:-}
mkdir -p "$out"

# Tiers of each build: `<tier>:<SKIA_ORACLE_CPU_CAP>` (derived from oracle/tiers.toml the way
# `cargo xtask oracle tiers` does it).
case $build in
  x64-sse2-rgba)   tiers="cpu-x64-sse2-rgba:baseline cpu-x64-sse2-rgba-rt-ml3:ml3 cpu-x64-sse2-rgba-rt-ml4:ml4" ;;
  x64-sse41-rgba)  tiers="cpu-x64-sse41-rgba:ssse3" ;;
  x64-scalar-rgba) tiers="cpu-x64-scalar-rgba:baseline" ;;
  # BGRA-order builds (-DSK_R32_SHIFT=16 on Linux): the Windows N32, so they also reproduce the
  # 565 and f16 goldens and every tier of the BGRA goldens.
  x64-sse2)   tiers="cpu-x64-sse2:baseline cpu-x64-sse2-rt-ssse3:ssse3 cpu-x64-sse2-rt-ml3:ml3 cpu-x64-sse2-rt-ml4:ml4" ;;
  x64-sse41)  tiers="cpu-x64-sse41:ssse3 cpu-x64-sse41-rt-ml3:ml3 cpu-x64-sse41-rt-ml4:ml4" ;;
  x64-scalar) tiers="cpu-x64-scalar:baseline" ;;
  *) echo "unknown build $build"; exit 1 ;;
esac

if [[ $build == *-rgba ]]; then
  gm_configs=(8888); image_configs=(8888 565 f16)
else
  gm_configs=(8888 565 f16); image_configs=(8888)
fi

# What the runner CPU can run. ml3 needs AVX2 (+BMI/FMA/F16C), ml4 AVX-512 F/DQ/CD/BW/VL.
flags=$(grep -m1 '^flags' /proc/cpuinfo)
has() { for f in "$@"; do [[ " $flags " == *" $f "* ]] || return 1; done; }
{
  grep -m1 'model name' /proc/cpuinfo
  grep -m1 'vendor_id' /proc/cpuinfo
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
  [[ -z $only || $only == "$tier" ]] || continue
  tdir=$out/$tier
  mkdir -p "$tdir"
  case $cap in
    ml3) has avx2 bmi1 bmi2 fma f16c || { echo "::error::this runner lacks AVX2/FMA, cannot run $tier"; exit 3; } ;;
    ml4) has avx512f avx512dq avx512cd avx512bw avx512vl || { echo "::error::this runner lacks AVX-512, cannot run $tier; re-run the job"; exit 3; } ;;
  esac
  # ml4 uses the AVX-512 `rcp14`/`rsqrt14` estimates, which are specified exactly: an Intel Xeon
  # 8573C and an AMD EPYC 9V45 gave identical hashes for all 7,885 gm, image and lottie results.
  # The other tiers use `rcpps`/`rsqrtps`, which differ between vendors.
  if [[ $cap != ml4 && ${REQUIRE_AMD:-1} == 1 ]] && ! grep -q AuthenticAMD /proc/cpuinfo; then
    echo "::error::this runner is not an AMD CPU (see $out/cpu.txt); re-run the job to get another runner"
    exit 3
  fi
  # The CPU tier the DM metadata reports for this cap.
  case $cap in baseline|ssse3|ml3|ml4) cpu_tier=$cap ;; esac
  echo "::group::$tier (cap $cap): --src gm"
  rm -rf "$tdir/raw-gm"; mkdir -p "$tdir/raw-gm"   # DM only creates the levels below it
  SKIA_ORACLE_CPU_CAP=$cap ./dm "${dm_common[@]}" --oracleRawPath "$tdir/raw-gm" \
      --config "${gm_configs[@]}" --src gm > "$tdir/dm-gm.log" 2>&1
  echo "dm exit: $?" | tee -a "$tdir/dm-gm.log"
  tail -n 20 "$tdir/dm-gm.log"
  echo "::endgroup::"
  $ci hash "$tdir/raw-gm" "$tier" "$cpu_tier" "$bin/toolchain.txt" "$tdir/hashes-gm.json" || status=1
  $ci compare "$tdir/hashes-gm.json" "$goldens/hashes-m156.json" "$tier" "$tdir/raw-gm" \
      "$goldens/objects-m156.tar" "$tdir/equivalence" || status=1
  cat "$tdir/equivalence.md" >> "${GITHUB_STEP_SUMMARY:-/dev/null}" 2>/dev/null || true
  if [[ ${SHIM:-0} == 1 && -f $here/libm_shim.so ]]; then
    # Diagnostic: re-render only the mismatched GMs with float libm calls computed in double.
    mapfile -t pats < <($ci patterns "$tdir/equivalence.json")
    if (( ${#pats[@]} )); then
      mkdir -p "$tdir/raw-shim"
      LD_PRELOAD=$here/libm_shim.so SKIA_ORACLE_CPU_CAP=$cap ./dm "${dm_common[@]}" \
          --oracleRawPath "$tdir/raw-shim" --config "${gm_configs[@]}" --src gm --match "${pats[@]}" \
          > "$tdir/dm-shim.log" 2>&1
      $ci hash "$tdir/raw-shim" "$tier" "$cpu_tier" "$bin/toolchain.txt" "$tdir/hashes-shim.json" || true
      $ci compare "$tdir/hashes-shim.json" "$goldens/hashes-m156.json" "$tier" "$tdir/raw-shim" \
          "$goldens/objects-m156.tar" "$tdir/equivalence-shim" || true
      rm -rf "$tdir/raw-shim"
    fi
  fi
  rm -rf "$tdir/raw-gm"   # keep only hashes (each GM tier is ~1 GB raw)

  if [[ ${NANOBENCH:-0} == 1 && -x ./nanobench && ( $tier == cpu-x64-sse2-rgba || $tier == cpu-x64-sse2-rgba-rt-ml3 ) ]]; then
    # Bench goldens (bench.md B3): every bench once (--loops 1 --samples 1), twice; benches whose
    # two runs differ are reported and left out. Tiers: bench-x64-sse2-rgba[-rt-ml3].
    btier=bench-${tier#cpu-}
    echo "::group::$btier: nanobench"
    ./nanobench --dryRun --pre_log --config 8888 565 f16 --nativeFonts false --resourcePath resources \
        > "$tdir/bench-run-list.txt" 2>&1
    for pass in 1 2; do
      rm -rf "$tdir/raw-bench$pass"; mkdir -p "$tdir/raw-bench$pass"
      SKIA_ORACLE_CPU_CAP=$cap timeout 2400 ./nanobench --config 8888 565 f16 --loops 1 --samples 1 \
          --cpu true --gpu false --nativeFonts false --resourcePath resources -q \
          --oracleRawPath "$tdir/raw-bench$pass" > "$tdir/nanobench-$pass.log" 2>&1
      echo "nanobench exit: $?" | tee -a "$tdir/nanobench-$pass.log"
      $ci hash "$tdir/raw-bench$pass" "$btier" "$cpu_tier" "$bin/toolchain.txt" "$tdir/hashes-bench$pass.json" "${btier/-rgba/}" || status=1
    done
    $ci stable "$tdir/hashes-bench1.json" "$tdir/hashes-bench2.json" "$tdir/hashes-bench.json" | tee "$tdir/bench-stability.txt"
    rm -rf "$tdir"/raw-bench1 "$tdir"/raw-bench2
    echo "::endgroup::"
  fi

  if [[ ${EXTRAS:-0} == 1 ]]; then
    # 565 and f16 do not depend on the N32 byte order, so their hashes are filed under the BGRA
    # tier name (same file layout as goldens-m156); 8888 stays under the RGBA tier.
    free_tier=""
    [[ $build == *-rgba ]] && free_tier=${tier/-rgba/}
    for src in image lottie; do
      echo "::group::$tier (cap $cap): --src $src"
      rm -rf "$tdir/raw-$src"; mkdir -p "$tdir/raw-$src"
      if [[ $src == image ]]; then
        configs=("${image_configs[@]}"); extra=(--images resources/images)
      else
        configs=(${LOTTIE_CONFIGS:-8888}); extra=(--lotties resources/skottie)
      fi
      SKIA_ORACLE_CPU_CAP=$cap ./dm "${dm_common[@]}" --oracleRawPath "$tdir/raw-$src" \
          "${extra[@]}" --config "${configs[@]}" --src "$src" > "$tdir/dm-$src.log" 2>&1
      echo "dm exit: $?" | tee -a "$tdir/dm-$src.log"
      tail -n 20 "$tdir/dm-$src.log"
      echo "::endgroup::"
      if [[ -n $(ls -A "$tdir/raw-$src") ]]; then
        $ci hash "$tdir/raw-$src" "$tier" "$cpu_tier" "$bin/toolchain.txt" "$tdir/hashes-$src.json" "$free_tier" || status=1
      fi
      rm -rf "$tdir/raw-$src"
    done
  fi
done
exit $status
