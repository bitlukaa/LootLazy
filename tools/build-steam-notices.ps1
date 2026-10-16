$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$assets=Get-Content -LiteralPath (Join-Path $projectRoot 'data/asf-build/ArchiSteamFarm/obj/project.assets.json') -Raw | ConvertFrom-Json
$cache=$assets.packageFolders.PSObject.Properties.Name | Select-Object -First 1
$parts=[Collections.Generic.List[string]]::new()
$parts.Add('LootLazy Steam account engine - based on ArchiSteamFarm 6.3.10.6')
$parts.Add('Copyright 2015-2026 Lukasz "JustArchi" Domeradzki. Licensed under Apache-2.0.')
$parts.Add('Modified for LootLazy: private pipe protocol, encrypted account storage, restricted account idling and parent-owned lifecycle.')
$version=(Get-Content -LiteralPath (Join-Path $projectRoot 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json).version
$parts.Add('Editable ASF source, corresponding SteamKit source, and build instructions: https://github.com/bitlukaa/LootLazy/tree/v'+$version)
$parts.Add((Get-Content -LiteralPath (Join-Path $projectRoot 'third_party/ArchiSteamFarm/LICENSE.txt') -Raw))
$licenses=[Collections.Generic.HashSet[string]]::new()
$target=$assets.targets.'net10.0/win-x64'
$packages=@($target.PSObject.Properties | Where-Object {$_.Value.type -eq 'package' -and ($_.Value.runtime -or $_.Value.native -or $_.Value.resource)} | ForEach-Object { $_.Name.ToLowerInvariant() })
foreach($dependency in $assets.project.frameworks.'net10.0'.downloadDependencies){if($dependency.name -match '(\.Runtime\.win-x64|\.Host\.win-x64)$'){ $version=([regex]::Match($dependency.version,'\d+\.\d+\.\d+')).Value; $packages+=($dependency.name.ToLowerInvariant()+'/'+$version)}}
foreach($package in ($packages|Sort-Object -Unique)){
 $folder=Join-Path $cache $package
 $spec=Get-ChildItem -LiteralPath $folder -Filter '*.nuspec' | Select-Object -First 1
 if(!$spec){throw "License metadata missing: $package"}
 [xml]$xml=Get-Content -LiteralPath $spec.FullName -Raw
 $metadata=$xml.SelectSingleNode('//*[local-name()="metadata"]')
 $license=$metadata.SelectSingleNode('*[local-name()="license"]')
 $authors=$metadata.SelectSingleNode('*[local-name()="authors"]')
 $copyright=$metadata.SelectSingleNode('*[local-name()="copyright"]')
 $project=$metadata.SelectSingleNode('*[local-name()="projectUrl"]')
 $parts.Add("`n--- $package ---")
 if($authors){$parts.Add('Authors: '+$authors.InnerText)}
 if($copyright){$parts.Add($copyright.InnerText)}
 if($project){$parts.Add('Project: '+$project.InnerText)}
 if($license){$parts.Add('License: '+$license.InnerText);if($license.type -eq 'expression'){[void]$licenses.Add($license.InnerText)}}
 $files=@(Get-ChildItem -LiteralPath $folder -File | Where-Object {$_.Name -match '^(license|copying|notice|third.party|3rd party)' -and $_.Length -lt 3000000})
 if($license -and $license.type -eq 'file'){ $file=Join-Path $folder $license.InnerText; if(Test-Path -LiteralPath $file){$files+=Get-Item -LiteralPath $file} }
 foreach($file in ($files|Sort-Object FullName -Unique)){$parts.Add((Get-Content -LiteralPath $file.FullName -Raw))}
}
# Include exact license texts once even for NuGet packages containing only an SPDX expression.
$licenseDir=Join-Path $projectRoot 'data/engine-license-texts'
New-Item -ItemType Directory -Force $licenseDir | Out-Null
foreach($expression in ($licenses|Sort-Object)){
 foreach($identifier in ($expression -split '[\s()]+' | Where-Object {$_ -and $_ -notin @('AND','OR','WITH')} | Sort-Object -Unique)){
  if($identifier -notmatch '^[A-Za-z0-9.+-]+$'){throw 'Invalid SPDX identifier'}
  $textPath=Join-Path $licenseDir ($identifier+'.txt')
  if(!(Test-Path -LiteralPath $textPath)){Invoke-WebRequest ('https://raw.githubusercontent.com/spdx/license-list-data/main/text/'+$identifier+'.txt') -OutFile $textPath}
 }
}
foreach($file in (Get-ChildItem -LiteralPath $licenseDir -File | Sort-Object Name)){$parts.Add("`n--- License text: $($file.BaseName) ---");$parts.Add((Get-Content -LiteralPath $file.FullName -Raw))}
$content=(($parts -join "`n").Replace("`r`n","`n") -replace '(?m)[ \t]+$', '').TrimEnd()+"`n"
[IO.File]::WriteAllText((Join-Path $projectRoot 'public/assets/third-party/steam-engine.txt'),$content,[Text.UTF8Encoding]::new($false))
Write-Output 'Steam engine dependency notices collected.'
