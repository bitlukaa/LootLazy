$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$assetsPath=Join-Path $projectRoot 'engine/obj/project.assets.json'
if(-not (Test-Path -LiteralPath $assetsPath)){
    Write-Output 'Project assets not found; skipping notices.'
    return
}
$assets=Get-Content -LiteralPath $assetsPath -Raw -Encoding UTF8 | ConvertFrom-Json
$cache=$assets.packageFolders.PSObject.Properties.Name | Select-Object -First 1
$parts=[Collections.Generic.List[string]]::new()
$parts.Add('LootLazy Steam account engine - Standalone SteamKit2 client')
$parts.Add('Copyright 2026 LootLazy. All rights reserved.')
$parts.Add('Private pipe protocol, encrypted DPAPI storage, parent-owned lifecycle.')
$version=(Get-Content -LiteralPath (Join-Path $projectRoot 'src-tauri/tauri.conf.json') -Raw -Encoding UTF8 | ConvertFrom-Json).version
$parts.Add('Source repository: https://github.com/bitlukaa/LootLazy/tree/v'+$version)

$licenses=[Collections.Generic.HashSet[string]]::new()
$target=$assets.targets.'net9.0/win-x64'
if(-not $target){
    $targetProp = $assets.targets.PSObject.Properties | Select-Object -First 1
    if($targetProp){ $target = $targetProp.Value }
}
if($target){
    $packages=@($target.PSObject.Properties | Where-Object {$_.Value.type -eq 'package' -and ($_.Value.runtime -or $_.Value.native -or $_.Value.resource)} | ForEach-Object { $_.Name.ToLowerInvariant() })
    foreach($package in ($packages|Sort-Object -Unique)){
        $folder=Join-Path $cache $package
        $spec=Get-ChildItem -LiteralPath $folder -Filter '*.nuspec' -ErrorAction SilentlyContinue | Select-Object -First 1
        if(!$spec){continue}
        [xml]$xml=Get-Content -LiteralPath $spec.FullName -Raw -Encoding UTF8
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
        $files=@(Get-ChildItem -LiteralPath $folder -File -ErrorAction SilentlyContinue | Where-Object {$_.Name -match '^(license|copying|notice|third.party|3rd party)' -and $_.Length -lt 3000000})
        if($license -and $license.type -eq 'file'){ $file=Join-Path $folder $license.InnerText; if(Test-Path -LiteralPath $file){$files+=Get-Item -LiteralPath $file} }
        foreach($file in ($files|Sort-Object FullName -Unique)){$parts.Add((Get-Content -LiteralPath $file.FullName -Raw -Encoding UTF8))}
    }
}
$noticeDir=Join-Path $projectRoot 'public/assets/third-party'
New-Item -ItemType Directory -Force $noticeDir | Out-Null
$content=(($parts -join "`n").Replace("`r`n","`n") -replace '(?m)[ \t]+$', '').TrimEnd()+"`n"
[IO.File]::WriteAllText((Join-Path $noticeDir 'steam-engine.txt'),$content,[Text.UTF8Encoding]::new($false))
Write-Output 'Steam engine dependency notices collected.'
