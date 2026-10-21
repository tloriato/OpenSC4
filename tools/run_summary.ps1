<#
Summarise a FreeCity4.log from an in-game run (coordinator tooling): the verdict, warning counts, the city's money
over the run, and for every subsystem its month lines (count, "enabled" values, first and last line) and shadow
lines (how many compared clean).

Usage:
  powershell -File tools\run_summary.ps1 -Log build-runs\run69-soak2.log [-Width 220]

Counts that should be 0 in a good run: foreign exceptions, watchdog stalls, reference warnings, Rust panics,
structured exceptions, and with terrain_shadow = 1 the terrain shadow's unfollowed vertices (changes no message it
followed covered). The terrain shadow also gets a block of its own: exact/total per query, unfollowed, followed late
(exact: its queued message came after the month re-read) and pending at shutdown, from its city shutdown line.
The x87 control-word warning at start-up is expected (foundations §18.2) and is listed apart.
#>
param(
    [Parameter(Mandatory = $true)][string]$Log,
    [int]$Width = 220
)
$ErrorActionPreference = "Stop"
if (-not (Test-Path $Log)) { Write-Output "no such log: $Log"; exit 2 }

function Body([string]$line) {
    # Drop the "date time [pid N tid M] " prefix.
    $i = $line.IndexOf('] ')
    $b = if ($i -ge 0) { $line.Substring($i + 2) } else { $line }
    if ($b.Length -gt $Width) { $b = $b.Substring(0, $Width) + " ..." }
    return $b
}
function Count([string]$pattern) { return @(Select-String -Path $Log -Pattern $pattern -CaseSensitive).Count }

$verdict = Select-String -Path $Log -Pattern "AUTOTEST VERDICT: .*" -CaseSensitive | Select-Object -Last 1
Write-Output ("verdict:   " + $(if ($verdict) { Body $verdict.Line } else { "(none)" }))
$first = (Select-String -Path $Log -Pattern "^\d{4}-\d\d-\d\d \d" -CaseSensitive | Select-Object -First 1).Line
$lastBeat = Select-String -Path $Log -Pattern "heartbeat \(periodic\): (\S+)" -CaseSensitive | Select-Object -Last 1
if ($lastBeat) { Write-Output ("last date: " + $lastBeat.Matches[0].Groups[1].Value + "   (log " + $first.Substring(0, 19) + " to " + $lastBeat.Line.Substring(0, 19) + ")") }

# The terrain shadow (terrain_shadow = 1, glue16) also counts vertices outside its lines. Its city shutdown line has
# the totals per query ("queries altitude 8078/8078 normal ... navigable N/N"), the vertices that changed without a
# message it followed (unfollowed: a problem, listed below), the ones whose queued message came after the month
# re-read had found them (followed late: exact; terrain messages are queued posts, up to ~40 game days late under
# turbo) and the ones still waiting for their message at shutdown (pending: exact so far). Without a shutdown line (a
# crash) the logged lines are summed instead (change lines past terrain_shadow_lines are not logged, so the query
# totals are partial).
$tShut = Select-String -Path $Log -Pattern "\] terrain shadow: city shutdown: .*" -CaseSensitive | Select-Object -Last 1
$tLines = @(Select-String -Path $Log -Pattern "\] terrain shadow (load|month|change|script|sea) " -CaseSensitive)
$terrain = $null
if ($tShut -or $tLines.Count -gt 0) {
    $qOrder = @("altitude", "normal", "water", "land", "depth", "type", "navigable")
    $qe = [ordered]@{}; $qt = [ordered]@{}
    function AddQueries([string]$text) {
        $m = [regex]::Match($text, "queries((?: [a-z]+ \d+/\d+)+)")
        if (-not $m.Success) { return }
        foreach ($p in [regex]::Matches($m.Groups[1].Value, "([a-z]+) (\d+)/(\d+)")) {
            $k = $p.Groups[1].Value
            if (-not $qe.Contains($k)) { $qe[$k] = [long]0; $qt[$k] = [long]0 }
            $qe[$k] += [long]$p.Groups[2].Value; $qt[$k] += [long]$p.Groups[3].Value
        }
    }
    $tMonths = @($tLines | Where-Object { $_.Line -match "\] terrain shadow month " })
    $terrain = @{
        months = $tMonths.Count
        monthsUnfollowed = @($tMonths | Where-Object { $_.Line -match ": unfollowed=[1-9]" }).Count
        monthsAhead = @($tMonths | Where-Object { $_.Line -match "; ahead=[1-9]" }).Count
        other = ""
    }
    if ($tShut) {
        $terrain.src = "its city shutdown line"
        AddQueries $tShut.Line
        $terrain.unfollowed = if ($tShut.Line -match "unfollowed vertices (\d+)") { $Matches[1] } else { "-" }
        $terrain.late = if ($tShut.Line -match "followed late (\d+)") { $Matches[1] } else { "-" }
        $terrain.pending = if ($tShut.Line -match "pending at shutdown (\d+)") { $Matches[1] } else { "-" }
        if ($tShut.Line -match "other checks (\d+), mismatched (\d+)") { $terrain.other = "other checks $($Matches[1]), mismatched $($Matches[2])" }
    } else {
        $terrain.src = "the logged lines: NO city shutdown line, query totals partial"
        $uSum = 0; $lateSum = 0
        foreach ($l in $tLines) {
            AddQueries $l.Line
            if ($l.Line -match "\] terrain shadow month [^:]*: unfollowed=(\d+)") { $uSum += [int]$Matches[1] }
            if ($l.Line -match "; late (\d+) \(") { $lateSum += [int]$Matches[1] }
        }
        $terrain.unfollowed = "$uSum"; $terrain.late = "$lateSum"; $terrain.pending = "-"
    }
    $terrain.queries = @(foreach ($k in ($qOrder + @($qe.Keys | Where-Object { $qOrder -notcontains $_ }))) { if ($qe.Contains($k)) { "{0} {1}/{2}" -f $k, $qe[$k], $qt[$k] } })
    $terrain.exact = "{0}/{1}" -f @($qe.Keys | Where-Object { $qe[$_] -eq $qt[$_] }).Count, $qe.Count
}

Write-Output ""
Write-Output "problems (all should be 0):"
$checks = [ordered]@{
    "foreign first-chance exceptions" = "first-chance exception OUTSIDE FreeCity4\.dll"
    "watchdog stalls"                 = "WATCHDOG: main thread.*stalled"
    "reference warnings"              = "WARNING: (released last reference|over-release)"
    "Rust panics"                     = "Rust PANIC"
    "structured exceptions"           = "structured exception in"
    "shadow INCOMPLETE lines"         = "shadow.*INCOMPLETE"
}
foreach ($k in $checks.Keys) { Write-Output ("  {0,-34} {1}" -f $k, (Count $checks[$k])) }
# Only with the terrain shadow on (other runs keep their list); "-" never counts.
if ($terrain -and $terrain.unfollowed -ne "-") { Write-Output ("  {0,-34} {1}" -f "terrain unfollowed vertices", $terrain.unfollowed) }
$fpu = @(Select-String -Path $Log -Pattern "WARNING.*(control word changed|not at 24-bit)" -CaseSensitive).Count
$other = @(Select-String -Path $Log -Pattern "WARNING" -CaseSensitive | Where-Object {
        $_.Line -notmatch "control word changed|not at 24-bit|released last reference|over-release|AUTOTEST VERDICT" })
Write-Output ("  {0,-34} {1}" -f "other WARNING lines", $other.Count)
$other | Select-Object -First 5 | ForEach-Object { Write-Output ("    " + (Body $_.Line)) }
Write-Output ("  (expected: {0} x87 start-up warning line(s), foundations 18.2; the plugin re-asserts 24-bit)" -f $fpu)

$obs = @(Select-String -Path $Log -Pattern "observe month \S+: date=(\S+) julian=\d+ funds=(-?\d+) income=(-?\d+) expense=(-?\d+)" -CaseSensitive)
if ($obs.Count -gt 0) {
    $a = $obs[0].Matches[0].Groups; $z = $obs[$obs.Count - 1].Matches[0].Groups
    Write-Output ""
    Write-Output ("money ({0} observed months): {1} funds {2} (income {3}, expense {4})  ->  {5} funds {6} (income {7}, expense {8})" -f
        $obs.Count, $a[1].Value, $a[2].Value, $a[3].Value, $a[4].Value, $z[1].Value, $z[2].Value, $z[3].Value, $z[4].Value)
}

# Month status lines: "<name> month:", "<name> month (<part>):", "<name> month yyyy-mm:" (crime) or
# "<name> month yyyy-mm-dd:" (flora).
$months = Select-String -Path $Log -Pattern "\] ([a-z][a-z ]*?) month( \([a-z ]+\))?( \d{4}-\d\d(-\d\d)?)?:" -CaseSensitive
$groups = $months | Group-Object { $_.Matches[0].Groups[1].Value + $_.Matches[0].Groups[2].Value }
if ($groups) {
    Write-Output ""
    Write-Output "month lines:"
    foreach ($g in ($groups | Sort-Object Name)) {
        $en = $g.Group | ForEach-Object { if ($_.Line -match "\benabled=(\d)") { $Matches[1] } } | Group-Object | ForEach-Object { "enabled=$($_.Name) x$($_.Count)" }
        Write-Output ("  {0,-28} {1,5} lines  {2}" -f $g.Name, $g.Count, ($en -join ", "))
        Write-Output ("    first: " + (Body $g.Group[0].Line))
        if ($g.Count -gt 1) { Write-Output ("    last:  " + (Body $g.Group[$g.Count - 1].Line)) }
    }
}

# Shadow comparison lines: any "<name> shadow ... <date> ...: ..." line that reports "mismatches=N",
# "mismatched N" / "mismatched=N" or a bare "clean" (budget and demand per day or month, "pollution shadow month to
# <date>" and its day lines, "protection shadow (fire) <date>", "flora shadow month <date>: ... mismatched N",
# "landfill shadow month to <date>: clean|mismatches=N"). A line differs when any of those counts is non-zero.
$shadows = Select-String -Path $Log -Pattern "\] ([a-z]+) shadow [^:]*\d{4}-\d\d-\d\d[^:]*: .*(mismatch(es|ed)[= ]\d|: clean\b)" -CaseSensitive
$differs = "mismatch(es|ed)[= ][1-9]|differences [1-9]"
# A line with its own total ("; mismatches=N", nothing between "; " and the word) is judged by that total alone:
# other counts on it can be notes (land value: "with the live map total mismatched=391" when the previous desirability map matched).
function Differs([string]$line) {
    $m = [regex]::Matches($line, "; mismatches=(\d+)")
    if ($m.Count -gt 0) { return [int]$m[$m.Count - 1].Groups[1].Value -ne 0 }
    return $line -match $differs
}
function Resynced([string]$line) {
    # A reset can make subsequent comparisons match without proving the model.
    return ($line -match '\bresynced:\s*(?!none\b|0\b)\S') -or
           ($line -match '\bresyncs?\s*[=:]\s*[1-9]\d*\b')
}
$sg = $shadows | Group-Object { $_.Matches[0].Groups[1].Value }
if ($sg) {
    Write-Output ""
    Write-Output "shadow lines:"
    foreach ($g in $sg) {
        $clean = @($g.Group | Where-Object { -not (Differs $_.Line) }).Count
        Write-Output ("  {0,-12} {1,5} lines, {2} with every mismatch count 0" -f $g.Name, $g.Count, $clean)
        $resynced = @($g.Group | Where-Object { Resynced $_.Line }).Count
        Write-Output ("  {0} resynced lines: {1}" -f $g.Name, $resynced)
        $bad = $g.Group | Where-Object { Differs $_.Line } | Select-Object -First 2
        foreach ($b in $bad) { Write-Output ("    differs: " + (Body $b.Line)) }
    }
}

if ($terrain) {
    Write-Output ""
    Write-Output "terrain shadow (totals from $($terrain.src)):"
    Write-Output ("  terrain queries   {0} exact: {1}{2}" -f $terrain.exact, ($terrain.queries -join ", "), $(if ($terrain.other) { "; $($terrain.other)" } else { "" }))
    Write-Output ("  terrain vertices  unfollowed {0}, followed late {1}, pending at shutdown {2}; month lines {3} (unfollowed>0 in {4}, ahead>0 in {5})" -f $terrain.unfollowed, $terrain.late, $terrain.pending, $terrain.months, $terrain.monthsUnfollowed, $terrain.monthsAhead)
}

# Names may hold digits ("clock24 Shutdown"); per-object ids ("growth 0x00001010 Shutdown") are folded into one name.
$shut = Select-String -Path $Log -Pattern "\] [a-zA-Z][a-zA-Z0-9 ]* Shutdown( \(|:|$)" -CaseSensitive
if ($shut) {
    Write-Output ""
    Write-Output ("shutdown lines: " + (($shut | ForEach-Object { ((Body $_.Line) -replace '( \(|:).*$', '') -replace ' 0x[0-9A-Fa-f]+', '' } | Select-Object -Unique) -join "; "))
}
