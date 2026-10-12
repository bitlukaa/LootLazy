$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'signing-path.ps1')
$testRoot=Join-Path (Split-Path -Parent $PSScriptRoot) ('data/.test-temp/signing-migration-'+[guid]::NewGuid().ToString())
$legacy=Join-Path $testRoot 'QuestroSigning/update-key.dpapi'
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $legacy) | Out-Null
[IO.File]::WriteAllBytes($legacy,[byte[]]@(1,2,3,4))
$preferred=Get-LootLazySigningKeyPath -StorageRoot $testRoot
if(!([BitConverter]::ToString([IO.File]::ReadAllBytes($preferred))).Equals('01-02-03-04')){throw 'Legacy key copy failed'}
[IO.File]::WriteAllBytes($preferred,[byte[]]@(9,8))
$null=Get-LootLazySigningKeyPath -StorageRoot $testRoot
if(!([BitConverter]::ToString([IO.File]::ReadAllBytes($preferred))).Equals('09-08')){throw 'Existing key was overwritten'}
if(!(Test-Path -LiteralPath $legacy)){throw 'Backup was removed'}
Write-Output 'Protected key path migration fixtures passed; existing key and backup are preserved.'
