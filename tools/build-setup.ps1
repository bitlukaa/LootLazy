$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$env:CARGO_TARGET_DIR=Join-Path $projectRoot 'data/.tauri-build'
$env:TEMP=Join-Path $projectRoot 'data/.test-temp'
$env:TMP=$env:TEMP
if(!(Get-Command cargo.exe -ErrorAction SilentlyContinue)){$env:PATH=(Join-Path $env:USERPROFILE '.cargo/bin')+';'+$env:PATH}
New-Item -ItemType Directory -Force -Path $env:TEMP | Out-Null
Push-Location $projectRoot
try{
& npx.cmd --yes @tauri-apps/cli@2.12.0 bundle --bundles nsis --ci --no-binary-patching --config src-tauri/tauri.setup.conf.json
if($LASTEXITCODE -ne 0){throw 'Setup bundling failed'}
$version=(Get-Content -LiteralPath src-tauri/tauri.conf.json -Raw|ConvertFrom-Json).version
$source=Get-ChildItem -LiteralPath (Join-Path $env:CARGO_TARGET_DIR 'release/bundle/nsis') -Filter '*.exe' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if(!$source){throw 'Setup file not found'}
$releases=Join-Path $projectRoot 'releases'
New-Item -ItemType Directory -Force -Path $releases | Out-Null
$destination=Join-Path $releases "LootLazy-$version-Setup.exe"
Copy-Item -LiteralPath $source.FullName -Destination $destination -Force
Write-Output "Setup ready: $destination"
}finally{Pop-Location}
