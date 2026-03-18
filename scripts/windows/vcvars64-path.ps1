$ErrorActionPreference = 'Stop'

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere)) {
  throw "vswhere.exe not found at expected path: $vswhere"
}

$installPath = & $vswhere -products * -latest -property installationPath
if (-not $installPath) {
  throw 'No Visual Studio installation found by vswhere.exe (need VS 2017+ Build Tools with VC tools + Windows SDK)'
}

$vcvars = Join-Path $installPath 'VC\Auxiliary\Build\vcvars64.bat'
if (-not (Test-Path -LiteralPath $vcvars)) {
  throw "vcvars64.bat not found at expected path: $vcvars"
}

# Print the path only (stdout). Makefile callers should strip CR if needed.
Write-Output $vcvars
