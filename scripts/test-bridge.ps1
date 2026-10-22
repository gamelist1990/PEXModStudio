$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$fixture=Join-Path $projectRoot ('work/bridge-test-' + [guid]::NewGuid().ToString('N'))
$modDir=Join-Path $fixture 'PEXMods/probe'
New-Item -ItemType Directory -Path $modDir -Force | Out-Null
$exe=Join-Path $fixture 'Minecraft.Windows.exe'
rustc --edition 2021 (Join-Path $PSScriptRoot 'bridge-harness.rs') -o $exe
if ($LASTEXITCODE -ne 0) { throw 'Harness compile failed' }
rustc --edition 2021 --crate-type cdylib (Join-Path $PSScriptRoot 'probe.rs') -o (Join-Path $modDir 'probe.dll')
if ($LASTEXITCODE -ne 0) { throw 'Probe compile failed' }
$root=Join-Path $fixture 'PEXMods'
$utf8=New-Object System.Text.UTF8Encoding($false)
[IO.File]::WriteAllText((Join-Path $root 'active.txt'), "probe/probe.dll`n",$utf8)
$bridge=Join-Path $projectRoot 'src-tauri/resources/PEXModBridge.asi'
& $exe $bridge
$log=Join-Path $root 'loader.log'
if (!([IO.File]::ReadAllText($log).Contains('LOADED probe/probe.dll'))) { throw 'DLL was not loaded' }
Write-Output 'PASS: bridge loads enabled x64 DLL outside DllMain'
[IO.File]::WriteAllText((Join-Path $root 'loading.txt'),'probe/probe.dll',$utf8)
& $exe $bridge
& $exe $bridge
$text=[IO.File]::ReadAllText($log)
if (($text.Split([string[]]@('SKIP probe/probe.dll'),[StringSplitOptions]::None).Length-1) -ne 2) { throw 'Recovery skip did not persist' }
Write-Output 'PASS: interrupted initialization is skipped across subsequent sessions'
[IO.File]::WriteAllText((Join-Path $root 'active.txt'),"../outside.dll`n",$utf8)
& $exe $bridge
if (!([IO.File]::ReadAllText($log).Contains('ERROR: invalid path'))) { throw 'Path traversal was not rejected' }
Write-Output 'PASS: native bridge rejects paths outside its managed directory'
