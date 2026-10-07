# Builds draw.exe (the SkBitmapDevice / skcpu::Draw oracle, task D5) against oracle builds' static
# libraries (Windows, clang-cl + lld-link as the oracle uses), runs it on the case script
# crates/skia-rust-raster/src/draw_tests/cases.txt (regenerate it with gen_cases.py first) and
# writes one dump per CPU code path next to the cases:
#   skia_dump.txt         x64-sse2,   SKIA_ORACLE_CPU_CAP=baseline
#   skia_dump_ml3.txt     x64-sse2,   SKIA_ORACLE_CPU_CAP=ml3
#   skia_dump_ml4.txt     x64-sse2,   SKIA_ORACLE_CPU_CAP=ml4
#   skia_dump_sse41.txt   x64-sse41,  SKIA_ORACLE_CPU_CAP=baseline
#   skia_dump_scalar.txt  x64-scalar, SKIA_ORACLE_CPU_CAP=baseline
# Usage, from this directory:
#   ./build.ps1 -Skia ../../third_party/skia            # all five dumps
#   ./build.ps1 -Build x64-sse41                         # only the dumps of that build
# If clang-cl fails on long paths, run it from a short junction to the repository.
param(
    [string]$Skia = "../../third_party/skia",
    [string]$Build = "",
    [string]$Llvm = "C:/Program Files/LLVM/bin",
    [string]$Msvc = "C:/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/VC/Tools/MSVC/14.44.35207",
    [string]$Sdk = "C:/Program Files (x86)/Windows Kits/10",
    [string]$SdkVer = "10.0.26100.0"
)
$ErrorActionPreference = "Stop"
$skia = (Resolve-Path $Skia).Path
$dest = Join-Path $PSScriptRoot "../../crates/skia-rust-raster/src/draw_tests"

# (build, cpu cap, dump file)
$runs = @(
    @("x64-sse2", "baseline", "skia_dump.txt"),
    @("x64-sse2", "ml3", "skia_dump_ml3.txt"),
    @("x64-sse2", "ml4", "skia_dump_ml4.txt"),
    @("x64-sse41", "baseline", "skia_dump_sse41.txt"),
    @("x64-scalar", "baseline", "skia_dump_scalar.txt")
)
if ($Build) {
    $runs = @($runs | Where-Object { $_[0] -eq $Build })
    # Any other build (e.g. x64-sse2-rgba, to check that the output does not depend on the N32 byte
    # order) runs at the baseline cap into out/<build>/dump_baseline.txt only.
    if ($runs.Count -eq 0) { $runs = @(, @($Build, "baseline", $null)) }
}

function Build-Harness([string]$b) {
    $lib = Join-Path $skia "out/oracle/$b"
    $out = Join-Path $PSScriptRoot "out/$b"
    New-Item -ItemType Directory -Force $out | Out-Null
    # The oracle's compile flags (out/oracle/<build>/obj/dm.ninja) and the build's extra cflags
    # (oracle/tiers.toml).
    $extra = @()
    switch -Regex ($b) {
        "sse41" { $extra += "/clang:-msse4.1" }
        "sse42" { $extra += "/clang:-msse4.2" }
        "scalar" { $extra += "/clang:-DSKRP_CPU_SCALAR" }
        "rgba" { $extra += "/clang:-DSK_R32_SHIFT=0" }
    }
    & "$Llvm/clang-cl.exe" /nologo /c /std:c++20 /GR- /O2 /fp:precise /clang:-ffp-contract=off `
        -D_HAS_EXCEPTIONS=0 -DNDEBUG -DNOMINMAX -DWIN32_LEAN_AND_MEAN -DSK_ENABLE_AVX512_OPTS `
        -DSK_USE_PARTITION_ALLOC $extra `
        -imsvc "$Msvc/include" -imsvc "$Sdk/Include/$SdkVer/ucrt" -imsvc "$Sdk/Include/$SdkVer/um" `
        -imsvc "$Sdk/Include/$SdkVer/shared" -I "$skia" `
        -I "$skia/third_party/externals/partition_alloc/src" `
        -I "$lib/gen/third_party/externals/partition_alloc/src" `
        "$PSScriptRoot/draw.cpp" "/Fo$out/draw.obj"
    if ($LASTEXITCODE) { exit $LASTEXITCODE }

    $libs = Get-ChildItem "$lib/*.lib" | Where-Object { $_.Name -ne "dm.lib" } | ForEach-Object { $_.FullName }
    & "$Llvm/lld-link.exe" /nologo /SUBSYSTEM:CONSOLE /OPT:REF "/OUT:$out/draw.exe" "$out/draw.obj" `
        $libs Ole32.lib OleAut32.lib FontSub.lib User32.lib Usp10.lib winmm.lib Gdi32.lib DbgHelp.lib `
        Advapi32.lib "/LIBPATH:$Sdk/Lib/$SdkVer/ucrt/x64" "/LIBPATH:$Sdk/Lib/$SdkVer/um/x64" `
        "/LIBPATH:$Msvc/lib/x64"
    if ($LASTEXITCODE) { exit $LASTEXITCODE }
}

$built = @{}
foreach ($r in $runs) {
    $b = $r[0]
    if (-not $built.ContainsKey($b)) {
        Build-Harness $b
        $built[$b] = $true
    }
}

$cases = (Resolve-Path "$dest/cases.txt").Path
foreach ($r in $runs) {
    $b = $r[0]
    $out = Join-Path $PSScriptRoot "out/$b"
    $env:SKIA_ORACLE_CPU_CAP = $r[1]
    $raw = "$out/dump_$($r[1]).txt"
    $ErrorActionPreference = "Continue"
    & "$out/draw.exe" $cases 2> "$out/stderr_$($r[1]).txt" | Out-File -Encoding ascii $raw
    $ErrorActionPreference = "Stop"
    if ($LASTEXITCODE) {
        Get-Content "$out/stderr_$($r[1]).txt" | Write-Error
        exit $LASTEXITCODE
    }
    if (-not $r[2]) { Write-Host "$b/$($r[1]) -> $raw"; continue }
    # Normalize to LF line endings.
    [IO.File]::WriteAllText((Join-Path $dest $r[2]), ([IO.File]::ReadAllText($raw) -replace "`r`n", "`n"))
    Write-Host "$b/$($r[1]) -> $($r[2])"
}
$env:SKIA_ORACLE_CPU_CAP = $null
exit 0
