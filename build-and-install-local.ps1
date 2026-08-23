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

function Assert-PackageMatches {
    param(
        [string]$SourceDir,
        [string]$DestinationDir
    )

    $sourceRoot = (Resolve-Path -LiteralPath $SourceDir).Path.TrimEnd("\")
    $destinationRoot = (Resolve-Path -LiteralPath $DestinationDir).Path.TrimEnd("\")
    $sourceFiles = @(Get-ChildItem -LiteralPath $sourceRoot -Recurse -File)
    $destinationFiles = @(Get-ChildItem -LiteralPath $destinationRoot -Recurse -File)
    if ($sourceFiles.Count -ne $destinationFiles.Count) {
        throw "$DestinationDir contains $($destinationFiles.Count) files; expected $($sourceFiles.Count)."
    }

    foreach ($sourceFile in $sourceFiles) {
        $relativePath = $sourceFile.FullName.Substring($sourceRoot.Length).TrimStart("\")
        $destinationPath = Join-Path $destinationRoot $relativePath
        if (-not (Test-Path -LiteralPath $destinationPath -PathType Leaf)) {
            throw "$DestinationDir is missing $relativePath."
        }
        if ((Get-FileSha256 $sourceFile.FullName) -ne (Get-FileSha256 $destinationPath)) {
            throw "$destinationPath does not match the canonical package."
        }
    }
}

function Get-VerifiedCodexVersion {
    param(
        [string]$CodexPath,
        [string]$ExpectedVersion
    )

    $reportedVersionLines = @(& $CodexPath --version)
    $exitCode = $LASTEXITCODE
    $reportedVersion = ($reportedVersionLines -join [Environment]::NewLine).Trim()
    if ($exitCode -ne 0) {
        throw "$CodexPath exited with code $exitCode."
    }
    if ($reportedVersion -notlike "*$ExpectedVersion*") {
        throw "$CodexPath reported '$reportedVersion'; expected version $ExpectedVersion."
    }

    return $reportedVersion
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
$targetInfo = switch ($architecture) {
    "X64" {
        [pscustomobject]@{
            Target = "x86_64-pc-windows-msvc"
            PlatformPackage = "codex-win32-x64"
        }
    }
    "Arm64" {
        [pscustomobject]@{
            Target = "aarch64-pc-windows-msvc"
            PlatformPackage = "codex-win32-arm64"
        }
    }
    default { throw "Unsupported Windows architecture: $architecture" }
}
$target = $targetInfo.Target

$justCommand = Get-Command just.exe -ErrorAction SilentlyContinue
if ($null -eq $justCommand) {
    $justCommand = Get-Command just -ErrorAction SilentlyContinue
}
if ($null -eq $justCommand) {
    throw "just is required to build the canonical Codex package."
}

$npmCommand = Get-Command npm.cmd -ErrorAction SilentlyContinue
if ($null -eq $npmCommand) {
    throw "npm is required to locate the global Codex installation."
}

$npmRootLines = @(& $npmCommand.Source root --global)
$npmRoot = ($npmRootLines -join [Environment]::NewLine).Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($npmRoot)) {
    throw "Could not locate the global npm package directory."
}
$npmPrefixLines = @(& $npmCommand.Source prefix --global)
$npmPrefix = ($npmPrefixLines -join [Environment]::NewLine).Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($npmPrefix)) {
    throw "Could not locate the global npm command directory."
}

$codexPackageDir = Join-Path $npmRoot "@openai\codex"
$codexPackageMetadataPath = Join-Path $codexPackageDir "package.json"
if (-not (Test-Path -LiteralPath $codexPackageMetadataPath -PathType Leaf)) {
    throw "Could not find the global @openai/codex npm package. Run npm install -g @openai/codex first."
}
$codexPackageMetadata = Get-Content -LiteralPath $codexPackageMetadataPath -Raw | ConvertFrom-Json
if ($codexPackageMetadata.name -cne "@openai/codex") {
    throw "$codexPackageMetadataPath does not describe the @openai/codex package."
}

$nestedPlatformPackageDir = Join-Path $codexPackageDir "node_modules\@openai\$($targetInfo.PlatformPackage)"
$hoistedPlatformPackageDir = Join-Path $npmRoot "@openai\$($targetInfo.PlatformPackage)"
if (Test-Path -LiteralPath $nestedPlatformPackageDir -PathType Container) {
    $platformPackageDir = $nestedPlatformPackageDir
} elseif (Test-Path -LiteralPath $hoistedPlatformPackageDir -PathType Container) {
    $platformPackageDir = $hoistedPlatformPackageDir
} else {
    throw "Could not find the installed @openai/$($targetInfo.PlatformPackage) npm package."
}

$installDir = Join-Path $platformPackageDir "vendor\$target"
if (-not (Test-Path -LiteralPath $installDir -PathType Container)) {
    throw "Could not find the npm Codex package location at $installDir."
}
$codexShim = Join-Path $npmPrefix "codex.cmd"
if (-not (Test-Path -LiteralPath $codexShim -PathType Leaf)) {
    throw "Could not find the npm Codex command shim at $codexShim."
}

Assert-PathWithin -Path $codexPackageDir -Root $npmRoot
Assert-PathWithin -Path $platformPackageDir -Root $npmRoot
Assert-PathWithin -Path $installDir -Root $platformPackageDir

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

$installSuffix = "$PID.$([Guid]::NewGuid().ToString('N'))"
$installParent = Split-Path -Parent $installDir
$stagingDir = Join-Path $installParent ".$target.installing.$installSuffix"
$backupDir = Join-Path $installParent ".$target.backup.$installSuffix"
Assert-PathWithin -Path $stagingDir -Root $installParent
Assert-PathWithin -Path $backupDir -Root $installParent

if (Test-Path -LiteralPath $stagingDir) {
    throw "Staging path already exists: $stagingDir."
}
if (Test-Path -LiteralPath $backupDir) {
    throw "Backup path already exists: $backupDir."
}

$backupCreated = $false
$replacementInstalled = $false
try {
    Write-Step "Staging Codex $version beside the npm package"
    Copy-Item -LiteralPath $packageDir -Destination $stagingDir -Recurse
    Assert-PackageMatches -SourceDir $packageDir -DestinationDir $stagingDir
    $stagedCodexPath = Join-Path $stagingDir "bin\codex.exe"
    $null = Get-VerifiedCodexVersion -CodexPath $stagedCodexPath -ExpectedVersion $version

    Write-Step "Replacing npm package at $installDir"
    Move-Item -LiteralPath $installDir -Destination $backupDir
    $backupCreated = $true
    Move-Item -LiteralPath $stagingDir -Destination $installDir
    $replacementInstalled = $true

    Assert-PackageMatches -SourceDir $packageDir -DestinationDir $installDir
    $installedCodexPath = Join-Path $installDir "bin\codex.exe"
    $reportedVersion = Get-VerifiedCodexVersion `
        -CodexPath $installedCodexPath `
        -ExpectedVersion $version
    $null = Get-VerifiedCodexVersion -CodexPath $codexShim -ExpectedVersion $version
} catch {
    $installFailure = $_
    if ($backupCreated) {
        try {
            if ($replacementInstalled -and (Test-Path -LiteralPath $installDir)) {
                Assert-PathWithin -Path $installDir -Root $platformPackageDir
                Remove-Item -LiteralPath $installDir -Recurse -Force
            }
            if (Test-Path -LiteralPath $backupDir) {
                Move-Item -LiteralPath $backupDir -Destination $installDir
                $backupCreated = $false
            }
        } catch {
            throw "Installing the local build failed, and restoring the npm package also failed. The backup remains at $backupDir. Original error: $installFailure"
        }
    }
    throw $installFailure
} finally {
    if (Test-Path -LiteralPath $stagingDir) {
        Assert-PathWithin -Path $stagingDir -Root $installParent
        Remove-Item -LiteralPath $stagingDir -Recurse -Force
    }
}

if ($backupCreated -and (Test-Path -LiteralPath $backupDir)) {
    Assert-PathWithin -Path $backupDir -Root $installParent
    try {
        Remove-Item -LiteralPath $backupDir -Recurse -Force
    } catch {
        Write-Warning "The local build is installed, and the previous npm package remains at $backupDir because it is still in use."
    }
}

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$updatedUserPath = Prepend-PathEntry -PathValue $userPath -Entry $npmPrefix
if ($updatedUserPath -cne $userPath) {
    [Environment]::SetEnvironmentVariable("Path", $updatedUserPath, "User")
}
$env:Path = Prepend-PathEntry -PathValue $env:Path -Entry $npmPrefix

Write-Host $reportedVersion
Write-Host "==> npm command: $codexShim"
Write-Host "==> Replaced npm package: $installDir"
