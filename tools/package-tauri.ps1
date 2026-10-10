param([string]$OutputName='Questro-3.1.0',[switch]$UpdateExisting)
$ErrorActionPreference='Stop'
if ($OutputName -notmatch '^Questro-\d+\.\d+\.\d+(?:-[A-Za-z0-9]+)?$') { throw 'Invalid package name.' }
$projectRoot=Split-Path -Parent $PSScriptRoot
$sourceExe=Join-Path $projectRoot 'dist/Questro-Tauri/Questro.exe'
if (Test-Path -LiteralPath ($sourceExe+'.pending')) { $sourceExe=$sourceExe+'.pending' }
if (!(Test-Path -LiteralPath $sourceExe)) { throw 'Build the native application first.' }
$releasesRoot=[IO.Path]::GetFullPath((Join-Path $projectRoot 'releases'))
$output=[IO.Path]::GetFullPath((Join-Path $releasesRoot $OutputName))
if (!$output.StartsWith($releasesRoot+'\',[StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid distribution path.' }
if ((Test-Path -LiteralPath $output) -and !$UpdateExisting) { throw 'Distribution folder already exists. Use a new OutputName or UpdateExisting; saved data is always preserved.' }
New-Item -ItemType Directory -Force -Path $output | Out-Null
Copy-Item -LiteralPath $sourceExe -Destination (Join-Path $output 'Questro.exe')
foreach($license in @('LUCIDE-LICENSE.txt','DISCORD-QUEST-HELPER-LICENSE.txt','TAURI-LICENSE-MIT.txt')) {
  Copy-Item -LiteralPath (Join-Path $projectRoot "public/assets/$license") -Destination $output
}
@'
# Questro 3.1

Open Questro.exe to start. Keep this folder together in a writable location.

This is the clean Tauri/Rust portable application, without source code, old versions, cached games, artwork, browser profiles or personal data. It uses the Microsoft Edge WebView2 runtime installed on Windows.

On first launch, Questro creates its data folder beside Questro.exe. The catalog, artwork, preferences and game helpers will increase the folder's size as you use the application. These files are separate from the approximately 3.85 MiB executable.

No old installation folder is required. WebView2: https://developer.microsoft.com/microsoft-edge/webview2/
'@ | Set-Content -LiteralPath (Join-Path $output 'README.md') -Encoding utf8
$sourceHash=(Get-FileHash -LiteralPath $sourceExe -Algorithm SHA256).Hash
$packageHash=(Get-FileHash -LiteralPath (Join-Path $output 'Questro.exe') -Algorithm SHA256).Hash
if ($sourceHash -ne $packageHash) { throw 'Packaged executable does not match the tested build.' }
$archive=Join-Path $releasesRoot "$OutputName.zip"
if ((Test-Path -LiteralPath $archive) -and !$UpdateExisting) { throw 'Archive already exists.' }
$payloadNames=@('Questro.exe','README.md','LUCIDE-LICENSE.txt','DISCORD-QUEST-HELPER-LICENSE.txt','TAURI-LICENSE-MIT.txt')
$payloadFiles=$payloadNames|ForEach-Object {Join-Path $output $_}
Compress-Archive -LiteralPath $payloadFiles -DestinationPath $archive -CompressionLevel Optimal -Force
$folderBytes=($payloadFiles|ForEach-Object {(Get-Item -LiteralPath $_).Length}|Measure-Object -Sum).Sum
[pscustomobject]@{Folder=$output;FolderBytes=$folderBytes;FolderMiB=[math]::Round($folderBytes/1MB,2);Zip=$archive;ZipMiB=[math]::Round((Get-Item -LiteralPath $archive).Length/1MB,2);SHA256=$packageHash}|ConvertTo-Json -Compress
