param([Parameter(Mandatory=$true)][string]$BinaryPath,[string]$Version)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Security
$projectRoot=Split-Path -Parent $PSScriptRoot
if(!$Version){$Version=(Get-Content -LiteralPath (Join-Path $projectRoot 'src-tauri/tauri.conf.json') -Raw|ConvertFrom-Json).version}
if((Get-Item -LiteralPath $BinaryPath).VersionInfo.ProductVersion -ne $Version){throw 'Binary version does not match the release being signed.'}
$keyFile=Join-Path $env:LOCALAPPDATA 'QuestroSigning/update-key.dpapi'
if(!(Test-Path -LiteralPath $keyFile)){throw 'Initialize the local update signing key first.'}
$keyBytes=[Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($keyFile),$null,[Security.Cryptography.DataProtectionScope]::CurrentUser)
try{
 $payload=@{privateKey=[Convert]::ToBase64String($keyBytes);binary=[IO.Path]::GetFullPath($BinaryPath);version=$Version;publicKeyFile=(Join-Path $projectRoot 'src-tauri/update-public-key.hex')}|ConvertTo-Json -Compress
 $payload | node (Join-Path $PSScriptRoot 'update-signing.cjs') sign
 if($LASTEXITCODE -ne 0){throw 'Release signing failed'}
}finally{[Array]::Clear($keyBytes,0,$keyBytes.Length);$payload=$null}