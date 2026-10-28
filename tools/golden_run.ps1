<# Isolated golden run. Holds the queue's slot claim until the end save is copied.
   Never stages into Documents; the existing harness mirrors the selected region. #>
param(
    [Parameter(Mandatory=$true)][string]$HarnessRoot,
    [Parameter(Mandatory=$true)][string]$Dll,
    [Parameter(Mandatory=$true)][string]$IniFile,
    [Parameter(Mandatory=$true)][string]$Output,
    [Parameter(Mandatory=$true)][string]$City,
    [Parameter(Mandatory=$true)][string]$Until,
    [string]$Region = 'FC4 Test',
    [string]$SlotRoot = '',
    [ValidateRange(1,16)][int]$Slot = 6,
    [ValidateRange(30,3600)][int]$Seconds = 180
)
$ErrorActionPreference = 'Stop'
$runner = Join-Path $HarnessRoot 'tools/ingame_run.ps1'
Write-Output "golden runner: $runner"
$parameters = (Get-Command -Name $runner).Parameters
if ($Region -ne 'FC4 Test' -and -not $parameters.ContainsKey('Region')) {
    throw "The harness does not support -Region yet; cannot run '$Region' (no fallback to FC4 Test)."
}
if ($City -notmatch '^[A-Za-z0-9 ._()''-]+$' -or $Region -notmatch '^[A-Za-z0-9 ._()''-]+$') {
    throw 'City and region must be single folder/save names.'
}
$Output = [IO.Path]::GetFullPath($Output)
if (Test-Path -LiteralPath $Output) { throw "Run output already exists: $Output" }
if (-not $SlotRoot) { $SlotRoot = Join-Path $HarnessRoot 'build-runs/sc4slots' }
$slotRoot = $SlotRoot
$userDir = [IO.Path]::GetFullPath((Join-Path $slotRoot ("slot{0:D2}" -f $Slot)))
$docs = [IO.Path]::GetFullPath((Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'SimCity 4'))
if ($userDir -match '\\\.' -or $userDir.StartsWith($docs, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Unsafe slot directory: $userDir"
}
$claim = New-Object Threading.Mutex($false, "FC4QueueSlot-$Slot")
$held = $false
try {
    try { $held = $claim.WaitOne(0) } catch [Threading.AbandonedMutexException] { $held = $true }
    if (-not $held) { throw "Slot $Slot is claimed by the game queue; choose another --slot." }
    # The harness takes FC4UserDir itself; the queue claim prevents reuse during export.
    New-Item -ItemType Directory -Path $Output | Out-Null
    $label = 'golden-' + [Guid]::NewGuid().ToString('N')
    $argsRun = @{ Slot=$Slot; SlotRoot=$slotRoot; Dll=$Dll; City=$City; IniFile=$IniFile;
                  Label=$label; EndDate=$Until; AfterLoadSeconds=$Seconds; Seconds=($Seconds+150) }
    if ($parameters.ContainsKey('Region')) { $argsRun.Region = $Region }
    $watch = [Diagnostics.Stopwatch]::StartNew()
    # A child process isolates the harness's exit statements, Add-Type and mutex lifetime.
    $arguments = @('-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', $runner)
    foreach ($key in $argsRun.Keys) { $arguments += @("-$key", [string]$argsRun[$key]) }
    $lines = @(& powershell @arguments 2>&1 | ForEach-Object { "$_" })
    $code = $LASTEXITCODE
    $watch.Stop()
    $utf8 = New-Object Text.UTF8Encoding($false)
    [IO.File]::WriteAllText((Join-Path $Output 'harness.txt'), (($lines -join "`n") + "`n"), $utf8)
    $log = Join-Path $HarnessRoot "build-runs/$label.log"
    if (Test-Path -LiteralPath $log) { Copy-Item -LiteralPath $log -Destination (Join-Path $Output 'game.log') }
    $save = Join-Path $userDir "Regions/FC4 Test/City - $City.sc4"
    if (Test-Path -LiteralPath $save) { Copy-Item -LiteralPath $save -Destination (Join-Path $Output 'end.sc4') }
    $meta = @{ slot=$Slot; label=$label; exit_code=$code; wall_seconds=$watch.Elapsed.TotalSeconds;
               region=$Region; city=$City; save_source=$save }
    [IO.File]::WriteAllText((Join-Path $Output 'run.json'), (($meta | ConvertTo-Json) + "`n"), $utf8)
    Write-Output ("golden harness: slot {0}, exit {1}, wall {2:N3}s, output {3}" -f $Slot,$code,$watch.Elapsed.TotalSeconds,$Output)
    if ($code -ne 0) { exit $code }
} finally {
    if ($held) { $claim.ReleaseMutex() }
    $claim.Dispose()
}
