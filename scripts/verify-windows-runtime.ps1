param([string]$Path = 'src-tauri/target/release/meme-manager.exe')

$ErrorActionPreference = 'Stop'
$binary = (Resolve-Path -LiteralPath $Path).Path
$dumpbin = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
if ($dumpbin) {
  $dumpbinPath = $dumpbin.Source
} else {
  $candidates = @(
    "${env:ProgramFiles}/Microsoft Visual Studio/*/*/VC/Tools/MSVC/*/bin/Hostx64/x64/dumpbin.exe",
    "${env:ProgramFiles(x86)}/Microsoft Visual Studio/*/*/VC/Tools/MSVC/*/bin/Hostx64/x64/dumpbin.exe"
  )
  $dumpbinPath = Get-ChildItem -Path $candidates -File -ErrorAction SilentlyContinue |
    Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
}
if (-not $dumpbinPath) {
  throw 'dumpbin.exe not found. Install Visual Studio C++ Build Tools or run from a Developer PowerShell.'
}

$output = @(& $dumpbinPath /DEPENDENTS $binary 2>&1)
if ($LASTEXITCODE -ne 0) { throw "Cannot inspect Windows binary: $binary" }
$dependencies = @($output | ForEach-Object {
  if ($_ -match '^\s+([\w.-]+\.dll)\s*$') { $Matches[1] }
} | Sort-Object -Unique)
if ($dependencies.Count -eq 0) { throw "No DLL dependencies found; cannot verify $binary" }

# /DEPENDENTS lists both normal and delay-loaded imports. Windows system DLLs
# are expected; the application must not require the separate VC Redistributable.
$runtime = @($dependencies | Where-Object {
  $_ -match '^(vcruntime|msvcp|msvcr|concrt|vccorlib|vcomp)\d.*\.dll$|^ucrtbased\.dll$'
})
if ($runtime.Count -gt 0) {
  throw "Windows runtime verification failed: $binary imports $($runtime -join ', '). Rebuild with static MSVC CRT linkage."
}
Write-Output "Verified Windows runtime imports: $binary has no external MSVC runtime DLL dependency."
