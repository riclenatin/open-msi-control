#Requires -Version 5.1
<#
.SYNOPSIS
Builds the release artifacts into dist/.

.DESCRIPTION
Runs `cargo build --release --workspace`, then writes
  dist/open-msi-control-<version>-portable.zip
  dist/open-msi-control-<version>-setup.exe   (Inno Setup 6, packaging/windows/installer.iss)
where <version> is the workspace version from Cargo.toml (no "v" prefix).

The asset names are a contract with the in-app update checker; do not change them.

.PARAMETER Iscc
Path to Inno Setup's ISCC.exe. Defaults to $env:ISCC, then the standard Inno Setup 6 locations.
#>
[CmdletBinding()]
param(
    [string]$Iscc
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Invoke-Native {
    param([Parameter(Mandatory)][string]$File, [string[]]$Arguments = @())
    & $File @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$File exited with code $LASTEXITCODE"
    }
}

function Resolve-Iscc {
    param([string]$Explicit)
    foreach ($given in @($Explicit, $env:ISCC)) {
        if ($given) {
            if (-not (Test-Path -LiteralPath $given -PathType Leaf)) {
                throw "ISCC.exe not found at '$given'"
            }
            return $given
        }
    }
    $defaults = @(
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
        'D:\packages\InnoSetup6\ISCC.exe'
    )
    foreach ($path in $defaults) {
        if (Test-Path -LiteralPath $path -PathType Leaf) {
            return $path
        }
    }
    throw "ISCC.exe not found. Install Inno Setup 6, or pass -Iscc / set `$env:ISCC. Looked in: $($defaults -join ', ')"
}

$root = Split-Path -Parent $PSScriptRoot
$manifest = Join-Path $root 'Cargo.toml'
$isccPath = Resolve-Iscc $Iscc

Invoke-Native cargo @('build', '--release', '--workspace', '--manifest-path', $manifest)

# The app crate inherits the workspace version; this is what it reports as its own version.
$metadata = (Invoke-Native cargo @('metadata', '--format-version', '1', '--no-deps', '--manifest-path', $manifest)) -join "`n" |
    ConvertFrom-Json
$app = @($metadata.packages | Where-Object { $_.name -eq 'open-msi-control' })
if ($app.Count -ne 1) {
    throw "Could not find the open-msi-control package in $manifest"
}
$version = $app[0].version
$binDir = Join-Path $metadata.target_directory 'release'

$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force -Path $dist | Out-Null

$zip = Join-Path $dist "open-msi-control-$version-portable.zip"
$setup = Join-Path $dist "open-msi-control-$version-setup.exe"

$portableFiles = @(
    (Join-Path $binDir 'open-msi-control.exe'),
    (Join-Path $binDir 'msi-probe.exe'),
    (Join-Path $root 'README.md'),
    (Join-Path $root 'LICENSE'),
    (Join-Path $root 'CHANGELOG.md')
)
foreach ($file in $portableFiles) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) {
        throw "Missing release input: $file"
    }
}

Write-Host "Packing $zip"
if (Test-Path -LiteralPath $zip) {
    Remove-Item -LiteralPath $zip
}
Compress-Archive -LiteralPath $portableFiles -DestinationPath $zip -CompressionLevel Optimal

Write-Host "Compiling installer with $isccPath"
Invoke-Native $isccPath @(
    "/DAppVersion=$version",
    "/DBinDir=$binDir",
    (Join-Path $PSScriptRoot 'windows\installer.iss')
)
if (-not (Test-Path -LiteralPath $setup -PathType Leaf)) {
    throw "ISCC finished but $setup is missing; check OutputDir/OutputBaseFilename in installer.iss"
}

Get-Item -LiteralPath $setup, $zip | Format-Table Name, Length -AutoSize
