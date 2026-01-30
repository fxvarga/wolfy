# Install Wolfy to run at Windows startup
# Run this script as administrator for system-wide install, or as regular user for current user only

param(
    [switch]$Uninstall,
    [switch]$AllUsers
)

$ErrorActionPreference = "Stop"

# Find wolfy.exe - check common locations
$wolfy = $null
$searchPaths = @(
    "$PSScriptRoot\wolfy.exe",
    "$PSScriptRoot\..\target\release\wolfy.exe",
    "$PSScriptRoot\..\target\debug\wolfy.exe",
    "$PSScriptRoot\..\wolfy.exe",
    "$env:LOCALAPPDATA\wolfy\wolfy.exe",
    "$env:ProgramFiles\wolfy\wolfy.exe"
)

foreach ($path in $searchPaths) {
    $resolved = [System.IO.Path]::GetFullPath($path)
    if (Test-Path $resolved) {
        $wolfy = $resolved
        break
    }
}

if (-not $wolfy -and -not $Uninstall) {
    Write-Host "Error: Could not find wolfy.exe" -ForegroundColor Red
    Write-Host "Searched in:"
    foreach ($path in $searchPaths) {
        Write-Host "  - $([System.IO.Path]::GetFullPath($path))"
    }
    Write-Host ""
    Write-Host "Please build wolfy first with: cargo build --release"
    exit 1
}

$taskName = "Wolfy"

if ($Uninstall) {
    Write-Host "Removing Wolfy from startup..." -ForegroundColor Yellow
    
    # Remove scheduled task if exists
    $existingTask = Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
    if ($existingTask) {
        Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
        Write-Host "Removed scheduled task: $taskName" -ForegroundColor Green
    }
    
    # Remove from startup folder
    $startupFolder = if ($AllUsers) {
        "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\Startup"
    } else {
        "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Startup"
    }
    $shortcutPath = Join-Path $startupFolder "Wolfy.lnk"
    if (Test-Path $shortcutPath) {
        Remove-Item $shortcutPath -Force
        Write-Host "Removed startup shortcut: $shortcutPath" -ForegroundColor Green
    }
    
    Write-Host "Wolfy removed from startup." -ForegroundColor Green
    exit 0
}

Write-Host "Installing Wolfy to startup..." -ForegroundColor Cyan
Write-Host "  Executable: $wolfy"

# Method 1: Create a scheduled task (preferred - more control)
$action = New-ScheduledTaskAction -Execute $wolfy
$trigger = New-ScheduledTaskTrigger -AtLogon
$principal = if ($AllUsers) {
    New-ScheduledTaskPrincipal -GroupId "BUILTIN\Users" -RunLevel Limited
} else {
    New-ScheduledTaskPrincipal -UserId $env:USERNAME -RunLevel Limited
}
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable

# Remove existing task if present
$existingTask = Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
if ($existingTask) {
    Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
    Write-Host "  Removed existing task" -ForegroundColor Yellow
}

# Register new task
Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Description "Wolfy application launcher" | Out-Null

Write-Host ""
Write-Host "Wolfy installed to startup successfully!" -ForegroundColor Green
Write-Host ""
Write-Host "Wolfy will now start automatically when you log in."
Write-Host ""
Write-Host "To remove from startup, run:"
Write-Host "  .\install-startup.ps1 -Uninstall" -ForegroundColor Cyan
