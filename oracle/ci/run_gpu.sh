#!/usr/bin/env bash
# Renders the Graphite+Dawn GM suite on a software adapter in CI, see oracle/README.md,
# "CI-hosted oracle (GPU)".
#
#   run_gpu.sh <bin-dir> <goldens-dir> <out-dir>
#
# Linux (default): tier `gpu-dawn-vk-lavapipe`, `--config grdawn_vk`, adapter `llvmpipe`.
# Windows (GPU_KIND=windows, Git Bash): tier `gpu-dawn-d3d12-warp`, `--config grdawn_d3d12`,
# adapter `Microsoft Basic Render Driver` (WARP).
# Both run `dm --src gm` with SKIA_ORACLE_CPU_CAP=ml3 (the cap of the published GPU tiers) TWICE, to
# measure determinism. Hashes land in <out-dir>/hashes-gm.json; the comparisons with the published
# GPU tiers in <out-dir>/vs-<tier>.{md,json}.
set -uo pipefail

bin=$(realpath "$1")
goldens=$(realpath "$2")
out=$(realpath -m "$3")
here=$(dirname "$(realpath "$0")")
ci="python3 -I $here/oracle_ci.py"
kind=${GPU_KIND:-linux}
mkdir -p "$out"

if [[ $kind == windows ]]; then
  tier=gpu-dawn-d3d12-warp; config=grdawn_d3d12; adapter="Microsoft Basic Render Driver"; exe=./dm.exe
  golds="gpu-dawn-d3d12-warp:grdawn_d3d12"
  powershell -NoProfile -Command "Get-CimInstance Win32_Processor | Select-Object Name,Manufacturer | Format-List; (Get-Item C:\Windows\System32\d3d10warp.dll).VersionInfo.FileVersion" | tee "$out/cpu.txt"
else
  tier=gpu-dawn-vk-lavapipe; config=grdawn_vk; adapter=llvmpipe; exe=./dm
  golds="gpu-dawn-vk-rtx4070s:grdawn_vk gpu-dawn-d3d12-warp:grdawn_d3d12"
  flags=$(grep -m1 '^flags' /proc/cpuinfo)
  {
    grep -m1 'model name' /proc/cpuinfo
    grep -m1 'vendor_id' /proc/cpuinfo
    echo "flags: $flags"
  } | tee "$out/cpu.txt"
  for f in avx2 bmi1 bmi2 fma f16c; do
    [[ " $flags " == *" $f "* ]] || { echo "::error::this runner lacks $f (ml3)"; exit 3; }
  done
  icd=$(ls /usr/share/vulkan/icd.d/lvp_icd*.json 2>/dev/null | head -n 1)
  [[ -n $icd ]] || { echo "::error::no lavapipe ICD found"; exit 3; }
  export VK_ICD_FILENAMES=$icd
  {
    echo "icd: $icd"
    dpkg-query -W -f='${Package} ${Version}\n' mesa-vulkan-drivers 'libllvm*' 2>/dev/null
    vulkaninfo --summary 2>&1 | head -n 40
  } | tee "$out/vulkan.txt"
fi

cd "$bin"
status=0
for pass in 1 2; do
  raw=$out/raw-$pass
  rm -rf "$raw"; mkdir -p "$raw"
  echo "::group::$tier pass $pass"
  SKIA_ORACLE_CPU_CAP=ml3 SKIA_ORACLE_DAWN_ADAPTER="$adapter" $exe --resourcePath resources \
      --nativeFonts false --cpu false --gpu false --graphite true --oracleRawPath "$raw" \
      --config "$config" --src gm > "$out/dm-gm-$pass.log" 2>&1
  echo "dm exit: $?" | tee -a "$out/dm-gm-$pass.log"
  tail -n 25 "$out/dm-gm-$pass.log"
  echo "::endgroup::"
  $ci hash "$raw" "$tier" ml3 "$bin/toolchain.txt" "$out/hashes-gm-$pass.json" || status=1
done
$ci stable "$out/hashes-gm-1.json" "$out/hashes-gm-2.json" "$out/hashes-gm-stable.json" | tee "$out/determinism.txt"
cp "$out/hashes-gm-1.json" "$out/hashes-gm.json"

for spec in $golds; do
  gold=${spec%%:*}; gcfg=${spec##*:}
  COMPARE_CONFIG_MAP=$config:$gcfg $ci compare "$out/hashes-gm.json" "$goldens/hashes-m156.json" "$gold" "$out/raw-1" \
      "$goldens/objects-m156.tar" "$out/vs-$gold" || true
  cat "$out/vs-$gold.md" >> "${GITHUB_STEP_SUMMARY:-/dev/null}" 2>/dev/null || true
done
rm -rf "$out"/raw-1 "$out"/raw-2
exit $status
