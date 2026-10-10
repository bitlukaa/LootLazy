$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$launcher=Join-Path $projectRoot 'Questro.exe'
$binary=if(Test-Path -LiteralPath $launcher){$launcher}else{Join-Path $projectRoot 'dist/Questro-Tauri/Questro.exe'}
if (!(Test-Path -LiteralPath $binary)) { throw 'Run npm run build before starting Questro.' }
Start-Process -FilePath $binary -WorkingDirectory (Split-Path -Parent $binary) -WindowStyle Hidden
