# Prepare the bundled payloads and produce the NSIS installer.

$ErrorActionPreference = 'Stop'

& "$PSScriptRoot\prepare-runtime.ps1"
if ($LASTEXITCODE -ne 0) {
    throw 'prepare-runtime.ps1 failed.'
}
& "$PSScriptRoot\prepare-node.ps1"
if ($LASTEXITCODE -ne 0) {
    throw 'prepare-node.ps1 failed.'
}

$appDirectory = Split-Path -Parent $PSScriptRoot
Push-Location $appDirectory
try {
    cargo tauri build
    if ($LASTEXITCODE -ne 0) {
        throw "cargo tauri build failed with exit code $LASTEXITCODE."
    }
}
finally {
    Pop-Location
}

$installer = Get-ChildItem (Join-Path $appDirectory 'target\release\bundle\nsis\*.exe') |
    Sort-Object LastWriteTime |
    Select-Object -Last 1
if (-not $installer) {
    throw 'cargo tauri build did not produce an NSIS installer.'
}
$size = [math]::Round($installer.Length / 1MB, 1)
Write-Host "build-installer: $($installer.FullName) ($size MB)"
