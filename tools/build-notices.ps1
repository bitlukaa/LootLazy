$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$parts=@('LootLazy third-party notices', '')
foreach($entry in @(@('Icon components','icons.txt'),@('Brand icons','simple-icons.txt'),@('UI component portions','ui-components.txt'),@('Desktop runtime','runtime.txt'),@('Update signature cryptography','cryptography.txt'))){
  $parts += $entry[0]
  $parts += Get-Content -LiteralPath (Join-Path $projectRoot ('public/assets/third-party/'+$entry[1])) -Raw
  $parts += ''
}
[IO.File]::WriteAllText((Join-Path $projectRoot 'public/assets/THIRD-PARTY-NOTICES.txt'),(($parts -join [Environment]::NewLine) -replace '(?m)[ \t]+$', '').TrimEnd()+[Environment]::NewLine)
