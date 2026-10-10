$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$binary=Join-Path $projectRoot 'dist/Questro-Tauri/Questro.exe'
if (!(Test-Path -LiteralPath $binary)) { throw 'Run npm run build before starting Questro.' }
Start-Process -FilePath $binary -WorkingDirectory (Split-Path -Parent $binary) -WindowStyle Hidden
