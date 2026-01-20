# Beeper Automations Windows Installer
# Usage: powershell -c "irm https://github.com/ErdemGKSL/beeper-automations/releases/latest/download/install.ps1 | iex"

$ErrorActionPreference = "Stop"

# Configuration
$GITHUB_REPO = "ErdemGKSL/beeper-automations"
$SERVICE_NAME = "auto-beeper-windows-service"
$CONFIGURATOR_NAME = "auto-beeper-configurator"
$INSTALL_DIR = "$env:ProgramFiles\BeeperAutomations"
$SCHEDULED_TASK_NAME = "BeeperAutomations"
$SERVICE_DESCRIPTION = "Background service for Beeper automations (runs in user session with hidden window)"

# Color functions
function Write-InfoMessage {
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor Green
}

function Write-WarnMessage {
    param([string]$Message)
    Write-Host "[WARN] $Message" -ForegroundColor Yellow
}

function Write-ErrorMessage {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor Red
}

# Check if running as administrator
function Test-Administrator {
    $currentUser = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    return $currentUser.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# Detect architecture
function Get-Architecture {
    $arch = $env:PROCESSOR_ARCHITECTURE
    
    switch ($arch) {
        "AMD64" { return "x86_64" }
        "x86" { return "x86" }
        "ARM64" { return "aarch64" }
        default {
            Write-ErrorMessage "Unsupported architecture: $arch"
            exit 1
        }
    }
}

# Get latest release
function Get-LatestRelease {
    Write-InfoMessage "Fetching latest release information..."
    
    $apiUrl = "https://api.github.com/repos/$GITHUB_REPO/releases"
    Write-InfoMessage "API URL: $apiUrl"
    
    try {
        $releases = Invoke-RestMethod -Uri $apiUrl -Method Get
        
        if ($releases.Count -eq 0) {
            Write-ErrorMessage "No releases found"
            exit 1
        }
        
        $latestRelease = $releases[0]
        $tag = $latestRelease.tag_name
        
        Write-InfoMessage "Latest release: $tag"
        return $tag
    }
    catch {
        Write-ErrorMessage "Failed to fetch release information: $_"
        exit 1
    }
}

# Download binaries
function Get-Binaries {
    param(
        [string]$Tag,
        [string]$Target
    )
    
    Write-InfoMessage "Downloading binaries for $Target..."
    
    $tempDir = New-Item -ItemType Directory -Path "$env:TEMP\beeper-install-$(Get-Random)" -Force
    
    $baseUrl = "https://github.com/$GITHUB_REPO/releases/download/$Tag"
    $serviceBinary = "$SERVICE_NAME-$Target.exe"
    $configuratorBinary = "$CONFIGURATOR_NAME-$Target.exe"
    
    $serviceUrl = "$baseUrl/$serviceBinary"
    $configuratorUrl = "$baseUrl/$configuratorBinary"
    
    $servicePath = Join-Path $tempDir "$SERVICE_NAME.exe"
    $configuratorPath = Join-Path $tempDir "$CONFIGURATOR_NAME.exe"
    
    try {
        Write-InfoMessage "Downloading service binary..."
        Invoke-WebRequest -Uri $serviceUrl -OutFile $servicePath -UseBasicParsing
        
        Write-InfoMessage "Downloading configurator binary..."
        Invoke-WebRequest -Uri $configuratorUrl -OutFile $configuratorPath -UseBasicParsing
        
        Write-InfoMessage "Binaries downloaded successfully"
        return $tempDir
    }
    catch {
        Write-ErrorMessage "Failed to download binaries: $_"
        Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
        exit 1
    }
}

# Install binaries
function Install-Binaries {
    param([string]$SourceDir)
    
    Write-InfoMessage "Installing binaries to $INSTALL_DIR..."
    
    # Create installation directory
    if (-not (Test-Path $INSTALL_DIR)) {
        New-Item -ItemType Directory -Path $INSTALL_DIR -Force | Out-Null
    }
    
    # Check if scheduled task is running and stop it
    $existingTask = Get-ScheduledTask -TaskName $SCHEDULED_TASK_NAME -ErrorAction SilentlyContinue
    $wasRunning = $false
    
    if ($existingTask) {
        if ($existingTask.State -eq "Running") {
            Write-InfoMessage "Stopping existing scheduled task for update..."
            Stop-ScheduledTask -TaskName $SCHEDULED_TASK_NAME -ErrorAction SilentlyContinue
            Start-Sleep -Seconds 3
            $wasRunning = $true
        }
    }
    
    # Copy binaries
    try {
        Copy-Item -Path (Join-Path $SourceDir "$SERVICE_NAME.exe") -Destination $INSTALL_DIR -Force
        Copy-Item -Path (Join-Path $SourceDir "$CONFIGURATOR_NAME.exe") -Destination $INSTALL_DIR -Force
        
        Write-InfoMessage "Binaries installed successfully"
    }
    catch {
        Write-ErrorMessage "Failed to copy binaries: $_"
        
        # Try to restart scheduled task if it was running
        if ($wasRunning) {
            Write-InfoMessage "Attempting to restart scheduled task..."
            Start-ScheduledTask -TaskName $SCHEDULED_TASK_NAME -ErrorAction SilentlyContinue
        }
        
        throw
    }
    
    # Return service state
    return $wasRunning
}

# Setup Scheduled Task
function Install-ScheduledTask {
    param([bool]$WasRunning = $false)
    
    Write-InfoMessage "Setting up user service (Scheduled Task)..."
    
    $servicePath = Join-Path $INSTALL_DIR "$SERVICE_NAME.exe"
    
    # Initialize directories
    $ProgramDataDir = Join-Path $env:ProgramData "BeeperAutomations"
    if (-not (Test-Path $ProgramDataDir)) {
        New-Item -ItemType Directory -Path $ProgramDataDir -Force | Out-Null
    }
    
    # Check if old Windows service exists and remove it
    $oldService = Get-Service -Name "BeeperAutomations" -ErrorAction SilentlyContinue
    if ($oldService) {
        Write-WarnMessage "Old Windows service found. Removing it..."
        
        # Stop the service if running
        if ($oldService.Status -eq "Running") {
            Write-InfoMessage "Stopping old Windows service..."
            Stop-Service -Name "BeeperAutomations" -Force -ErrorAction SilentlyContinue
            Start-Sleep -Seconds 3
        }
        
        # Delete the service
        Write-InfoMessage "Deleting old Windows service..."
        sc.exe delete "BeeperAutomations" | Out-Null
        Start-Sleep -Seconds 2
        Write-InfoMessage "Old Windows service removed"
    }
    
    # Check if scheduled task already exists
    $existingTask = Get-ScheduledTask -TaskName $SCHEDULED_TASK_NAME -ErrorAction SilentlyContinue
    
    if ($existingTask) {
        Write-InfoMessage "Scheduled task already exists..."
        
        # If it's still running somehow, stop it
        if ($existingTask.State -eq "Running") {
            Stop-ScheduledTask -TaskName $SCHEDULED_TASK_NAME -ErrorAction SilentlyContinue
            Start-Sleep -Seconds 2
        }
        
        # Remove old task configuration
        Write-InfoMessage "Removing old scheduled task..."
        Unregister-ScheduledTask -TaskName $SCHEDULED_TASK_NAME -Confirm:$false -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 1
    }
    
    # Create the scheduled task trigger (at logon)
    $trigger = New-ScheduledTaskTrigger -AtLogOn
    
    # Create the scheduled task action
    $action = New-ScheduledTaskAction `
        -Execute $servicePath `
        -WorkingDirectory $INSTALL_DIR
    
    # Create principal to run as the logged-on user with highest privileges
    $principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive -RunLevel Highest
    
    # Create settings for the task
    $settings = New-ScheduledTaskSettingsSet `
        -AllowStartIfOnBatteries `
        -DontStopIfGoingOnBatteries `
        -StartWhenAvailable `
        -RestartCount 3 `
        -RestartInterval (New-TimeSpan -Minutes 1) `
        -ExecutionTimeLimit (New-TimeSpan -Days 365) `
        -MultipleInstances IgnoreNew
    
    # Register the scheduled task
    Write-InfoMessage "Creating scheduled task..."
    Register-ScheduledTask `
        -TaskName $SCHEDULED_TASK_NAME `
        -Description $SERVICE_DESCRIPTION `
        -Action $action `
        -Trigger $trigger `
        -Principal $principal `
        -Settings $settings `
        -Force | Out-Null
    
    Write-InfoMessage "Scheduled task created successfully"
    
    # Start the task if user is currently logged in
    Write-InfoMessage "Starting scheduled task..."
    try {
        Start-ScheduledTask -TaskName $SCHEDULED_TASK_NAME -ErrorAction Stop
        Write-InfoMessage "Scheduled task started successfully"
    }
    catch {
        Write-WarnMessage "Could not start scheduled task (user may not be logged in). It will start automatically on next logon."
    }
    
    Write-InfoMessage "Use 'Get-ScheduledTask -TaskName $SCHEDULED_TASK_NAME' to check task status"
}

# Add to PATH
function Add-ToPath {
    Write-InfoMessage "Adding installation directory to PATH..."
    
    $currentPath = [Environment]::GetEnvironmentVariable("Path", "Machine")
    
    if ($currentPath -notlike "*$INSTALL_DIR*") {
        # Remove trailing semicolon if exists
        $currentPath = $currentPath.TrimEnd(';')
        
        # Add new path with semicolon
        $newPath = "$currentPath;$INSTALL_DIR"
        
        try {
            [Environment]::SetEnvironmentVariable("Path", $newPath, "Machine")
            Write-InfoMessage "Added to system PATH"
            
            # Update current session's PATH
            $env:Path = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [Environment]::GetEnvironmentVariable("Path", "User")
            Write-InfoMessage "Updated current session PATH"
            
            Write-InfoMessage "You can now run '$CONFIGURATOR_NAME' from any location"
            Write-WarnMessage "Note: Other open terminals may need to be restarted to see the PATH changes"
        }
        catch {
            Write-WarnMessage "Failed to add to PATH: $_"
            Write-InfoMessage "You can manually add '$INSTALL_DIR' to your PATH"
        }
    } else {
        Write-InfoMessage "Already in PATH"
    }
}

# Main installation flow
function Main {
    Write-Host ""
    Write-Host "╔════════════════════════════════════════╗" -ForegroundColor Cyan
    Write-Host "║  Beeper Automations Installer          ║" -ForegroundColor Cyan
    Write-Host "║           Windows Edition              ║" -ForegroundColor Cyan
    Write-Host "╚════════════════════════════════════════╝" -ForegroundColor Cyan
    Write-Host ""
    
    # Check for administrator privileges
    if (-not (Test-Administrator)) {
        Write-ErrorMessage "This installer must be run as Administrator"
        Write-InfoMessage "Please run PowerShell as Administrator and try again"
        exit 1
    }
    
    # Detect architecture
    $arch = Get-Architecture
    $target = "x86_64-pc-windows-msvc"  # Default to MSVC build
    Write-InfoMessage "Detected architecture: $arch (target: $target)"
    
    # Get latest release
    $tag = Get-LatestRelease
    
    # Download binaries
    $tempDir = Get-Binaries -Tag $tag -Target $target
    
    try {
        # Install binaries (returns true if service was running)
        $wasRunning = Install-Binaries -SourceDir $tempDir
        
        # Add to PATH
        Add-ToPath
        
        # Setup scheduled task (pass whether it was running)
        Install-ScheduledTask -WasRunning $wasRunning
        
        Write-Host ""
        Write-InfoMessage "✓ Installation complete!"
        Write-InfoMessage "Service binary: $INSTALL_DIR\$SERVICE_NAME.exe"
        Write-InfoMessage "Configurator: $INSTALL_DIR\$CONFIGURATOR_NAME.exe"
        Write-Host ""
        Write-InfoMessage "The service is configured as a user service (runs in your session)."
        Write-InfoMessage "It will automatically start when you log in to Windows."
        Write-InfoMessage "This enables proper user idle detection."
        Write-Host ""
        Write-InfoMessage "You can manage it using:"
        Write-InfoMessage "  - Start: Start-ScheduledTask -TaskName '$SCHEDULED_TASK_NAME'"
        Write-InfoMessage "  - Stop: Stop-ScheduledTask -TaskName '$SCHEDULED_TASK_NAME'"
        Write-InfoMessage "  - Status: Get-ScheduledTask -TaskName '$SCHEDULED_TASK_NAME'"
        Write-InfoMessage "  - Restart: Stop-ScheduledTask -TaskName '$SCHEDULED_TASK_NAME'; Start-ScheduledTask -TaskName '$SCHEDULED_TASK_NAME'"
        Write-Host ""
        Write-InfoMessage "Run '$CONFIGURATOR_NAME' to configure automations"
        Write-InfoMessage "The service will automatically pick up configuration changes"
        Write-Host ""
    }
    finally {
        # Cleanup
        if (Test-Path $tempDir) {
            Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

# Run main function
Main
