param([string]$DotnetPath='dotnet')
$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$stage=Join-Path $projectRoot 'data/asf-build'
New-Item -ItemType Directory -Force -Path $stage | Out-Null
# Re-copy the immutable source; never patch the vendor snapshot or download upstream code at runtime.
Copy-Item -Path (Join-Path $projectRoot 'third_party/ArchiSteamFarm/*') -Destination $stage -Recurse -Force
& node (Join-Path $PSScriptRoot 'prepare-steam-engine.cjs')
if($LASTEXITCODE -ne 0){throw 'Steam engine source patching failed'}
$output=Join-Path $projectRoot 'data/steam-engine'
$arguments=@('publish',(Join-Path $stage 'ArchiSteamFarm/ArchiSteamFarm.csproj'),'-c','Release','-r','win-x64','-p:RuntimeIdentifiers=win-x64','--self-contained','true','-p:StartupObject=ArchiSteamFarm.LootLazyBridge','-p:PublishSingleFile=true','-p:IncludeNativeLibrariesForSelfExtract=true','-p:EnableCompressionInSingleFile=true','-p:PublishTrimmed=true','-p:JsonSerializerIsReflectionEnabledByDefault=true','-p:EnableNETAnalyzers=false','-p:TreatWarningsAsErrors=false',"-p:ApplicationIcon=$(Join-Path $projectRoot 'LootLazy.ico')",'-o',$output,'--nologo')
& $DotnetPath @arguments
if($LASTEXITCODE -ne 0){throw 'Steam engine build failed'}
$source=Join-Path $output 'ArchiSteamFarm.exe'
if(!(Test-Path -LiteralPath $source)){throw 'Engine output missing'}
Copy-Item -LiteralPath $source -Destination (Join-Path $output 'LootLazy.Steam.exe') -Force
$env:DOTNET_BUNDLE_EXTRACT_BASE_DIR=Join-Path $projectRoot 'data/engine-test-runtime'
& (Join-Path $output 'LootLazy.Steam.exe') --self-test
if($LASTEXITCODE -ne 0){throw 'Steam engine vault self-test failed'}
& (Join-Path $PSScriptRoot 'build-steam-notices.ps1')
Get-FileHash -LiteralPath (Join-Path $output 'LootLazy.Steam.exe') -Algorithm SHA256
