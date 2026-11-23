# Write the pinned Node, npm, npx, and pnpm toolchain into resources/.
#
# Node is verified against the published SHASUMS256.txt manifest and pnpm
# against the npm registry's SHA-512 integrity before extraction.

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$nodeVersion = '24.22.3'
$pnpmVersion = '11.28.5'
$appDirectory = Split-Path -Parent $PSScriptRoot
$resources = Join-Path $appDirectory 'resources'
$nodeDestination = Join-Path $resources 'node'
$pnpmDestination = Join-Path $resources 'pnpm'
$temporary = Join-Path $env:TEMP ("dsh-desktop-toolchain-" + [guid]::NewGuid())

function Clear-Directory([string]$path) {
    New-Item -ItemType Directory -Force -Path $path | Out-Null
    Get-ChildItem -Force -Path $path | Remove-Item -Recurse -Force
}

try {
    New-Item -ItemType Directory -Force -Path $temporary | Out-Null

    $archiveName = "node-v$nodeVersion-win-x64.zip"
    $baseUri = "https://nodejs.org/dist/v$nodeVersion"
    $manifestPath = Join-Path $temporary 'SHASUMS256.txt'
    Invoke-WebRequest -Uri "$baseUri/SHASUMS256.txt" -OutFile $manifestPath
    $manifestLine = Get-Content $manifestPath |
        Where-Object { $_ -match "\s$([regex]::Escape($archiveName))$" } |
        Select-Object -First 1
    if (-not $manifestLine) {
        throw "Node $nodeVersion does not publish $archiveName in SHASUMS256.txt."
    }
    $expectedNodeHash = ($manifestLine -split '\s+')[0].ToLowerInvariant()
    $archivePath = Join-Path $temporary $archiveName
    Invoke-WebRequest -Uri "$baseUri/$archiveName" -OutFile $archivePath
    $actualNodeHash = (Get-FileHash -Algorithm SHA256 $archivePath).Hash.ToLowerInvariant()
    if ($actualNodeHash -ne $expectedNodeHash) {
        throw "Node checksum mismatch: expected $expectedNodeHash, got $actualNodeHash."
    }
    Expand-Archive -Path $archivePath -DestinationPath (Join-Path $temporary 'node')
    $nodeRoot = Join-Path $temporary "node\node-v$nodeVersion-win-x64"
    Clear-Directory $nodeDestination
    Copy-Item (Join-Path $nodeRoot '*') $nodeDestination -Recurse -Force

    $pnpmMetadata = Invoke-RestMethod -Uri "https://registry.npmjs.org/pnpm/$pnpmVersion"
    $integrity = $pnpmMetadata.dist.integrity
    if ($integrity -notmatch '^sha512-') {
        throw "pnpm $pnpmVersion has an unexpected registry integrity: $integrity"
    }
    $pnpmArchive = Join-Path $temporary 'pnpm.tgz'
    Invoke-WebRequest -Uri $pnpmMetadata.dist.tarball -OutFile $pnpmArchive
    $expectedPnpmHash = -join (
        [Convert]::FromBase64String($integrity.Substring('sha512-'.Length)) |
            ForEach-Object { $_.ToString('x2') }
    )
    $actualPnpmHash = (Get-FileHash -Algorithm SHA512 $pnpmArchive).Hash.ToLowerInvariant()
    if ($actualPnpmHash -ne $expectedPnpmHash) {
        throw "pnpm integrity mismatch: expected $expectedPnpmHash, got $actualPnpmHash."
    }
    Clear-Directory $pnpmDestination
    tar -xzf $pnpmArchive -C $pnpmDestination --strip-components=1
    if ($LASTEXITCODE -ne 0) {
        throw "tar failed to extract $pnpmArchive."
    }

    $shim = @'
@echo off
"%~dp0..\node\node.exe" "%~dp0bin\pnpm.cjs" %*
'@
    Set-Content -Path (Join-Path $pnpmDestination 'pnpm.cmd') -Value $shim -Encoding ascii

    $versions = @{
        node = (& (Join-Path $nodeDestination 'node.exe') -v)
        npm  = (& (Join-Path $nodeDestination 'npm.cmd') -v)
        npx  = (& (Join-Path $nodeDestination 'npx.cmd') -v)
        pnpm = (& (Join-Path $pnpmDestination 'pnpm.cmd') -v)
    }
    Write-Host "prepare-node: node $($versions.node), npm $($versions.npm), npx $($versions.npx), pnpm $($versions.pnpm)"
}
finally {
    Remove-Item -Recurse -Force $temporary -ErrorAction SilentlyContinue
}
