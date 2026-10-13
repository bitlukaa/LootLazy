function Get-LootLazySigningKeyPath {
 param([string]$StorageRoot=$env:LOCALAPPDATA)
 return (Join-Path $StorageRoot 'LootLazySigning/update-key.dpapi')
}
