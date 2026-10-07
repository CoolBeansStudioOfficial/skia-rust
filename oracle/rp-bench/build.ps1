# Builds rp_bench.exe against an oracle build's static libraries (Windows, clang-cl + lld-link as
# the oracle uses). Usage, from this directory:
#   ./build.ps1 -Skia ../../third_party/skia -Build x64-sse2
#   $env:SKIA_ORACLE_CPU_CAP = "baseline"; ./out/x64-sse2/rp_bench.exe
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
    "$PSScriptRoot/rp_bench.cpp" "/Fo$out/rp_bench.obj"
if ($LASTEXITCODE) { exit $LASTEXITCODE }

$libs = Get-ChildItem "$lib/*.lib" | Where-Object { $_.Name -ne "dm.lib" } | ForEach-Object { $_.FullName }
& "$Llvm/lld-link.exe" /nologo /SUBSYSTEM:CONSOLE /OPT:REF "/OUT:$out/rp_bench.exe" "$out/rp_bench.obj" `
    $libs Ole32.lib OleAut32.lib FontSub.lib User32.lib Usp10.lib winmm.lib Gdi32.lib DbgHelp.lib `
    Advapi32.lib "/LIBPATH:$Sdk/Lib/$SdkVer/ucrt/x64" "/LIBPATH:$Sdk/Lib/$SdkVer/um/x64" `
    "/LIBPATH:$Msvc/lib/x64"
exit $LASTEXITCODE
