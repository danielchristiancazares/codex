#Requires -Version 7.0

<#
.SYNOPSIS
Build Codex and update the CLI, desktop backend, plugin app-server, and managed daemon.
.DESCRIPTION
Finds the native package behind the codex command, the desktop backend in the
most recently created cache directory containing codex.exe, and the plugin
app-server in CODEX_HOME (or the default .codex directory). Builds one release
package and replaces the existing CLI and helper executables in all three
installations, preserving their directory layouts. Copies the built package to
the existing managed app-server daemon and pins it, restarting it if running.

Requires Python 3.11+, Rust through rustup, and the Windows native build tools.
Run from a non-administrator PowerShell window for managed daemon updates.
Close the desktop app and Codex sessions before running this script, and keep
them closed until it finishes so the installed binaries remain unlocked. Older
cached desktop versions are left unchanged. All three installations must exist
and use the same CPU architecture. File access is checked before the build and
again before replacement. A replacement failure attempts to restore all changed
files from backups and reports any files that need manual recovery.
The repository's pinned Rust toolchain and package builder control the build.
Existing npm launchers, PATH entries, and Codex settings are preserved.
Voice-runtime packaging and the separately installed Windows sandbox service
are outside this script's replacement set.
.EXAMPLE
pwsh -File .\scripts\build-and-install.ps1
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'This script builds and replaces existing Windows Codex installations.'
}

function Get-DesktopExecutable {
    [OutputType([System.IO.FileInfo])]
    param([Parameter(Mandatory)][System.IO.DirectoryInfo]$BinRoot)

    # Directory creation time survives replacing its executables on later runs.
    [System.IO.FileInfo[]]$candidates = @(Get-ChildItem -LiteralPath $BinRoot.FullName -Directory |
        Sort-Object -Property CreationTimeUtc, FullName -Descending | ForEach-Object {
            Get-ChildItem -LiteralPath $_.FullName -File -Filter codex.exe
        })

    switch ($candidates.Count) {
        0 { throw "No cached desktop codex.exe was found in $BinRoot." }
        default { $candidates[0] }
    }
}

function Assert-ReplacementFilesWritable {
    [OutputType([void])]
    param([Parameter(Mandatory)][System.IO.FileInfo[]]$Files)

    foreach ($file in $Files) {
        try {
            [System.IO.File]::Open(
                $file.FullName,
                [System.IO.FileMode]::Open,
                [System.IO.FileAccess]::ReadWrite,
                [System.IO.FileShare]::None
            ).Dispose()
        } catch {
            throw "Cannot replace $file. Close the desktop app and all Codex sessions, then rerun this script. $($_.Exception.Message)"
        }
    }
}

function Get-ExecutableArchitecture {
    [OutputType([System.Runtime.InteropServices.Architecture])]
    param([Parameter(Mandatory)][System.IO.FileInfo]$Executable)

    $reader = [System.IO.BinaryReader]::new([System.IO.File]::OpenRead($Executable.FullName))
    try {
        if ($reader.ReadUInt16() -ne 0x5A4D) {
            throw "Invalid Windows executable: $Executable"
        }
        $reader.BaseStream.Position = 0x3C
        $reader.BaseStream.Position = $reader.ReadInt32()
        if ($reader.ReadUInt32() -ne 0x00004550) {
            throw "Invalid PE header: $Executable"
        }
        switch ($reader.ReadUInt16()) {
            0x8664 { [System.Runtime.InteropServices.Architecture]::X64 }
            0xAA64 { [System.Runtime.InteropServices.Architecture]::Arm64 }
            default { throw "Unsupported Windows executable architecture: $Executable" }
        }
    } finally {
        $reader.Dispose()
    }
}

[System.IO.DirectoryInfo]$repository = Get-Item -LiteralPath "$PSScriptRoot/.."
[System.IO.FileInfo]$launcher = Get-Command codex -ErrorAction Stop | Select-Object -First 1 | ForEach-Object {
    if ($_.CommandType -in 'Application', 'ExternalScript') {
        $_.Source
    } else {
        throw 'The codex command must resolve to an executable or an npm launcher.'
    }
}
[System.IO.FileInfo]$installedCodex = switch ($launcher.Extension) {
    '.exe' { Get-Item -LiteralPath $launcher.FullName }
    { $_ -in '.ps1', '.cmd' } {
        [System.IO.DirectoryInfo]$npmPackage = Get-Item -LiteralPath (
            Join-Path $launcher.DirectoryName 'node_modules/@openai/codex'
        )
        [System.IO.FileInfo[]]$candidates = @(Get-ChildItem -LiteralPath $npmPackage.FullName -Recurse -File -Filter codex.exe)
        if ($candidates.Count -ne 1) {
            throw "Expected one native codex.exe in $npmPackage, found $($candidates.Count)."
        }
        $candidates[0]
    }
    default { throw "Unsupported Codex launcher: $launcher" }
}

if ($installedCodex.Directory.Name -ne 'bin') {
    throw "Expected codex.exe in a Codex package's bin directory: $installedCodex"
}
[System.IO.DirectoryInfo]$installation = $installedCodex.Directory.Parent
[System.IO.FileInfo]$manifest = Get-Item -LiteralPath (Join-Path $installation.FullName 'codex-package.json')
$metadata = [System.Text.Json.JsonDocument]::Parse([System.IO.File]::ReadAllText($manifest.FullName))
try {
    if ($metadata.RootElement.GetProperty('layoutVersion').GetInt32() -ne 1 -or
        $metadata.RootElement.GetProperty('variant').GetString() -ne 'codex' -or
        $metadata.RootElement.GetProperty('entrypoint').GetString() -ne 'bin/codex.exe' -or
        $metadata.RootElement.GetProperty('resourcesDir').GetString() -ne 'codex-resources' -or
        $metadata.RootElement.GetProperty('pathDir').GetString() -ne 'codex-path') {
        throw "Unsupported Codex package layout: $manifest"
    }
    $targetArguments = switch ($metadata.RootElement.GetProperty('target').GetString()) {
        'x86_64-pc-windows-msvc' { '--target', 'x86_64-pc-windows-msvc' }
        'aarch64-pc-windows-msvc' { '--target', 'aarch64-pc-windows-msvc' }
        default { throw "Unsupported Windows target in $manifest" }
    }
} finally {
    $metadata.Dispose()
}

[System.IO.DirectoryInfo]$desktopBinRoot = Get-Item -LiteralPath (Join-Path $env:LOCALAPPDATA 'OpenAI/Codex/bin')
[System.IO.FileInfo]$desktopCodex = Get-DesktopExecutable -BinRoot $desktopBinRoot
[System.IO.DirectoryInfo]$configurationRoot = if ([string]::IsNullOrWhiteSpace($env:CODEX_HOME)) {
    Get-Item -LiteralPath (Join-Path $env:USERPROFILE '.codex')
} else {
    Get-Item -LiteralPath $env:CODEX_HOME
}
[System.IO.FileInfo]$pluginCodex = Get-Item -LiteralPath (Join-Path $configurationRoot.FullName 'plugins/.plugin-appserver/codex.exe')
[System.IO.FileInfo[]]$entrypoints = @($installedCodex, $desktopCodex, $pluginCodex)
[System.Runtime.InteropServices.Architecture]$architecture = Get-ExecutableArchitecture -Executable $installedCodex
foreach ($entrypoint in $entrypoints) {
    if ((Get-ExecutableArchitecture -Executable $entrypoint) -ne $architecture) {
        throw "All three installations must use the same CPU architecture: $entrypoint"
    }
}

$sharedExecutables = @(
    'bin/codex.exe'
    'bin/codex-code-mode-host.exe'
    'codex-resources/codex-command-runner.exe'
    'codex-resources/codex-windows-sandbox-setup.exe'
)
$packageFiles = $sharedExecutables + @(
    'codex-path/rg.exe'
    'codex-package.json'
)
foreach ($command in 'cargo', 'rustup', 'python') {
    Get-Command $command -CommandType Application -ErrorAction Stop | Out-Null
}

[System.Guid]$buildId = [System.Guid]::NewGuid()
[System.IO.DirectoryInfo]$temporaryRoot = [System.IO.Path]::TrimEndingDirectorySeparator([System.IO.Path]::GetTempPath())
[System.IO.DirectoryInfo]$staging = Join-Path $temporaryRoot.FullName "codex-source-$buildId"
if ($staging.Parent.FullName -ne $temporaryRoot.FullName) {
    throw "Build staging must be inside the temporary directory: $staging"
}

# Map existing destinations to source-package files before starting the build.
# The desktop and plugin launchers expect their four executables side by side.
$replacements = [System.Collections.Generic.Dictionary[System.IO.FileInfo, System.IO.FileInfo]]::new()
foreach ($relativePath in $packageFiles) {
    [System.IO.FileInfo]$destination = Get-Item -LiteralPath (Join-Path $installation.FullName $relativePath)
    $replacements.Add($destination, (Join-Path $staging.FullName $relativePath))
}
foreach ($entrypoint in @($desktopCodex, $pluginCodex)) {
    foreach ($relativePath in $sharedExecutables) {
        [System.IO.FileInfo]$destination = Get-Item -LiteralPath (
            Join-Path $entrypoint.DirectoryName ([System.IO.Path]::GetFileName($relativePath))
        )
        $replacements.Add($destination, (Join-Path $staging.FullName $relativePath))
    }
}

Assert-ReplacementFilesWritable -Files @($replacements.Keys)

Write-Host 'Building one release package for the CLI, desktop backend, plugin app-server, and managed daemon. Keep the desktop app and Codex sessions closed:'
foreach ($entrypoint in $entrypoints) {
    Write-Host "  $entrypoint"
}
try {
    # Set the repository root only in the builder's environment. ArgumentList
    # preserves spaces in checkout and temporary paths without shell quoting.
    $build = [System.Diagnostics.ProcessStartInfo]::new((Get-Command python -CommandType Application | Select-Object -First 1).Source)
    $build.WorkingDirectory = $repository.FullName
    $build.Environment['CODEX_REPO_ROOT'] = $repository.FullName
    foreach ($argument in @(
        '-X', 'utf8', '-u', (Join-Path $PSScriptRoot 'build_codex_package.py'),
        '--cargo-profile', 'release', '--package-dir', $staging.FullName
    ) + $targetArguments) {
        $build.ArgumentList.Add($argument)
    }
    $builder = [System.Diagnostics.Process]::Start($build)
    try {
        $builder.WaitForExit()
        if ($builder.ExitCode -ne 0) {
            throw "Package build failed with exit code $($builder.ExitCode). Existing binaries are unchanged."
        }
    } finally {
        $builder.Dispose()
    }

    # Validate every replacement before touching any installed file.
    foreach ($relativePath in $packageFiles) {
        if (-not (Test-Path -LiteralPath (Join-Path $staging.FullName $relativePath) -PathType Leaf)) {
            throw "The built Codex package is incomplete: $relativePath"
        }
    }
    if ((Get-ExecutableArchitecture -Executable (Join-Path $staging.FullName 'bin/codex.exe')) -ne $architecture) {
        throw 'The built executable does not match the installed CPU architecture. Existing binaries are unchanged.'
    }
    & (Join-Path $staging.FullName 'bin/codex.exe') --version
    if ($LASTEXITCODE -ne 0) {
        throw 'The built Codex executable failed verification. Existing binaries are unchanged.'
    }
    if ((Get-DesktopExecutable -BinRoot $desktopBinRoot).FullName -ne $desktopCodex.FullName) {
        throw 'The newest cached desktop backend changed during the build. Rerun this script to update the current version.'
    }
    Assert-ReplacementFilesWritable -Files @($replacements.Keys)

    $backups = [System.Collections.Generic.Dictionary[System.IO.FileInfo, System.IO.FileInfo]]::new()
    try {
        foreach ($replacement in $replacements.GetEnumerator()) {
            [System.IO.FileInfo]$destination = $replacement.Key
            [System.IO.FileInfo]$backup = "$($destination.FullName).source-backup-$buildId"
            # Keep the original file available for rollback until verification succeeds.
            Move-Item -LiteralPath $destination.FullName -Destination $backup.FullName
            $backups.Add($destination, $backup)
            Copy-Item -LiteralPath $replacement.Value.FullName -Destination $destination.FullName
        }

        foreach ($entrypoint in $entrypoints) {
            & $entrypoint.FullName --version
            if ($LASTEXITCODE -ne 0) {
                throw "The installed Codex executable failed verification: $entrypoint"
            }
        }
    } catch {
        foreach ($entry in $backups.GetEnumerator()) {
            try {
                Move-Item -LiteralPath $entry.Value.FullName -Destination $entry.Key.FullName -Force
            } catch {
                Write-Warning "Restore $($entry.Key.FullName) from $($entry.Value.FullName) after closing Codex."
            }
        }
        throw
    }

    foreach ($backup in $backups.Values) {
        try {
            Remove-Item -LiteralPath $backup.FullName -Force
        } catch {
            Write-Warning "Could not remove $backup. Close Codex and remove this backup. $($_.Exception.Message)"
        }
    }
    Write-Host 'Replaced the npm CLI, newest cached desktop backend, and plugin app-server binaries.'
    Write-Host 'Updating the managed daemon from the built package, restarting it if running.'
    & (Join-Path $staging.FullName 'bin/codex.exe') app-server daemon update --from-cli --yes
    if ($LASTEXITCODE -ne 0) {
        throw 'The CLI, desktop backend, and plugin app-server were updated, but the managed daemon update failed. Retry with: codex app-server daemon update --from-cli --yes'
    }
    Write-Host 'The managed daemon is pinned to the built package.'
    Write-Host 'You can now reopen the desktop app and Codex sessions. npm and desktop app updates may replace this build.'
} finally {
    if (Test-Path -LiteralPath $staging.FullName) {
        Remove-Item -LiteralPath $staging.FullName -Recurse -Force
    }
}
