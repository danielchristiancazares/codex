[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

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
}

function Set-LocalJunction {
    [CmdletBinding(SupportsShouldProcess)]
    param(
        [string]$LinkPath,
        [string]$TargetPath,
        [string]$InstallerOwnedTargetPrefix
    )

    Assert-JunctionCanBeSet `
        -LinkPath $LinkPath `
        -InstallerOwnedTargetPrefix $InstallerOwnedTargetPrefix
    if (-not $PSCmdlet.ShouldProcess($LinkPath, "Point junction at $TargetPath")) {
        return
    }

    $existingTarget = $null
    $backupPath = $null
    if (Test-Path -LiteralPath $LinkPath) {
        $item = Get-Item -LiteralPath $LinkPath -Force
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            $existingTarget = [string]$item.Target
            if ($existingTarget.Equals($TargetPath, [System.StringComparison]::OrdinalIgnoreCase)) {
                return
            }
            Remove-Item -LiteralPath $LinkPath -Force
        } elseif ($null -eq (Get-ChildItem -LiteralPath $LinkPath -Force | Select-Object -First 1)) {
            Remove-Item -LiteralPath $LinkPath -Force
        } else {
            $backupPath = "$LinkPath.backup.$(Get-Date -Format 'yyyyMMdd-HHmmss').$PID"
            Write-Step "Preserving existing directory at $backupPath"
            Move-Item -LiteralPath $LinkPath -Destination $backupPath
        }
    }

    try {
        New-Item -ItemType Junction -Path $LinkPath -Target $TargetPath | Out-Null
    } catch {
        if ($null -ne $existingTarget) {
            New-Item -ItemType Junction -Path $LinkPath -Target $existingTarget | Out-Null
        } elseif ($null -ne $backupPath) {
            Move-Item -LiteralPath $backupPath -Destination $LinkPath
        }
        throw
    }
}

if ($env:OS -ne "Windows_NT") {
    throw "This script is intended for Windows."
}

$repoRoot = $PSScriptRoot
Set-Location $repoRoot

$architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
$targetInfo = switch ($architecture) {
    "X64" {
        [pscustomobject]@{
            Target = "x86_64-pc-windows-msvc"
        }
    }
    "Arm64" {
        [pscustomobject]@{
            Target = "aarch64-pc-windows-msvc"
        }
    }
    default { throw "Unsupported Windows architecture: $architecture" }
}
$target = $targetInfo.Target

$pythonCommand = Get-Command python -ErrorAction SilentlyContinue
if ($null -eq $pythonCommand) {
    $pythonCommand = Get-Command python3 -ErrorAction SilentlyContinue
}
if ($null -eq $pythonCommand) {
    throw "Python 3 is required to build the Codex package."
}

$python = $pythonCommand.Source
$versionScript = 'import tomllib; print(tomllib.load(open("codex-rs/Cargo.toml", "rb"))["workspace"]["package"]["version"])'
$version = (& $python -c $versionScript).Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($version)) {
    throw "Could not read the Codex workspace version."
}

$packageDir = Join-Path $repoRoot "dist\codex-package-$target"
$codexHome = if ([string]::IsNullOrWhiteSpace($env:CODEX_HOME)) {
    Join-Path $env:USERPROFILE ".codex"
} else {
    $env:CODEX_HOME
}
$installRoot = Join-Path $codexHome "packages\standalone"
$releasesDir = Join-Path $installRoot "releases"
$releaseDir = Join-Path $releasesDir "$version-$target"
$currentDir = Join-Path $installRoot "current"
$currentBinDir = Join-Path $currentDir "bin"
$visibleBinDir = Join-Path $env:LOCALAPPDATA "Programs\OpenAI\Codex\bin"

Write-Step "Building Codex $version for $target"
Invoke-Checked -FilePath $python -Arguments @(
    "scripts/build_codex_package.py",
    "--target", $target,
    "--cargo-profile", "release",
    "--package-dir", $packageDir,
    "--force"
)

$installSuffix = "$PID.$([Guid]::NewGuid().ToString('N'))"
$stagingDir = Join-Path $releasesDir ".staging.$version-$target.$installSuffix"
New-Item -ItemType Directory -Path $releasesDir -Force | Out-Null

try {
    Write-Step "Staging Codex in $stagingDir"
    Copy-Item -LiteralPath $packageDir -Destination $stagingDir -Recurse

    $expectedFiles = @(
        "codex-package.json",
        "bin\codex.exe",
        "bin\codex-code-mode-host.exe",
        "codex-path\rg.exe",
        "codex-resources\codex-command-runner.exe",
        "codex-resources\codex-windows-sandbox-setup.exe"
    )
    foreach ($relativePath in $expectedFiles) {
        $stagedPath = Join-Path $stagingDir $relativePath
        if (-not (Test-Path -LiteralPath $stagedPath -PathType Leaf)) {
            throw "Staged Codex package is missing $stagedPath."
        }
    }

    $sourceRoot = (Resolve-Path -LiteralPath $packageDir).Path.TrimEnd("\")
    $sourceFiles = @(Get-ChildItem -LiteralPath $sourceRoot -Recurse -File)
    $stagedFiles = @(Get-ChildItem -LiteralPath $stagingDir -Recurse -File)
    if ($sourceFiles.Count -ne $stagedFiles.Count) {
        throw "Staged Codex package contains $($stagedFiles.Count) files; expected $($sourceFiles.Count)."
    }
    foreach ($sourceFile in $sourceFiles) {
        $relativePath = $sourceFile.FullName.Substring($sourceRoot.Length).TrimStart("\")
        $stagedPath = Join-Path $stagingDir $relativePath
        if (-not (Test-Path -LiteralPath $stagedPath -PathType Leaf)) {
            throw "Staged Codex package is missing $stagedPath."
        }
        if ((Get-FileSha256 $sourceFile.FullName) -ne (Get-FileSha256 $stagedPath)) {
            throw "Staged file does not match the build output: $stagedPath."
        }
    }

    $stagedCodexPath = Join-Path $stagingDir "bin\codex.exe"
    $stagedVersionLines = @(& $stagedCodexPath --version)
    $stagedExitCode = $LASTEXITCODE
    $stagedVersion = ($stagedVersionLines -join [Environment]::NewLine).Trim()
    if ($stagedExitCode -ne 0) {
        throw "$stagedCodexPath exited with code $stagedExitCode."
    }
    if ($stagedVersion -notlike "*$version*") {
        throw "Staged Codex reported '$stagedVersion'; expected version $version."
    }

    Assert-JunctionCanBeSet `
        -LinkPath $currentDir `
        -InstallerOwnedTargetPrefix $releasesDir
    Assert-JunctionCanBeSet `
        -LinkPath $visibleBinDir `
        -InstallerOwnedTargetPrefix $installRoot

    $releaseBackup = $null
    if (Test-Path -LiteralPath $releaseDir) {
        $releaseBackup = "$releaseDir.backup.$installSuffix"
        Write-Step "Preserving existing release at $releaseBackup"
        Move-Item -LiteralPath $releaseDir -Destination $releaseBackup
    }

    try {
        Write-Step "Installing Codex in $releaseDir"
        Move-Item -LiteralPath $stagingDir -Destination $releaseDir
    } catch {
        if ($null -ne $releaseBackup -and -not (Test-Path -LiteralPath $releaseDir)) {
            Move-Item -LiteralPath $releaseBackup -Destination $releaseDir
        }
        throw
    }

    New-Item -ItemType Directory -Path $installRoot -Force | Out-Null
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
        Remove-Item -LiteralPath $stagingDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$userPathEntries = @($userPath -split ";" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
if (-not ($userPathEntries | Where-Object { $_.TrimEnd("\") -ieq $visibleBinDir.TrimEnd("\") })) {
    $updatedUserPath = (@($visibleBinDir) + $userPathEntries) -join ";"
    [Environment]::SetEnvironmentVariable("Path", $updatedUserPath, "User")
}
if (-not (($env:Path -split ";") | Where-Object { $_.TrimEnd("\") -ieq $visibleBinDir.TrimEnd("\") })) {
    $env:Path = "$visibleBinDir;$env:Path"
}

Write-Step "Verifying installation"
$codexPath = Join-Path $visibleBinDir "codex.exe"
$reportedVersionLines = @(& $codexPath --version)
$exitCode = $LASTEXITCODE
$reportedVersion = ($reportedVersionLines -join [Environment]::NewLine).Trim()
if ($exitCode -ne 0) {
    throw "$codexPath exited with code $exitCode."
}
if ($reportedVersion -notlike "*$version*") {
    throw "Installed Codex reported '$reportedVersion'; expected version $version."
}
Write-Host $reportedVersion

Write-Host "==> Installed Codex $version"
Write-Host "==> Installed at $codexPath"
