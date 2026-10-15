$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$testBase=[IO.Path]::GetFullPath((Join-Path $projectRoot 'data/.test-temp'))
$testRoot=[IO.Path]::GetFullPath((Join-Path $testBase ('lootlazy-uninstall-'+[guid]::NewGuid().ToString('N'))))
if(!$testRoot.StartsWith($testBase+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Invalid fixture path'}
$compiler=Join-Path $env:LOCALAPPDATA 'tauri/NSIS/makensis.exe'
if(!(Test-Path -LiteralPath $compiler)){throw 'Build Setup first to provision the NSIS compiler'}
$bin=Join-Path $testRoot 'bin'
New-Item -ItemType Directory -Path $bin -Force|Out-Null
$setup=Join-Path $bin 'probe-setup.exe'
$hook=Join-Path $projectRoot 'src-tauri/installer-hooks.nsh'
 . (Join-Path $PSScriptRoot 'configure-nsis-uninstall.ps1')
$fixtureScript=Join-Path $bin 'installer.nsi'
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'test-uninstall.nsi') -Destination $fixtureScript
Set-LootLazyUninstallDefaults -Path $fixtureScript
$patched=[IO.File]::ReadAllText($fixtureScript)
Set-LootLazyUninstallDefaults -Path $fixtureScript
if([IO.File]::ReadAllText($fixtureScript) -cne $patched){throw 'Defaults patch is not idempotent'}
& $compiler /V2 "/DHOOK_PATH=$hook" "/DOUTPUT_PATH=$setup" $fixtureScript
if($LASTEXITCODE -ne 0){throw 'Uninstall fixture compilation failed'}
$process=Start-Process -FilePath $setup -ArgumentList '/S' -WindowStyle Hidden -PassThru -Wait
if($process.ExitCode -ne 0){throw 'Fixture setup failed'}
$uninstaller=Join-Path $bin 'probe-uninstall.exe'
function Assert-Condition($condition,[string]$message){if(!$condition){throw $message}}
function Invoke-Fixture([string]$folder,[int]$delete,[int]$update){
 $choice=if($delete -lt 0){''}else{"/DELETE=$delete"}
 $arguments="/S $choice /UPDATE=$update _?=$folder"
 $process=Start-Process -FilePath $uninstaller -ArgumentList $arguments -WindowStyle Hidden -PassThru -Wait
 return $process.ExitCode
}
$results=@()
foreach($scenario in @('delete','default','keep','update','foreign','junction','readonly','locked','repeat','spaces','bulk','longpath')){
 $folder=Join-Path $testRoot $(if($scenario -eq 'spaces'){'install with spaces'}else{$scenario})
 $dataFile=Join-Path $folder 'data/webview/cache/example.bin'
 $gameFile=Join-Path $folder 'games/fixture/game.exe'
 New-Item -ItemType Directory -Path (Split-Path $dataFile),(Split-Path $gameFile) -Force|Out-Null
 [IO.File]::WriteAllText($dataFile,'Synthetic cache, no account data')
 [IO.File]::WriteAllText($gameFile,'Synthetic generated helper')
 foreach($name in @('LootLazy.exe','README.md','LootLazy.ico','LootLazy.exe.sig','LootLazy.exe.pending','CHANGELOG.md')){[IO.File]::WriteAllText((Join-Path $folder $name),'fixture')}
 if($scenario -eq 'longpath'){
  $deep=Join-Path $folder ('data/'+('a'*100)+'/'+('b'*100))
  [IO.Directory]::CreateDirectory($deep)|Out-Null
  [IO.File]::WriteAllText((Join-Path $deep 'long-cache-entry.bin'),'long path cache')
 }
 if($scenario -eq 'bulk'){
  for($index=0;$index -lt 100;$index++){
   $child=Join-Path $folder ('data/cache-'+$index+'/nested')
   New-Item -ItemType Directory -Path $child -Force|Out-Null
   [IO.File]::WriteAllText((Join-Path $child 'entry.bin'),'cache')
  }
 }
 $outside=Join-Path $testRoot 'junction-target'
 if($scenario -eq 'foreign'){[IO.File]::WriteAllText((Join-Path $folder 'personal.txt'),'Unrelated file')}
 if($scenario -eq 'junction'){
  New-Item -ItemType Directory -Path $outside -Force|Out-Null
  [IO.File]::WriteAllText((Join-Path $outside 'keep.txt'),'Must survive')
  New-Item -ItemType Junction -Path (Join-Path $folder 'data/external-link') -Target $outside|Out-Null
 }
 if($scenario -eq 'readonly'){(Get-Item -LiteralPath $dataFile).IsReadOnly=$true}
 $handle=$null
 try{
  if($scenario -eq 'locked'){$handle=[IO.File]::Open($dataFile,[IO.FileMode]::Open,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)}
  $delete=if($scenario -eq 'keep'){0}elseif($scenario -eq 'default'){-1}else{1}
  $update=if($scenario -eq 'update'){1}else{0}
  $exitCode=Invoke-Fixture $folder $delete $update
  if($scenario -eq 'locked'){
   Assert-Condition ($exitCode -eq 2) 'Locked files must return an incomplete-cleanup exit code'
   Assert-Condition (Test-Path -LiteralPath $dataFile) 'A locked file was unexpectedly removed'
  }else{Assert-Condition ($exitCode -eq 0) "Unexpected exit code for $scenario`: $exitCode"}
  if($scenario -in @('keep','update')){
   Assert-Condition (Test-Path -LiteralPath $dataFile) "$scenario lost saved data"
   Assert-Condition (Test-Path -LiteralPath $gameFile) "$scenario lost generated helpers"
   if($scenario -eq 'update'){Assert-Condition (Test-Path -LiteralPath (Join-Path $folder 'LootLazy.exe.pending')) 'Update removed staged binary'}
   else{Assert-Condition (!(Test-Path -LiteralPath (Join-Path $folder 'LootLazy.exe.pending'))) 'Normal uninstall left a staged executable'}
  }elseif($scenario -eq 'foreign'){
   Assert-Condition (Test-Path -LiteralPath (Join-Path $folder 'personal.txt')) 'Unrelated file was removed'
   Assert-Condition (!(Test-Path -LiteralPath (Join-Path $folder 'data'))) 'Owned data remains beside an unrelated file'
   Assert-Condition (!(Test-Path -LiteralPath (Join-Path $folder 'games'))) 'Owned helpers remain beside an unrelated file'
  }elseif($scenario -ne 'locked'){
   Assert-Condition (!(Test-Path -LiteralPath $folder)) "$scenario left owned files behind"
  }
  if($scenario -eq 'junction'){Assert-Condition ((Get-Content -LiteralPath (Join-Path $outside 'keep.txt') -Raw) -eq 'Must survive') 'Junction target was modified'}
 }finally{if($handle){$handle.Dispose()}}
 if($scenario -eq 'locked'){
  Assert-Condition ((Invoke-Fixture $folder 1 0) -eq 0) 'Cleanup could not be retried after releasing the file'
  Assert-Condition (!(Test-Path -LiteralPath $folder)) 'Retry left locked data behind'
 }
 if($scenario -eq 'repeat'){Assert-Condition ((Invoke-Fixture $folder 1 0) -eq 0) 'Repeated cleanup is not idempotent'}
 $results+=$scenario
 Write-Output "Passed: $scenario"
}
@{scope='Synthetic files only; no installed app, registry, or account data changed';passed=$results}|ConvertTo-Json -Depth 4|Set-Content -LiteralPath (Join-Path $testRoot 'result.json')
Write-Output 'All uninstall cleanup fixtures passed.'
