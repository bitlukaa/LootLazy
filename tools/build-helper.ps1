$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$compiler=Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
if(!(Test-Path -LiteralPath $compiler)){throw 'The Windows .NET Framework C# compiler is required to build the session helper.'}
$output=Join-Path $projectRoot 'runner/src-win.exe'
& $compiler /nologo /target:winexe /optimize+ /r:System.Windows.Forms.dll /r:System.Drawing.dll "/out:$output" (Join-Path $projectRoot 'runner/dummy.cs')
if($LASTEXITCODE -ne 0){throw 'Session helper build failed'}
Write-Output 'Session helper rebuilt from runner/dummy.cs.'