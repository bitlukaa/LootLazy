$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$env:CARGO_TARGET_DIR=Join-Path $projectRoot 'data/.tauri-build'
$env:TEMP=Join-Path $projectRoot 'data/.test-temp'
$env:TMP=$env:TEMP
New-Item -ItemType Directory -Force -Path $env:TEMP | Out-Null
& (Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe') test --release --locked --manifest-path (Join-Path $projectRoot 'src-tauri/Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
