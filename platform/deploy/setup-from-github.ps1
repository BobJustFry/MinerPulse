#Requires -Version 5.1
<#
.SYNOPSIS
  Deploy MinerPulse platform/ to VPS: clone/pull monorepo from GitHub, install.

.USAGE
  1. powershell -ExecutionPolicy Bypass -File deploy\configure.ps1
  2. powershell -ExecutionPolicy Bypass -File deploy\setup-from-github.ps1
  (configure runs automatically if deploy.config is missing)
#>
param(
  [string]$ConfigFile = "",
  [ValidateSet("install", "update")]
  [string]$Action = "install",
  [switch]$SkipIntegrate
)

$ErrorActionPreference = "Stop"
$DeployDir = $PSScriptRoot
if (-not $ConfigFile) { $ConfigFile = Join-Path $DeployDir "deploy.config" }

if (-not (Test-Path $ConfigFile)) {
  Write-Host "deploy.config not found — starting wizard..." -ForegroundColor Yellow
  & (Join-Path $DeployDir "configure.ps1")
  if (-not (Test-Path $ConfigFile)) {
    throw "deploy.config was not created. Run: deploy\configure.ps1"
  }
}

function Read-DeployConfig {
  param([string]$Path)
  if (-not (Test-Path $Path)) {
    throw "Config not found: $Path`nCopy deploy\deploy.config.example -> deploy\deploy.config"
  }
  $cfg = @{}
  Get-Content $Path | ForEach-Object {
    $line = $_.Trim()
    if (-not $line -or $line.StartsWith("#")) { return }
    $idx = $line.IndexOf("=")
    if ($idx -lt 1) { return }
    $cfg[$line.Substring(0, $idx).Trim()] = $line.Substring($idx + 1).Trim()
  }
  return $cfg
}

function Get-Cfg {
  param([hashtable]$Cfg, [string]$Key, [string]$Default = "")
  if ($Cfg.ContainsKey($Key) -and $Cfg[$Key]) { return $Cfg[$Key] }
  return $Default
}

function Invoke-Ssh {
  param([string[]]$BaseArgs, [string]$Target, [string]$RemoteCommand)
  & ssh @BaseArgs $Target $RemoteCommand
  if ($LASTEXITCODE -ne 0) { throw "ssh failed (exit $LASTEXITCODE)" }
}

function Invoke-Scp {
  param([string[]]$BaseArgs, [string]$Source, [string]$Dest)
  & scp @($BaseArgs + $Source, $Dest)
  if ($LASTEXITCODE -ne 0) { throw "scp failed (exit $LASTEXITCODE)" }
}

$cfg = Read-DeployConfig -Path $ConfigFile
$vpsHost = Get-Cfg $cfg "VPS_HOST"
$vpsUser = Get-Cfg $cfg "VPS_USER" "root"
$githubRepo = Get-Cfg $cfg "GITHUB_REPO"
$sshKey = Get-Cfg $cfg "SSH_KEY"

if (-not $vpsHost) { throw "VPS_HOST is required" }
if (-not $githubRepo) { throw "GITHUB_REPO is required (e.g. https://github.com/you/MinerPulse.git)" }

$sshTarget = "${vpsUser}@${vpsHost}"
$sshArgs = @("-o", "ConnectTimeout=20", "-o", "StrictHostKeyChecking=accept-new")
$scpArgs = @("-o", "ConnectTimeout=20", "-o", "StrictHostKeyChecking=accept-new")
if ($sshKey) {
  $sshArgs += @("-i", $sshKey, "-o", "BatchMode=yes")
  $scpArgs += @("-i", $sshKey, "-o", "BatchMode=yes")
} else {
  Write-Host "[!] SSH_KEY not set in deploy.config — using default key/agent (password auth may hang)." -ForegroundColor Yellow
}

Write-Host "=== MinerPulse GitHub deploy -> ${sshTarget} (${Action}) ===" -ForegroundColor Cyan
Write-Host "Repo: $githubRepo" -ForegroundColor DarkGray

$remoteBootstrapDir = "/tmp/minerpulse-bootstrap"
$bundleTar = Join-Path $env:TEMP "minerpulse-bootstrap.tgz"

Write-Host "[1/3] Uploading bootstrap scripts..." -ForegroundColor Yellow
Push-Location (Split-Path $DeployDir -Parent)
try {
  $bundleItems = @(
    "deploy/bootstrap-from-github.sh",
    "deploy/lib/load-config.sh",
    "deploy/lib/export-install-env.sh",
    "deploy/lib/docker-install.sh",
    "deploy/lib/prompt.sh",
    "deploy/lib/preflight.sh",
    "deploy/lib/render-templates.sh",
    "deploy/templates"
  )
  foreach ($item in $bundleItems) {
    if (-not (Test-Path $item)) { throw "Missing $item" }
  }
  if (Test-Path $bundleTar) { Remove-Item $bundleTar -Force }
  & tar -czf $bundleTar -C deploy bootstrap-from-github.sh lib templates
  if ($LASTEXITCODE -ne 0) { throw "tar failed" }
} finally {
  Pop-Location
}

Invoke-Ssh $sshArgs $sshTarget "rm -rf '$remoteBootstrapDir' && mkdir -p '$remoteBootstrapDir'"
Invoke-Scp $scpArgs $bundleTar "${sshTarget}:${remoteBootstrapDir}/bootstrap.tgz"
Invoke-Scp $scpArgs $ConfigFile "${sshTarget}:${remoteBootstrapDir}/deploy.config"
Remove-Item $bundleTar -Force -ErrorAction SilentlyContinue

Invoke-Ssh $sshArgs $sshTarget @"
set -e
cd '$remoteBootstrapDir'
tar -xzf bootstrap.tgz
rm -f bootstrap.tgz
chmod +x bootstrap-from-github.sh lib/*.sh
bash bootstrap-from-github.sh '$remoteBootstrapDir/deploy.config' '$Action'
"@

$autoIntegrate = (Get-Cfg $cfg "AUTO_INTEGRATE_PROXY" "0") -eq "1"
if ($Action -eq "install" -and $autoIntegrate -and -not $SkipIntegrate) {
  Write-Host "[ok] Proxy integration handled by bootstrap (AUTO_INTEGRATE_PROXY=1)" -ForegroundColor Green
}

Write-Host ""
Write-Host "=== Done ===" -ForegroundColor Green
Write-Host "Show credentials:"
$remotePlatform = "$(Get-Cfg $cfg 'REMOTE_DIR' '/opt/minerpulse')/$(Get-Cfg $cfg 'PLATFORM_SUBDIR' 'platform')"
Write-Host "  ssh ${sshTarget} `"cat ${remotePlatform}/deploy/generated/credentials.txt`""
Write-Host "Update from GitHub:"
Write-Host "  powershell -ExecutionPolicy Bypass -File deploy\setup-from-github.ps1 -Action update"
