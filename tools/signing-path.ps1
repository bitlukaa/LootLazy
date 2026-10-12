function Get-LootLazySigningKeyPath {
 param([string]$StorageRoot=$env:LOCALAPPDATA)
 $preferred=Join-Path $StorageRoot 'LootLazySigning/update-key.dpapi'
 $legacy=Join-Path $StorageRoot 'QuestroSigning/update-key.dpapi'
 if(!(Test-Path -LiteralPath $preferred) -and (Test-Path -LiteralPath $legacy)){
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $preferred) | Out-Null
  # Copy the existing DPAPI-protected key; retain the original as a backup.
  Copy-Item -LiteralPath $legacy -Destination $preferred -ErrorAction Stop
 }
 return $preferred
}
