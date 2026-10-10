param([Parameter(Mandatory=$true)][string]$FixtureRoot)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$resolvedFixture=[IO.Path]::GetFullPath($FixtureRoot)
$allowedPrefix=Join-Path $projectRoot 'data\.test-temp\questro-tauri-ui-'
if (!$resolvedFixture.StartsWith($allowedPrefix,[StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid test cleanup path' }
# WebView2 can outlive its hidden test host briefly. Match only this unique test profile.
Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object {$_.CommandLine -and $_.CommandLine.Contains($resolvedFixture)} | ForEach-Object {Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue}
for ($attempt=0; $attempt -lt 20; $attempt++) {
  if (!(Test-Path -LiteralPath $resolvedFixture)) { break }
  try { Remove-Item -LiteralPath $resolvedFixture -Recurse -Force; break }
  catch { if ($attempt -eq 19) { throw }; Start-Sleep -Milliseconds 500 }
}
