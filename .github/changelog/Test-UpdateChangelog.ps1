#!/usr/bin/env pwsh
#Requires -Version 7.4

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Invoke-Git {
    [CmdletBinding()]
    [OutputType([object[]])]

    param([string[]] $Arguments)
    & git @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "git $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

function Write-TestFile {
    [CmdletBinding()]
    [OutputType([void])]

    param([string]$Path, [string]$Content)
    $fullPath = Join-Path (Get-Location) $Path
    $dir = Split-Path -Parent $fullPath
    if ($dir -and -not (Test-Path -LiteralPath $dir)) {
        $null = New-Item -ItemType Directory -Force -Path $dir
    }
    [System.IO.File]::WriteAllText($fullPath, $Content, $Utf8NoBom)
}

function Save-TestChange {
    [CmdletBinding()]
    [OutputType([void])]

    param([string]$Path, [string]$Content, [string]$Subject)
    Write-TestFile -Path $Path -Content $Content
    Invoke-Git -Arguments @('add', '--', $Path)
    Invoke-Git -Arguments @('commit', '-q', '-m', $Subject)
}

function Assert-IncludedText {
    [CmdletBinding()]
    [OutputType([void])]

    param([string]$Text, [string]$Expected)
    if (-not $Text.Contains($Expected)) {
        throw "Expected text to contain '$Expected'."
    }
}

function Assert-ExcludedText {
    [CmdletBinding()]
    [OutputType([void])]

    param([string]$Text, [string]$Unexpected)
    if ($Text.Contains($Unexpected)) {
        throw "Expected text not to contain '$Unexpected'."
    }
}

$ScriptRoot = Split-Path -Parent $PSCommandPath
$Updater = Join-Path $ScriptRoot 'Update-Changelog.ps1'
$TempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('pageframe.Changelog.' + [Guid]::NewGuid().ToString('N'))
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$PushedLocation = $false

try {
    $null = New-Item -ItemType Directory -Force -Path $TempRoot
    Push-Location -LiteralPath $TempRoot
    $PushedLocation = $true

    Invoke-Git -Arguments @('init', '-q')
    Invoke-Git -Arguments @('config', 'core.autocrlf', 'false')
    Invoke-Git -Arguments @('config', 'user.name', 'WhyKnot')
    Invoke-Git -Arguments @('config', 'user.email', 'whyknot@example.invalid')

    $initialChangelog = @'
# Changelog

## Unreleased

_No notable changes yet._

---

'@

    Save-TestChange -Path 'CHANGELOG.md' -Content $initialChangelog -Subject 'chore: initial changelog'
    Save-TestChange -Path 'src.txt' -Content "feature`n" -Subject 'feat(mesh): test changelog append'

    & $Updater -Mode Append -Range 'HEAD~1..HEAD' -RepoRoot $TempRoot
    if ($LASTEXITCODE -ne 0) { throw "Update-Changelog Append failed with exit code $LASTEXITCODE" }

    $changelog = [System.IO.File]::ReadAllText((Join-Path $TempRoot 'CHANGELOG.md'), $Utf8NoBom)
    Assert-IncludedText -Text $changelog -Expected '### Added'
    Assert-IncludedText -Text $changelog -Expected '**mesh:** Test changelog append'

    Save-TestChange -Path 'src.txt' -Content "bang`n" -Subject 'feat(api)!: drop the old paint call'
    Write-TestFile -Path 'src.txt' -Content "footer`n"
    Invoke-Git -Arguments @('add', '--', 'src.txt')
    $footer = 'BREAKING CHANGE: run is start.'
    Invoke-Git -Arguments @('commit', '-q', '-m', 'docs: rename the shell entry', '-m', $footer)

    & $Updater -Mode Append -Range 'HEAD~2..HEAD' -RepoRoot $TempRoot
    if ($LASTEXITCODE -ne 0) { throw "Update-Changelog breaking Append failed with exit code $LASTEXITCODE" }

    $breaking = [System.IO.File]::ReadAllText((Join-Path $TempRoot 'CHANGELOG.md'), $Utf8NoBom)
    $breakingSection = $breaking.Substring($breaking.IndexOf('### Breaking'))
    $breakingSection = $breakingSection.Substring(0, $breakingSection.IndexOf('### Added'))
    Assert-IncludedText -Text $breakingSection -Expected '**api:** Drop the old paint call'
    Assert-IncludedText -Text $breakingSection -Expected 'Rename the shell entry'

    Save-TestChange -Path 'src.txt' -Content "plain`n" -Subject 'Bump the font stack'
    Invoke-Git -Arguments @('checkout', '-q', '-b', 'upstream')
    Save-TestChange -Path 'upstream.txt' -Content "upstream`n" -Subject 'feat: upstream feature'
    Invoke-Git -Arguments @('checkout', '-q', '-')
    Invoke-Git -Arguments @('merge', '-q', '--no-ff', '-m', 'Merge upstream', 'upstream')

    & $Updater -Mode Append -Range 'HEAD~2..HEAD' -RepoRoot $TempRoot
    if ($LASTEXITCODE -ne 0) { throw "Update-Changelog merge Append failed with exit code $LASTEXITCODE" }

    $merged = [System.IO.File]::ReadAllText((Join-Path $TempRoot 'CHANGELOG.md'), $Utf8NoBom)
    Assert-ExcludedText -Text $merged -Unexpected 'Upstream feature'
    Assert-ExcludedText -Text $merged -Unexpected 'Bump the font stack'

    $callOptions1 = @{
        Mode     = 'Promote'
        Version  = 'v0.1.0'
        RepoRoot = $TempRoot
        Repo     = 'RealWhyKnot/Pageframe'
        NowUtc   = ([datetime]::Parse('2026-06-16T01:30:00Z'))
    }
    & $Updater @callOptions1
    if ($LASTEXITCODE -ne 0) { throw "Update-Changelog Promote failed with exit code $LASTEXITCODE" }

    $promoted = [System.IO.File]::ReadAllText((Join-Path $TempRoot 'CHANGELOG.md'), $Utf8NoBom)
    $heading = '## [v0.1.0](https://github.com/RealWhyKnot/Pageframe/releases/tag/' +
    'v0.1.0) - 2026-06-15'
    Assert-IncludedText -Text $promoted -Expected $heading

    $notes = (& $Updater -Mode Notes -ForVersion -Version 'v0.1.0' -RepoRoot $TempRoot) -join "`n"
    Assert-IncludedText -Text $notes -Expected '**mesh:** Test changelog append'

    $callOptions2 = @{
        Mode     = 'Promote'
        Version  = 'v0.1.1'
        RepoRoot = $TempRoot
        Repo     = 'RealWhyKnot/Pageframe'
        NowUtc   = ([datetime]::Parse('2026-06-16T01:30:00Z'))
    }
    & $Updater @callOptions2
    if ($LASTEXITCODE -ne 0) { throw "Update-Changelog empty Promote failed with exit code $LASTEXITCODE" }

    $emptyPromoted = [System.IO.File]::ReadAllText((Join-Path $TempRoot 'CHANGELOG.md'), $Utf8NoBom)
    Assert-IncludedText -Text $emptyPromoted -Expected '_No user-visible changes in this release._'
    Assert-ExcludedText -Text $emptyPromoted -Unexpected 'Maintenance release'

    Save-TestChange -Path 'src.txt' -Content "tagged`n" -Subject 'fix: tagged fix (#1270)'
    Invoke-Git -Arguments @('tag', 'v0.2.0')
    Save-TestChange -Path 'src.txt' -Content "later`n" -Subject 'feat: later feature'

    & $Updater -Mode Append -Range 'HEAD~2..HEAD' -RepoRoot $TempRoot
    if ($LASTEXITCODE -ne 0) { throw "Update-Changelog tagged Append failed with exit code $LASTEXITCODE" }

    $appended = [System.IO.File]::ReadAllText((Join-Path $TempRoot 'CHANGELOG.md'), $Utf8NoBom)
    Assert-ExcludedText -Text $appended -Unexpected '#1270'

    $callOptions3 = @{
        Mode     = 'Promote'
        Version  = 'v0.2.0'
        Through  = 'v0.2.0'
        RepoRoot = $TempRoot
        Repo     = 'RealWhyKnot/Pageframe'
        NowUtc   = ([datetime]::Parse('2026-06-16T01:30:00Z'))
    }
    & $Updater @callOptions3
    if ($LASTEXITCODE -ne 0) { throw "Update-Changelog tagged Promote failed with exit code $LASTEXITCODE" }

    $taggedNotes = (& $Updater -Mode Notes -ForVersion -Version 'v0.2.0' -RepoRoot $TempRoot) -join "`n"
    Assert-IncludedText -Text $taggedNotes -Expected 'Tagged fix'
    Assert-ExcludedText -Text $taggedNotes -Unexpected 'Later feature'

    $taggedChangelog = [System.IO.File]::ReadAllText((Join-Path $TempRoot 'CHANGELOG.md'), $Utf8NoBom)
    $unreleased = $taggedChangelog.Substring(0, $taggedChangelog.IndexOf('## [v0.2.0]'))
    Assert-IncludedText -Text $unreleased -Expected 'Later feature'
    Assert-ExcludedText -Text $unreleased -Unexpected 'Tagged fix'

    Write-Host 'Update-Changelog tests passed.'
} finally {
    if ($PushedLocation) { Pop-Location }
    if (Test-Path -LiteralPath $TempRoot) {
        Remove-Item -LiteralPath $TempRoot -Recurse -Force
    }
}

exit 0
