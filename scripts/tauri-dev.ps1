# MinerPulse — dev launcher (ensures Rust/cargo is on PATH)
$ErrorActionPreference = "Stop"

$env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" +
  [System.Environment]::GetEnvironmentVariable("Path", "User")

$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  if (Test-Path (Join-Path $cargoBin "cargo.exe")) {
    $env:Path = "$cargoBin;$env:Path"
  } else {
    Write-Host "Rust/cargo not found. Install: winget install Rustlang.Rustup" -ForegroundColor Red
    Write-Host "Then close this terminal, open a new one, and run again." -ForegroundColor Yellow
    exit 1
  }
}

Write-Host ("cargo: " + (cargo --version)) -ForegroundColor DarkGray

# Stop stale dev instances that lock minerpulse-desktop.exe or port 1420
Get-Process -Name "minerpulse-desktop" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Get-NetTCPConnection -LocalPort 1420 -ErrorAction SilentlyContinue |
  ForEach-Object { Stop-Process -Id $_.OwningProcess -Force -ErrorAction SilentlyContinue }
Start-Sleep -Milliseconds 500

$root = Split-Path $PSScriptRoot -Parent
Set-Location (Join-Path $root "minerpulse-desktop")
npm run tauri dev
