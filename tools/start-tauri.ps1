$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$launcher=Join-Path $projectRoot 'LootLazy.exe'
$binary=if(Test-Path -LiteralPath $launcher){$launcher}else{Join-Path $projectRoot 'dist/LootLazy-Tauri/LootLazy.exe'}
if (!(Test-Path -LiteralPath $binary)) { throw 'Run npm run build before starting LootLazy.' }
Start-Process -FilePath $binary -WorkingDirectory (Split-Path -Parent $binary) -WindowStyle Hidden
