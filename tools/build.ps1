# FreeCity4 symbol-port + decompile pipeline.
#   .\tools\build.ps1            # match, merge, apply names in Ghidra, export decompilation
#   .\tools\build.ps1 -SkipExport
#   .\tools\build.ps1 -SkipMatch     # apply the committed re\names_windows.tsv without re-running the symbol port
#   .\tools\build.ps1 -SkipMatch -ProjectRoot <dir> [-DecompOut <dir>]
#       Verify a project built elsewhere (e.g. a scratch project from setup_ghidra.ps1 -ProjectRoot) without
#       disturbing ghidra\project. -DecompOut sends the decompilation somewhere other than decomp\.
# Prereqs: Ghidra 12.1.x at C:\tools\ghidra_12.1.4_PUBLIC, JDK 21+, Python with pefile + capstone,
# external\sc4-ghidra-symbols, and the projects and tables from tools\setup_ghidra.ps1 (-Mac too, unless -SkipMatch).
param([switch]$SkipExport, [switch]$Refeature, [switch]$SkipMatch, [string]$ProjectRoot, [string]$DecompOut)
# native tools (Ghidra/Java) write warnings to stderr; do not treat them as fatal
$ErrorActionPreference = "Continue"
$root = Split-Path -Parent $PSScriptRoot
if (-not $env:JAVA_HOME) { $env:JAVA_HOME = if ((-not ($IsMacOS -or $IsLinux))) { "C:\Program Files\Java\jdk-26.0.1" } else { (& /usr/libexec/java_home 2>$null) } }
$ghidra = if ($env:GHIDRA_HOME) { "$($env:GHIDRA_HOME)/support/analyzeHeadless" + $(if ((-not ($IsMacOS -or $IsLinux))) { ".bat" } else { "" }) } elseif ((-not ($IsMacOS -or $IsLinux))) { "C:\tools\ghidra_12.1.4_PUBLIC\support\analyzeHeadless.bat" } else { "/opt/homebrew/opt/ghidra/libexec/support/analyzeHeadless" }
$gdt = Join-Path $root "external/sc4-ghidra-symbols"
# Ghidra's project location and name are separate arguments; -ProjectRoot points them elsewhere.
if ($ProjectRoot) {
    $projectDir = $ProjectRoot
    if (-not (Test-Path (Join-Path $projectDir "FreeCity4.gpr"))) {
        throw "no FreeCity4.gpr under $projectDir - run tools/setup_ghidra.ps1 -ProjectRoot `"$projectDir`" first"
    }
} else {
    $projectDir = "ghidra/project"
}

Push-Location $root
try {
    if (-not $SkipMatch) {
        if ($Refeature -or -not (Test-Path re/symport/win_features.pkl)) { python tools/symport/features.py }
        if ($Refeature -or -not (Test-Path re/symport/vtables.pkl)) { python tools/symport/vtables.py }
        python tools/symport/match.py
        python tools/symport/merge_names.py
    }

    $env:MAXMEM = "12G"
    & $ghidra $projectDir FreeCity4 -process "SimCity 4.exe*" -noanalysis -scriptPath ghidra/scripts `
        -postScript FC4ApplyNames.java (Resolve-Path re/names_windows.tsv) "$gdt/SimCity4.gdt" "$gdt/Lua5.0.gdt" `
        "$gdt/Gimex.gdt" "$gdt/DirectX7.gdt" `
        -postScript FC4ApplyLabels.java (Resolve-Path re/labels_windows.tsv) `
        -postScript FC4FillStructs.java `
        -postScript FC4FieldNames.java (Resolve-Path re/fields_windows.tsv) *> ghidra/logs/apply.log
    Select-String ghidra/logs/apply.log -Pattern "renamed=|created=|missing=|labels=|classes=|fieldsRenamed=" | ForEach-Object { $_.Line }

    if (-not $SkipExport) {
        $decompOut = if ($DecompOut) { $DecompOut } else { Join-Path $root "decomp" }
        if (Test-Path $decompOut) { Remove-Item -Recurse -Force $decompOut }
        New-Item -ItemType Directory -Force -Path $decompOut | Out-Null
        $env:MAXMEM = "16G"
        & $ghidra $projectDir FreeCity4 -process "SimCity 4.exe*" -noanalysis -readOnly -scriptPath ghidra/scripts `
            -postScript FC4ExportDecomp.java $decompOut *> ghidra/logs/export.log
        Select-String ghidra/logs/export.log -Pattern "wrote " | ForEach-Object { $_.Line }
    }
}
finally { Pop-Location }
