<#
Create the Ghidra projects and analysis tables from your own game files. Nothing is downloaded and nothing
third-party is needed besides Ghidra and 0xC0000054/sc4-ghidra-symbols (types and Function ID databases).

  ./tools/setup_ghidra.ps1 -Windows
      Imports bin/SimCity4.exe into ghidra/project/FreeCity4 (as "SimCity 4.exe"), runs auto-analysis with the
      Function ID databases attached, and exports the tables symport reads to ghidra/export/windows.
  ./tools/setup_ghidra.ps1 -Mac <path to the Mac Rev A Intel executable>
      Optional, only needed to re-run the symbol port or to read named Mac code. Imports the Mac binary (it carries
      debug symbols) into ghidra/mac/MacReference, exports its tables to ghidra/export/mac and its named
      pseudocode to decomp-mac/.

Both refuse to touch an existing project; pass -ProjectRoot to build somewhere else (for example to compare).
Afterwards run ./tools/build.ps1 to apply re/names_windows.tsv and export decomp/.
#>
param(
    [switch]$Windows,
    [string]$Mac,
    [string]$ProjectRoot,
    [switch]$SkipMacDecomp
)
$ErrorActionPreference = "Continue"
$root = Split-Path -Parent $PSScriptRoot
if (-not $ProjectRoot) { $ProjectRoot = Join-Path $root "ghidra" }
if (-not $env:JAVA_HOME) { $env:JAVA_HOME = if ((-not ($IsMacOS -or $IsLinux))) { "C:\Program Files\Java\jdk-26.0.1" } else { (& /usr/libexec/java_home 2>$null) } }
$ghidra = if ($env:GHIDRA_HOME) { "$($env:GHIDRA_HOME)/support/analyzeHeadless" + $(if ((-not ($IsMacOS -or $IsLinux))) { ".bat" } else { "" }) } elseif ((-not ($IsMacOS -or $IsLinux))) { "C:\tools\ghidra_12.1.4_PUBLIC\support\analyzeHeadless.bat" } else { "/opt/homebrew/opt/ghidra/libexec/support/analyzeHeadless" }
$sym = Join-Path $root "external/sc4-ghidra-symbols"
$scripts = Join-Path $root "ghidra/scripts"
$exportRoot = Join-Path $ProjectRoot "export"
New-Item -ItemType Directory -Force (Join-Path $root "ghidra/logs") | Out-Null

if (-not (Test-Path (Join-Path $sym "function-id"))) {
    Write-Output "Missing $sym. Clone https://github.com/0xC0000054/sc4-ghidra-symbols there first."
    exit 1
}

if ($Windows) {
    $exe = Join-Path $root "bin/SimCity4.exe"
    $want = "3BC5C7FE807FE5AA24A3ABB06E40F13486670BC1150612C09BAF4ECEDAFE1A4B"
    if (-not (Test-Path $exe)) { Write-Output "Copy your 'SimCity 4.exe' to $exe first."; exit 1 }
    if ((Get-FileHash $exe).Hash -ne $want) { Write-Output "WARNING: $exe is not the GOG 1.1.641 build ($want)." }
    $proj = Join-Path $ProjectRoot "project"
    if (Test-Path (Join-Path $proj "FreeCity4.gpr")) { Write-Output "$proj/FreeCity4.gpr exists; not overwriting."; exit 1 }
    New-Item -ItemType Directory -Force $proj | Out-Null
    # The project program must be named "SimCity 4.exe" (tools\build.ps1 processes it by that name).
    $stage = Join-Path $ProjectRoot "import"
    New-Item -ItemType Directory -Force $stage | Out-Null
    Copy-Item $exe (Join-Path $stage "SimCity 4.exe") -Force
    $env:MAXMEM = "12G"
    & $ghidra $proj FreeCity4 -import (Join-Path $stage "SimCity 4.exe") -scriptPath $scripts `
        -preScript FC4AttachFid.java (Join-Path $sym "function-id") `
        -postScript FC4DiscoverFunctions.java `
        -postScript FC4ExportTables.java (Join-Path $exportRoot "windows") *> (Join-Path $root "ghidra/logs/setup-windows.log")
    Remove-Item -Recurse -Force $stage
    Select-String (Join-Path $root "ghidra/logs/setup-windows.log") -Pattern "FC4AttachFid attached|FC4ExportTables wrote|ERROR" |
        Select-Object -First 20 | ForEach-Object { $_.Line }
}

if ($Mac) {
    if (-not (Test-Path $Mac)) { Write-Output "Mac executable not found: $Mac"; exit 1 }
    $proj = Join-Path $ProjectRoot "mac"
    if (Test-Path (Join-Path $proj "MacReference.gpr")) { Write-Output "$proj/MacReference.gpr exists; not overwriting."; exit 1 }
    New-Item -ItemType Directory -Force $proj | Out-Null
    Write-Output ("Mac executable SHA-256: " + (Get-FileHash $Mac).Hash)
    $env:MAXMEM = "12G"
    # A universal (fat) binary holds PowerPC and Intel slices; the Intel slice is the reference.
    & $ghidra $proj MacReference -import $Mac -loader MachoLoader -processor "x86:LE:32:default" -scriptPath $scripts `
        -postScript FC4ExportTables.java (Join-Path $exportRoot "mac") *> (Join-Path $root "ghidra/logs/setup-mac.log")
    Select-String (Join-Path $root "ghidra/logs/setup-mac.log") -Pattern "FC4ExportTables wrote|ERROR" |
        Select-Object -First 20 | ForEach-Object { $_.Line }
    if (-not $SkipMacDecomp) {
        $env:MAXMEM = "16G"
        & $ghidra $proj MacReference -process -noanalysis -readOnly -scriptPath $scripts `
            -postScript FC4ExportDecomp.java (Join-Path $root "decomp-mac") *> (Join-Path $root "ghidra/logs/export-mac.log")
        Select-String (Join-Path $root "ghidra/logs/export-mac.log") -Pattern "wrote " | ForEach-Object { $_.Line }
    }
}

if (-not $Windows -and -not $Mac) { Get-Help $PSCommandPath -Detailed | Out-String | Write-Output }
