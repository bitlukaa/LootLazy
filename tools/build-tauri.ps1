$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$cmd = Get-Command cargo.exe -ErrorAction SilentlyContinue
$cargoExe = if ($cmd) { $cmd.Source } else { $null }
if (!$cargoExe -or !(Test-Path -LiteralPath $cargoExe)) {
  $cargoExe = Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe'
}
if (!(Test-Path -LiteralPath $cargoExe)) { throw 'Install Rust with the MSVC toolchain first.' }
& (Join-Path $PSScriptRoot 'build-helper.ps1')
& (Join-Path $PSScriptRoot 'build-notices.ps1')
$env:CARGO_TARGET_DIR = Join-Path $projectRoot 'data/.tauri-build'
$env:TEMP = Join-Path $projectRoot 'data/.test-temp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Force -Path $env:TEMP | Out-Null
& $cargoExe build --release --locked --manifest-path (Join-Path $projectRoot 'src-tauri/Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'Tauri build failed.' }
$output = Join-Path $projectRoot 'dist/LootLazy-Tauri'
New-Item -ItemType Directory -Force -Path $output | Out-Null
$builtBinary = Join-Path $env:CARGO_TARGET_DIR 'release/lootlazy.exe'
try {
  Copy-Item -LiteralPath $builtBinary -Destination (Join-Path $output 'LootLazy.exe') -Force
  $staged=Join-Path $output 'LootLazy.exe.pending'
  if (Test-Path -LiteralPath $staged) { Remove-Item -LiteralPath $staged -Force }
}
catch {
  Copy-Item -LiteralPath $builtBinary -Destination (Join-Path $output 'LootLazy.exe.pending')
  Write-Output 'Application is open. Update staged for the next launcher start.'
}
Copy-Item -LiteralPath (Join-Path $projectRoot 'public/assets/THIRD-PARTY-NOTICES.txt') -Destination $output
Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md') -Destination $output
Copy-Item -LiteralPath (Join-Path $projectRoot 'LICENSE') -Destination $output
$compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
if (Test-Path -LiteralPath $compiler) {
  & $compiler /nologo /target:winexe /r:System.Windows.Forms.dll "/win32icon:$(Join-Path $projectRoot 'LootLazy.ico')" "/out:$(Join-Path $projectRoot 'LootLazy.exe')" (Join-Path $projectRoot 'src/Launcher.cs')
  if ($LASTEXITCODE -ne 0) { throw 'Launcher build failed.' }
  $parentDir = Split-Path -Parent $projectRoot
  if (Test-Path -LiteralPath $parentDir) {
    Copy-Item -LiteralPath $builtBinary -Destination (Join-Path $parentDir 'LootLazy.exe') -Force
  }
  if ($LASTEXITCODE -ne 0) { throw 'Launcher build failed.' }
}
# Preserve the prior development profile when moving the build directory.
$legacyData=Join-Path $projectRoot 'dist/Questro-Tauri/data'
$newData=Join-Path $output 'data'
if((Test-Path -LiteralPath $legacyData) -and !(Test-Path -LiteralPath $newData)){
 $legacyExe=Join-Path $projectRoot 'dist/Questro-Tauri/Questro.exe'
 if(@(Get-CimInstance Win32_Process | Where-Object {$_.ExecutablePath -eq $legacyExe}).Count){throw 'Close the old development app before migrating its profile.'}
 Copy-Item -LiteralPath $legacyData -Destination $output -Recurse -ErrorAction Stop
}
# Portable user data is never replaced by a rebuild.
$dataFolder = Join-Path $output 'data'
New-Item -ItemType Directory -Force -Path $dataFolder | Out-Null
$binary = Get-Item -LiteralPath $builtBinary
Write-Output "Built $($binary.FullName) ($([math]::Round($binary.Length / 1MB,2)) MiB)"
