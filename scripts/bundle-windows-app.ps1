Param(
  [string]$Target = "",
  [string]$OutDir = "dist\\windows",
  [string]$AppName = "CliSwitch",
  [switch]$SkipUi
)

$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $root

$binName = "cliswitch"

$versionLine = Select-String -Path "Cargo.toml" -Pattern '^\s*version\s*=\s*"(.*)"\s*$' | Select-Object -First 1
$version = if ($versionLine) { $versionLine.Matches[0].Groups[1].Value } else { "0.0.0" }

if (-not $SkipUi) {
  Push-Location ui
  if (-not (Test-Path node_modules)) {
    npm ci
  }
  npm run build
  Pop-Location
}

if ([string]::IsNullOrWhiteSpace($Target)) {
  cargo build --release
  $exePath = Join-Path $root "target\\release\\$binName.exe"
} else {
  cargo build --release --target $Target
  $exePath = Join-Path $root "target\\$Target\\release\\$binName.exe"
}

if (-not (Test-Path $exePath)) {
  throw "build output not found: $exePath"
}

$outRoot = Join-Path $root $OutDir
$appDir = Join-Path $outRoot $AppName

if (Test-Path $appDir) { Remove-Item -Recurse -Force $appDir }
New-Item -ItemType Directory -Force -Path $appDir | Out-Null

Copy-Item $exePath (Join-Path $appDir "$AppName.exe") -Force
Copy-Item "README.md","LICENSE" $appDir -Force

Write-Output "OK: $appDir (version=$version)"
