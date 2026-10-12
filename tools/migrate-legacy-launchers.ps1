param([switch]$RemoveLaunchers)
$ErrorActionPreference='Stop'
$projectRoot=[IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$installRoot=Split-Path -Parent $projectRoot
$pairs=@(
 @{Old=(Join-Path $projectRoot 'Questro.exe');New=(Join-Path $projectRoot 'LootLazy.exe')},
 @{Old=(Join-Path $installRoot 'Questro.exe');New=(Join-Path $installRoot 'LootLazy.exe')},
 @{Old=(Join-Path $projectRoot 'dist/Questro-Tauri/Questro.exe');New=(Join-Path $projectRoot 'dist/LootLazy-Tauri/LootLazy.exe')},
 @{Old=(Join-Path $projectRoot 'dist/Questro-Tauri/LootLazy.exe');New=(Join-Path $projectRoot 'dist/LootLazy-Tauri/LootLazy.exe')}
)
$runPath='HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$properties=Get-ItemProperty -LiteralPath $runPath -ErrorAction SilentlyContinue
$legacyValue=if($properties){$properties.PSObject.Properties['Questro']}else{$null}
if($legacyValue){
 $legacyCommand=[string]$legacyValue.Value
 $pair=$pairs | Where-Object {$legacyCommand.StartsWith(('"'+$_.Old+'"'),[StringComparison]::OrdinalIgnoreCase) -or $legacyCommand.Equals($_.Old,[StringComparison]::OrdinalIgnoreCase)} | Select-Object -First 1
 if(!$pair){throw 'Legacy startup entry is not owned by this installation; launchers were not removed.'}
 if(!(Test-Path -LiteralPath $pair.New)){throw 'New startup target is missing.'}
 $newCommand=if($legacyCommand.StartsWith('"')){'"'+$pair.New+'"'+$legacyCommand.Substring($pair.Old.Length+2)}else{$pair.New}
 $existing=$properties.PSObject.Properties['LootLazy']
 if($existing -and [string]$existing.Value -ne $newCommand){throw 'A different LootLazy startup entry exists; launchers were not removed.'}
 New-ItemProperty -LiteralPath $runPath -Name 'LootLazy' -Value $newCommand -PropertyType String -Force | Out-Null
 Remove-ItemProperty -LiteralPath $runPath -Name 'Questro'
 Write-Output 'Owned startup entry migrated to the new executable.'
}
# A renamed startup value may still point at the previous build directory.
$properties=Get-ItemProperty -LiteralPath $runPath -ErrorAction SilentlyContinue
$currentValue=if($properties){$properties.PSObject.Properties['LootLazy']}else{$null}
if($currentValue){
 $command=[string]$currentValue.Value
 $owned=$pairs | Where-Object {$command.StartsWith(('"'+$_.Old+'"'),[StringComparison]::OrdinalIgnoreCase) -or $command.Equals($_.Old,[StringComparison]::OrdinalIgnoreCase)} | Select-Object -First 1
 if($owned){
  if(!(Test-Path -LiteralPath $owned.New)){throw 'New startup target is missing.'}
  $replacement=if($command.StartsWith('"')){'"'+$owned.New+'"'+$command.Substring($owned.Old.Length+2)}else{$owned.New}
  New-ItemProperty -LiteralPath $runPath -Name 'LootLazy' -Value $replacement -PropertyType String -Force | Out-Null
 }
}
foreach($pair in $pairs){
 if(!(Test-Path -LiteralPath $pair.Old)){continue}
 if(!(Test-Path -LiteralPath $pair.New)){throw 'New launcher is missing; refusing legacy cleanup.'}
 if(@(Get-CimInstance Win32_Process | Where-Object {$_.ExecutablePath -eq $pair.Old}).Count){throw 'A legacy launcher is running; close it before cleanup.'}
 if($RemoveLaunchers){Remove-Item -LiteralPath $pair.Old -Force}
}
Write-Output 'Legacy launcher checks completed; saved profiles were preserved.'
