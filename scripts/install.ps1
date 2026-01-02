# Beeper Automations Windows Installer
# Usage: powershell -c "irm https://github.com/ErdemGKSL/beeper-auotmations/releases/latest/download/install.ps1 | iex"

$ErrorActionPreference = "Stop"

# Configuration
$GITHUB_REPO = "ErdemGKSL/beeper-auotmations"
$SERVICE_NAME = "auto-beeper-service"
$WINDOWS_SERVICE_NAME = "auto-beeper-windows-service"
$CONFIGURATOR_NAME = "auto-beeper-configurator"
$INSTALL_DIR = "$env:ProgramFiles\BeeperAutomations"
$SERVICE_DISPLAY_NAME = "Beeper Automations Service"
$SERVICE_DESCRIPTION = "Background service for Beeper automations"

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
    $windowsServiceBinary = "$WINDOWS_SERVICE_NAME-$Target.exe"
    $configuratorBinary = "$CONFIGURATOR_NAME-$Target.exe"
    
    $serviceUrl = "$baseUrl/$serviceBinary"
    $windowsServiceUrl = "$baseUrl/$windowsServiceBinary"
    $configuratorUrl = "$baseUrl/$configuratorBinary"
    
    $servicePath = Join-Path $tempDir "$SERVICE_NAME.exe"
    $windowsServicePath = Join-Path $tempDir "$WINDOWS_SERVICE_NAME.exe"
    $configuratorPath = Join-Path $tempDir "$CONFIGURATOR_NAME.exe"
    
    try {
        Write-InfoMessage "Downloading service binary..."
        Invoke-WebRequest -Uri $serviceUrl -OutFile $servicePath -UseBasicParsing
        
        Write-InfoMessage "Downloading Windows service binary..."
        Invoke-WebRequest -Uri $windowsServiceUrl -OutFile $windowsServicePath -UseBasicParsing
        
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
    
    # Check if service is running and stop it
    $existingService = Get-Service -Name "BeeperAutomations" -ErrorAction SilentlyContinue
    $wasRunning = $false
    
    if ($existingService) {
        if ($existingService.Status -eq "Running") {
            Write-InfoMessage "Stopping existing service for update..."
            Stop-Service -Name "BeeperAutomations" -Force
            Start-Sleep -Seconds 3
            $wasRunning = $true
        }
    }
    
    # Copy binaries
    try {
        Copy-Item -Path (Join-Path $SourceDir "$SERVICE_NAME.exe") -Destination $INSTALL_DIR -Force
        Copy-Item -Path (Join-Path $SourceDir "$WINDOWS_SERVICE_NAME.exe") -Destination $INSTALL_DIR -Force
        Copy-Item -Path (Join-Path $SourceDir "$CONFIGURATOR_NAME.exe") -Destination $INSTALL_DIR -Force
        
        Write-InfoMessage "Binaries installed successfully"
    }
    catch {
        Write-ErrorMessage "Failed to copy binaries: $_"
        
        # Try to restart service if it was running
        if ($wasRunning) {
            Write-InfoMessage "Attempting to restart service..."
            Start-Service -Name "BeeperAutomations" -ErrorAction SilentlyContinue
        }
        
        throw
    }
    
    # Return service state
    return $wasRunning
}

# Setup Windows service
function Install-WindowsService {
    param([bool]$WasRunning = $false)
    
    Write-InfoMessage "Setting up Windows service..."
    
    $servicePath = Join-Path $INSTALL_DIR "$WINDOWS_SERVICE_NAME.exe"
    
    # Check if service already exists
    $existingService = Get-Service -Name "BeeperAutomations" -ErrorAction SilentlyContinue
    
    if ($existingService) {
        Write-InfoMessage "Service already exists..."
        
        # If it's still running somehow, stop it
        if ($existingService.Status -eq "Running") {
            Stop-Service -Name "BeeperAutomations" -Force
            Start-Sleep -Seconds 2
        }
        
        # Only recreate if not updating an existing installation
        if (-not $WasRunning) {
            Write-InfoMessage "Removing old service configuration..."
            sc.exe delete "BeeperAutomations" | Out-Null
            Start-Sleep -Seconds 2
            
            # Create new service
            Write-InfoMessage "Creating service..."
            $createResult = sc.exe create "BeeperAutomations" `
                binPath= "`"$servicePath`"" `
                DisplayName= "$SERVICE_DISPLAY_NAME" `
                start= auto `
                obj= "LocalSystem"
            
            if ($LASTEXITCODE -ne 0) {
                Write-ErrorMessage "Failed to create service: $createResult"
                exit 1
            }
            
            # Set service description
            sc.exe description "BeeperAutomations" "$SERVICE_DESCRIPTION" | Out-Null
            
            # Configure service recovery options (restart on failure)
            sc.exe failure "BeeperAutomations" reset= 86400 actions= restart/60000/restart/60000/restart/60000 | Out-Null
        }
        
        # Start the service (whether it's new or updated)
        Write-InfoMessage "Starting service..."
        Start-Service -Name "BeeperAutomations"
        Write-InfoMessage "Service started successfully"
    }
    else {
        # Service doesn't exist, create it
        Write-InfoMessage "Creating service..."
        $createResult = sc.exe create "BeeperAutomations" `
            binPath= "`"$servicePath`"" `
            DisplayName= "$SERVICE_DISPLAY_NAME" `
            start= auto `
            obj= "LocalSystem"
        
        if ($LASTEXITCODE -ne 0) {
            Write-ErrorMessage "Failed to create service: $createResult"
            exit 1
        }
        
        # Set service description
        sc.exe description "BeeperAutomations" "$SERVICE_DESCRIPTION" | Out-Null
        
        # Configure service recovery options (restart on failure)
        sc.exe failure "BeeperAutomations" reset= 86400 actions= restart/60000/restart/60000/restart/60000 | Out-Null
        
        # Start the service
        Write-InfoMessage "Starting service..."
        Start-Service -Name "BeeperAutomations"
        
        Write-InfoMessage "Service installed and started successfully"
    }
    
    Write-InfoMessage "Use 'sc query BeeperAutomations' to check service status"
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
        
        # Setup service (pass whether it was running)
        Install-WindowsService -WasRunning $wasRunning
        
        Write-Host ""
        Write-InfoMessage "✓ Installation complete!"
        Write-InfoMessage "Service binary (console): $INSTALL_DIR\$SERVICE_NAME.exe"
        Write-InfoMessage "Service binary (Windows): $INSTALL_DIR\$WINDOWS_SERVICE_NAME.exe"
        Write-InfoMessage "Configurator: $INSTALL_DIR\$CONFIGURATOR_NAME.exe"
        Write-Host ""
        Write-InfoMessage "The service is now running in the background as a native Windows service."
        Write-InfoMessage "You can manage it using:"
        Write-InfoMessage "  - Start: Start-Service BeeperAutomations"
        Write-InfoMessage "  - Stop: Stop-Service BeeperAutomations"
        Write-InfoMessage "  - Status: Get-Service BeeperAutomations"
        Write-InfoMessage "  - Restart: Restart-Service BeeperAutomations"
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
