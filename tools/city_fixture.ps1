# Optional read-only fixture input for isolated game runs. Never stages into Documents.
function Resolve-HarnessCityFixture([string]$Ini, [int]$Slot, [string]$City, [string]$FixtureRoot) {
    $pattern = '(?im)^[ \t]*fixture_city[ \t]*=[ \t]*([^\r\n]*)\r?$'
    $fixtureMatches = [regex]::Matches($Ini, $pattern)
    if ($fixtureMatches.Count -eq 0) { return [pscustomobject]@{ Ini = $Ini; Source = '' } }
    if ($fixtureMatches.Count -ne 1) { throw 'fixture_city must appear exactly once' }
    if ($Slot -le 0 -or $City -notmatch '^[A-Za-z0-9 ._()-]+$') { throw 'fixture_city requires an isolated -Slot and a valid -City alias' }
    $source = $fixtureMatches[0].Groups[1].Value.Trim()
    if (-not $source -or -not [IO.Path]::IsPathRooted($source)) { throw 'fixture_city requires an absolute file path' }
    $source = [IO.Path]::GetFullPath($source)
    $prefix = [IO.Path]::GetFullPath($FixtureRoot).TrimEnd('\') + '\'
    if (-not $source.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) { throw 'fixture_city must be under the main build-runs/fixtures directory' }
    if ([IO.Path]::GetExtension($source) -ine '.sc4' -or -not (Test-Path -LiteralPath $source -PathType Leaf)) { throw 'fixture_city must name an existing .sc4 file' }
    $stream = [IO.File]::OpenRead($source)
    try {
        $magic = New-Object byte[] 4
        if ($stream.Read($magic, 0, 4) -ne 4 -or [Text.Encoding]::ASCII.GetString($magic) -cne 'DBPF') { throw 'fixture_city is not a DBPF save' }
    } finally { $stream.Dispose() }
    return [pscustomobject]@{ Ini = [regex]::Replace($Ini, $pattern, ''); Source = $source }
}

function Resolve-HarnessFixtureDestination([string]$UserDir, [string]$DocsUser, [string]$City) {
    if ($City -notmatch "^[A-Za-z0-9 ._()'-]+$") { throw 'invalid fixture city alias' }
    $userFull = [IO.Path]::GetFullPath($UserDir).TrimEnd('\')
    $docsFull = [IO.Path]::GetFullPath($DocsUser).TrimEnd('\')
    if ($userFull.Equals($docsFull, [StringComparison]::OrdinalIgnoreCase) -or
        $userFull.StartsWith($docsFull + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'fixture destination must not be Documents' }
    $destination = Join-Path $userFull "Regions\FC4 Test\City - $City.sc4"
    # Reject junctions/symlinks anywhere in the destination chain before creating or copying.
    $cursor = $destination
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            $item = Get-Item -LiteralPath $cursor -Force
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'fixture destination traverses a reparse point' }
        }
        $parent = Split-Path -Parent $cursor
        if (-not $parent -or $parent -eq $cursor) { break }
        $cursor = $parent
    }
    return $destination
}

function Copy-HarnessCityFixture([string]$Source, [string]$UserDir, [string]$DocsUser, [string]$City) {
    $destination = Resolve-HarnessFixtureDestination $UserDir $DocsUser $City
    if ([IO.Path]::GetFullPath($Source).Equals($destination, [StringComparison]::OrdinalIgnoreCase)) { throw 'fixture source and destination must differ' }
    $before = (Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash
    New-Item -ItemType Directory -Force (Split-Path -Parent $destination) | Out-Null
    # Unlink an existing alias before copying, rather than overwriting a possible hard link.
    # No -Force: a read-only alias must fail without changing the original file's attributes.
    $cityPrefix = [IO.Path]::GetFullPath($UserDir).TrimEnd('\') + '\Regions\FC4 Test\'
    if (-not $destination.StartsWith($cityPrefix, [StringComparison]::OrdinalIgnoreCase)) { throw 'fixture destination escaped the isolated city directory' }
    if (Test-Path -LiteralPath $destination) {
        if ((Get-Item -LiteralPath $destination -Force).PSIsContainer) { throw 'fixture destination is a directory' }
        Remove-Item -LiteralPath $destination -ErrorAction Stop
    }
    Copy-Item -LiteralPath $Source -Destination $destination
    (Get-Item -LiteralPath $destination).IsReadOnly = $false
    $after = (Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash
    $copied = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
    if ($before -ne $after -or $before -ne $copied) { throw 'fixture changed during staging' }
    return [pscustomobject]@{ Source = $Source; Destination = $destination; SHA256 = $copied.ToLower() }
}

# Prepared suite saves are harness inputs/outputs, never plugin settings. Both
# ends live under an ignored suite/states directory, in main or a lane worktree.
function Resolve-HarnessSuiteState([string]$Ini, [int]$Slot, [string]$MainRoot) {
    $result = @{ Ini = $Ini; Source = ''; Export = '' }
    $mainFull = [IO.Path]::GetFullPath($MainRoot).TrimEnd('\')
    foreach ($key in @('prepared_city', 'export_city')) {
        $pattern = '(?im)^[ \t]*' + $key + '[ \t]*=[ \t]*([^\r\n]*)\r?$'
        $found = [regex]::Matches($Ini, $pattern)
        if ($found.Count -eq 0) { continue }
        if ($Slot -le 0 -or $found.Count -ne 1) { throw "$key requires an isolated slot and one declaration" }
        $value = $found[0].Groups[1].Value.Trim()
        if (-not [IO.Path]::IsPathRooted($value)) { throw "$key requires an absolute path" }
        $path = [IO.Path]::GetFullPath($value)
        $relative = if ($path.StartsWith($mainFull + '\', [StringComparison]::OrdinalIgnoreCase)) { $path.Substring($mainFull.Length + 1) } else { '' }
        if ($relative -notmatch '^(?:\.claude\\worktrees\\[^\\]+\\)?build-runs\\suite\\states\\.+\.sc4$') { throw "$key must be under build-runs/suite/states in main or a worktree" }
        $cursor = $path
        while ($cursor) {
            if ((Test-Path -LiteralPath $cursor) -and ((Get-Item -LiteralPath $cursor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw "$key traverses a reparse point" }
            $parent = Split-Path -Parent $cursor
            if (-not $parent -or $parent -eq $cursor) { break }
            $cursor = $parent
        }
        if ($key -eq 'prepared_city') {
            if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw 'prepared_city does not exist' }
            $stream = [IO.File]::OpenRead($path)
            try {
                $magic = New-Object byte[] 4
                if ($stream.Read($magic, 0, 4) -ne 4 -or [Text.Encoding]::ASCII.GetString($magic) -cne 'DBPF') { throw 'prepared_city is not a DBPF save' }
            } finally { $stream.Dispose() }
            $result.Source = $path
        } else {
            if (Test-Path -LiteralPath $path) { throw 'export_city already exists; use a new state tag' }
            $result.Export = $path
        }
        $result.Ini = [regex]::Replace($result.Ini, $pattern, '')
    }
    return [pscustomobject]$result
}

function Export-HarnessSuiteState([string]$Source, [string]$Destination, [string]$BeforeHash) {
    if (Test-Path -LiteralPath $Destination) { throw 'export_city already exists' }
    $hash = (Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash
    if ($hash -eq $BeforeHash) { throw 'export_city requires a changed saved city, not the loaded input' }
    New-Item -ItemType Directory -Force (Split-Path -Parent $Destination) | Out-Null
    Copy-Item -LiteralPath $Source -Destination $Destination -ErrorAction Stop
    if ((Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash -ne $hash) { throw 'export_city hash mismatch' }
    return $hash.ToLower()
}
