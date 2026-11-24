$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$resourceDir = Join-Path $projectRoot 'src-tauri/resources'
$workDir = Join-Path $projectRoot 'work'
New-Item -ItemType Directory -Force -Path $resourceDir,$workDir | Out-Null
$zipPath = Join-Path $workDir 'Ultimate-ASI-Loader-NoPDB_x64-v10.0.0.zip'
$expected = '3385950d6af79257d18420b1d6904bc47480170b3dabe345f8c1339d0a49183f'
if (!(Test-Path -LiteralPath $zipPath)) {
    # The old x64-latest winhttp asset was removed. Pin the same version's
    # universal proxy from its tagged release and keep the checksum guard.
    Invoke-WebRequest 'https://github.com/ThirteenAG/Ultimate-ASI-Loader/releases/download/v10.0.0/Ultimate-ASI-Loader-NoPDB_x64.zip' -OutFile $zipPath
}
$sha = [System.Security.Cryptography.SHA256]::Create()
$actual = [BitConverter]::ToString($sha.ComputeHash([IO.File]::ReadAllBytes($zipPath))).Replace('-','').ToLowerInvariant()
$sha.Dispose()
if ($actual -ne $expected) { throw 'Ultimate ASI Loader archive checksum mismatch' }
Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [IO.Compression.ZipFile]::OpenRead($zipPath)
try {
    $entry = $archive.GetEntry('dinput8.dll')
    if (!$entry) { throw 'The universal proxy dinput8.dll is missing from the archive' }
    # The universal proxy selects the forwarding target from its DLL filename.
    [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, (Join-Path $resourceDir 'winhttp.dll'), $true)
} finally { $archive.Dispose() }
cargo build --manifest-path (Join-Path $projectRoot 'native/Cargo.toml') --release
if ($LASTEXITCODE -ne 0) { throw 'Bridge build failed' }
Copy-Item -LiteralPath (Join-Path $projectRoot 'native/target/release/pex_mod_bridge.dll') -Destination (Join-Path $resourceDir 'PEXModBridge.asi') -Force
Write-Output 'Native loader ready (verified Ultimate ASI Loader 10.0.0 + PEX Bridge).'
