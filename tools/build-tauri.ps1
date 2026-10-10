$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$cargoExe = Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe'
if (!(Test-Path -LiteralPath $cargoExe)) { throw 'Install Rust with the MSVC toolchain first.' }
$env:CARGO_TARGET_DIR = Join-Path $projectRoot 'data/.tauri-build'
$env:TEMP = Join-Path $projectRoot 'data/.test-temp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Force -Path $env:TEMP | Out-Null
& $cargoExe build --release --locked --manifest-path (Join-Path $projectRoot 'src-tauri/Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'Tauri build failed.' }
$output = Join-Path $projectRoot 'dist/Questro-Tauri'
New-Item -ItemType Directory -Force -Path $output | Out-Null
Copy-Item -LiteralPath (Join-Path $env:CARGO_TARGET_DIR 'release/questro.exe') -Destination (Join-Path $output 'Questro.exe')
foreach ($license in @('LUCIDE-LICENSE.txt','DISCORD-QUEST-HELPER-LICENSE.txt','TAURI-LICENSE-MIT.txt')) { Copy-Item -LiteralPath (Join-Path $projectRoot "public/assets/$license") -Destination $output }
Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md') -Destination $output
$compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
if (Test-Path -LiteralPath $compiler) {
  & $compiler /nologo /target:winexe /r:System.Windows.Forms.dll "/win32icon:$(Join-Path $projectRoot 'Questro.ico')" "/out:$(Join-Path $projectRoot 'Questro.exe')" (Join-Path $projectRoot 'src/Launcher.cs')
  if ($LASTEXITCODE -ne 0) { throw 'Launcher build failed.' }
}
# Portable user data is never replaced by a rebuild.
$dataFolder = Join-Path $output 'data'
New-Item -ItemType Directory -Force -Path $dataFolder | Out-Null
$binary = Get-Item -LiteralPath (Join-Path $output 'Questro.exe')
Write-Output "Built $($binary.FullName) ($([math]::Round($binary.Length / 1MB,2)) MiB)"
