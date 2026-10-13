$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'signing-path.ps1')
$testRoot=Join-Path $PSScriptRoot '../data/.test-temp/signing-fixture'
$expected=Join-Path $testRoot 'LootLazySigning/update-key.dpapi'
if((Get-LootLazySigningKeyPath -StorageRoot $testRoot) -ne $expected){throw 'Incorrect protected key path'}
Write-Output 'Protected signing path fixture passed.'
