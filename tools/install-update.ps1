param([Parameter(Mandatory=$true)][string]$ManifestPath)
$ErrorActionPreference='Stop'
$manifest=Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json
$target=[IO.Path]::GetFullPath([string]$manifest.target)
$source=[IO.Path]::GetFullPath([string]$manifest.source)
$appFolder=Split-Path -Parent $target
$updateFolder=[IO.Path]::GetFullPath((Join-Path $appFolder 'data/updates'))
if((Split-Path -Leaf $target) -ine 'Questro.exe' -or !(Split-Path -Parent $source).Equals($updateFolder,[StringComparison]::OrdinalIgnoreCase)){throw 'Invalid update paths'}
$statusPath=Join-Path $updateFolder 'status.json'
$backup=Join-Path $updateFolder 'previous.exe'
$backupCreated=$false
try{
if((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -ine [string]$manifest.sha256){throw 'Update checksum mismatch'}
Wait-Process -Id ([int]$manifest.parentPid) -ErrorAction SilentlyContinue
Copy-Item -LiteralPath $target -Destination $backup -Force
$backupCreated=$true
$replaced=$false
for($attempt=0;$attempt -lt 60;$attempt++){
try{
Copy-Item -LiteralPath $source -Destination $target -Force
if((Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash -ine [string]$manifest.sha256){throw 'Installed checksum mismatch'}
$replaced=$true
break
}catch{Start-Sleep -Milliseconds 500}
}
if(!$replaced){throw 'Could not replace the executable'}
@{status='installed';version=$manifest.version}|ConvertTo-Json|Set-Content -LiteralPath $statusPath
Remove-Item -LiteralPath $source -Force
if($manifest.restart -ne $false){Start-Process -FilePath $target -WorkingDirectory $appFolder -WindowStyle Hidden}
}catch{
if($backupCreated -and (Test-Path -LiteralPath $backup)){Copy-Item -LiteralPath $backup -Destination $target -Force -ErrorAction SilentlyContinue}
@{status='failed';message=$_.Exception.Message}|ConvertTo-Json|Set-Content -LiteralPath $statusPath
if($manifest.restart -ne $false){Start-Process -FilePath $target -WorkingDirectory $appFolder -WindowStyle Hidden}
exit 1
}
