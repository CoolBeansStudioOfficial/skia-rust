# Builds d4_blitters.exe against an oracle build's static libraries (Windows, clang-cl + lld-link
# as the oracle uses), runs it and writes the expected output of the legacy blitter tests
# (crates/skia-rust-raster/src/skia_d4_dump.txt and the ml3/ml4 variants; the `full` run, with
# pixels, goes to out/<build>/d4_full.txt).
# Usage, from this directory:
#   ./build-d4.ps1 -Skia ../../third_party/skia -Build x64-sse2
param(
    [string]$Skia = "../../third_party/skia",
    [string]$Build = "x64-sse2",
    [string]$Llvm = "C:/Program Files/LLVM/bin",
    [string]$Msvc = "C:/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/VC/Tools/MSVC/14.44.35207",
    [string]$Sdk = "C:/Program Files (x86)/Windows Kits/10",
    [string]$SdkVer = "10.0.26100.0"
)
$ErrorActionPreference = "Stop"
$skia = (Resolve-Path $Skia).Path
$lib = Join-Path $skia "out/oracle/$Build"
$out = Join-Path $PSScriptRoot "out/$Build"
New-Item -ItemType Directory -Force $out | Out-Null

# The oracle's compile flags (out/oracle/<build>/obj/dm.ninja) and the build's extra cflags.
$extra = @()
switch -Regex ($Build) {
    "sse41" { $extra += "/clang:-msse4.1" }
    "sse42" { $extra += "/clang:-msse4.2" }
    "avx$" { $extra += "/arch:AVX" }
    "v3" { $extra += "/arch:AVX2" }
    "v4" { $extra += "/arch:AVX512" }
}
& "$Llvm/clang-cl.exe" /nologo /c /std:c++20 /GR- /O2 /fp:precise /clang:-ffp-contract=off `
    -D_HAS_EXCEPTIONS=0 -DNDEBUG -DNOMINMAX -DWIN32_LEAN_AND_MEAN -DSK_ENABLE_AVX512_OPTS `
    -DSK_USE_PARTITION_ALLOC $extra `
    -imsvc "$Msvc/include" -imsvc "$Sdk/Include/$SdkVer/ucrt" -imsvc "$Sdk/Include/$SdkVer/um" `
    -imsvc "$Sdk/Include/$SdkVer/shared" -I "$skia" `
    -I "$skia/third_party/externals/partition_alloc/src" `
    -I "$lib/gen/third_party/externals/partition_alloc/src" `
    "$PSScriptRoot/d4_blitters.cpp" "/Fo$out/d4_blitters.obj"
if ($LASTEXITCODE) { exit $LASTEXITCODE }

$libs = Get-ChildItem "$lib/*.lib" | Where-Object { $_.Name -ne "dm.lib" } | ForEach-Object { $_.FullName }
& "$Llvm/lld-link.exe" /nologo /SUBSYSTEM:CONSOLE /OPT:REF "/OUT:$out/d4_blitters.exe" "$out/d4_blitters.obj" `
    $libs Ole32.lib OleAut32.lib FontSub.lib User32.lib Usp10.lib winmm.lib Gdi32.lib DbgHelp.lib `
    Advapi32.lib "/LIBPATH:$Sdk/Lib/$SdkVer/ucrt/x64" "/LIBPATH:$Sdk/Lib/$SdkVer/um/x64" `
    "/LIBPATH:$Msvc/lib/x64"
if ($LASTEXITCODE) { exit $LASTEXITCODE }

# The baseline tier's output is the main dump; the ml3 and ml4 runtime caps give the pipeline
# sprite lines of those tiers (their highp stages differ).
$dest = Join-Path $PSScriptRoot "../../crates/skia-rust-raster/src"
$ErrorActionPreference = "Continue"
foreach ($cap in @(@("baseline", "skia_d4_dump.txt"), @("ml3", "skia_d4_dump_ml3.txt"), @("ml4", "skia_d4_dump_ml4.txt"))) {
    $env:SKIA_ORACLE_CPU_CAP = $cap[0]
    & "$out/d4_blitters.exe" 2> "$out/d4_stderr.txt" | Out-File -Encoding ascii "$out/d4_$($cap[0]).txt"
    if ($LASTEXITCODE) { exit $LASTEXITCODE }
    [IO.File]::WriteAllText("$dest/$($cap[1])", ([IO.File]::ReadAllText("$out/d4_$($cap[0]).txt") -replace "`r`n", "`n"))
}
$env:SKIA_ORACLE_CPU_CAP = "baseline"
& "$out/d4_blitters.exe" full 2> "$out/d4_stderr.txt" | Out-File -Encoding ascii "$out/d4_full.txt"
exit 0
