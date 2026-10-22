$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$resourceDir = Join-Path $projectRoot 'src-tauri/resources'
$workDir = Join-Path $projectRoot 'work'
New-Item -ItemType Directory -Force -Path $resourceDir,$workDir | Out-Null
$zipPath = Join-Path $workDir 'winhttp-x64-v10.0.0.zip'
$expected = '422645c19da0cc5904170c81903a5a9c9a1384131496a295de870c8c1789f5be'
if (!(Test-Path -LiteralPath $zipPath)) {
    # Immutable asset ID; do not resolve the moving x64-latest tag during builds.
    Invoke-WebRequest 'https://api.github.com/repos/ThirteenAG/Ultimate-ASI-Loader/releases/assets/662745698' -Headers @{Accept='application/octet-stream'} -OutFile $zipPath
}
$sha = [System.Security.Cryptography.SHA256]::Create()
$actual = [BitConverter]::ToString($sha.ComputeHash([IO.File]::ReadAllBytes($zipPath))).Replace('-','').ToLowerInvariant()
$sha.Dispose()
if ($actual -ne $expected) { throw 'Ultimate ASI Loader archive checksum mismatch' }
$extractDir = Join-Path $workDir 'asi-v10.0.0'
Add-Type -AssemblyName System.IO.Compression.FileSystem
New-Item -ItemType Directory -Path $extractDir -Force | Out-Null
$archive = [IO.Compression.ZipFile]::OpenRead($zipPath)
try { foreach ($entry in $archive.Entries) { if ($entry.Name -eq 'winhttp.dll') { [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, (Join-Path $extractDir 'winhttp.dll'), $true) } } } finally { $archive.Dispose() }
$dll = Get-ChildItem -LiteralPath $extractDir -Recurse -Filter winhttp.dll | Select-Object -First 1
if (!$dll) { throw 'winhttp.dll missing from archive' }
Copy-Item -LiteralPath $dll.FullName -Destination (Join-Path $resourceDir 'winhttp.dll') -Force
cargo build --manifest-path (Join-Path $projectRoot 'native/Cargo.toml') --release
if ($LASTEXITCODE -ne 0) { throw 'Bridge build failed' }
Copy-Item -LiteralPath (Join-Path $projectRoot 'native/target/release/pex_mod_bridge.dll') -Destination (Join-Path $resourceDir 'PEXModBridge.asi') -Force
Write-Output 'Native loader ready (verified Ultimate ASI Loader 10.0.0 + PEX Bridge).'
