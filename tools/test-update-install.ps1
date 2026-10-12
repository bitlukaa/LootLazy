$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$testBase=[IO.Path]::GetFullPath((Join-Path $projectRoot 'data/.test-temp'))
$testRoot=[IO.Path]::GetFullPath((Join-Path $testBase ('questro-update-'+[guid]::NewGuid().ToString())))
if(!$testRoot.StartsWith($testBase+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Invalid test root'}
$updates=Join-Path $testRoot 'data/updates'
New-Item -ItemType Directory -Force -Path $updates | Out-Null
$target=Join-Path $testRoot 'Questro.exe'
$source=Join-Path $updates 'Questro-update.exe'
$manifestPath=Join-Path $updates 'install.json'
$settings=Join-Path $testRoot 'data/settings.json'
Set-Content -LiteralPath $target -Value 'original executable fixture'
Set-Content -LiteralPath $settings -Value 'preserved user settings'
$oldHash=(Get-FileHash -LiteralPath $target).Hash
Set-Content -LiteralPath $source -Value 'replacement executable fixture'
$newHash=(Get-FileHash -LiteralPath $source).Hash
$manifest=@{target=$target;source=$source;parentPid=2147483647;sha256=$newHash;version='99.0.0';restart=$false}
$manifest|ConvertTo-Json|Set-Content -LiteralPath $manifestPath
$engine=(Get-Process -Id $PID).Path
& $engine -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'install-update.ps1') -ManifestPath $manifestPath
if($LASTEXITCODE -ne 0){throw 'Fixture install failed'}
if((Get-FileHash -LiteralPath $target).Hash -ne $newHash){throw 'Replacement mismatch'}
if((Get-FileHash -LiteralPath (Join-Path $updates 'previous.exe')).Hash -ne $oldHash){throw 'Backup mismatch'}
if((Get-Content -LiteralPath $settings -Raw).Trim() -ne 'preserved user settings'){throw 'Settings were changed'}
Set-Content -LiteralPath $source -Value 'corrupted update fixture'
& $engine -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'install-update.ps1') -ManifestPath $manifestPath
if($LASTEXITCODE -eq 0){throw 'Corrupt update accepted'}
if((Get-FileHash -LiteralPath $target).Hash -ne $newHash){throw 'Corrupt update changed or downgraded the current executable'}
if((Get-Content -LiteralPath $settings -Raw).Trim() -ne 'preserved user settings'){throw 'Settings were changed'}
if(!([IO.Path]::GetFullPath($testRoot)).StartsWith($testBase+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Cleanup path escaped test folder'}
Remove-Item -LiteralPath $testRoot -Recurse -Force
Write-Output 'Update installation fixtures passed: replacement, backup, settings retention, and corrupt-update rejection. No installed app was replaced.'

exit 0
