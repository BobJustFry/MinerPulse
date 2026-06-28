#Requires -Version 5.1
<#
.SYNOPSIS
  Upload minerpulse-platform to VPS and run non-interactive install.

.USAGE
  1. Copy deploy/deploy.config.example -> deploy/deploy.config
  2. Fill VPS_HOST, MPULSE_BOOTSTRAP_ADMIN_EMAIL, etc.
  3. From repo root:  powershell -ExecutionPolicy Bypass -File deploy/deploy.ps1
#>
param(
  [string]$ConfigFile = "",
  [switch]$SkipUpload,
  [switch]$SkipInstall,
  [switch]$IntegrateProxy
)

$ErrorActionPreference = "Stop"
$DeployDir = $PSScriptRoot
$RepoRoot = Split-Path $DeployDir -Parent
if (-not $ConfigFile) { $ConfigFile = Join-Path $DeployDir "deploy.config" }

function Read-DeployConfig {
  param([string]$Path)
  if (-not (Test-Path $Path)) {
    throw "Config not found: $Path`nCopy deploy/deploy.config.example -> deploy/deploy.config"
  }
  $cfg = @{}
  Get-Content $Path | ForEach-Object {
    $line = $_.Trim()
    if (-not $line -or $line.StartsWith("#")) { return }
    $idx = $line.IndexOf("=")
    if ($idx -lt 1) { return }
    $key = $line.Substring(0, $idx).Trim()
    $val = $line.Substring($idx + 1).Trim()
    $cfg[$key] = $val
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
  $args = @() + $BaseArgs + @($Source, $Dest)
  & scp @args
  if ($LASTEXITCODE -ne 0) { throw "scp failed (exit $LASTEXITCODE)" }
}

$cfg = Read-DeployConfig -Path $ConfigFile
$vpsHost = Get-Cfg $cfg "VPS_HOST"
$vpsUser = Get-Cfg $cfg "VPS_USER" "root"
$remoteDir = Get-Cfg $cfg "REMOTE_DIR" "/opt/mpulse"
$sshKey = Get-Cfg $cfg "SSH_KEY"

if (-not $vpsHost) { throw "VPS_HOST is required in deploy.config" }

$sshTarget = "${vpsUser}@${vpsHost}"
$sshArgs = @()
$scpArgs = @()
if ($sshKey) {
  $sshArgs += @("-i", $sshKey, "-o", "BatchMode=yes")
  $scpArgs += @("-i", $sshKey)
}

Write-Host "=== MinerPulse deploy -> ${sshTarget}:${remoteDir} ===" -ForegroundColor Cyan

if (-not $SkipUpload) {
  $tarName = "minerpulse-platform.tgz"
  $tarPath = Join-Path $env:TEMP $tarName
  if (Test-Path $tarPath) { Remove-Item $tarPath -Force }

  Write-Host "[1/4] Packing project..." -ForegroundColor Yellow
  Push-Location $RepoRoot
  try {
    $excludes = @(
      "--exclude=node_modules",
      "--exclude=.env",
      "--exclude=secrets",
      "--exclude=deploy/generated",
      "--exclude=deploy/deploy.config",
      "--exclude=.git"
    )
    & tar -czf $tarPath @excludes .
    if ($LASTEXITCODE -ne 0) { throw "tar pack failed" }
  } finally {
    Pop-Location
  }

  Write-Host "[2/4] Uploading to VPS..." -ForegroundColor Yellow
  Invoke-Ssh $sshArgs $sshTarget "mkdir -p '$remoteDir'"
  Invoke-Scp $scpArgs $tarPath "${sshTarget}:${remoteDir}/${tarName}"
  Invoke-Ssh $sshArgs $sshTarget @"
set -e
cd '$remoteDir'
tar -xzf '$tarName'
rm -f '$tarName'
chmod +x deploy/*.sh deploy/lib/*.sh apps/api/docker-entrypoint.sh apps/web/docker-entrypoint.sh apps/admin/docker-entrypoint.sh 2>/dev/null || true
"@
  Remove-Item $tarPath -Force -ErrorAction SilentlyContinue
} else {
  Write-Host "[skip] Upload" -ForegroundColor DarkGray
}

if (-not $SkipInstall) {
  Write-Host "[3/4] Running install on VPS (non-interactive)..." -ForegroundColor Yellow

  $envLines = @(
    "export MPULSE_NONINTERACTIVE=1",
    "export MPULSE_BASE_DOMAIN='$(Get-Cfg $cfg 'MPULSE_BASE_DOMAIN' 'mpulse.bob4.fun')'",
    "export MPULSE_LETSENCRYPT_EMAIL='$(Get-Cfg $cfg 'MPULSE_LETSENCRYPT_EMAIL' 'admin@bob4.fun')'",
    "export MPULSE_DEPLOY_MODE='$(Get-Cfg $cfg 'MPULSE_DEPLOY_MODE' 'external-proxy')'",
    "export MPULSE_ADMIN_IP_ALLOWLIST='$(Get-Cfg $cfg 'MPULSE_ADMIN_IP_ALLOWLIST')'",
    "export MPULSE_LICENSE_OFFLINE_GRACE_DAYS='$(Get-Cfg $cfg 'MPULSE_LICENSE_OFFLINE_GRACE_DAYS' '14')'",
    "export MPULSE_BOOTSTRAP_ADMIN_USERNAME='$(Get-Cfg $cfg 'MPULSE_BOOTSTRAP_ADMIN_USERNAME' 'mpulse-admin')'",
    "export MPULSE_BOOTSTRAP_ADMIN_PASSWORD='$(Get-Cfg $cfg 'MPULSE_BOOTSTRAP_ADMIN_PASSWORD')'",
    "export MPULSE_POSTGRES_PASSWORD='$(Get-Cfg $cfg 'MPULSE_POSTGRES_PASSWORD')'"
  )
  $remoteInstall = ($envLines -join "`n") + "`n" + @"
set -e
cd '$remoteDir'
bash deploy/install.sh
"@ -join "`n"

  Invoke-Ssh $sshArgs $sshTarget $remoteInstall
} else {
  Write-Host "[skip] Install" -ForegroundColor DarkGray
}

$autoIntegrate = (Get-Cfg $cfg "AUTO_INTEGRATE_PROXY" "0") -eq "1"
if ($IntegrateProxy -or $autoIntegrate) {
  Write-Host "[4/4] Integrating external proxy (SharedAI)..." -ForegroundColor Yellow
  $integrateEnv = @(
    "export SHARED_AI_DIR='$(Get-Cfg $cfg 'SHARED_AI_DIR' '/opt/sharedai')'",
    "export SHARED_AI_CADDYFILE='$(Get-Cfg $cfg 'SHARED_AI_CADDYFILE')'",
    "export SHARED_AI_COMPOSE_FILE='$(Get-Cfg $cfg 'SHARED_AI_COMPOSE_FILE')'",
    "export SHARED_AI_CADDY_SERVICE='$(Get-Cfg $cfg 'SHARED_AI_CADDY_SERVICE' 'caddy')'"
  )
  $remoteIntegrate = ($integrateEnv -join "`n") + "`n" + @"
set -e
cd '$remoteDir'
bash deploy/integrate-external-proxy.sh
"@ -join "`n"
  Invoke-Ssh $sshArgs $sshTarget $remoteIntegrate
} else {
  Write-Host "[4/4] Proxy integration skipped (set AUTO_INTEGRATE_PROXY=1 or -IntegrateProxy)" -ForegroundColor DarkGray
}

Write-Host ""
Write-Host "=== Done ===" -ForegroundColor Green
Write-Host "Credentials on VPS: ${remoteDir}/deploy/generated/credentials.txt"
Write-Host "Proxy snippet:        ${remoteDir}/deploy/generated/host-proxy.conf"
Write-Host ""
Write-Host "Show credentials:" -ForegroundColor Cyan
Write-Host "  ssh ${sshTarget} `"cat ${remoteDir}/deploy/generated/credentials.txt`""
