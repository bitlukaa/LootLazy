param([switch]$Run)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$version=(Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw|ConvertFrom-Json).version
$testRoot=[IO.Path]::GetFullPath((Join-Path $projectRoot 'data/clean-install-check'))
$payload=Join-Path $testRoot 'payload'
$results=Join-Path $testRoot 'results'
New-Item -ItemType Directory -Force -Path $payload,$results | Out-Null
Copy-Item -LiteralPath (Join-Path $projectRoot "releases/LootLazy-$version-Setup.exe") -Destination (Join-Path $payload 'Setup.exe') -Force
$binary=Join-Path $projectRoot "releases/LootLazy-$version/LootLazy.exe"
@{version=$version;sha256=(Get-FileHash -LiteralPath $binary).Hash} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $payload 'expected.json') -Encoding UTF8
$guest=@'
$ErrorActionPreference='Stop'
$report=@{passed=$false;checks=@();error=$null}
try {
 $expected=Get-Content -LiteralPath C:\Payload\expected.json -Raw|ConvertFrom-Json
 $setup=Start-Process -FilePath C:\Payload\Setup.exe -ArgumentList '/S','/D=C:\LootLazy' -WindowStyle Hidden -PassThru
 if(!$setup.WaitForExit(600000)){throw 'Installer timed out'}
 if($setup.ExitCode -ne 0){throw "Installer exit code: $($setup.ExitCode)"}
 $exe='C:\LootLazy\LootLazy.exe'
 if(!(Test-Path -LiteralPath $exe)){throw 'Installed executable is missing'}
 if((Get-FileHash -LiteralPath $exe).Hash -ne $expected.sha256){throw 'Installed payload hash mismatch'}
 $report.checks+='Silent installation and payload integrity'
 $probe=Start-Process -FilePath $exe -ArgumentList '--verify-backend' -WindowStyle Hidden -PassThru
 if(!$probe.WaitForExit(30000) -or $probe.ExitCode -ne 0){throw 'Backend verification failed'}
 $info=Get-Content -LiteralPath C:\LootLazy\data\tauri-verification.json -Raw|ConvertFrom-Json
 if($info.version -ne $expected.version){throw 'Installed version mismatch'}
 $report.checks+='Fresh-profile backend and version'
 $app=Start-Process -FilePath $exe -ArgumentList '--minimized' -WindowStyle Hidden -PassThru
 Start-Sleep -Seconds 15
 if($app.HasExited){throw 'Normal WebView startup failed'}
 $report.checks+='Normal startup with the installed WebView runtime'
 & taskkill.exe /PID $app.Id /T /F | Out-Null
 $report.passed=$true
} catch {$report.error=$_.Exception.Message}
$report | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath C:\Results\result.json -Encoding UTF8
'@
Set-Content -LiteralPath (Join-Path $payload 'test.ps1') -Value $guest -Encoding UTF8
$escape={param($value)[Security.SecurityElement]::Escape($value)}
$config=@"
<Configuration>
 <Networking>Enable</Networking><ClipboardRedirection>Disable</ClipboardRedirection>
 <MappedFolders>
  <MappedFolder><HostFolder>$(& $escape $payload)</HostFolder><SandboxFolder>C:\Payload</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder>
  <MappedFolder><HostFolder>$(& $escape $results)</HostFolder><SandboxFolder>C:\Results</SandboxFolder><ReadOnly>false</ReadOnly></MappedFolder>
 </MappedFolders>
 <LogonCommand><Command>powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\Payload\test.ps1</Command></LogonCommand>
</Configuration>
"@
$configPath=Join-Path $testRoot 'LootLazy.wsb'
Set-Content -LiteralPath $configPath -Value $config -Encoding UTF8
Write-Output "Prepared clean-install plan: $configPath"
Write-Output "Results: $(Join-Path $results 'result.json')"
if($Run){
 $sandbox=Get-Command WindowsSandbox.exe -ErrorAction SilentlyContinue
 if(!$sandbox){throw 'Windows Sandbox is unavailable; the plan is prepared but has not been run.'}
 Start-Process -FilePath $sandbox.Source -ArgumentList $configPath -WindowStyle Hidden
}
