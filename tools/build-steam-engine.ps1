param([string]$DotnetPath='')
$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
if(-not $DotnetPath){
    $cmd = Get-Command 'dotnet' -ErrorAction SilentlyContinue
    if($cmd){ $DotnetPath = $cmd.Source }
    elseif(Test-Path 'C:\Program Files\dotnet\dotnet.exe'){ $DotnetPath = 'C:\Program Files\dotnet\dotnet.exe' }
    else { $DotnetPath = 'dotnet' }
}
$output=Join-Path $projectRoot 'data/steam-engine'
New-Item -ItemType Directory -Force -Path $output | Out-Null
$arguments=@('publish',(Join-Path $projectRoot 'engine/LootLazy.Steam.csproj'),'-c','Release','-r','win-x64','-p:RuntimeIdentifiers=win-x64','--self-contained','true','-p:PublishSingleFile=true','-p:IncludeNativeLibrariesForSelfExtract=true','-p:EnableCompressionInSingleFile=true','-p:PublishTrimmed=true','-p:JsonSerializerIsReflectionEnabledByDefault=true','-p:EnableNETAnalyzers=false','-p:TreatWarningsAsErrors=false',"-p:ApplicationIcon=$(Join-Path $projectRoot 'LootLazy.ico')",'-o',$output,'--nologo')
& $DotnetPath @arguments
if($LASTEXITCODE -ne 0){throw 'Steam engine build failed'}
$engineExe=Join-Path $output 'LootLazy.Steam.exe'
if(!(Test-Path -LiteralPath $engineExe)){throw 'Engine output missing'}
$env:DOTNET_BUNDLE_EXTRACT_BASE_DIR=Join-Path $projectRoot 'data/engine-test-runtime'
& $engineExe --self-test
if($LASTEXITCODE -ne 0){throw 'Steam engine vault self-test failed'}
& (Join-Path $PSScriptRoot 'build-steam-notices.ps1')
Get-FileHash -LiteralPath $engineExe -Algorithm SHA256
