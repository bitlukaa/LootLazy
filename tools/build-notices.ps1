$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$parts=@('Questro third-party notices', '')
foreach($entry in @(@('Icon components','icons.txt'),@('UI component portions','ui-components.txt'),@('Desktop runtime','runtime.txt'))){
  $parts += $entry[0]
  $parts += Get-Content -LiteralPath (Join-Path $projectRoot ('public/assets/third-party/'+$entry[1])) -Raw
  $parts += ''
}
[IO.File]::WriteAllText((Join-Path $projectRoot 'public/assets/THIRD-PARTY-NOTICES.txt'),($parts -join [Environment]::NewLine))
