#!/usr/bin/env pwsh
# Update script for P2P node (Windows)
# Downloads the latest release binary from GitHub and restarts the node.
#
# Usage: .\update.ps1 [-Repo] <user/repo> [-Port] <port>
#   Default port: 3000

param(
    [string]$Repo = "",
    [int]$Port = 3000
)

$AppName = "p2p-node"
$Ext = ".exe"

# Try to detect repo from git remote
if (-not $Repo) {
    $remote = git config --get remote.origin.url 2>$null
    if ($remote) {
        $Repo = $remote -replace '.*github.com[/:]', '' -replace '\.git$', ''
    }
}

if (-not $Repo) {
    Write-Host "Usage: .\update.ps1 -Repo <github_user/repo> [-Port <port>]"
    exit 1
}

# Detect platform
$arch = $env:PROCESSOR_ARCHITECTURE
switch ($arch) {
    "AMD64" { $Label = "windows-x86_64" }
    "ARM64" { $Label = "windows-arm64" }
    default {
        Write-Host "Unsupported architecture: $arch"
        exit 1
    }
}

Write-Host "=== P2P Node Update ==="
Write-Host "  Repo:  $Repo"
Write-Host "  Label: $Label"
Write-Host "  Port:  $Port"
Write-Host ""

# Fetch latest release
Write-Host "Fetching latest release..."
$releaseUrl = "https://api.github.com/repos/$Repo/releases/latest"
$latest = Invoke-RestMethod -Uri $releaseUrl -UseBasicParsing
$tag = $latest.tag_name
Write-Host "  Latest: $tag"
Write-Host ""

$downloadUrl = "https://github.com/$Repo/releases/download/$tag/$AppName-$Label$Ext"

$downloadDir = Join-Path $env:TEMP "p2p-update-$(Get-Random)"
New-Item -ItemType Directory -Path $downloadDir -Force | Out-Null

$downloadedFile = Join-Path $downloadDir "$AppName$Ext"
Write-Host "Downloading $AppName-$Label..."
try {
    Invoke-WebRequest -Uri $downloadUrl -OutFile $downloadedFile -UseBasicParsing
} catch {
    Write-Host "Download failed. Check if $downloadUrl exists."
    exit 1
}

# Find and stop the running node
$nodeProcess = Get-Process -Name $AppName -ErrorAction SilentlyContinue
if ($nodeProcess) {
    Write-Host "Stopping running node (PID: $($nodeProcess.Id))..."
    $nodeProcess | Stop-Process -Force
    Start-Sleep -Seconds 2
}

# Replace binary
$installPath = (Get-Command $AppName -ErrorAction SilentlyContinue).Source
if (-not $installPath) {
    $installPath = "$PSScriptRoot\..\target\release\$AppName$Ext"
}

Write-Host "Installing to $installPath..."
$installDir = Split-Path $installPath -Parent
New-Item -ItemType Directory -Path $installDir -Force | Out-Null
Copy-Item -Path $downloadedFile -Destination $installPath -Force

# Restart
Write-Host ""
Write-Host "Starting $AppName on port $Port..."
$env:P2P_PORT = $Port
$logFile = "$env:TEMP\p2p_node_update.log"
$process = Start-Process -FilePath $installPath -WindowStyle Hidden -PassThru -RedirectStandardOutput $logFile

Start-Sleep -Seconds 2
if (-not $process.HasExited) {
    Write-Host "Node restarted (PID: $($process.Id))"
    Write-Host "  http://127.0.0.1:$Port"
} else {
    Write-Host "Node failed to start. Check: $logFile"
    exit 1
}
