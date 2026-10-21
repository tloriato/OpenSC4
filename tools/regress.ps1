<#
In-game regression suite (coordinator tooling): runs the staged FreeCity4.dll through a fixed set of unattended
runs on the test city and prints one table. Use it after staging a new DLL.

Runs (each through tools\ingame_run.ps1 with popup_dialogs = 0 and autotest_end = quit, so the full Shutdown path
is part of every verdict):
  shadows  every same-process shadow at once, on the original subsystems: budget (needs disable = budget), demand,
           pollution, protection, crime, aura, flora, landfill, power, plumbing, land value, flammability,
           residential, neighbours, zones, development, tract, zone developer (zonedev), lot developer (lotdev, exact
           since apply13 applied LD29; added in run 132), history warehouse (history),
           24-hour clock (clock; under turbo the clock steps rarely, CK22), commercial (CO$$/CO$$$ map rebuilds),
           civic buildings (civic), parks, traffic (traffic_shadow_hypotheses = 7, the alternative readings of
           TS55-TS57; traffic_shadow_lines = 1 keeps one day line per month next to the month line) and terrain
           (terrain_shadow = 1, glue16; added in run 138: 105/105 lines and every query exact to 2032, 0 unfollowed; see "Terrain" below) and the automata layer (automata_shadow = 1,
           glue19; added in the fix round: the six saved components byte-identical at every city load and month —
           the glue19 runs). disasters_shadow stays out (it needs
           scripted disasters and is not exact, glue13). A shadow line is
           clean when every mismatch count on it is 0. Known difference: the traffic shadow is not exact to 2032 (run
           121: 43 of 72 month lines clean, 3066 of 3165 deliveries, 0 incomplete; 49 of 72 alone at 4.5 days/s; the
           scanning iterator after state 13 (TS64), the original's stop part-way through state 15 (TS63) and rare
           trip-list differences (a reversible trip's return route, TS58)), and its count moves with the pacing, so
           read its column against the baseline, not as 72/72. Shadow mismatches never fail the verdict (only
           INCOMPLETE lines count as problems; a shadow with more mismatching lines than the baseline is a "worse"
           line of the delta). The traffic shadow makes this run the long one (2.5-3.5 game days/s, 10-15 minutes).
  replace  every merged replacement at once (-Replace overrides the list; 27 keys: the 20 of run 114
           plus commercial, industrial, civic, parks, lotdev and zonedev (run 118) and traffic (run 121). With development (our development unit runs about 3x
           slower per game day than the original: 23-29 game days/s under turbo against 80-90) the replace run takes
           longest.

Usage:
  powershell -File tools\regress.ps1 -Tag r1 [-EndDate 2032-01-01] [-Only shadows|replace] [-Replace "landvalue, weather"]
           [-City <path to .sc4>] [-Dll <path to FreeCity4.dll>] [-Parallel | -Queue [-QueueWait 3600]]
           [-NormalSpeed [-Seconds 150]] [-Baseline <tag or .out path>]

-Dll stages that DLL (and the .pdb next to it, if any) as Plugins\FreeCity4.dll first; it refuses while the game
runs. Every run prints the staged DLL's SHA-256 prefix, so the record names the build it tested.
-Parallel runs the two runs at the same time in game slots 1 and 2 (ingame_run -Slot; each slot gets the DLL, -Dll
or the one staged in Documents; Documents is not touched). Same table and verdict line. Use it while the run queue
is idle (a slot the queue holds blocks it after 60 s).
-Queue submits the runs to the run queue (tools\game_queue.ps1) as requests of the main checkout (the coordinator's
root: build-runs\game-queue\regress-<Tag>-<run>.req, with priority = high when the running queue supports it, so they
go before every waiting lane request and run at the same time, each in the next free slot), waits for their verdict
files, copies their logs to build-runs\regress-<Tag>-<run>.log and prints the same table, delta and verdict line. The
DLL (-Dll, else the one staged in Documents) must be inside the main checkout for the queue (e.g. build-runs\dlls\);
one outside it is copied to build-runs\dlls\regress-<Tag>.dll (with its .pdb) first. -City must be a city of the FC4
Test region in Documents. -QueueWait (default 3600 s): how long a request may wait for a slot before regress takes it
back and reports it BLOCKED. A queue older than the priority change runs the two requests one after the other.
Profiles. The default (since main c367684 made the power shadow exact under turbo) is the turbo profile: each run
goes to the game date -EndDate (default 2032-01-01, six game years on Konradshohe) with our simulator's turbo clock
(sim_turbo = 1; a plugin from 0786064 on starts it after the city's first ConnectionsReady, so popups are already
off), so every run gives the same month and shadow lines. -TurboFrom <date> starts it with a script action at that
game date instead (for older DLLs). -TurboTimeout (default 1800 s since the traffic shadow joined the shadows run:
that run took 618 s and 874 s to 2032 in run 121's two attempts, the traffic shadow alone 479 s) is the timeout. -Turbo is accepted and changes
nothing. -NormalSpeed: the old timed profile, -Seconds (default 150) of real time after the load at normal speed
(an explicit -Seconds without -Turbo selects it too).

Logs: build-runs\regress-<Tag>-<run>.log (ini: .ini). A summary per run is printed by tools\run_summary.ps1; the
table at the end reads: verdict, problem count (foreign exceptions, stalls, reference warnings, panics, structured
exceptions, INCOMPLETE shadow lines, terrain unfollowed vertices), the terrain column and per shadow "clean/lines".

Terrain. Besides its lines ("terrain N/N" among the shadows), the terrain shadow counts vertices: run_summary reads
its city shutdown line into "terrain queries" (exact/total per query: altitude, normal, water, land, depth, type,
navigable) and "terrain vertices" (unfollowed, followed late, pending at shutdown). The table's terrain column shows
"7/7 exact" (or the queries that are not, with their counts) and "unf U, late L, pend P". Terrain messages are queued
posts: under turbo they arrive up to ~40 game days after the change, so the month re-read finds some vertices ahead
of their message. Followed late (the message came later) and pending (its message had not come at shutdown) are
exact; unfollowed (no message covered the change by the next month) is a problem and counts in the problem column.

Delta: after the table, one line per run verdict, problem count and shadow against an earlier regress, read from
its build-runs\regress-<tag>.out (the coordinator's redirected output): -Baseline <tag> or a path; default the newest
earlier regress-*.out with a PASS line and the same pacing. A shadow is "worse" when both the number and the share of
its lines with a mismatch grew ("better" when both fell; a timed run's line count varies). The terrain totals get one
line per run: a query is worse by the same rule, and more unfollowed vertices is worse. Any worse line adds
"REGRESS <tag>: N worse (vs <baseline>)" after the verdict line.

Pacing. With the original simulators a city run at about 85 game days per real second (turbo) ends with other income
and funds than the same city at 8-20 days/s: some original work is bound to frames or real time. Shadow lines are
unaffected (both sides run in one process at one pacing), but a comparison across runs is only meaningful at the same
pacing. The "profile:" line records it (turbo to <end date>, or normal with its seconds; an .out without a profile
line is normal), the table's days/s column the measured speed of each run. The delta refuses a baseline of the other
profile (-AnyPacing compares anyway, with a warning) and warns when the end dates differ or a run's measured days/s
differ by more than 2x (machine load slows turbo runs).
#>
param(
    [Parameter(Mandatory = $true)][string]$Tag,
    [int]$Seconds = 150,
    [ValidateSet("", "shadows", "replace")][string]$Only = "",
    [string]$Replace = "landvalue, weather, ordinance, demand, power, plumbing, flammability, pollution, protection, crime, aura, flora, landfill, residential, neighbors, zones, development, tract, history, clock24, commercial, industrial, civic, parks, lotdev, zonedev, traffic, automata, netmgr, netlot, nettool, demolition, occmgr, propmgr, lotcfg",
    [string]$City = "C:\Users\tlori\Documents\SimCity 4\Regions\FC4 Test\City - Konradshohe.sc4",
    [string]$Dll = "",
    [switch]$Parallel,
    [switch]$Queue,
    [int]$QueueWait = 3600,
    [switch]$Turbo,
    [switch]$NormalSpeed,
    [string]$EndDate = "2032-01-01",
    [string]$TurboFrom = "",
    [int]$TurboTimeout = 1800,
    [string]$Baseline = "",
    [switch]$AnyPacing
)
$ErrorActionPreference = "Continue"
$wallStart = Get-Date
$Plugins = Join-Path $env:USERPROFILE "Documents\SimCity 4\Plugins"
if ($Dll -and -not (Test-Path -LiteralPath $Dll)) { Write-Output "REGRESS: no such DLL: $Dll"; exit 2 }
if ($Queue -and $Parallel) { Write-Output "REGRESS: give -Queue or -Parallel, not both"; exit 2 }
if ($Turbo -and $NormalSpeed) { Write-Output "REGRESS: give -Turbo (the default) or -NormalSpeed, not both"; exit 2 }
if ($NormalSpeed -and ($PSBoundParameters.ContainsKey("EndDate") -or $TurboFrom)) { Write-Output "REGRESS: -EndDate and -TurboFrom belong to the turbo profile, not -NormalSpeed"; exit 2 }
if (-not $NormalSpeed -and -not $Turbo -and $PSBoundParameters.ContainsKey("Seconds")) { $NormalSpeed = [switch]$true; Write-Output "note: -Seconds without -Turbo: the normal-speed profile (-NormalSpeed)" }
$Turbo = -not $NormalSpeed   # the turbo profile is the default
if ($Tag -notmatch '^[A-Za-z0-9][A-Za-z0-9._-]{0,48}$') { Write-Output "REGRESS: -Tag: letters, digits, '.', '_', '-' (it names files and queue labels)"; exit 2 }
function MainCheckout {
    $here = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
    $i = $here.IndexOf("\.claude\worktrees\", [StringComparison]::OrdinalIgnoreCase)
    if ($i -gt 0) { return $here.Substring(0, $i) }
    return $here.TrimEnd('\')
}
$queuePrio = $false
if ($Queue) {
    # The queue's root is the main checkout: the DLL must be inside it, the city is given by name.
    $Main = MainCheckout
    $src = if ($Dll) { (Resolve-Path -LiteralPath $Dll).Path } else { Join-Path $Plugins "FreeCity4.dll" }
    if (-not (Test-Path -LiteralPath $src)) { Write-Output "REGRESS: no such DLL: $src"; exit 2 }
    if ($src.StartsWith($Main + '\', [StringComparison]::OrdinalIgnoreCase)) { $staged = $src }
    else {
        $dd = Join-Path $Main "build-runs\dlls"
        New-Item -ItemType Directory -Force $dd | Out-Null
        $staged = Join-Path $dd "regress-$Tag.dll"
        Copy-Item -LiteralPath $src -Destination $staged -Force
        $pdb = [IO.Path]::ChangeExtension($src, ".pdb")
        $stagedPdb = [IO.Path]::ChangeExtension($staged, ".pdb")
        if (Test-Path -LiteralPath $pdb) { Copy-Item -LiteralPath $pdb -Destination $stagedPdb -Force }
        elseif (Test-Path -LiteralPath $stagedPdb) { Remove-Item -LiteralPath $stagedPdb -Force }
        Write-Output "queue: copied $src to $staged (the queue takes a DLL from inside the main checkout)"
    }
    $region = Join-Path ([Environment]::GetFolderPath("MyDocuments")) "SimCity 4\Regions\FC4 Test"
    $cityFull = [IO.Path]::GetFullPath($City)
    if (-not [string]::Equals([IO.Path]::GetDirectoryName($cityFull), $region, [StringComparison]::OrdinalIgnoreCase) -or
        [IO.Path]::GetFileName($cityFull) -notmatch '^City - (.+)\.sc4$') { Write-Output "REGRESS: -Queue runs a city of the FC4 Test region by name; $City is not one"; exit 2 }
    $cityName = $Matches[1]
    $qm = $null
    $queueUp = $false
    try { $queueUp = [System.Threading.Mutex]::TryOpenExisting("FC4GameQueue", [ref]$qm); if ($qm) { $qm.Dispose() } } catch { $queueUp = $true }
    if (-not $queueUp) { Write-Output "REGRESS: no game_queue is running (start it from the main checkout, or use -Parallel)"; exit 2 }
    # The running queue's start line says whether it knows priority = high (older queues reject unknown keys).
    $qlog = Join-Path $Main "build-runs\game-queue.log"
    $startLine = if (Test-Path -LiteralPath $qlog) { Get-Content -LiteralPath $qlog | Where-Object { $_ -match ' game_queue: (\d+ slot|slots )' } | Select-Object -Last 1 } else { $null }
    $queuePrio = [bool]($startLine -and $startLine -match 'priority:')
}
if ($Dll -and -not $Parallel -and -not $Queue) {
    # Only a game on the Documents user dir blocks staging (slot games use their own Plugins).
    $docsGames = @(Get-CimInstance Win32_Process -Filter "Name='SimCity 4.exe'" -ErrorAction SilentlyContinue | Where-Object { "$($_.CommandLine)" -notmatch '-UserDir:' })
    if ($docsGames.Count -gt 0) { Write-Output "REGRESS: the game is running; not staging $Dll"; exit 2 }
    Copy-Item -LiteralPath $Dll -Destination (Join-Path $Plugins "FreeCity4.dll") -Force
    $pdb = [IO.Path]::ChangeExtension($Dll, ".pdb")
    if (Test-Path -LiteralPath $pdb) { Copy-Item -LiteralPath $pdb -Destination (Join-Path $Plugins "FreeCity4.pdb") -Force }
}
if (-not $Queue) { $staged = if ($Dll -and $Parallel) { (Resolve-Path -LiteralPath $Dll).Path } else { Join-Path $Plugins "FreeCity4.dll" } }
$hash = if (Test-Path -LiteralPath $staged) { (Get-FileHash -Algorithm SHA256 -LiteralPath $staged).Hash.Substring(0, 8).ToLower() } else { "none" }
Write-Output "staged DLL: $staged (sha256 $hash)"
$runProfile = if ($Turbo) { "turbo to $EndDate$(if ($TurboFrom) { " from $TurboFrom" }), timeout $TurboTimeout s" } else { "normal, $Seconds s after load" }
$mode = if ($Parallel) { "parallel (slots 1 and 2)" } elseif ($Queue) { "queue ($(if ($queuePrio) { 'priority high, at the same time' } else { 'the running queue predates priority: one after the other' }))" } else { "serial" }
Write-Output ("profile: {0}; {1}" -f $runProfile, $mode)
$Run = Join-Path $PSScriptRoot "ingame_run.ps1"
$Sum = Join-Path $PSScriptRoot "run_summary.ps1"
$Out = Join-Path $PSScriptRoot "..\build-runs"
New-Item -ItemType Directory -Force $Out | Out-Null

$runs = [ordered]@{
    shadows = "disable = budget`nbudget_shadow = 1`ndemand_shadow = 1`npollution_shadow = 1`nprotection_shadow = 1`ncrime_shadow = 1`naura_shadow = 1`nflora_shadow = 1`nlandfill_shadow = 1`npower_shadow = 1`nplumbing_shadow = 1`nlandvalue_shadow = 1`nflammability_shadow = 1`nresidential_shadow = 1`nneighbours_shadow = 1`nzones_shadow = 1`ndevelopment_shadow = 1`ntract_shadow = 1`nzonedev_shadow = 1`nlotdev_shadow = 1`nhistory_shadow = 1`nclock24_shadow = 1`ncommercial_shadow = 1`ncivic_shadow = 1`nparks_shadow = 1`ntraffic_shadow = 1`ntraffic_shadow_hypotheses = 7`ntraffic_shadow_lines = 1`nterrain_shadow = 1`nautomata_shadow = 1`nnetmgr_shadow = 1`nmysims_shadow = 1"
    replace = "enable = $Replace"
}
$after = if ($Turbo) { $TurboTimeout } else { $Seconds }

# Start the runs (all at once with -Parallel, else one after the other) and collect each one's output.
$results = [ordered]@{}
$jobs = [ordered]@{}
$slot = 0
foreach ($name in $runs.Keys) {
    if ($Only -and $Only -ne $name) { continue }
    $slot++
    $label = "regress-$Tag-$name"
    $ini = $runs[$name] + "`nautotest_unpause = 2`nautotest_end = quit`npopup_dialogs = 0`nautoload_city = $City"
    $iniPath = Join-Path $Out "$label.ini"
    Set-Content -Path $iniPath -Value $ini -Encoding ascii
    if ($Queue) {
        # A request in the main checkout's queue folder (written whole, then renamed to .req).
        $qdir = Join-Path $Main "build-runs\game-queue"; $res = Join-Path $Main "build-runs\game-results"
        New-Item -ItemType Directory -Force $qdir, $res | Out-Null
        $reqPath = Join-Path $qdir "$label.req"
        if (Test-Path -LiteralPath $reqPath) { Write-Output "REGRESS: $reqPath is already queued (an earlier regress -Tag $Tag?)"; exit 2 }
        if (Test-Path -LiteralPath (Join-Path $res "$label.running")) { Write-Output "REGRESS: a run labelled $label is running ($res\$label.running)"; exit 2 }
        $oldVerdict = Join-Path $res "$label.verdict"
        if (Test-Path -LiteralPath $oldVerdict) { Remove-Item -LiteralPath $oldVerdict -Force }
        $req = @("# tools\regress.ps1 -Queue -Tag $Tag", "label = $label", "dll = $staged", "city = $cityName", "seconds = $after", "timeout = $($after + 150)")
        if ($Turbo) { $req += "end_date = $EndDate"; if ($TurboFrom) { $req += "turbo_from = $TurboFrom" } else { $req += "turbo = 1" } }
        if ($queuePrio) { $req += "priority = high" }
        $req += "[ini]"
        $req += @(($runs[$name] + "`nautotest_unpause = 2`nautotest_end = quit`npopup_dialogs = 0") -split "`n")
        $tmp = "$reqPath.tmp"
        [IO.File]::WriteAllText($tmp, (($req -join "`r`n") + "`r`n"), (New-Object Text.UTF8Encoding($false)))
        Move-Item -LiteralPath $tmp -Destination $reqPath -Force
        $jobs[$name] = @{ label = $label; req = $reqPath; submitted = (Get-Date); started = $null; slot = ""; done = $false }
        $results[$name] = @()   # keeps the table in run order whichever verdict comes first
        Write-Output ("queue: submitted {0} ({1})" -f $reqPath, $(if ($queuePrio) { "priority high" } else { "no priority key" }))
        Start-Sleep -Seconds 1
        continue
    }
    $a = @("-IniFile", $iniPath, "-AfterLoadSeconds", "$after", "-Seconds", "$($after + 150)", "-Label", $label)
    if ($Turbo) { $a += @("-EndDate", $EndDate); if ($TurboFrom) { $a += @("-TurboFrom", $TurboFrom) } else { $a += "-Turbo" } }
    if ($Parallel) {
        $a += @("-Slot", "$slot"); if ($Dll) { $a += @("-Dll", $staged) }
        $jobs[$name] = Start-Job -ScriptBlock { param($script, $argv) & powershell -NoProfile -ExecutionPolicy Bypass -File $script @argv 2>&1 | ForEach-Object { "$_" } } -ArgumentList $Run, $a
        Start-Sleep -Seconds 2
    } else {
        $results[$name] = @(& powershell -NoProfile -ExecutionPolicy Bypass -File $Run @a 2>&1 | ForEach-Object { "$_" })
    }
}
$queueInfo = [ordered]@{}
if ($Queue) {
    # Wait for each request's verdict file; a request that gets no slot within -QueueWait is taken back.
    $res = Join-Path $Main "build-runs\game-results"
    while (@($jobs.Values | Where-Object { -not $_.done }).Count -gt 0) {
        Start-Sleep -Seconds 5
        foreach ($name in @($jobs.Keys)) {
            $j = $jobs[$name]
            if ($j.done) { continue }
            $vf = Join-Path $res "$($j.label).verdict"; $rf = Join-Path $res "$($j.label).running"
            if (Test-Path -LiteralPath $vf) {
                Start-Sleep -Milliseconds 500   # written in one go; let the writer close it
                $vt = @(Get-Content -LiteralPath $vf)
                $h = @(); $inH = $false
                foreach ($l in $vt) { if ($l -eq "harness:") { $inH = $true; continue }; if ($inH) { if ($l -eq "") { break }; $h += ($l -replace '^  ', '') } }
                if ($h.Count -eq 0) { $h = @("[$($j.label)] " + (($vt | Where-Object { $_ -match '^line: ' } | Select-Object -First 1) -replace '^line: ', '')) }
                $results[$name] = $h
                $queueInfo[$name] = @($vt | Where-Object { $_ -match '^(verdict|request|note): ' })
                $ql = Join-Path $res "$($j.label).log"
                $outLog = Join-Path $Out "$($j.label).log"
                if (Test-Path -LiteralPath $ql) { Copy-Item -LiteralPath $ql -Destination $outLog -Force }
                elseif (Test-Path -LiteralPath $outLog) { Remove-Item -LiteralPath $outLog -Force }
                $j.done = $true
                Write-Output ("queue: {0}: {1} ({2} s after submission)" -f $j.label, (($vt | Select-Object -First 1) -replace '^verdict: ', 'verdict '), [int]((Get-Date) - $j.submitted).TotalSeconds)
                continue
            }
            if (-not $j.started -and (Test-Path -LiteralPath $rf)) {
                $j.started = Get-Date
                $rt = ""; try { $rt = [IO.File]::ReadAllText($rf) } catch { }
                if ($rt -match 'slot (\d+)') { $j.slot = $Matches[1] }
                Write-Output ("queue: {0} started in slot {1} after {2} s" -f $j.label, $j.slot, [int]($j.started - $j.submitted).TotalSeconds)
            }
            $err = $null
            if (-not $j.started -and ((Get-Date) - $j.submitted).TotalSeconds -gt $QueueWait) {
                if (Test-Path -LiteralPath $j.req) { Remove-Item -LiteralPath $j.req -Force -ErrorAction SilentlyContinue }
                if (-not (Test-Path -LiteralPath $rf)) { $err = "BLOCKED: no queue slot within $QueueWait s (request taken back)" }
            } elseif ($j.started -and ((Get-Date) - $j.started).TotalSeconds -gt $after + 150 + 300) {
                $err = "BLOCKED: no verdict file $vf $([int]((Get-Date) - $j.started).TotalSeconds) s after the start (is the queue still running?)"
            }
            if ($err) { $results[$name] = @("[$($j.label)] $err"); $j.done = $true; Write-Output "queue: $($j.label): $err" }
        }
    }
}
foreach ($name in $jobs.Keys) {
    if ($Queue) { continue }
    $null = Wait-Job $jobs[$name]
    $results[$name] = @(Receive-Job $jobs[$name])
    Remove-Job $jobs[$name]
}
$wall = [int]((Get-Date) - $wallStart).TotalSeconds

# The terrain shadow's totals, from run_summary's "terrain queries" / "terrain vertices" lines (also read back from an
# .out, where they carry two more spaces): per query [exact, total], and the vertex counts (strings; "-" when unknown).
function TerrainOf($lines) {
    $t = $null
    foreach ($line in $lines) {
        if ($line -match "^\s+terrain queries\s+\d+/\d+ exact: (.+?)(; other checks .*)?$") {
            if (-not $t) { $t = @{ q = [ordered]@{}; unfollowed = "-"; late = "-"; pending = "-" } }
            foreach ($p in [regex]::Matches($Matches[1], "([a-z]+) (\d+)/(\d+)")) { $t.q[$p.Groups[1].Value] = @([long]$p.Groups[2].Value, [long]$p.Groups[3].Value) }
        } elseif ($line -match "^\s+terrain vertices\s+unfollowed (\S+), followed late (\S+), pending at shutdown (\S+);") {
            if (-not $t) { $t = @{ q = [ordered]@{}; unfollowed = "-"; late = "-"; pending = "-" } }
            $t.unfollowed = $Matches[1]; $t.late = $Matches[2]; $t.pending = $Matches[3]
        }
    }
    return $t
}
# The table column: "7/7 exact" (or the queries that are not, "normal 7340/7348"), then the vertex counts.
function TerrainText($t) {
    if (-not $t) { return "" }
    $bad = @($t.q.Keys | Where-Object { $t.q[$_][0] -ne $t.q[$_][1] } | ForEach-Object { "{0} {1}/{2}" -f $_, $t.q[$_][0], $t.q[$_][1] })
    $qs = if ($bad.Count -eq 0) { "$($t.q.Count)/$($t.q.Count) exact" } else { "$($t.q.Count - $bad.Count)/$($t.q.Count) exact ($($bad -join ', '))" }
    return "{0}; unf {1}, late {2}, pend {3}" -f $qs, $t.unfollowed, $t.late, $t.pending
}

$rows = @()
$cur = @{}
foreach ($name in $results.Keys) {
    $label = "regress-$Tag-$name"
    Write-Output "=== $label ($(if ($Turbo) { "to $EndDate, timeout $after s" } else { "$Seconds s after load" })) ==="
    $r = $results[$name]
    $r | Select-Object -First 3 | ForEach-Object { Write-Output "  $_" }
    if ($queueInfo.Contains($name)) { $queueInfo[$name] | Where-Object { $_ -notmatch '^verdict: ' } | ForEach-Object { Write-Output "  queue $_" } }
    $r | Select-Object -Skip 3 | Where-Object { $_ -match "^\s+(game:|sc4mutex:|HELD:|simulator: turbo month)" } | ForEach-Object { Write-Output "  $_" }
    $log = Join-Path $Out "$label.log"
    $s = & powershell -ExecutionPolicy Bypass -File $Sum -Log $log -Width 160 2>&1
    $s | ForEach-Object { Write-Output "  $_" }

    $verdict = ($s | Select-String "^verdict:\s+AUTOTEST VERDICT: ([a-z]+)" | Select-Object -First 1)
    $v = if ($verdict) { $verdict.Matches[0].Groups[1].Value } else { "NONE" }
    $problems = 0
    $inProblems = $false
    foreach ($line in $s) {
        if ($line -match "^problems") { $inProblems = $true; continue }
        if ($inProblems -and $line -match "^\s+(.+?)\s{2,}(\d+)$") { if ($Matches[1] -ne "other WARNING lines") { $problems += [int]$Matches[2] } }
        elseif ($inProblems -and $line -notmatch "^\s") { $inProblems = $false }
    }
    $sh = [ordered]@{}
    foreach ($m in ($s | Select-String "^\s+([a-z]+)\s+(\d+) lines, (\d+) with every")) { $sh[$m.Matches[0].Groups[1].Value] = @([int]$m.Matches[0].Groups[3].Value, [int]$m.Matches[0].Groups[2].Value) }
    $shadow = ($sh.Keys | ForEach-Object { "{0} {1}/{2}" -f $_, $sh[$_][0], $sh[$_][1] }) -join ", "
    $months = ($s | Select-String "^\s+([a-z ()]+?)\s+(\d+) lines" | Where-Object { $_.Line -notmatch "with every" } | Measure-Object).Count
    $dpsm = @($r) + @($s) | Select-String "AUTOTEST VERDICT: .*days_per_second=([\d.]+)" | Select-Object -First 1   # run_summary cuts its verdict line
    $dps = if ($dpsm) { $dpsm.Matches[0].Groups[1].Value } else { "-" }
    $ter = TerrainOf $s
    $rows += [pscustomobject]@{ run = $name; verdict = $v; problems = $problems; monthGroups = $months; "days/s" = $dps; terrain = (TerrainText $ter); shadows = $shadow }
    $cur[$name] = @{ verdict = $v; problems = $problems; shadows = $sh; dps = $dps; terrain = $ter }
}
Write-Output ""
$slotsUsed = @(foreach ($qi in $queueInfo.Values) { foreach ($l in $qi) { if ($l -match '^request: .* in slot (\d+)') { $Matches[1] } } }) -join " and "
Write-Output ("regress wall time: {0} s ({1}; {2})" -f $wall, $(if ($Parallel) { "parallel" } elseif ($Queue) { "queue$(if ($slotsUsed) { ", slots $slotsUsed" })" } else { "serial" }), $runProfile)
Write-Output "=== regress $Tag ==="
$rows | Format-Table -AutoSize | Out-String -Width 200 | Write-Output

# Delta against an earlier regress (its redirected .out).
function ReadOut([string]$path) {
    $runsOut = @{}; $runName = $null; $inTable = $false
    foreach ($line in (Get-Content -LiteralPath $path)) {
        if ($line -match "^=== regress-.+-(shadows|replace) \(") { $runName = $Matches[1]; $runsOut[$runName] = @{ verdict = "NONE"; problems = $null; shadows = [ordered]@{}; tlines = @() }; continue }
        if ($line -match "^=== regress \S+ ===") { $inTable = $true; $runName = $null; continue }
        if ($inTable -and $line -match "^(shadows|replace)\s+(\S+)\s+(\d+)\s+\d+") { if ($runsOut[$Matches[1]]) { $runsOut[$Matches[1]].problems = [int]$Matches[3] }; continue }
        if (-not $runName) { continue }
        if ($line -match "^\s+\[[^\]]+\] AUTOTEST VERDICT: .*days_per_second=([\d.]+)") { $runsOut[$runName].dps = $Matches[1]; continue }
        if ($line -match "^\s+verdict:\s+AUTOTEST VERDICT: ([a-z]+)") { $runsOut[$runName].verdict = $Matches[1] }
        elseif ($line -match "^\s+([a-z]+)\s+(\d+) lines, (\d+) with every") { $runsOut[$runName].shadows[$Matches[1]] = @([int]$Matches[3], [int]$Matches[2]) }
        elseif ($line -match "^\s+terrain (queries|vertices)\s") { $runsOut[$runName].tlines += $line }
    }
    foreach ($k in @($runsOut.Keys)) { $runsOut[$k].terrain = TerrainOf $runsOut[$k].tlines }
    return $runsOut
}
# Pacing of an .out: "turbo <end date>" or "normal" (no profile line: an older regress, always normal).
function PacingOf([string]$text) {
    if ($text -match "(?m)^profile: turbo to (\d{4}-\d\d-\d\d)") { return @("turbo", $Matches[1]) }
    return @("normal", "")
}
$curPacing = if ($Turbo) { @("turbo", $EndDate) } else { @("normal", "") }
$basePath = $null
if ($Baseline) {
    $basePath = if (Test-Path -LiteralPath $Baseline) { $Baseline } else { Join-Path $Out "regress-$Baseline.out" }
    if (-not (Test-Path -LiteralPath $basePath)) { Write-Output "delta: no baseline $basePath"; $basePath = $null }
} else {
    $cands = @(Get-ChildItem -Path (Join-Path $Out "regress-*.out") -ErrorAction SilentlyContinue | Where-Object { $_.Name -ne "regress-$Tag.out" } | Sort-Object LastWriteTime -Descending)
    foreach ($pass in 1, 2) {
        if ($pass -eq 2 -and -not $AnyPacing) { break }
        foreach ($c in $cands) {
            $txt = (Get-Content -LiteralPath $c.FullName) -join "`n"
            if ($txt -notmatch "(?m)^REGRESS \S+: PASS") { continue }
            if ($pass -eq 1 -and (PacingOf $txt)[0] -ne $curPacing[0]) { continue }
            $basePath = $c.FullName; break
        }
        if ($basePath) { break }
    }
    if (-not $basePath) { Write-Output "delta: no earlier regress-*.out with a PASS line and $($curPacing[0]) pacing in $Out (-Baseline <tag> -AnyPacing compares across pacings)" }
}
$worse = 0
if ($basePath) {
    $baseTxt = (Get-Content -LiteralPath $basePath) -join "`n"
    $basePacing = PacingOf $baseTxt
    if ($basePacing[0] -ne $curPacing[0]) {
        $why = "baseline $basePath ran at $($basePacing[0]) pacing, this regress at $($curPacing[0]): with the original simulators income, funds and other results depend on game days per real second"
        if ($AnyPacing) { Write-Output "delta: WARNING: $why (-AnyPacing: comparing anyway)" }
        else { Write-Output "delta: REFUSED: $why; give a baseline of the same profile, or -AnyPacing"; $basePath = $null }
    } elseif ($basePacing[0] -eq "turbo" -and $basePacing[1] -ne $curPacing[1]) {
        Write-Output "delta: note: the baseline ran to $($basePacing[1]), this regress to $($curPacing[1]) (line counts differ; the shares still compare)"
    }
}
if ($basePath) {
    $baseTag = [IO.Path]::GetFileNameWithoutExtension($basePath) -replace '^regress-', ''
    $base = ReadOut $basePath
    $pm = Get-Content -LiteralPath $basePath | Select-String "^profile: (.*)" | Select-Object -First 1
    $baseProfile = if ($pm) { $pm.Matches[0].Groups[1].Value } else { "" }
    Write-Output ("delta vs {0} ({1}; {2}):" -f $baseTag, $basePath, $(if ($baseProfile) { $baseProfile } else { "normal" }))
    foreach ($name in $cur.Keys) {
        $b = $base[$name]
        if ($b -and $b.dps -and $cur[$name].dps -ne "-") {
            $r = [double]$cur[$name].dps / [Math]::Max(0.01, [double]$b.dps)
            if ($r -gt 2 -or $r -lt 0.5) { Write-Output ("  {0}: WARNING: pacing differs: {1} days/s in the baseline, {2} now (machine load?); results of the original simulators may differ" -f $name, $b.dps, $cur[$name].dps) }
        }
    }
    function Rate($pair) { if ($pair[1] -le 0) { return 1.0 } else { return ($pair[1] - $pair[0]) / [double]$pair[1] } }
    foreach ($name in $cur.Keys) {
        $b = $base[$name]
        if (-not $b) { Write-Output "  ${name}: not in the baseline"; continue }
        $c = $cur[$name]
        $how = if ($c.verdict -eq $b.verdict) { "same" } elseif ($c.verdict -eq "clean") { "better" } else { "worse" }
        if ($how -eq "worse") { $worse++ }
        Write-Output "  ${name}: verdict $($b.verdict) -> $($c.verdict) ($how)"
        if ($null -ne $b.problems) {
            $how = if ($c.problems -eq $b.problems) { "same" } elseif ($c.problems -lt $b.problems) { "better" } else { "worse" }
            if ($how -eq "worse") { $worse++ }
            Write-Output "  ${name}: problems $($b.problems) -> $($c.problems) ($how)"
        }
        foreach ($k in $b.shadows.Keys) {
            $o = $b.shadows[$k]
            if (-not $c.shadows.Contains($k)) { $worse++; Write-Output ("  {0}: {1} {2}/{3} -> missing (worse)" -f $name, $k, $o[0], $o[1]); continue }
            $n = $c.shadows[$k]
            # Worse: more lines with a mismatch, and a larger share of them (a longer run has more lines).
            $ro = Rate $o; $rn = Rate $n; $mo = $o[1] - $o[0]; $mn = $n[1] - $n[0]
            $how = if ($mn -gt $mo -and $rn -gt $ro) { "worse" } elseif ($mn -lt $mo -and $rn -lt $ro) { "better" } else { "same" }
            if ($how -eq "worse") { $worse++ }
            Write-Output ("  {0}: {1} {2}/{3} -> {4}/{5} ({6})" -f $name, $k, $o[0], $o[1], $n[0], $n[1], $how)
        }
        foreach ($k in $c.shadows.Keys) { if (-not $b.shadows.Contains($k)) { $n = $c.shadows[$k]; Write-Output ("  {0}: {1} new {2}/{3}" -f $name, $k, $n[0], $n[1]) } }
        # Terrain totals: a query is worse as a shadow is (more mismatches and a larger share); more unfollowed vertices
        # is worse; followed late and pending are exact and only shown.
        $tb = $b.terrain; $tc = $c.terrain
        if ($tc -and -not $tb) { Write-Output ("  {0}: terrain totals new: {1}" -f $name, (TerrainText $tc)) }
        elseif ($tb -and -not $tc) { $worse++; Write-Output ("  {0}: terrain totals {1} -> missing (worse)" -f $name, (TerrainText $tb)) }
        elseif ($tb -and $tc) {
            $tw = @(); $tbt = @()
            foreach ($k in $tb.q.Keys) {
                if (-not $tc.q.Contains($k)) { $tw += "$k missing"; continue }
                $o = @($tb.q[$k][0], $tb.q[$k][1]); $n = @($tc.q[$k][0], $tc.q[$k][1])
                $mo = $o[1] - $o[0]; $mn = $n[1] - $n[0]; $ro = Rate $o; $rn = Rate $n
                if ($mn -gt $mo -and $rn -gt $ro) { $tw += "$k $($o[0])/$($o[1]) -> $($n[0])/$($n[1])" }
                elseif ($mn -lt $mo -and $rn -lt $ro) { $tbt += "$k $($o[0])/$($o[1]) -> $($n[0])/$($n[1])" }
            }
            $uo = $tb.unfollowed -as [int]; $un = $tc.unfollowed -as [int]
            if ($null -ne $uo -and $null -ne $un) { if ($un -gt $uo) { $tw += "unfollowed $uo -> $un" } elseif ($un -lt $uo) { $tbt += "unfollowed $uo -> $un" } }
            $how = if ($tw.Count -gt 0) { "worse: $($tw -join ', ')" } elseif ($tbt.Count -gt 0) { "better: $($tbt -join ', ')" } else { "same" }
            if ($tw.Count -gt 0) { $worse++ }
            Write-Output ("  {0}: terrain totals {1} -> {2} ({3})" -f $name, (TerrainText $tb), (TerrainText $tc), $how)
        }
    }
}
$bad = @($rows | Where-Object { $_.verdict -ne "clean" -or $_.problems -ne 0 }).Count
Write-Output ("REGRESS {0}: {1}" -f $Tag, $(if ($bad -eq 0) { "PASS (verdicts clean, 0 problems; read the shadow column)" } else { "FAIL ($bad run(s))" }))
if ($worse -gt 0) { Write-Output ("REGRESS {0}: {1} worse (vs {2})" -f $Tag, $worse, $baseTag) }
