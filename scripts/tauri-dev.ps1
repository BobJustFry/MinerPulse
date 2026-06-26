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
$root = Split-Path $PSScriptRoot -Parent
Set-Location (Join-Path $root "minerpulse-desktop")
npm run tauri dev
