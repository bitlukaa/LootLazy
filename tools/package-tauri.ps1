param([string]$OutputName='LootLazy-3.6.16',[switch]$UpdateExisting)
$ErrorActionPreference='Stop'
if ($OutputName -notmatch '^LootLazy-\d+\.\d+\.\d+(?:-[A-Za-z0-9]+)?$') { throw 'Invalid package name.' }
$projectRoot=Split-Path -Parent $PSScriptRoot
$sourceExe=Join-Path $projectRoot 'dist/LootLazy-Tauri/LootLazy.exe'
if (Test-Path -LiteralPath ($sourceExe+'.pending')) { $sourceExe=$sourceExe+'.pending' }
if (!(Test-Path -LiteralPath $sourceExe)) { throw 'Build the native application first.' }
$releasesRoot=[IO.Path]::GetFullPath((Join-Path $projectRoot 'releases'))
$output=[IO.Path]::GetFullPath((Join-Path $releasesRoot $OutputName))
if (!$output.StartsWith($releasesRoot+'\',[StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid distribution path.' }
if ((Test-Path -LiteralPath $output) -and !$UpdateExisting) { throw 'Distribution folder already exists. Use a new OutputName or UpdateExisting; saved data is always preserved.' }
New-Item -ItemType Directory -Force -Path $output | Out-Null
Copy-Item -LiteralPath $sourceExe -Destination (Join-Path $output 'LootLazy.exe')
Copy-Item -LiteralPath (Join-Path $projectRoot 'public/assets/THIRD-PARTY-NOTICES.txt') -Destination $output
Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md') -Destination (Join-Path $output 'README.md')
Copy-Item -LiteralPath (Join-Path $projectRoot 'LICENSE') -Destination (Join-Path $output 'LICENSE')
$sourceHash=(Get-FileHash -LiteralPath $sourceExe -Algorithm SHA256).Hash
$packageHash=(Get-FileHash -LiteralPath (Join-Path $output 'LootLazy.exe') -Algorithm SHA256).Hash
if ($sourceHash -ne $packageHash) { throw 'Packaged executable does not match the tested build.' }
$archive=Join-Path $releasesRoot "$OutputName.zip"
if ((Test-Path -LiteralPath $archive) -and !$UpdateExisting) { throw 'Archive already exists.' }
$payloadNames=@('LootLazy.exe','README.md','LICENSE','THIRD-PARTY-NOTICES.txt')
$payloadFiles=$payloadNames|ForEach-Object {Join-Path $output $_}
Compress-Archive -LiteralPath $payloadFiles -DestinationPath $archive -CompressionLevel Optimal -Force
$folderBytes=($payloadFiles|ForEach-Object {(Get-Item -LiteralPath $_).Length}|Measure-Object -Sum).Sum
[pscustomobject]@{Folder=$output;FolderBytes=$folderBytes;FolderMiB=[math]::Round($folderBytes/1MB,2);Zip=$archive;ZipMiB=[math]::Round((Get-Item -LiteralPath $archive).Length/1MB,2);SHA256=$packageHash}|ConvertTo-Json -Compress
