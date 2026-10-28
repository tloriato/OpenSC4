<#
Run SimCity 4 once with FreeCity4 and a given FreeCity4.ini, unattended, and report a verdict.

The game is started from this shell (normal integrity), so it can be stopped here. Starting it from the
Start-menu shortcut via the desktop app made it elevated and unkillable.

Usage:
  powershell -File tools\ingame_run.ps1 -Ini "budget_default = 0`nlot_scan = 1" [-Seconds 240] [-Label run8]
                                        [-Actions "20=ui cheetah; 40=ui pause; 60=save <city file>"] [-NoDismiss]
                                        [-KeepSaves]
  powershell -File tools\ingame_run.ps1 -Slot 2 -Dll <FreeCity4.dll> -City Konradshohe -IniFile <ini text file>
                                        [-EndDate 2028-01-01] [-Turbo | -TurboFrom 2026-04-01] [-NoMinimize] ...

Verdict sources, in order:
  1. "AUTOTEST VERDICT:" line written by the plugin (ini autotest_seconds), which also ends the process.
  2. Fallback: a foreign first-chance AV or a WATCHDOG stall in the log => corrupt/stalled; otherwise "clean" once
     -Seconds have passed after the city loaded.
  A start that shows a "Hardware Driver Error" window, or writes no plugin attach line within 60 s, is stopped at
  once: "failed start: <window title or reason>".

Win32 dialogs. A plain MessageBox (or another #32770 dialog with buttons) of the game stops all plugin code while it is
open, so the harness looks for them from outside every round: each is logged once with its title, the text of its
static controls and its buttons ("DIALOG (<phase>, <time>): '<title>': <text> [buttons: OK=2]"), then closed with
WM_COMMAND IDOK if it has that button, else IDCANCEL, else WM_CLOSE (up to 3 tries; -NoDismiss only logs). One whose
title has "Error" before the plugin attached is a failed start. The DIALOG lines follow the verdict line.

Modal dialogs. Advisor popups (and the save confirmation) call HiddenPause() on our simulator and hold the city until
a button is clicked; Esc and Enter do not close them (runs 48-50). Unless -NoDismiss is given, the harness runs
`sc4ctl.ps1 clear` whenever a new HiddenPause shows up in the log after the load, or the date stands still across two
periodic heartbeats with OnTick counting and nothing delivered. `clear` only clicks dialogs it recognises, saves a PNG
of each to build-runs\<label>-popupN.png, and stops once the log shows hidden=0. It is a best-effort fallback: use
popup_dialogs = 0. A city held by a dialog (hidden > 0) for -HeldReportSeconds is reported: "HELD: ... since <date>".

Actions. -Actions takes "seconds-after-load=sc4ctl command" pairs separated by ';'. Each runs once, in order, through
tools\sc4ctl.ps1 (see that file: ui <control>, click x y, key, realkey, save, snap, state, clear). Use
autotest_unpause = 0 in the ini when an action pauses the game, or the autotest will undo it.

Several games at once (slots). Every run, slot or not, starts its game under a launch lock and swaps the game's
single-instance mutex (tools\sc4mutex.ps1) as soon as the game holds it, so the next game can start. -Slot N runs the
game with -UserDir:"<slot root>\slotNN\" (two digits: slot01..slot16, so every slot's paths have one length; the game
copies them into its heap, and a longer path moves the blocks after it) instead of the Documents user dir; the slot gets its own Plugins
(FreeCity4.dll and .pdb from -Dll, else the DLL staged in Documents), FreeCity4.ini, FreeCity4.log, SimCity 4.cfg and
Regions\<Region>, mirrored from Documents before every run (so a city a run changed is restored). The slot root is
<main checkout>\build-runs\sc4slots (override: -SlotRoot or $env:FC4_SLOT_ROOT). A path with a component that starts
with '.' (like .claude) is refused: the game ignores such a -UserDir and silently uses Documents. A slot game runs on
its own CPU (affinity 2N; two games on the game's default CPU 0 starve each other), starts visible without
activation and is minimized once the city has loaded (it simulates at the same rate minimized; -NoMinimize keeps it
up). Slot runs never use real input: realkey, realclick, realdrag and save actions are refused.
-Region <folder> defaults to "FC4 Test". -City <name> sets autoload_city to the user dir's "Regions\<Region>\City - <name>.sc4"; in a slot run an
autoload_city under Documents\SimCity 4 is remapped to the slot. -Dll without -Slot stages the DLL in Documents.
-EndDate YYYY-MM-DD adds autotest_end_date (the verdict comes at that game date; -AfterLoadSeconds stays the
timeout). -Turbo adds sim_turbo = 1. -TurboFrom YYYY-MM-DD starts the turbo clock with a script action at that date
instead (a script file next to the ini), so the first game months, where popups are switched off, run at normal speed.
-IniFile <path> reads the ini text from a file instead of -Ini.
#>
param(
    [string]$Ini = "",
    [int]$Seconds = 240,
    [int]$AfterLoadSeconds = 75,
    [string]$Label = (Get-Date -Format "HHmmss"),
    [string]$Actions = "",
    [switch]$NoDismiss,
    # The autoload city is backed up before the run and restored afterwards if the run changed it (a save action,
    # or a save by hand). -KeepSaves keeps the changed file.
    [switch]$KeepSaves,
    # The plugin's verdict timer starts at "clock started", a few seconds after the load line this script waits
    # for, so its verdict can come just after -AfterLoadSeconds. Wait this long for it before falling back.
    [int]$VerdictGraceSeconds = 30,
    [string]$IniFile = "",
    [ValidateRange(0, 32)][int]$Slot = 0,
    [string]$Dll = "",
    [string]$City = "",
    [string]$Region = "FC4 Test",
    [string]$EndDate = "",
    [switch]$Turbo,
    [string]$TurboFrom = "",
    [string]$SlotRoot = "",
    [switch]$NoMinimize,
    [int]$HeldReportSeconds = 30,
    # A calendar that stays on one date this long while the simulator ticks (an emergency pause that never ends, a stuck
    # clock) ends the run as stalled; 0 turns it off. The plugin's watchdog only sees a stalled main thread.
    [int]$FrozenCalendarSeconds = 600,
    # Windows priority class for the game process (the queue passes AboveNormal for priority = high requests).
    [ValidateSet("", "Idle", "BelowNormal", "Normal", "AboveNormal", "High")][string]$GamePriority = ""
)

$ErrorActionPreference = "Stop"
$GameDir = "C:\Program Files (x86)\GOG Galaxy\Games\SimCity 4 Deluxe Edition\Apps"
$Exe = Join-Path $GameDir "SimCity 4.exe"
$DocsUser = Join-Path ([Environment]::GetFolderPath("MyDocuments")) "SimCity 4"
$OutDir = Join-Path $PSScriptRoot "..\build-runs"
$Ctl = Join-Path $PSScriptRoot "sc4ctl.ps1"
$MutexTool = Join-Path $PSScriptRoot "sc4mutex.ps1"
. (Join-Path $PSScriptRoot 'city_fixture.ps1')
. (Join-Path $PSScriptRoot 'process_cmdline.ps1')

Add-Type -ErrorAction SilentlyContinue @"
using System; using System.Collections.Generic; using System.Runtime.InteropServices; using System.Text;
public static class FC4Run {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] struct SI {
        public int cb; public string r, desk, title; public int x, y, w, h, cx, cy, fill, flags; public short show, r2; public IntPtr r3, i, o, e; }
    [StructLayout(LayoutKind.Sequential)] struct PI { public IntPtr hp, ht; public int pid, tid; }
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)] static extern bool CreateProcessW(string app, StringBuilder cmd, IntPtr pa, IntPtr ta, bool inh, uint flags, IntPtr env, string dir, ref SI si, out PI pi);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
    // Start a process with a given first show state (STARTF_USESHOWWINDOW), e.g. 4 = SW_SHOWNOACTIVATE.
    public static System.Diagnostics.Process Start(string exe, string args, string dir, short show) {
        SI si = new SI(); si.cb = Marshal.SizeOf(typeof(SI)); si.flags = 1; si.show = show;
        PI pi; StringBuilder cmd = new StringBuilder("\"" + exe + "\" " + args);
        if (!CreateProcessW(exe, cmd, IntPtr.Zero, IntPtr.Zero, false, 0, IntPtr.Zero, dir, ref si, out pi))
            throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        System.Diagnostics.Process p = System.Diagnostics.Process.GetProcessById(pi.pid);
        IntPtr keep = p.Handle;   // keep a handle so the exit code stays readable
        CloseHandle(pi.ht); CloseHandle(pi.hp);
        return p;
    }
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    // Titles of the process's visible top-level windows.
    public static List<string> Titles(int pid) {
        List<string> r = new List<string>();
        EnumWindows(delegate(IntPtr h, IntPtr l) {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p == (uint)pid && IsWindowVisible(h)) { StringBuilder t = new StringBuilder(256); GetWindowText(h, t, 256); r.Add(t.ToString()); }
            return true; }, IntPtr.Zero);
        return r;
    }
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    // The game's own window (class GDriverClass--DirectX). Process.MainWindowHandle is 0 at times for these games.
    public static IntPtr GameWindow(int pid) {
        IntPtr found = IntPtr.Zero;
        EnumWindows(delegate(IntPtr h, IntPtr l) {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p == (uint)pid) { StringBuilder c = new StringBuilder(256); GetClassName(h, c, 256); if (c.ToString().StartsWith("GDriverClass")) { found = h; return false; } }
            return true; }, IntPtr.Zero);
        return found;
    }
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    // Plain Win32 dialogs (MessageBox and other #32770 windows) of the process: while one is open no plugin code
    // runs, so the harness reads and closes them from outside. Only dialogs with at least one button.
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr p, EnumProc f, IntPtr l);
    [DllImport("user32.dll")] static extern IntPtr GetDlgItem(IntPtr h, int id);
    [DllImport("user32.dll")] static extern int GetDlgCtrlID(IntPtr h);
    [DllImport("user32.dll")] static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern IntPtr SendMessageTimeout(IntPtr h, uint m, IntPtr w, StringBuilder l, uint flags, uint ms, out IntPtr res);
    static string ClassOf(IntPtr h) { StringBuilder c = new StringBuilder(256); GetClassName(h, c, 256); return c.ToString(); }
    // WM_GETTEXT with a timeout (works across processes; a hung window gives ""), else the stored caption.
    public static string TextOf(IntPtr h) {
        StringBuilder t = new StringBuilder(2048); IntPtr r;
        if (SendMessageTimeout(h, 0x000D, (IntPtr)2048, t, 2 /* SMTO_ABORTIFHUNG */, 1000, out r) != IntPtr.Zero && t.Length > 0) return t.ToString();
        t = new StringBuilder(2048); GetWindowText(h, t, 2048); return t.ToString();
    }
    static List<IntPtr> Children(IntPtr h, string cls) {
        List<IntPtr> r = new List<IntPtr>();
        EnumChildWindows(h, delegate(IntPtr c, IntPtr l) { if (ClassOf(c) == cls) r.Add(c); return true; }, IntPtr.Zero);
        return r;
    }
    public static List<IntPtr> Dialogs(int pid) {
        List<IntPtr> r = new List<IntPtr>();
        EnumWindows(delegate(IntPtr h, IntPtr l) {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p == (uint)pid && ClassOf(h) == "#32770" && Children(h, "Button").Count > 0) r.Add(h);
            return true; }, IntPtr.Zero);
        return r;
    }
    // The texts of the dialog's static controls, joined with " / ".
    public static string DialogText(IntPtr h) {
        List<string> r = new List<string>();
        foreach (IntPtr c in Children(h, "Static")) { string t = TextOf(c).Trim(); if (t.Length > 0) r.Add(t.Replace("\r\n", " / ").Replace("\n", " / ")); }
        return string.Join(" / ", r.ToArray());
    }
    public static string DialogButtons(IntPtr h) {
        List<string> r = new List<string>();
        foreach (IntPtr c in Children(h, "Button")) r.Add(TextOf(c).Replace("&", "") + "=" + GetDlgCtrlID(c));
        return string.Join(", ", r.ToArray());
    }
    // Close: WM_COMMAND IDOK if the dialog has an OK button, else IDCANCEL, else WM_CLOSE. Returns what was posted.
    public static string CloseDialog(IntPtr h) {
        IntPtr b = GetDlgItem(h, 1);
        if (b != IntPtr.Zero) { PostMessage(h, 0x0111, (IntPtr)1, b); return "WM_COMMAND IDOK"; }
        b = GetDlgItem(h, 2);
        if (b != IntPtr.Zero) { PostMessage(h, 0x0111, (IntPtr)2, b); return "WM_COMMAND IDCANCEL"; }
        PostMessage(h, 0x0010, IntPtr.Zero, IntPtr.Zero); return "WM_CLOSE";
    }
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
}
"@

# The game keeps the log open for writing, so it must be read with FileShare.ReadWrite (ReadAllText fails).
function ReadShared([string]$path, [long]$from = 0) {
    try {
        $fs = [IO.File]::Open($path, 'Open', 'Read', 'ReadWrite,Delete')
        try { if ($from -gt 0) { $fs.Seek([Math]::Min($from, $fs.Length), 'Begin') | Out-Null }; return (New-Object IO.StreamReader($fs)).ReadToEnd() } finally { $fs.Close() }
    } catch { return "" }
}
function Ctl([string]$cmdline) {
    # "save", "snap" and "clear" take one path, which may contain spaces; the others take whitespace-separated words.
    $head = $cmdline.Trim() -split '\s+', 2
    if ($head[0] -in "save", "snap", "clear") { $parts = @($head | Where-Object { $_ }) }
    else { $parts = @($cmdline.Trim() -split '\s+' | Where-Object { $_ }) }
    # PowerShell 5.1 turns a native command's stderr into terminating errors under "Stop"; a failed action must
    # not end the run.
    # Each call gets 60 s: PrintWindow on a game window that has stopped responding never returns (slot 4,
    # 2026-10-24: the end snap blocked the run, and its game slot, for 25 minutes after the timeout).
    $psi = New-Object System.Diagnostics.ProcessStartInfo "powershell.exe"
    $psi.Arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$Ctl`" " + (($parts | ForEach-Object { '"' + ($_ -replace '"', '\"') + '"' }) -join " ")
    $psi.UseShellExecute = $false; $psi.RedirectStandardOutput = $true; $psi.RedirectStandardError = $true; $psi.CreateNoWindow = $true
    $proc = [System.Diagnostics.Process]::Start($psi)
    $stdout = $proc.StandardOutput.ReadToEndAsync(); $stderr = $proc.StandardError.ReadToEndAsync()
    if (-not $proc.WaitForExit(60000)) {
        try { $proc.Kill() } catch { }
        return "sc4ctl $($parts[0]) timed out after 60 s"
    }
    $out = @(($stdout.Result + $stderr.Result) -split "\r?\n" | Where-Object { $_ })
    return ($out | ForEach-Object { "$_" }) -join " | "
}
function MainCheckout {
    $here = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
    $i = $here.IndexOf("\.claude\worktrees\", [StringComparison]::OrdinalIgnoreCase)
    if ($i -gt 0) { return $here.Substring(0, $i) }
    return $here
}
# PIDs of the SimCity 4 processes that use user dir $dir (from their command lines; no -UserDir = Documents).
function GamesUsing([string]$dir) {
    $want = $dir.TrimEnd('\').ToLower()
    $all = @(Get-HarnessGameProcesses)
    $r = @()
    foreach ($w in $all) {
        # Still fail closed when neither query can identify a live process.
        if (-not $w.CommandLine) { $r += [int]$w.ProcessId; continue }
        $ud = $DocsUser
        if ("$($w.CommandLine)" -match '-UserDir:"?([^"]+)') { $ud = $Matches[1].Trim() }
        if ($ud.TrimEnd('\').ToLower() -eq $want) { $r += [int]$w.ProcessId }
    }
    return $r
}
function HasKey([string]$text, [string]$key) { return $text -match "(?m)^\s*$key\s*=" }
# The last periodic heartbeat's date against the first time this loop saw that date. $state is @{ date; since }.
# Returns the stalled verdict text once the date has not moved for $limit seconds while OnTick counts, else $null.
function FrozenCalendar([string]$text, [hashtable]$state, [datetime]$now, [int]$limit) {
    $beats = [regex]::Matches($text, 'heartbeat \(periodic\): (\d{4}-\d\d-\d\d) \(julian \d+\)[^
]*?pause (\d+) hidden (\d+) emergency (\d+)[^
]*?\+(\d+) OnTick')
    if ($beats.Count -eq 0) { return $null }
    $b = $beats[$beats.Count - 1]
    if ($state.date -ne $b.Groups[1].Value) { $state.date = $b.Groups[1].Value; $state.since = $now; return $null }
    if ([int64]$b.Groups[5].Value -le 0) { return $null }
    $for = [int]($now - $state.since).TotalSeconds
    if ($for -lt $limit) { return $null }
    return "stalled (fallback): frozen calendar at $($state.date) for $for s while the simulator ticked (pause $($b.Groups[2].Value) hidden $($b.Groups[3].Value) emergency $($b.Groups[4].Value))"
}
function JulianDate([int64]$jd) { return ([DateTime]"2000-01-01").AddDays($jd - 2451545).ToString("yyyy-MM-dd") }
# Win32 dialogs of the game (see FC4Run.Dialogs). Each one is logged once with its title, text and buttons, and closed
# unless -NoDismiss; a dialog still open after 3 close attempts is reported as such. Returns new notes (for the output).
$dialogState = @{}; $dialogNotes = New-Object System.Collections.ArrayList
function DialogInfo([IntPtr]$h) { return @{ h = $h; title = [FC4Run]::TextOf($h); text = [FC4Run]::DialogText($h); buttons = [FC4Run]::DialogButtons($h) } }
function HandleDialogs([string]$phase, [switch]$NoClose) {
    $out = @()
    if (-not $p) { return $out }
    foreach ($k in @($dialogState.Keys)) {   # dialogs that went away after a close
        $st = $dialogState[$k]
        if (-not $st.gone -and $st.tries -gt 0 -and -not [FC4Run]::IsWindow([IntPtr]$k)) {
            $st.gone = $true; $dialogNotes[$st.idx] += ", closed"; $out += "dialog '$($st.title)' closed"
        }
    }
    if ($p.HasExited) { return $out }
    foreach ($h in @([FC4Run]::Dialogs($p.Id))) {
        $k = [int64]$h
        if (-not $dialogState.ContainsKey($k)) {
            $d = DialogInfo $h
            $dialogState[$k] = @{ idx = $dialogNotes.Count; tries = 0; gone = $false; title = $d.title; ended = $false }
            [void]$dialogNotes.Add("DIALOG ($phase, $(Get-Date -Format 'HH:mm:ss')): '$($d.title)': $($d.text) [buttons: $($d.buttons)]")
            $out += $dialogNotes[$dialogState[$k].idx]
        }
        $st = $dialogState[$k]
        if ($NoClose -or $NoDismiss) {
            if ($phase -eq "end" -and -not $st.ended) { $st.ended = $true; $dialogNotes[$st.idx] += ", still open when the game was stopped" }
            continue
        }
        if ($st.tries -lt 3) {
            $how = [FC4Run]::CloseDialog($h); $st.tries++
            $dialogNotes[$st.idx] += $(if ($st.tries -eq 1) { " -> posted $how" } else { ", again (try $($st.tries))" })
            $out += "dialog '$($st.title)': posted $how (try $($st.tries))"
        } elseif ($st.tries -eq 3) {
            $st.tries++; $dialogNotes[$st.idx] += ", still open after 3 tries"; $out += "dialog '$($st.title)': still open after 3 tries"
        }
    }
    return $out
}
function TakeMutex([string]$name, [int]$ms) {
    $m = New-Object System.Threading.Mutex($false, $name)
    try { if ($m.WaitOne($ms)) { return $m } } catch [System.Threading.AbandonedMutexException] { return $m }
    $m.Dispose(); return $null
}

if (-not $IniFile -and -not $PSBoundParameters.ContainsKey("Ini")) { Write-Output "BAD ARGS: give -Ini or -IniFile"; exit 4 }
if ($IniFile) {
    if (-not (Test-Path -LiteralPath $IniFile)) { Write-Output "BAD ARGS: no such ini file: $IniFile"; exit 4 }
    $Ini = [IO.File]::ReadAllText((Resolve-Path -LiteralPath $IniFile).Path)
}
foreach ($d in @($EndDate, $TurboFrom)) { if ($d -and $d -notmatch '^\d{4}-\d\d-\d\d$') { Write-Output "BAD ARGS: '$d' is not YYYY-MM-DD"; exit 4 } }
if ($Region -notmatch '^[A-Za-z0-9][A-Za-z0-9 ._()-]*$' -or $Region.EndsWith('.') -or $Region.EndsWith(' ')) { Write-Output "BAD ARGS: region '$Region' must be one folder name"; exit 4 }
$regionSource = Join-Path $DocsUser "Regions\$Region"
if (-not (Test-Path -LiteralPath $regionSource -PathType Container)) { Write-Output "BAD ARGS: no such region: $regionSource"; exit 4 }

if ($Slot -gt 0) {
    $root = if ($SlotRoot) { $SlotRoot } elseif ($env:FC4_SLOT_ROOT) { $env:FC4_SLOT_ROOT } else { Join-Path (MainCheckout) "build-runs\sc4slots" }
    $UserDir = [IO.Path]::GetFullPath((Join-Path $root ("slot{0:D2}" -f $Slot)))
    if ($UserDir -match '\\\.') { Write-Output "BLOCKED: slot user dir $UserDir has a path component starting with '.'; the game would ignore it and use Documents (use -SlotRoot)"; exit 2 }
} else { $UserDir = $DocsUser }
$Plugins = Join-Path $UserDir "Plugins"
$Log = Join-Path $Plugins "FreeCity4.log"
# A slot mirrors the selected region under the name FC4 Test, as its only region (see the mirror below).
$CityDir = if ($Slot -gt 0) { Join-Path $UserDir "Regions\FC4 Test" } else { Join-Path $UserDir "Regions\$Region" }
if ($Slot -eq 0 -and $Region -ne 'FC4 Test') { Write-Output "BAD ARGS: -Region other than FC4 Test needs -Slot"; exit 4 }
$cityFixture = Resolve-HarnessCityFixture $Ini $Slot $City (Join-Path (MainCheckout) 'build-runs\fixtures')
if ($cityFixture.Source -and $Region -ne 'FC4 Test') { Write-Output "BAD ARGS: fixture_city requires region FC4 Test"; exit 4 }
$Ini = $cityFixture.Ini
if ($cityFixture.Source) { $null = Resolve-HarnessFixtureDestination $UserDir $DocsUser $City }
$suiteState = Resolve-HarnessSuiteState $Ini $Slot (MainCheckout)
$Ini = $suiteState.Ini
$monthlyConfig = ''
$monthlyPattern = '(?im)^[ \t]*golden_monthly[ \t]*=[ \t]*([^\r\n]*)\r?$'
$monthlyMatches = [regex]::Matches($Ini, $monthlyPattern)
if ($monthlyMatches.Count) {
    if ($Slot -le 0 -or $monthlyMatches.Count -ne 1) { throw 'golden_monthly requires one declaration and an isolated slot' }
    $monthlyConfig = [IO.Path]::GetFullPath($monthlyMatches[0].Groups[1].Value.Trim())
    $mainPrefix = [IO.Path]::GetFullPath((MainCheckout)).TrimEnd('\') + '\'
    if (-not $monthlyConfig.StartsWith($mainPrefix, [StringComparison]::OrdinalIgnoreCase) -or
        $monthlyConfig.Substring($mainPrefix.Length) -notmatch '^(?:\.claude\\worktrees\\[^\\]+\\)?build-runs\\golden-runs\\[^\\]+\\monthly-config\.json$') {
        throw 'golden_monthly must name a run configuration under build-runs/golden-runs'
    }
    $Ini = [regex]::Replace($Ini, $monthlyPattern, '')
    Write-Output "golden monthly: configuration $monthlyConfig"
}
if ($suiteState.Export -and $KeepSaves) { throw 'export_city requires normal slot backup restoration' }
if ($suiteState.Source -and $cityFixture.Source) { throw 'prepared_city and fixture_city cannot be combined' }
if ($suiteState.Source -or $suiteState.Export) { $null = Resolve-HarnessFixtureDestination $UserDir $DocsUser $City }
New-Item -ItemType Directory -Force $OutDir | Out-Null

$plan = @()
foreach ($a in ($Actions -split ';')) {
    if ($a.Trim() -match '^(\d+)\s*=\s*(.+)$') { $plan += , @([int]$Matches[1], $Matches[2].Trim()) }
    elseif ($a.Trim()) { Write-Output "BAD ACTION: '$a' (want seconds=command)"; exit 4 }
}
if ($Slot -gt 0) {
    foreach ($a in $plan) {
        if ($a[1] -match '^(realkey|realclick|realdrag|save)\b') { Write-Output "BAD ACTION: '$($a[1])' needs real input (the foreground), which slot runs never use$(if ($a[1] -match '^save') { '; a save needs a plugin-side script action' })"; exit 4 }
    }
}
$nextAction = 0

# One harness run per user dir at a time.
$dirLock = TakeMutex ("FC4UserDir-" + ($UserDir.TrimEnd('\').ToLower() -replace '[^a-z0-9]', '_')) 60000
if (-not $dirLock) { Write-Output "BLOCKED: another harness run is using $UserDir"; exit 2 }

# A previous run's game can still be exiting (a quit ending unloads for a few seconds): wait up to 60 s.
# One answer for the loop and the verdict: a game whose command line cannot be read counts as busy, and a
# second query can catch one that the loop did not.
$busy = @(GamesUsing $UserDir)
for ($i = 0; $busy.Count -gt 0 -and $i -lt 30; $i++) { Start-Sleep -Seconds 2; $busy = @(GamesUsing $UserDir) }
if ($busy.Count -gt 0) {
    Write-Output "BLOCKED: SimCity 4 is already running on $UserDir (pid $($busy -join ', ')); close it first"
    exit 2
}

# The DLL: a slot gets -Dll (else the one staged in Documents); without -Slot, -Dll is staged in Documents.
$dllSrc = if ($Dll) { $Dll } elseif ($Slot -gt 0) { Join-Path $DocsUser "Plugins\FreeCity4.dll" } else { "" }
if ($dllSrc -and -not (Test-Path -LiteralPath $dllSrc)) { Write-Output "BAD ARGS: no such DLL: $dllSrc"; exit 4 }
if ($Slot -gt 0) {
    New-Item -ItemType Directory -Force $Plugins, (Join-Path $UserDir "Regions") | Out-Null
    Copy-Item (Join-Path $DocsUser "SimCity 4.cfg") (Join-Path $UserDir "SimCity 4.cfg") -Force
    # The cfg may remember any region (Timbuktu, say); with FC4 Test the slot's only region, the game opens it.
    foreach ($d in Get-ChildItem -LiteralPath (Join-Path $UserDir "Regions") -Directory) { if ($d.Name -ne "FC4 Test") { Remove-Item -LiteralPath $d.FullName -Recurse -Force } }
    # Mirror only the selected region (copies only what differs, removes what the last run added).
    & robocopy $regionSource $CityDir /MIR /R:2 /W:1 /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -ge 8) { Write-Output "BLOCKED: could not mirror the region into $CityDir (robocopy $LASTEXITCODE)"; exit 3 }
    if ($cityFixture.Source) {
        $fixtureCopy = Copy-HarnessCityFixture $cityFixture.Source $UserDir $DocsUser $City
        Write-Output "fixture_city: $($fixtureCopy.SHA256) -> $($fixtureCopy.Destination) (source preserved)"
    }
    if ($suiteState.Source) {
        $stateCopy = Copy-HarnessCityFixture $suiteState.Source $UserDir $DocsUser $City
        Write-Output "prepared_city: $($stateCopy.SHA256) -> $($stateCopy.Destination) (source preserved)"
    }
    foreach ($f in Get-ChildItem -LiteralPath $Plugins -File) { if ($f.Name -notin "FreeCity4.dll", "FreeCity4.pdb", "FreeCity4.log") { Remove-Item -LiteralPath $f.FullName -Force } }
}
if ($Dll -or $Slot -gt 0) {
    Copy-Item -LiteralPath $dllSrc -Destination (Join-Path $Plugins "FreeCity4.dll") -Force
    $pdb = [IO.Path]::ChangeExtension($dllSrc, ".pdb")
    if (Test-Path -LiteralPath $pdb) { Copy-Item -LiteralPath $pdb -Destination (Join-Path $Plugins "FreeCity4.pdb") -Force }
    elseif (Test-Path -LiteralPath (Join-Path $Plugins "FreeCity4.pdb")) { Remove-Item -LiteralPath (Join-Path $Plugins "FreeCity4.pdb") -Force }
}
$stagedDll = Join-Path $Plugins "FreeCity4.dll"
$dllHash = if (Test-Path -LiteralPath $stagedDll) { (Get-FileHash -Algorithm SHA256 -LiteralPath $stagedDll).Hash.Substring(0, 8).ToLower() } else { "none" }

# The ini: the caller's text, the city, the autotest defaults, the turbo / end-date switches.
$text = $Ini
if ($City) {
    if ($City -notmatch '^[A-Za-z0-9 ._()''-]+$') { Write-Output "BAD ARGS: city name '$City'"; exit 4 }
    $cityFile = Join-Path $CityDir "City - $City.sc4"
    if (-not (Test-Path -LiteralPath $cityFile)) { Write-Output "BAD ARGS: no city file $cityFile"; exit 4 }
    $text = ($text -replace '(?m)^[ \t]*autoload_city[ \t]*=.*$', '') + "`nautoload_city = $cityFile"
} elseif ($Slot -gt 0) {
    $text = [regex]::Replace($text, '(?m)^([ \t]*autoload_city[ \t]*=[ \t]*)([^\r\n]+)', [Text.RegularExpressions.MatchEvaluator] {
            param($m)
            $v = $m.Groups[2].Value.Trim()
            if ($v.StartsWith($DocsUser + '\', [StringComparison]::OrdinalIgnoreCase)) { return $m.Groups[1].Value + $UserDir + $v.Substring($DocsUser.Length) }
            return $m.Value })
}
$auto = ""
if (-not (HasKey $text "autotest_seconds")) { $auto += "autotest_seconds = $AfterLoadSeconds`n" }
if (-not (HasKey $text "autotest_unpause")) { $auto += "autotest_unpause = 1`n" }
if ($EndDate -and -not (HasKey $text "autotest_end_date")) { $auto += "autotest_end_date = $EndDate`n" }
if ($Turbo -and -not (HasKey $text "sim_turbo")) { $auto += "sim_turbo = 1`n" }
if ($TurboFrom) {
    if (HasKey $text "script") { Write-Output "BAD ARGS: -TurboFrom writes its own script file, but the ini already has script = ..."; exit 4 }
    Set-Content -Path (Join-Path $Plugins "fc4-turbo-from.txt") -Value "# ingame_run -TurboFrom`n$TurboFrom turbo on`n" -Encoding ascii
    $auto += "script = fc4-turbo-from.txt`n"
}
Set-Content -Path (Join-Path $Plugins "FreeCity4.ini") -Value ("# ingame_run $Label`n" + $auto + $text) -Encoding ascii
# The log can stay locked for a few seconds while a previous game process finishes exiting.
for ($i = 0; ; $i++) {
    try { Set-Content -Path $Log -Value "" -Encoding ascii -ErrorAction Stop; break }
    catch { if ($i -ge 20) { Write-Output "BLOCKED: $Log is locked: $($_.Exception.Message)"; exit 3 }; Start-Sleep -Seconds 3 }
}

$cityPath = if ($text -match "autoload_city\s*=\s*([^
]+)") { $Matches[1].Trim() } else { $null }
$cityBak = $null
if ($Slot -eq 0 -and $cityPath -and (Test-Path $cityPath) -and -not $KeepSaves) {
    $cityHash = (Get-FileHash $cityPath).Hash
    $cityBak = Join-Path $OutDir ("city-backups\{0}.{1}.bak" -f [IO.Path]::GetFileNameWithoutExtension($cityPath), $cityHash.Substring(0, 8))
    if (-not (Test-Path $cityBak)) { New-Item -ItemType Directory -Force (Split-Path $cityBak) | Out-Null; Copy-Item $cityPath $cityBak }
}

# Start the game under the launch lock: no other harness game may start before this one's mutex is swapped.
$launch = TakeMutex "FC4GameLaunch" 300000
if (-not $launch) { Write-Output "BLOCKED: no launch turn within 300 s (another harness run is starting a game)"; exit 2 }
$verdict = $null; $failedStart = $null; $mutexLine = ""; $cpuNote = ""; $p = $null
try {
    $foreign = $true
    for ($i = 0; $i -lt 15; $i++) {
        $m = $null
        try { $exists = [System.Threading.Mutex]::TryOpenExisting("SimCity 4 Mutex", [ref]$m) } catch { $exists = $true }
        if ($m) { $m.Dispose() }
        if (-not $exists) { $foreign = $false; break }
        Start-Sleep -Seconds 2
    }
    if ($foreign) { $failedStart = "another SimCity 4 holds its single-instance mutex (a game this harness did not start?)" }
    else {
        $gameArgs = @("-CPUCount:1", "-w", "-intro:off")
        # The fixed schedule keeps the object pools to the main thread: models load there (the
        # background loader's worker can also hold the simulator paused, model-maker.md §15), and
        # audio is off (the sound update runs on the timer thread and frees exemplar properties).
        if ($Ini -match '(?im)^[ \t]*schedule[ \t]*=[ \t]*fixed[ \t]*\r?$') { $gameArgs += "-backgroundLoader:off", "-audio:off" }
        if ($Slot -gt 0) { $gameArgs += "-UserDir:`"$UserDir\`"" }
        # Optional Maxis command script; execute only a private slot copy, with the shared Lua helpers.
        $luaExtensions = $Ini -match '(?im)^[ \t]*lua_extensions[ \t]*=[ \t]*1[ \t]*\r?$'
        if ($luaExtensions) {
            if ($Slot -eq 0) { throw 'lua_extensions requires -Slot (third-party plugins are slot-only)' }
            $extensionSource = Join-Path (MainCheckout) 'build-runs\thirdparty\sc4-lua-extensions'
            foreach ($name in 'SC4LuaExtensions.dll', 'SC4LuaExtensions.dat') {
                $source = Join-Path $extensionSource $name
                if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "lua_extensions: missing $source" }
            }
            foreach ($name in 'SC4LuaExtensions.dll', 'SC4LuaExtensions.dat') {
                Copy-Item -LiteralPath (Join-Path $extensionSource $name) -Destination $Plugins -Force
            }
            Write-Output 'lua_extensions: staged DLL and DAT in the private slot Plugins folder'
        }
        if ($Ini -match '(?im)^[ \t]*lua_script[ \t]*=[ \t]*([^\r\n]+)') {
            if ($Slot -eq 0) { throw 'lua_script requires -Slot (scripts must never change Documents)' }
            $luaRelative = $Matches[1].Trim()
            if ([IO.Path]::IsPathRooted($luaRelative)) { throw 'lua_script must name an existing repo-relative .lua file' }
            $repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
            # The shared queue runs main's launcher with a lane's staged DLL.
            # Scripts and helpers must come from that DLL's checkout as well.
            if ($Dll) {
                $luaCheckout = [IO.DirectoryInfo](Split-Path -Parent ([IO.Path]::GetFullPath($Dll)))
                while ($luaCheckout -and -not (Test-Path -LiteralPath (Join-Path $luaCheckout.FullName '.git'))) {
                    $luaCheckout = $luaCheckout.Parent
                }
                if ($luaCheckout) { $repoRoot = $luaCheckout.FullName }
            }
            $luaSource = [IO.Path]::GetFullPath((Join-Path $repoRoot $luaRelative))
            if (-not $luaSource.StartsWith($repoRoot + '\', [StringComparison]::OrdinalIgnoreCase) -or
                [IO.Path]::GetExtension($luaSource) -ine '.lua' -or -not (Test-Path -LiteralPath $luaSource -PathType Leaf)) {
                throw 'lua_script must name an existing repo-relative .lua file'
            }
            $luaRoot = Join-Path $UserDir 'LuaScripts'
            $luaCopy = Join-Path $luaRoot $luaSource.Substring($repoRoot.Length + 1)
            $null = New-Item -ItemType Directory -Force (Split-Path -Parent $luaCopy)
            $luaHelpers = Join-Path $repoRoot 'engine\tests\scenarios\lua'
            if (Test-Path -LiteralPath $luaHelpers) {
                $luaHelperCopy = Join-Path $luaRoot 'engine\tests\scenarios\lua'
                $null = New-Item -ItemType Directory -Force $luaHelperCopy
                Copy-Item -Path (Join-Path $luaHelpers '*.lua') -Destination $luaHelperCopy -Force
            }
            $luaPrefix = "FC4_ROOT = '" + ($luaRoot.Replace('\', '/').Replace("'", "\'")) + "'`n"
            [IO.File]::WriteAllText($luaCopy, $luaPrefix + [IO.File]::ReadAllText($luaSource), (New-Object Text.UTF8Encoding($false)))
            $gameArgs += "-LuaScript:`"$luaCopy`""
            Write-Output "lua_script: $luaSource -> $luaCopy"
        }
        $docLog = Join-Path $DocsUser "Plugins\FreeCity4.log"
        $docLen = if (Test-Path $docLog) { (Get-Item $docLog).Length } else { 0 }
        if ($Slot -gt 0) { $p = [FC4Run]::Start($Exe, ($gameArgs -join " "), $GameDir, 4) }   # SW_SHOWNOACTIVATE
        else { $p = Start-Process -FilePath $Exe -ArgumentList $gameArgs -WorkingDirectory $GameDir -PassThru }
        if ($monthlyConfig) {
            # Run alongside the game at the SAME integrity level as this harness.
            # The caller's process may not have permission to open a queue game.
            $monthlyTool = Join-Path $PSScriptRoot 'golden_monthly.py'
            $monthlyArgs = @($monthlyTool, '--config', $monthlyConfig, '--slot', $UserDir, '--pid', [string]$p.Id)
            $monthlyQuoted = ($monthlyArgs | ForEach-Object { '"' + $_.Replace('"', '\"') + '"' }) -join ' '
            $monthlyProcess = Start-Process -FilePath 'python' -ArgumentList $monthlyQuoted -WindowStyle Hidden -PassThru `
                -RedirectStandardOutput ($monthlyConfig + '.out') -RedirectStandardError ($monthlyConfig + '.err')
            Write-Output "golden monthly: watcher $($monthlyProcess.Id) for game $($p.Id)"
        }
        # Keep the game resident: lane cargo builds (18 rustc, ~35 GB, commit 83%) paged the main thread out long enough
        # for the 10 s stall watchdog to fail a regress twice (train202, 2026-10-24). A 1.5 GB minimum working set makes
        # the memory manager trim the builds first.
        try { $p.MaxWorkingSet = [IntPtr](3GB); $p.MinWorkingSet = [IntPtr](1536MB) } catch { }
        # Wait for the plugin's attach line; watch for a driver error window and (slot) a fall-back to Documents.
        $attachDeadline = (Get-Date).AddSeconds(60)
        while ($true) {
            Start-Sleep -Milliseconds 250
            if ((ReadShared $Log) -match "\[pid $($p.Id) tid \d+\] DLL_PROCESS_ATTACH") { break }
            if ($p.HasExited) { $failedStart = "the game exited (code $($p.ExitCode)) before the plugin attached"; break }
            $dl = @([FC4Run]::Dialogs($p.Id))
            if ($dl.Count -gt 0) {
                $d = DialogInfo $dl[0]
                if ($d.title -match 'Error') {
                    [void]$dialogNotes.Add("DIALOG (start, $(Get-Date -Format 'HH:mm:ss')): '$($d.title)': $($d.text) [buttons: $($d.buttons)] -> failed start, game stopped")
                    $failedStart = "$($d.title): $($d.text)"; break
                }
                HandleDialogs "start" | ForEach-Object { Write-Output "  $_" }
            }
            $bad = @([FC4Run]::Titles($p.Id) | Where-Object { $_ -match 'Error' })
            if ($bad.Count -gt 0) { $failedStart = $bad[0]; break }
            if ($Slot -gt 0 -and (ReadShared $docLog $docLen) -match "\[pid $($p.Id) tid ") { $failedStart = "the game ignored -UserDir and used Documents"; break }
            if ((Get-Date) -gt $attachDeadline) { $failedStart = "no plugin attach line within 60 s"; break }
        }
        if (-not $failedStart) {
            $old = $ErrorActionPreference; $ErrorActionPreference = "Continue"
            try { $mutexLine = (& powershell -NoProfile -ExecutionPolicy Bypass -File $MutexTool -Pid $p.Id -WaitSeconds 30 2>&1 | ForEach-Object { "$_" }) -join " / " }
            finally { $ErrorActionPreference = $old }
            if ($Slot -gt 0) {
                $cpu = (2 * $Slot) % [Environment]::ProcessorCount; if ($cpu -eq 0) { $cpu = 1 }
                try { $p.ProcessorAffinity = [IntPtr]([int64]1 -shl $cpu); $cpuNote = "cpu $cpu" } catch { $cpuNote = "cpu affinity not set: $($_.Exception.Message)" }
            }
        }
        if ($failedStart -and -not $p.HasExited) { try { Stop-Process -Id $p.Id -Force -ErrorAction Stop } catch { } }
    }
} finally { $launch.ReleaseMutex(); $launch.Dispose() }

$env:FC4_GAME_PID = if ($p) { "$($p.Id)" } else { "" }
if ($GamePriority -and $p) { try { $p.PriorityClass = [Diagnostics.ProcessPriorityClass]$GamePriority } catch { Write-Output "game priority $GamePriority not set: $($_.Exception.Message)" } }
$env:FC4_GAME_LOG = $Log
if ($Slot -gt 0) { $env:FC4_NO_REAL_INPUT = "1" }
$start = Get-Date
$loadedAt = $null
$hpSeen = $null; $frozenDate = $null; $frozenCount = 0; $lastBeatCount = 0; $rounds = 0; $clicked = 0
$calState = @{ date = $null; since = $null }
$stallSeen = $null; $minimized = $false; $everMinimized = $false; $minTries = 0; $heldSince = $null; $heldDate = $null; $heldNote = $null; $heldMax = 0

while (-not $failedStart -and (Get-Date) - $start -lt [TimeSpan]::FromSeconds($Seconds)) {
    Start-Sleep -Seconds 3
    HandleDialogs "run" | ForEach-Object { Write-Output "  [$(Get-Date -Format 'HH:mm:ss')] $_" }
    $text = ReadShared $Log
    if ($text -match "AUTOTEST VERDICT: ([a-z]+)[^\r\n]*") { $verdict = $Matches[0]; break }
    if ($text -match "first-chance exception OUTSIDE FreeCity4\.dll[^\r\n]*") { $verdict = "corrupt (fallback): " + $Matches[0] }
    # A stall the watchdog reports as over ("stall over after") does not decide (FC4DiagLogic.h: finished stalls are
    # reported only; the plugin itself turns a stall past kAutotestStallMs into the verdict). The fallback is for a
    # plugin that cannot write one: a stall still not over 30 s after this loop first saw it.
    $stalls = [regex]::Matches($text, "WATCHDOG: main thread[^\r\n]*stalled[^\r\n]*")
    if ($stalls.Count -gt 0 -and $text.IndexOf("WATCHDOG: stall over", $stalls[$stalls.Count - 1].Index) -lt 0) {
        if (-not $stallSeen) { $stallSeen = Get-Date }
        elseif ((Get-Date) - $stallSeen -gt [TimeSpan]::FromSeconds(30)) { $verdict = "stalled (fallback): " + $stalls[$stalls.Count - 1].Value; Start-Sleep -Seconds 5; break }
    } else { $stallSeen = $null }
    # The budget Init hook is absent when budget is disabled (the shadows profile).
    # The plugin starts its autotest clock only after a loaded city's first tick.
    if (-not $loadedAt -and $text -match "config \(city load: budget Init\)|autotest: clock started;") { $loadedAt = Get-Date }
    if (-not $loadedAt) { if ($p.HasExited) { break }; continue }

    # A slot game leaves the screen once its city has loaded. SW_MINIMIZE, which hands the activation on when the game
    # has it: a game minimized while still active (SW_SHOWMINNOACTIVE) restores itself 3-4 s later, and one minimized
    # during the load can come back at its end, so this is checked again on every round (at most 5 times) until the
    # quit starts (the game shows its window again while it shuts down).
    if ($Slot -gt 0 -and -not $NoMinimize -and $text -notmatch "RequestQuit\(|AUTOTEST VERDICT") {
        $h = [FC4Run]::GameWindow($p.Id)
        if ($h -ne [IntPtr]::Zero) {
            if (-not [FC4Run]::IsIconic($h) -and $minTries -lt 5) { [FC4Run]::ShowWindow($h, 6) | Out-Null; $minTries++ }
            $minimized = [FC4Run]::IsIconic($h)
            if ($minimized) { $everMinimized = $true }
        }
    }

    if ($FrozenCalendarSeconds -gt 0) {
        $fz = FrozenCalendar $text $calState (Get-Date) $FrozenCalendarSeconds
        if ($fz) { $verdict = $fz; Start-Sleep -Seconds 2; break }
    }

    # A city held by a dialog: hidden > 0 in the last pause call.
    $hv = [regex]::Matches($text, "Hidden(?:Pause|Resume)\(\) -> 1 \(pause=\d+ hidden=(\d+) emergency=\d+; julian (\d+)")
    if ($hv.Count -gt 0 -and [int]$hv[$hv.Count - 1].Groups[1].Value -gt 0) {
        if (-not $heldSince) { $heldSince = Get-Date; $heldDate = JulianDate ([int64]$hv[$hv.Count - 1].Groups[2].Value) }
        $heldMax = [Math]::Max($heldMax, [int]$hv[$hv.Count - 1].Groups[1].Value)
        $heldFor = [int]((Get-Date) - $heldSince).TotalSeconds
        if ($heldFor -ge $HeldReportSeconds) { $heldNote = "HELD: the city was held by a dialog (hidden $heldMax) since game date $heldDate, for $heldFor s" }
    } else { $heldSince = $null }

    # Scheduled actions.
    while ($nextAction -lt $plan.Count -and ((Get-Date) - $loadedAt).TotalSeconds -ge $plan[$nextAction][0]) {
        $cmd = $plan[$nextAction][1]
        $res = Ctl $cmd
        Write-Output ("  [{0}] action @{1}s '{2}': {3}" -f (Get-Date -Format "HH:mm:ss"), $plan[$nextAction][0], $cmd, $res)
        $nextAction++
        $text = ReadShared $Log
    }

    # Modal dialogs: a new HiddenPause after the load, or a frozen date with OnTick counting and nothing delivered.
    # (Best effort; a minimized slot game cannot be looked at, so it is only reported, see HELD above.)
    if (-not $NoDismiss -and -not $minimized -and $text -notmatch "RequestQuit\(") {
        $hp = [regex]::Matches($text, "HiddenPause\(\) -> 1 \(pause=\d+ hidden=([1-9]\d*)").Count
        if ($null -eq $hpSeen) { $hpSeen = $hp }
        $why = $null
        if ($hp -gt $hpSeen) { $hpSeen = $hp; $why = "HiddenPause" }
        $beats = [regex]::Matches($text, "heartbeat \(periodic\): (\d{4}-\d\d-\d\d)[^\r\n]*?\+(\d+) OnTick, \+(\d+) deliveries")
        if ($beats.Count -gt $lastBeatCount) {
            $lastBeatCount = $beats.Count
            $b = $beats[$beats.Count - 1]
            $stuck = ([int64]$b.Groups[2].Value -gt 0) -and ([int64]$b.Groups[3].Value -eq 0)
            if ($stuck -and $b.Groups[1].Value -eq $frozenDate) { $frozenCount++ } elseif ($stuck) { $frozenDate = $b.Groups[1].Value; $frozenCount = 1 } else { $frozenDate = $null; $frozenCount = 0 }
            if ($frozenCount -ge 2 -and -not $why) { $why = "date frozen at $frozenDate" }
        }
        if ($why) {
            Start-Sleep -Seconds 1   # let the dialog draw
            $rounds++
            $res = Ctl ("clear " + (Join-Path $OutDir ("{0}-popup{1}-" -f $Label, $rounds)))
            $clicked += ([regex]::Matches($res, "clear: clicked")).Count
            Write-Output ("  [{0}] {1}: {2}" -f (Get-Date -Format "HH:mm:ss"), $why, $res)
            $text = ReadShared $Log
            $hpSeen = [regex]::Matches($text, "HiddenPause\(\) -> 1 \(pause=\d+ hidden=([1-9]\d*)").Count
        }
    }

    if (((Get-Date) - $loadedAt).TotalSeconds -ge $AfterLoadSeconds) {
        # Grace for the plugin's own verdict (regress r32d9: this fallback won by 0.6 s and killed a clean run).
        $graceEnd = (Get-Date).AddSeconds($VerdictGraceSeconds)
        while (-not $p.HasExited -and (Get-Date) -lt $graceEnd) {
            Start-Sleep -Seconds 1
            HandleDialogs "quit" | ForEach-Object { Write-Output "  [$(Get-Date -Format 'HH:mm:ss')] $_" }
            $text = ReadShared $Log
            if ($text -match "AUTOTEST VERDICT: ([a-z]+)[^\r\n]*") { $verdict = $Matches[0]; break }
        }
        if (-not $verdict) { $verdict = "clean (fallback): no foreign exception or stall $AfterLoadSeconds s after load" }
        break
    }
    if ($p.HasExited) { break }
}

# The loop can leave on "process exited" after a dialog round, with the verdict written since its last read
# (run 71: the city ended by the game, then the quit). Read the log once more.
if (-not $failedStart -and (-not $verdict -or $verdict -like "clean (fallback)*")) {
    $text = ReadShared $Log
    if ($text -match "AUTOTEST VERDICT: ([a-z]+)[^\r\n]*") { $verdict = $Matches[0] }
}
if ($failedStart) { $verdict = "failed start: $failedStart" }
elseif (-not $verdict) {
    if ($p.HasExited) { $verdict = "exited (code $($p.ExitCode)) without a verdict" }
    elseif (-not $loadedAt) { $verdict = "no city load within $Seconds s (region view? needs a click)" }
    else { $verdict = "timeout" }
}

if ($p -and -not $p.HasExited -and -not $failedStart) { HandleDialogs "end" -NoClose | Out-Null }
if ($p -and -not $p.HasExited) {
    # A run that ends badly keeps a picture of the window (diagnostics only; a minimized window is shown without
    # activation first).
    if ($Slot -gt 0 -and ($heldNote -or $verdict -notmatch "^AUTOTEST VERDICT")) {
        try {
            if ($minimized) { [FC4Run]::ShowWindow([FC4Run]::GameWindow($p.Id), 4) | Out-Null; Start-Sleep -Seconds 1 }
            $snap = Ctl ("snap " + (Join-Path $OutDir "$Label-end.png")); Write-Output "  end picture: $snap"
        } catch { }
    }
    try { Stop-Process -Id $p.Id -Force -ErrorAction Stop } catch { $verdict += " | WARNING: could not stop the game: $($_.Exception.Message)" }
}
Start-Sleep -Seconds 2
if ($monthlyProcess -and -not $monthlyProcess.HasExited) {
    # The final save is stable, but hashing it may outlive the game. Keep the
    # slot lease until capture finishes rather than killing a valid hash job.
    if (-not $monthlyProcess.WaitForExit(30000)) { Stop-Process -Id $monthlyProcess.Id -Force }
}
Copy-Item $Log (Join-Path $OutDir "$Label.log") -Force
# Preserve the optional driver stream with the log, before a slot is reused.
$drawStream = "$Log.draw.jsonl"
if (Test-Path -LiteralPath $drawStream) {
    Copy-Item -LiteralPath $drawStream -Destination (Join-Path $OutDir "$Label.draw.jsonl") -Force
}
if ($suiteState.Export) {
    try {
        if ($verdict -notmatch '^AUTOTEST VERDICT: clean\b' -or -not (Select-String -LiteralPath $Log -Pattern 'script: line .* save -> ok=1' -Quiet)) { throw 'export_city requires a clean run and a successful scripted save' }
        $null = Resolve-HarnessSuiteState ("export_city = " + $suiteState.Export) $Slot (MainCheckout)
        $exportHash = Export-HarnessSuiteState $cityPath $suiteState.Export $cityHash
        Write-Output "export_city: $exportHash -> $($suiteState.Export) (before slot restore)"
    } catch { $verdict = "state-export-failed: $($_.Exception.Message)" }
}
# Optional Lua evidence; never read another slot's or the install's shared logs.
if ($luaExtensions) {
    $luaEvidence = New-Object 'System.Collections.Generic.List[string]'
    foreach ($file in Get-ChildItem -LiteralPath $UserDir -Recurse -File -Filter '*.log' | Sort-Object FullName) {
        $content = ReadShared $file.FullName
        if ($file.Name -in 'SC4LuaExtensions.log', 'fc4-lua.log' -or $file.Name -match '^Lua') {
            $luaEvidence.Add("--- $($file.FullName.Substring($UserDir.Length + 1)) ---")
            $luaEvidence.Add($content)
        } else {
            foreach ($line in ($content -split '\r?\n')) {
                if ($line -match '(?i)\blua\b|FC4_LUA') { $luaEvidence.Add("$($file.Name): $line") }
            }
        }
    }
    $luaResult = Join-Path (MainCheckout) "build-runs\game-results\$Label.lua.log"
    $null = New-Item -ItemType Directory -Force (Split-Path -Parent $luaResult)
    [IO.File]::WriteAllLines($luaResult, $luaEvidence, (New-Object Text.UTF8Encoding($false)))
    Copy-Item -LiteralPath $luaResult -Destination (Join-Path $OutDir "$Label.lua.log") -Force
    Write-Output "lua log: $luaResult ($($luaEvidence.Count) records)"
}
# End optional Lua evidence.
if ($cityBak -and (Get-FileHash $cityPath).Hash -ne $cityHash) {
    Copy-Item $cityBak $cityPath -Force
    $restored = if ((Get-FileHash $cityPath).Hash -eq $cityHash) { "restored" } else { "RESTORE FAILED" }
    Write-Output "  the city file changed during the run (a save): $restored from $cityBak"
}
Write-Output "[$Label] $verdict"
foreach ($n in $dialogNotes) { Write-Output "  $n" }
if ($rounds -gt 0) { Write-Output "  dialog rounds: $rounds, buttons clicked: $clicked (screenshots: build-runs\$Label-popup*.png)" }
if ($nextAction -lt $plan.Count) { Write-Output "  WARNING: $($plan.Count - $nextAction) action(s) never ran" }
if ($heldNote) { Write-Output "  $heldNote" }
$t = ReadShared $Log
foreach ($pat in "budget switches OFF[^\r\n]*", "heartbeat \(periodic\)[^\r\n]*") {
    $m = [regex]::Matches($t, $pat)
    if ($m.Count -gt 0) { Write-Output ("  " + $m[$m.Count - 1].Value) }
}
$m = [regex]::Matches($t, "simulator: turbo month[^\r\n]*")
if ($m.Count -gt 0) { Write-Output ("  " + $m[$m.Count - 1].Value) }
Write-Output ("  game: {0}{1}, dll sha256 {2}{3}{4}" -f $(if ($p) { "pid $($p.Id)" } else { "not started" }),
    $(if ($Slot -gt 0) { ", slot $Slot ($UserDir)" } else { "" }), $dllHash, $(if ($cpuNote) { ", $cpuNote" } else { "" }),
    $(if ($Slot -gt 0 -and -not $NoMinimize) { $(if ($everMinimized) { ", minimized after the load" } else { ", NOT minimized" }) + " ($minTries ShowWindow call(s))" } else { "" }))
if ($mutexLine) { Write-Output "  $mutexLine" }
$dirLock.ReleaseMutex(); $dirLock.Dispose()
