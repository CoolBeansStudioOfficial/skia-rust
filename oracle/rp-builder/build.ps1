# Builds rp_builder.exe against an oracle build's static libraries (Windows, clang-cl + lld-link as
# the oracle uses), runs it and writes the expected outputs of the builder tests
# (crates/skia-rust-core/src/raster_pipeline/skia_dump.txt and skia_rp_dump.txt).
# Usage, from this directory:
#   ./build.ps1 -Skia ../../third_party/skia -Build x64-sse2
# The D3 destination hashes of the other tiers (skia_d3_pixels_<tier>.txt) come from
#   ./build.ps1 -Build x64-sse2 -Cap ml3 -Tier ml3      (also ml4)
#   ./build.ps1 -Build x64-sse41 -Cap ssse3 -Tier sse41
param(
    [string]$Skia = "../../third_party/skia",
    [string]$Build = "x64-sse2",
    [string]$Cap = "baseline",
    [string]$Tier = "",
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
    "$PSScriptRoot/rp_builder.cpp" "/Fo$out/rp_builder.obj"
if ($LASTEXITCODE) { exit $LASTEXITCODE }

$libs = Get-ChildItem "$lib/*.lib" | Where-Object { $_.Name -ne "dm.lib" } | ForEach-Object { $_.FullName }
& "$Llvm/lld-link.exe" /nologo /SUBSYSTEM:CONSOLE /OPT:REF "/OUT:$out/rp_builder.exe" "$out/rp_builder.obj" `
    $libs Ole32.lib OleAut32.lib FontSub.lib User32.lib Usp10.lib winmm.lib Gdi32.lib DbgHelp.lib `
    Advapi32.lib "/LIBPATH:$Sdk/Lib/$SdkVer/ucrt/x64" "/LIBPATH:$Sdk/Lib/$SdkVer/um/x64" `
    "/LIBPATH:$Msvc/lib/x64"
if ($LASTEXITCODE) { exit $LASTEXITCODE }

# Another tier's D3 destination hashes only.
if ($Tier) {
    $dest3 = Join-Path $PSScriptRoot "../../crates/skia-rust-raster/src/rp_blitter_oracle"
    New-Item -ItemType Directory -Force $dest3 | Out-Null
    $env:SKIA_ORACLE_CPU_CAP = $Cap
    Remove-Item Env:SKIA_ORACLE_RP_DUMP -ErrorAction SilentlyContinue
    $ErrorActionPreference = "Continue"
    & "$out/rp_builder.exe" d3 2> "$out/d3_dump_$Tier.txt"
    if ($LASTEXITCODE) { exit $LASTEXITCODE }
    [IO.File]::WriteAllText("$dest3/skia_d3_pixels_$Tier.txt", ([IO.File]::ReadAllText("$out/d3_dump_$Tier.txt") -replace "`r`n", "`n"))
    exit 0
}

# Run on the baseline tier (no ml3/ml4 upgrade); dump() goes to stderr (SkDebugf).
$dest = Join-Path $PSScriptRoot "../../crates/skia-rust-core/src/raster_pipeline"
$rpDump = Join-Path $out "rp_dump.txt"
Remove-Item -ErrorAction SilentlyContinue $rpDump
$env:SKIA_ORACLE_CPU_CAP = "baseline"
$env:SKIA_ORACLE_RP_DUMP = $rpDump
$ErrorActionPreference = "Continue"
& "$out/rp_builder.exe" 2> "$out/dump.txt"
if ($LASTEXITCODE) { exit $LASTEXITCODE }
# Normalize to LF line endings.
[IO.File]::WriteAllText("$dest/skia_dump.txt", ([IO.File]::ReadAllText("$out/dump.txt") -replace "`r`n", "`n"))
[IO.File]::WriteAllText("$dest/skia_rp_dump.txt", ([IO.File]::ReadAllText($rpDump) -replace "`r`n", "`n"))

# The D2 cases (blenders, color/empty shaders, MatrixRec): skia_d2_dump.txt, skia_d2_rp_dump.txt.
$rpDump = Join-Path $out "d2_rp_dump.txt"
Remove-Item -ErrorAction SilentlyContinue $rpDump
$env:SKIA_ORACLE_RP_DUMP = $rpDump
& "$out/rp_builder.exe" d2 2> "$out/d2_dump.txt"
if ($LASTEXITCODE) { exit $LASTEXITCODE }
[IO.File]::WriteAllText("$dest/skia_d2_dump.txt", ([IO.File]::ReadAllText("$out/d2_dump.txt") -replace "`r`n", "`n"))
[IO.File]::WriteAllText("$dest/skia_d2_rp_dump.txt", ([IO.File]::ReadAllText($rpDump) -replace "`r`n", "`n"))
# The D3 cases (SkRasterPipelineBlitter): skia_d3_pixels.txt (destination hashes after each blit),
# skia_d3_rp_dump.txt (the oracle's compile records, tagged <case>/<step>).
$dest3 = Join-Path $PSScriptRoot "../../crates/skia-rust-raster/src/rp_blitter_oracle"
New-Item -ItemType Directory -Force $dest3 | Out-Null
$rpDump = Join-Path $out "d3_rp_dump.txt"
Remove-Item -ErrorAction SilentlyContinue $rpDump
$env:SKIA_ORACLE_RP_DUMP = $rpDump
& "$out/rp_builder.exe" d3 2> "$out/d3_dump.txt"
if ($LASTEXITCODE) { exit $LASTEXITCODE }
[IO.File]::WriteAllText("$dest3/skia_d3_pixels.txt", ([IO.File]::ReadAllText("$out/d3_dump.txt") -replace "`r`n", "`n"))
[IO.File]::WriteAllText("$dest3/skia_d3_rp_dump.txt", ([IO.File]::ReadAllText($rpDump) -replace "`r`n", "`n"))
exit 0
