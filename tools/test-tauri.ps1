$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$rootItem = Get-Item -LiteralPath $projectRoot
if ($rootItem.Attributes -band [IO.FileAttributes]::ReparsePoint) {
    if ($rootItem.Target) { $projectRoot = $rootItem.Target[0] }
}
$env:CARGO_TARGET_DIR=Join-Path $projectRoot 'data/.tauri-build'
$env:TEMP=Join-Path $projectRoot 'data/.test-temp'
$env:TMP=$env:TEMP
New-Item -ItemType Directory -Force -Path $env:TEMP | Out-Null
$cmd = Get-Command cargo.exe -ErrorAction SilentlyContinue
$cargoCmd = if ($cmd) { $cmd.Source } else { $null }
if (!$cargoCmd -or !(Test-Path -LiteralPath $cargoCmd)) {
  $cargoCmd = Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe'
}
& $cargoCmd test --release --locked --manifest-path (Join-Path $projectRoot 'src-tauri/Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
