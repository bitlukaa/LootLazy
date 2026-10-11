$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$cmd = Get-Command cargo.exe -ErrorAction SilentlyContinue
$cargoExe = if ($cmd) { $cmd.Source } else { $null }
if (!$cargoExe -or !(Test-Path -LiteralPath $cargoExe)) {
  $cargoExe = Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe'
}
if (!(Test-Path -LiteralPath $cargoExe)) { throw 'Install Rust with the MSVC toolchain first.' }
& (Join-Path $PSScriptRoot 'build-notices.ps1')
$env:CARGO_TARGET_DIR = Join-Path $projectRoot 'data/.tauri-build'
$env:TEMP = Join-Path $projectRoot 'data/.test-temp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Force -Path $env:TEMP | Out-Null
& $cargoExe build --release --locked --manifest-path (Join-Path $projectRoot 'src-tauri/Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'Tauri build failed.' }
$output = Join-Path $projectRoot 'dist/Questro-Tauri'
New-Item -ItemType Directory -Force -Path $output | Out-Null
$builtBinary = Join-Path $env:CARGO_TARGET_DIR 'release/questro.exe'
try {
  Copy-Item -LiteralPath $builtBinary -Destination (Join-Path $output 'Questro.exe')
  $staged=Join-Path $output 'Questro.exe.pending'
  if (Test-Path -LiteralPath $staged) { Remove-Item -LiteralPath $staged -Force }
}
catch {
  Copy-Item -LiteralPath $builtBinary -Destination (Join-Path $output 'Questro.exe.pending')
  Write-Output 'Application is open. Update staged for the next launcher start.'
}
Copy-Item -LiteralPath (Join-Path $projectRoot 'public/assets/THIRD-PARTY-NOTICES.txt') -Destination $output
Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md') -Destination $output
$compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
if (Test-Path -LiteralPath $compiler) {
  & $compiler /nologo /target:winexe /r:System.Windows.Forms.dll "/win32icon:$(Join-Path $projectRoot 'Questro.ico')" "/out:$(Join-Path $projectRoot 'Questro.exe')" (Join-Path $projectRoot 'src/Launcher.cs')
  if ($LASTEXITCODE -ne 0) { throw 'Launcher build failed.' }
}
# Portable user data is never replaced by a rebuild.
$dataFolder = Join-Path $output 'data'
New-Item -ItemType Directory -Force -Path $dataFolder | Out-Null
$binary = Get-Item -LiteralPath $builtBinary
Write-Output "Built $($binary.FullName) ($([math]::Round($binary.Length / 1MB,2)) MiB)"
