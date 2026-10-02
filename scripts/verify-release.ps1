param([string]$Version)

$ErrorActionPreference = 'Stop'
if (-not $Version) { $Version = ([IO.File]::ReadAllText((Resolve-Path 'package.json'), [Text.Encoding]::UTF8) | ConvertFrom-Json).version }
$config = [IO.File]::ReadAllText((Resolve-Path 'src-tauri/tauri.conf.json'), [Text.Encoding]::UTF8) | ConvertFrom-Json
if ($config.version -ne $Version) { throw "Tauri version $($config.version) differs from $Version" }
$dir = 'src-tauri/target/release/bundle/nsis'
$name = "MemeJi_${Version}_x64-setup.exe"
$exe = Join-Path $dir $name
$signaturePath = "$exe.sig"
$manifestPath = Join-Path $dir 'latest.json'
foreach ($path in @($exe, $signaturePath, $manifestPath)) {
  if (-not (Test-Path $path)) { throw "Missing release artifact: $path" }
}
$signature = [IO.File]::ReadAllText((Resolve-Path $signaturePath)).Trim()
$decoded = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($signature))
$trustedComment = $decoded -split "`n" | Where-Object { $_ -like 'trusted comment:*' } | Select-Object -First 1
if (-not $trustedComment -or $trustedComment -notmatch "(?<!\S)version:$([regex]::Escape($Version))(?!\S)") {
  throw "Updater signature is not bound to version $Version"
}
$manifest = [IO.File]::ReadAllText((Resolve-Path $manifestPath), [Text.Encoding]::UTF8) | ConvertFrom-Json
$platform = $manifest.platforms.'windows-x86_64'
$expectedUrl = "https://github.com/wu66chen/MemeJi/releases/download/v$Version/$name"
if ($manifest.version -ne $Version -or $platform.url -ne $expectedUrl -or $platform.signature.Trim() -ne $signature) {
  throw 'Updater manifest does not match the version, installer URL, and signature'
}
$sha256 = [Security.Cryptography.SHA256]::Create()
try { $hash = [BitConverter]::ToString($sha256.ComputeHash([IO.File]::ReadAllBytes((Resolve-Path $exe)))).Replace('-', '').ToLowerInvariant() }
finally { $sha256.Dispose() }
$sumPath = Join-Path $dir 'SHA256SUMS.txt'
if (Test-Path $sumPath) {
  $sum = (Get-Content $sumPath -Raw).Trim()
  if ($sum -ne "$hash  $name") { throw 'SHA256SUMS.txt does not match the installer' }
}
Write-Output "Verified release $Version; signed version present; manifest and SHA-256 match."
