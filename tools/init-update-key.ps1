$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Security
$projectRoot=Split-Path -Parent $PSScriptRoot
$keyFolder=Join-Path $env:LOCALAPPDATA 'QuestroSigning'
$keyFile=Join-Path $keyFolder 'update-key.dpapi'
New-Item -ItemType Directory -Force -Path $keyFolder | Out-Null
if(Test-Path -LiteralPath $keyFile){
 $keyBytes=[Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($keyFile),$null,[Security.Cryptography.DataProtectionScope]::CurrentUser)
 $payload=@{privateKey=[Convert]::ToBase64String($keyBytes)}|ConvertTo-Json -Compress
 $publicKey=$payload | node (Join-Path $PSScriptRoot 'update-signing.cjs') public
 [Array]::Clear($keyBytes,0,$keyBytes.Length)
}else{
 $generated=node (Join-Path $PSScriptRoot 'update-signing.cjs') generate | ConvertFrom-Json
 if($LASTEXITCODE -ne 0){throw 'Signing key generation failed'}
 $keyBytes=[Convert]::FromBase64String($generated.privateKey)
 $protected=[Security.Cryptography.ProtectedData]::Protect($keyBytes,$null,[Security.Cryptography.DataProtectionScope]::CurrentUser)
 [IO.File]::WriteAllBytes($keyFile,$protected)
 [Array]::Clear($keyBytes,0,$keyBytes.Length)
 $publicKey=$generated.publicKey
}
if($LASTEXITCODE -ne 0 -or $publicKey -notmatch '^[0-9a-f]{64}$'){throw 'Could not derive the public key'}
$publicFile=Join-Path $projectRoot 'src-tauri/update-public-key.hex'
if((Test-Path -LiteralPath $publicFile) -and ([IO.File]::ReadAllText($publicFile).Trim() -ne $publicKey)){throw 'Refusing to replace an existing public key'}
[IO.File]::WriteAllText($publicFile,$publicKey)
Write-Output 'Update signing key is protected for this Windows user outside the repository. Only the public key is embedded.'