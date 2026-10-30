# Package a native x64 Windows build. Does not change application data.
param([ValidateSet('debug', 'release')][string]$BuildProfile = 'release')
$ErrorActionPreference = 'Stop'
if (-not $IsWindows -or $env:CARGO_BUILD_TARGET) {
    throw 'Run on Windows with CARGO_BUILD_TARGET unset.'
}
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    $hostInfo = & rustc -vV
    if ($LASTEXITCODE -ne 0 -or $hostInfo -notcontains 'host: x86_64-pc-windows-msvc') {
        throw 'This package requires the native x64 MSVC toolchain.'
    }
    $buildArgs = @('build', '-p', 'cepm-app', '--locked')
    if ($BuildProfile -eq 'release') { $buildArgs += '--release' }
    & cargo @buildArgs
    if ($LASTEXITCODE -ne 0) { throw 'Cargo build failed.' }
    $metadataJson = & cargo metadata --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed.' }
    $metadata = $metadataJson | ConvertFrom-Json
    $version = ($metadata.packages | Where-Object name -eq 'cepm-app').version
    $output = Join-Path $metadata.target_directory "bundle/$BuildProfile"
    New-Item -ItemType Directory -Force $output | Out-Null
    $staging = Join-Path $output ('.cepm-' + [guid]::NewGuid())
    try {
        $app = Join-Path $staging 'CEPM'
        New-Item -ItemType Directory $app -Force | Out-Null
        Copy-Item (Join-Path $metadata.target_directory "$BuildProfile/cepm-app.exe") (Join-Path $app 'CEPM.exe')
        Copy-Item 'docs/windows-package.txt' (Join-Path $app 'README.txt')
        $name = "CEPM-$version-windows-x64.zip"
        if ($BuildProfile -eq 'debug') { $name = 'CEPM-windows-x64-debug.zip' }
        $archive = Join-Path $output $name
        Compress-Archive -Path $app -DestinationPath (Join-Path $staging $name)
        Move-Item -Force (Join-Path $staging $name) $archive
        $hash = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
        Set-Content -Encoding ascii -Path "$archive.sha256" -Value "$hash  $name"
        Write-Output "ZIP: $archive (unsigned; Windows GUI/browser behavior not yet validated)"
    } finally {
        if (Test-Path $staging) { Remove-Item -Recurse -Force $staging }
    }
} finally {
    Pop-Location
}
