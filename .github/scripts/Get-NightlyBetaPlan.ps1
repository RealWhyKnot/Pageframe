#!/usr/bin/env pwsh
#Requires -Version 7.4
[CmdletBinding()]
param(
    [string] $RepoRoot = '',
    [string] $Timezone = 'America/Chicago',
    [datetime] $NowUtc = ([datetime]::UtcNow),
    [string] $OutputPath = $env:GITHUB_OUTPUT
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Set-PlanOutput {
    [CmdletBinding()]
    [OutputType([void])]
    param(
        [Parameter(Mandatory)][System.Collections.Specialized.OrderedDictionary] $Values,
        [string] $Path
    )
    foreach ($key in $Values.Keys) {
        $line = "$key=$($Values[$key])"
        Write-Host $line
        if ($Path) {
            $full = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Path)
            [System.IO.File]::AppendAllText($full, $line + [Environment]::NewLine)
        }
    }
}

function Get-GitLine {
    [CmdletBinding()]
    [OutputType([string[]])]
    param(
        [Parameter(Mandatory)][string[]] $Arguments
    )
    $output = @(& git -C $script:root @Arguments)
    if ($LASTEXITCODE -ne 0) {
        throw "git $($Arguments -join ' ') failed ($LASTEXITCODE)."
    }
    $output
}

function Get-WorkspaceVersion {
    [CmdletBinding()]
    [OutputType([version])]
    param(
        [Parameter(Mandatory)][string] $ManifestPath
    )
    $inSection = $false
    foreach ($line in [System.IO.File]::ReadAllLines($ManifestPath)) {
        $trimmed = $line.Trim()
        if ($trimmed -match '^\[.*\]$') {
            $inSection = $trimmed -eq '[workspace.package]'
            continue
        }
        if ($inSection -and $trimmed -match '^version\s*=\s*"([^"]*)"') {
            $text = $Matches[1]
            if ($text -notmatch '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$') {
                throw "[workspace.package] version '$text' in $ManifestPath is not MAJOR.MINOR.PATCH."
            }
            return [version]::new([int] $Matches[1], [int] $Matches[2], [int] $Matches[3])
        }
    }
    throw "No [workspace.package] version in $ManifestPath."
}

function ConvertTo-StableVersion {
    [CmdletBinding()]
    [OutputType([version])]
    param(
        [Parameter(Mandatory)][string] $Tag
    )
    if ($Tag -match '^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$') {
        return [version]::new([int] $Matches[1], [int] $Matches[2], [int] $Matches[3])
    }
    $null
}

function Format-Version {
    [CmdletBinding()]
    [OutputType([string])]
    param(
        [Parameter(Mandatory)][version] $Version
    )
    '{0}.{1}.{2}' -f $Version.Major, $Version.Minor, $Version.Build
}

function Get-CommitKind {
    [CmdletBinding()]
    [OutputType([string])]
    param(
        [Parameter(Mandatory)][string] $Range
    )
    $log = (Get-GitLine -Arguments @('log', '--format=%s%x1f%b%x1e', $Range)) -join "`n"
    $kind = 'none'
    foreach ($record in $log -split [char]0x1e) {
        if (-not $record.Trim()) {
            continue
        }
        $parts = $record.Split([char]0x1f, 2)
        $subject = $parts[0].Trim()
        $body = if ($parts.Count -gt 1) { $parts[1] } else { '' }
        if ($subject -match '^[a-z]+(\([^)]+\))?!:' -or $body -match '(?m)^BREAKING[ -]CHANGE:') {
            return 'breaking'
        }
        if ($subject -match '^feat(\([^)]+\))?:') {
            $kind = 'feature'
        } elseif ($kind -eq 'none') {
            $kind = 'fix'
        }
    }
    $kind
}

function Get-NextVersion {
    [CmdletBinding()]
    [OutputType([version])]
    param(
        [Parameter(Mandatory)][version] $Base,
        [Parameter(Mandatory)][string] $Kind
    )
    if ($Base.Major -eq 0) {
        if ($Kind -eq 'breaking') {
            return [version]::new(0, $Base.Minor + 1, 0)
        }
        return [version]::new(0, $Base.Minor, $Base.Build + 1)
    }
    switch ($Kind) {
        'breaking' { return [version]::new($Base.Major + 1, 0, 0) }
        'feature' { return [version]::new($Base.Major, $Base.Minor + 1, 0) }
        default { return [version]::new($Base.Major, $Base.Minor, $Base.Build + 1) }
    }
}

$script:root = if ($RepoRoot) { $RepoRoot } else { (Get-Location).Path }
$script:root = @(Get-GitLine -Arguments @('rev-parse', '--show-toplevel'))[0]
$cargoVersion = Get-WorkspaceVersion -ManifestPath (Join-Path $script:root 'Cargo.toml')
$tags = @(Get-GitLine -Arguments @('tag', '--list', 'v*'))

$released = @(foreach ($tag in $tags) {
        $parsed = ConvertTo-StableVersion -Tag $tag
        if ($parsed) {
            $parsed
        }
    })
$stable = @($released | Sort-Object -Descending)
$cargoTag = 'v' + (Format-Version -Version $cargoVersion)

if ($tags -notcontains $cargoTag) {
    $target = $cargoVersion
    Write-Host "Cargo.toml asks for $cargoTag, which has no release yet."
} else {
    $latest = $stable[0]
    $latestTag = 'v' + (Format-Version -Version $latest)
    $kind = Get-CommitKind -Range "$latestTag..HEAD"
    if ($kind -eq 'none') {
        Write-Host "No commits since $latestTag."
        Set-PlanOutput -Values ([ordered]@{ has_changes = 'false'; next_tag = '' }) -Path $OutputPath
        exit 0
    }
    $target = Get-NextVersion -Base $latest -Kind $kind
    Write-Host "Commits since $latestTag make a $kind release."
}

$zone = [System.TimeZoneInfo]::FindSystemTimeZoneById($Timezone)
$utc = if ($NowUtc.Kind -eq [System.DateTimeKind]::Utc) { $NowUtc } else { $NowUtc.ToUniversalTime() }
$day = [System.TimeZoneInfo]::ConvertTimeFromUtc($utc, $zone).ToString(
    'yyyyMMdd', [System.Globalization.CultureInfo]::InvariantCulture)

$prefix = 'v' + (Format-Version -Version $target) + "-beta.$day."
$pattern = '^' + [regex]::Escape($prefix) + '(0|[1-9]\d*)$'
$highest = -1
foreach ($tag in $tags) {
    if ($tag -match $pattern -and [int] $Matches[1] -gt $highest) {
        $highest = [int] $Matches[1]
    }
}

$nextTag = $prefix + ($highest + 1)
Set-PlanOutput -Values ([ordered]@{ has_changes = 'true'; next_tag = $nextTag }) -Path $OutputPath
exit 0
