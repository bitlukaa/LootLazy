$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$parts=@('LootLazy third-party notices', '')
foreach($entry in @(@('Icon components','icons.txt'),@('Brand icons','simple-icons.txt'),@('Platform branding','platform-branding.txt'),@('UI component portions','ui-components.txt'),@('Desktop runtime','runtime.txt'),@('Update signature cryptography','cryptography.txt'),@('Steam account engine','steam-engine.txt'))){
  $parts += $entry[0]
  $parts += Get-Content -LiteralPath (Join-Path $projectRoot ('public/assets/third-party/'+$entry[1])) -Raw
  $parts += ''
}
$content=(($parts -join "`n").Replace("`r`n","`n") -replace '(?m)[ \t]+$', '').TrimEnd()+"`n"
[IO.File]::WriteAllText((Join-Path $projectRoot 'public/assets/THIRD-PARTY-NOTICES.txt'),$content,[Text.UTF8Encoding]::new($false))
