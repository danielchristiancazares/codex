[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
if ($PSVersionTable.PSVersion -ge [Version]"7.3") {
    $PSNativeCommandUseErrorActionPreference = $true
}

function Write-Step {
    param([string]$Message)

    Write-Host "==> $Message"
}

function Invoke-Checked {
    param(
        [string]$FilePath,
        [string[]]$Arguments
    )

    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$FilePath exited with code $LASTEXITCODE."
    }
}

function Get-FileSha256 {
    param([string]$Path)

    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
}

function Assert-PathWithin {
    param(
        [string]$Path,
        [string]$Root
    )

    $resolvedPath = [IO.Path]::GetFullPath($Path).TrimEnd("\")
    $resolvedRoot = [IO.Path]::GetFullPath($Root).TrimEnd("\")
    $isWithin = $resolvedPath.Equals(
        $resolvedRoot,
        [System.StringComparison]::OrdinalIgnoreCase
    ) -or $resolvedPath.StartsWith(
        "$resolvedRoot\",
        [System.StringComparison]::OrdinalIgnoreCase
    )
    if (-not $isWithin) {
        throw "Refusing to modify $resolvedPath because it is outside $resolvedRoot."
    }
}

function Assert-JunctionCanBeSet {
    param(
        [string]$LinkPath,
        [string]$InstallerOwnedTargetPrefix
    )

    if (-not (Test-Path -LiteralPath $LinkPath)) {
        return
    }
    $item = Get-Item -LiteralPath $LinkPath -Force

    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        if ($item.LinkType -ne "Junction") {
            throw "Refusing to replace non-junction reparse point at $LinkPath."
        }

        $existingTarget = [IO.Path]::GetFullPath([string]$item.Target).TrimEnd("\")
        $ownedPrefix = [IO.Path]::GetFullPath($InstallerOwnedTargetPrefix).TrimEnd("\")
        $isOwned = $existingTarget.Equals(
            $ownedPrefix,
            [System.StringComparison]::OrdinalIgnoreCase
        ) -or $existingTarget.StartsWith(
            "$ownedPrefix\",
            [System.StringComparison]::OrdinalIgnoreCase
        )
        if (-not $isOwned) {
            throw "Refusing to retarget junction at $LinkPath because it is not managed by this installer."
        }
        return
    }

    if (-not $item.PSIsContainer) {
        throw "Refusing to replace file at $LinkPath with a junction."
    }
    if ($null -ne (Get-ChildItem -LiteralPath $LinkPath -Force | Select-Object -First 1)) {
        throw "Refusing to replace nonempty directory at $LinkPath with a junction."
    }
}

function Set-LocalJunction {
    param(
        [string]$LinkPath,
        [string]$TargetPath,
        [string]$InstallerOwnedTargetPrefix
    )

    Assert-JunctionCanBeSet `
        -LinkPath $LinkPath `
        -InstallerOwnedTargetPrefix $InstallerOwnedTargetPrefix

    $existingTarget = $null
    if (Test-Path -LiteralPath $LinkPath) {
        $item = Get-Item -LiteralPath $LinkPath -Force
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            $existingTarget = [string]$item.Target
            if ($existingTarget.Equals($TargetPath, [System.StringComparison]::OrdinalIgnoreCase)) {
                return
            }
            Remove-Item -LiteralPath $LinkPath -Force
        } else {
            Remove-Item -LiteralPath $LinkPath -Force
        }
    }

    try {
        New-Item -ItemType Junction -Path $LinkPath -Target $TargetPath | Out-Null
    } catch {
        if ($null -ne $existingTarget) {
            New-Item -ItemType Junction -Path $LinkPath -Target $existingTarget | Out-Null
        }
        throw
    }
}

function Prepend-PathEntry {
    param(
        [AllowNull()]
        [string]$PathValue,
        [string]$Entry
    )

    $needle = $Entry.TrimEnd("\")
    $segments = @($Entry)
    if (-not [string]::IsNullOrWhiteSpace($PathValue)) {
        $segments += $PathValue.Split(";", [System.StringSplitOptions]::RemoveEmptyEntries) |
            Where-Object { $_.TrimEnd("\") -ine $needle }
    }
    return $segments -join ";"
}

if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows
)) {
    throw "This script is intended for Windows."
}

$repoRoot = $PSScriptRoot
$architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
$target = switch ($architecture) {
    "X64" { "x86_64-pc-windows-msvc" }
    "Arm64" { "aarch64-pc-windows-msvc" }
    default { throw "Unsupported Windows architecture: $architecture" }
}

$justCommand = Get-Command just.exe -ErrorAction SilentlyContinue
if ($null -eq $justCommand) {
    $justCommand = Get-Command just -ErrorAction SilentlyContinue
}
if ($null -eq $justCommand) {
    throw "just is required to build the canonical Codex package."
}

$packageDir = Join-Path $repoRoot "dist\codex-package-$target"
Assert-PathWithin -Path $packageDir -Root (Join-Path $repoRoot "dist")

Write-Step "Building the canonical Codex package for $target"
Push-Location $repoRoot
try {
    Invoke-Checked -FilePath $justCommand.Source -Arguments @(
        "assemble-codex-package",
        "--target", $target,
        "--cargo-profile", "release",
        "--package-dir", $packageDir,
        "--force"
    )
} finally {
    Pop-Location
}

$metadataPath = Join-Path $packageDir "codex-package.json"
if (-not (Test-Path -LiteralPath $metadataPath -PathType Leaf)) {
    throw "Canonical package metadata is missing at $metadataPath."
}
$metadata = Get-Content -LiteralPath $metadataPath -Raw | ConvertFrom-Json
if ($metadata.layoutVersion -ne 1 -or $metadata.variant -ne "codex" -or $metadata.target -ne $target) {
    throw "Canonical package metadata does not describe a Codex package for $target."
}
$version = [string]$metadata.version
if ([string]::IsNullOrWhiteSpace($version)) {
    throw "Canonical package metadata does not contain a version."
}

$expectedFiles = @(
    "codex-package.json",
    "bin\codex.exe",
    "bin\codex-code-mode-host.exe",
    "codex-path\rg.exe",
    "codex-resources\codex-command-runner.exe",
    "codex-resources\codex-windows-sandbox-setup.exe"
)
foreach ($relativePath in $expectedFiles) {
    $builtPath = Join-Path $packageDir $relativePath
    if (-not (Test-Path -LiteralPath $builtPath -PathType Leaf)) {
        throw "Canonical package is missing $builtPath."
    }
}
$packageFingerprint = ($expectedFiles | ForEach-Object {
    (Get-FileSha256 (Join-Path $packageDir $_)).Substring(0, 8).ToLowerInvariant()
}) -join ""

$codexHome = if ([string]::IsNullOrWhiteSpace($env:CODEX_HOME)) {
    Join-Path $env:USERPROFILE ".codex"
} else {
    $env:CODEX_HOME
}
$installRoot = Join-Path $codexHome "packages\standalone"
$releasesDir = Join-Path $installRoot "releases"
$releaseDir = Join-Path $releasesDir "$version-$target-$packageFingerprint"
$currentDir = Join-Path $installRoot "current"
$currentBinDir = Join-Path $currentDir "bin"
$visibleBinDir = Join-Path $env:LOCALAPPDATA "Programs\OpenAI\Codex\bin"
$installSuffix = "$PID.$([Guid]::NewGuid().ToString('N'))"
$stagingDir = Join-Path $releasesDir ".staging.$version-$target.$packageFingerprint.$installSuffix"

Assert-PathWithin -Path $releaseDir -Root $releasesDir
Assert-PathWithin -Path $stagingDir -Root $releasesDir
Assert-PathWithin -Path $currentDir -Root $installRoot
Assert-PathWithin -Path $visibleBinDir -Root (Join-Path $env:LOCALAPPDATA "Programs\OpenAI\Codex")
New-Item -ItemType Directory -Path $releasesDir -Force | Out-Null

try {
    Write-Step "Staging Codex $version in $stagingDir"
    Copy-Item -LiteralPath $packageDir -Destination $stagingDir -Recurse

    $sourceRoot = (Resolve-Path -LiteralPath $packageDir).Path.TrimEnd("\")
    $sourceFiles = @(Get-ChildItem -LiteralPath $sourceRoot -Recurse -File)
    $stagedFiles = @(Get-ChildItem -LiteralPath $stagingDir -Recurse -File)
    if ($sourceFiles.Count -ne $stagedFiles.Count) {
        throw "Staged package contains $($stagedFiles.Count) files; expected $($sourceFiles.Count)."
    }
    foreach ($sourceFile in $sourceFiles) {
        $relativePath = $sourceFile.FullName.Substring($sourceRoot.Length).TrimStart("\")
        $stagedPath = Join-Path $stagingDir $relativePath
        if (-not (Test-Path -LiteralPath $stagedPath -PathType Leaf)) {
            throw "Staged package is missing $stagedPath."
        }
        if ((Get-FileSha256 $sourceFile.FullName) -ne (Get-FileSha256 $stagedPath)) {
            throw "Staged file does not match the build output: $stagedPath."
        }
    }

    $stagedCodexPath = Join-Path $stagingDir "bin\codex.exe"
    $stagedVersionLines = @(& $stagedCodexPath --version)
    $stagedVersion = ($stagedVersionLines -join [Environment]::NewLine).Trim()
    if ($LASTEXITCODE -ne 0) {
        throw "$stagedCodexPath exited with code $LASTEXITCODE."
    }
    if ($stagedVersion -notlike "*$version*") {
        throw "Staged Codex reported '$stagedVersion'; expected version $version."
    }

    $releaseAlreadyInstalled = $false
    if (Test-Path -LiteralPath $releaseDir) {
        if (-not (Test-Path -LiteralPath $releaseDir -PathType Container)) {
            throw "Immutable release path is not a directory: $releaseDir."
        }
        $releaseFiles = @(Get-ChildItem -LiteralPath $releaseDir -Recurse -File)
        if ($sourceFiles.Count -ne $releaseFiles.Count) {
            throw "Immutable release $releaseDir does not match its package fingerprint."
        }
        foreach ($sourceFile in $sourceFiles) {
            $relativePath = $sourceFile.FullName.Substring($sourceRoot.Length).TrimStart("\")
            $releasePath = Join-Path $releaseDir $relativePath
            if (-not (Test-Path -LiteralPath $releasePath -PathType Leaf)) {
                throw "Immutable release $releaseDir does not match its package fingerprint."
            }
            if ((Get-FileSha256 $sourceFile.FullName) -ne (Get-FileSha256 $releasePath)) {
                throw "Immutable release $releaseDir does not match its package fingerprint."
            }
        }
        $releaseAlreadyInstalled = $true
    }

    Assert-JunctionCanBeSet `
        -LinkPath $currentDir `
        -InstallerOwnedTargetPrefix $releasesDir
    Assert-JunctionCanBeSet `
        -LinkPath $visibleBinDir `
        -InstallerOwnedTargetPrefix $installRoot

    if (-not $releaseAlreadyInstalled) {
        Write-Step "Installing Codex in $releaseDir"
        Move-Item -LiteralPath $stagingDir -Destination $releaseDir
    }

    Set-LocalJunction `
        -LinkPath $currentDir `
        -TargetPath $releaseDir `
        -InstallerOwnedTargetPrefix $releasesDir

    $visibleBinParent = Split-Path -Parent $visibleBinDir
    New-Item -ItemType Directory -Path $visibleBinParent -Force | Out-Null
    Set-LocalJunction `
        -LinkPath $visibleBinDir `
        -TargetPath $currentBinDir `
        -InstallerOwnedTargetPrefix $installRoot
} finally {
    if (Test-Path -LiteralPath $stagingDir) {
        Assert-PathWithin -Path $stagingDir -Root $releasesDir
        Remove-Item -LiteralPath $stagingDir -Recurse -Force
    }
}

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$updatedUserPath = Prepend-PathEntry -PathValue $userPath -Entry $visibleBinDir
if ($updatedUserPath -cne $userPath) {
    [Environment]::SetEnvironmentVariable("Path", $updatedUserPath, "User")
}
$env:Path = Prepend-PathEntry -PathValue $env:Path -Entry $visibleBinDir

Write-Step "Verifying the native command and package resources"
$codexCommand = @(Get-Command codex -All -ErrorAction Stop)[0]
$expectedCodexPath = Join-Path $visibleBinDir "codex.exe"
if ($codexCommand.CommandType -ne [System.Management.Automation.CommandTypes]::Application) {
    throw "Codex is still shadowed by a $($codexCommand.CommandType): $($codexCommand.Name)."
}
if (-not $codexCommand.Path.Equals($expectedCodexPath, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Codex still resolves to $($codexCommand.Path); expected $expectedCodexPath."
}
$reportedVersionLines = @(& $codexCommand.Path --version)
$reportedVersion = ($reportedVersionLines -join [Environment]::NewLine).Trim()
if ($LASTEXITCODE -ne 0) {
    throw "$($codexCommand.Path) exited with code $LASTEXITCODE."
}
if ($reportedVersion -notlike "*$version*") {
    throw "Installed Codex reported '$reportedVersion'; expected version $version."
}

Write-Host $reportedVersion
Write-Host "==> Native Codex command: $($codexCommand.Path)"
Write-Host "==> Canonical package: $releaseDir"
Write-Host "==> New terminals will prefer this build over npm's codex shim."
