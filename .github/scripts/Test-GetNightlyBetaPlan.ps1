#!/usr/bin/env pwsh
#Requires -Version 7.4
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Invoke-Git {
    [CmdletBinding()]
    [OutputType([void])]
    param(
        [Parameter(Mandatory)][string[]] $Arguments
    )
    $output = & git -C $script:sandbox @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "git $($Arguments -join ' ') failed: $output"
    }
}

function Set-CargoVersion {
    [CmdletBinding()]
    [OutputType([void])]
    param(
        [Parameter(Mandatory)][string] $Version
    )
    $manifest = @(
        '[workspace]'
        'members = ["crates/app"]'
        ''
        '[workspace.package]'
        "version = `"$Version`""
        'publish = false'
        ''
        '[workspace.dependencies]'
        'serde = { version = "1.0.0" }'
        ''
    ) -join "`n"
    [System.IO.File]::WriteAllText((Join-Path $script:sandbox 'Cargo.toml'), $manifest)
}

function New-Commit {
    [CmdletBinding()]
    [OutputType([void])]
    param(
        [Parameter(Mandatory)][string] $Subject,
        [string] $Body = ''
    )
    [System.IO.File]::AppendAllText((Join-Path $script:sandbox 'log.txt'), $Subject + [Environment]::NewLine)
    Invoke-Git -Arguments @('add', '-A')
    $arguments = @('commit', '-q', '-m', $Subject)
    if ($Body) {
        $arguments = @('commit', '-q', '-m', $Subject, '-m', $Body)
    }
    Invoke-Git -Arguments $arguments
}

function Invoke-Plan {
    [CmdletBinding()]
    [OutputType([hashtable])]
    param(
        [datetime] $NowUtc = $script:noon
    )
    [System.IO.File]::WriteAllText($script:outFile, '')
    $parameters = @{
        RepoRoot   = $script:sandbox
        NowUtc     = $NowUtc
        OutputPath = $script:outFile
    }
    $global:LASTEXITCODE = 0
    $null = & $script:planner @parameters 6>&1
    if ($LASTEXITCODE -ne 0) {
        throw "The planner exited with $LASTEXITCODE."
    }
    $map = @{}
    foreach ($line in [System.IO.File]::ReadAllLines($script:outFile)) {
        if ($line -match '^([^=]+)=(.*)$') {
            $map[$Matches[1]] = $Matches[2]
        }
    }
    $map
}

function Assert-Plan {
    [CmdletBinding()]
    [OutputType([void])]
    param(
        [Parameter(Mandatory)][string] $Name,
        [AllowEmptyString()][string] $NextTag,
        [datetime] $NowUtc = $script:noon
    )
    $hasChanges = if ($NextTag) { 'true' } else { 'false' }
    $result = Invoke-Plan -NowUtc $NowUtc
    if ($result['has_changes'] -ne $hasChanges -or $result['next_tag'] -ne $NextTag) {
        Write-Host "FAIL  $Name -- got has_changes='$($result['has_changes'])' next_tag='$($result['next_tag'])'"
        Write-Host "      expected has_changes='$hasChanges' next_tag='$NextTag'"
        $script:failures++
        return
    }
    Write-Host "ok    $Name"
}

$script:planner = Join-Path $PSScriptRoot 'Get-NightlyBetaPlan.ps1'
$script:failures = 0
$script:sandbox = Join-Path ([System.IO.Path]::GetTempPath()) ('pageframe-beta-plan-' + [guid]::NewGuid().ToString('N'))
$script:outFile = Join-Path $script:sandbox '.git/plan-output.txt'
$script:noon = [datetime]::SpecifyKind([datetime]::Parse('2026-11-24T18:00:00'), [System.DateTimeKind]::Utc)
$day = '20261124'

try {
    $null = New-Item -ItemType Directory -Path $script:sandbox -Force
    Invoke-Git -Arguments @('init', '-q', '--initial-branch=main')
    Invoke-Git -Arguments @('config', 'user.name', 'Test')
    Invoke-Git -Arguments @('config', 'user.email', 'test@example.invalid')
    Invoke-Git -Arguments @('config', 'commit.gpgsign', 'false')
    Invoke-Git -Arguments @('config', 'core.hooksPath', '/dev/null')

    Set-CargoVersion -Version '0.1.0'
    New-Commit -Subject 'feat: first engine build'
    Assert-Plan -Name 'before the first release the beta targets Cargo.toml' -NextTag "v0.1.0-beta.$day.0"

    Invoke-Git -Arguments @('tag', "v0.1.0-beta.$day.0")
    New-Commit -Subject 'fix: second build on the same day'
    Assert-Plan -Name 'a second beta on the same day counts up' -NextTag "v0.1.0-beta.$day.1"

    Invoke-Git -Arguments @('tag', "v0.1.0-beta.$day.1")
    Invoke-Git -Arguments @('tag', 'v0.1.0')
    Assert-Plan -Name 'nothing since the stable release means no beta' -NextTag ''

    New-Commit -Subject 'fix(dom): keep focus after a reflow'
    Assert-Plan -Name 'a fix after v0.1.0 makes a patch beta' -NextTag "v0.1.1-beta.$day.0"

    New-Commit -Subject 'feat: add a layout hook'
    Assert-Plan -Name 'a feature before 1.0 stays a patch beta' -NextTag "v0.1.1-beta.$day.0"

    Invoke-Git -Arguments @('tag', "v0.1.1-beta.$day.0")
    New-Commit -Subject 'fix: same day again'
    Assert-Plan -Name 'the patch beta counts up on the same day' -NextTag "v0.1.1-beta.$day.1"

    $nextDay = $script:noon.AddDays(1)
    Assert-Plan -Name 'a new day starts again at .0' -NextTag 'v0.1.1-beta.20261125.0' -NowUtc $nextDay

    $lateUtc = [datetime]::SpecifyKind([datetime]::Parse('2026-11-25T03:00:00'), [System.DateTimeKind]::Utc)
    Assert-Plan -Name 'the Chicago date decides the day, not UTC' -NextTag "v0.1.1-beta.$day.1" -NowUtc $lateUtc

    New-Commit -Subject 'feat(script)!: drop the old event loop API'
    Assert-Plan -Name 'feat! after v0.1.0 makes a minor beta' -NextTag "v0.2.0-beta.$day.0"

    Set-CargoVersion -Version '1.0.0'
    New-Commit -Subject 'chore: mark the API stable'
    Invoke-Git -Arguments @('tag', 'v1.0.0')
    Assert-Plan -Name 'nothing since v1.0.0 means no beta' -NextTag ''

    New-Commit -Subject 'fix: trim a trailing space'
    Assert-Plan -Name 'a fix after v1.0.0 makes a patch beta' -NextTag "v1.0.1-beta.$day.0"

    New-Commit -Subject 'feat: add a paint callback'
    Assert-Plan -Name 'feat after v1.0.0 makes a minor beta' -NextTag "v1.1.0-beta.$day.0"

    $footer = 'BREAKING CHANGE: Shell::run is now Shell::start.'
    New-Commit -Subject 'refactor: rename the shell entry point' -Body $footer
    Assert-Plan -Name 'a BREAKING CHANGE footer after v1.0.0 makes a major beta' -NextTag "v2.0.0-beta.$day.0"

    Invoke-Git -Arguments @('tag', "v2.0.0-beta.$day.0")
    New-Commit -Subject 'fix: another fix' -Body 'BREAKING-CHANGE: the hyphen form counts too.'
    Assert-Plan -Name 'the major beta counts up on the same day' -NextTag "v2.0.0-beta.$day.1"

    Set-CargoVersion -Version '3.0.0'
    New-Commit -Subject 'chore: bump to 3.0.0'
    Assert-Plan -Name 'a Cargo.toml version with no tag yet wins' -NextTag "v3.0.0-beta.$day.0"
} finally {
    if (Test-Path -LiteralPath $script:sandbox) {
        Remove-Item -LiteralPath $script:sandbox -Recurse -Force -ErrorAction SilentlyContinue
    }
}

if ($script:failures -gt 0) {
    Write-Host "$($script:failures) planner test(s) failed."
    exit 1
}
Write-Host 'Get-NightlyBetaPlan tests passed.'
exit 0
