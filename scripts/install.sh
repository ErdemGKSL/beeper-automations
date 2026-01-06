#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
GITHUB_REPO="ErdemGKSL/beeper-automations"
SERVICE_NAME="auto-beeper-service"
CONFIGURATOR_NAME="auto-beeper-configurator"
INSTALL_DIR="/usr/local/bin"
SERVICE_USER="${SUDO_USER:-$USER}"

# Function to print colored messages
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running with necessary privileges
check_privileges() {
    if [ "$EUID" -eq 0 ] && [ "$OS" = "linux" ]; then
        print_warn "Running as root. Service will be installed system-wide."
        SERVICE_USER="${SUDO_USER:-root}"
        return 0
    fi
    
    if [ ! -w "$INSTALL_DIR" ]; then
        print_info "Installation requires elevated privileges"
        if ! command -v sudo &> /dev/null; then
            print_error "sudo is not available. Please run as root or install to a user-writable directory"
            exit 1
        fi
    fi
}

# Detect OS and architecture
detect_platform() {
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)
    
    case "$os" in
        linux*)
            OS="linux"
            ;;
        darwin*)
            OS="macos"
            ;;
        *)
            print_error "Unsupported operating system: $os"
            print_error "This installer only supports Linux and macOS"
            print_error "For Windows, please use the PowerShell installer"
            exit 1
            ;;
    esac
    
    case "$arch" in
        x86_64|amd64)
            ARCH="x86_64"
            ;;
        aarch64|arm64)
            ARCH="aarch64"
            ;;
        *)
            print_error "Unsupported architecture: $arch"
            exit 1
            ;;
    esac
    
    # Determine target triple
    case "$OS-$ARCH" in
        linux-x86_64)
            TARGET="x86_64-unknown-linux-gnu"
            ;;
        macos-x86_64)
            TARGET="x86_64-apple-darwin"
            ;;
        macos-aarch64)
            TARGET="aarch64-apple-darwin"
            ;;
        *)
            print_error "Unsupported platform: $OS-$ARCH"
            exit 1
            ;;
    esac
    
    print_info "Detected platform: $OS-$ARCH (target: $TARGET)"
}

# Check if service is currently installed and running
check_existing_installation() {
    SERVICE_EXISTS=false
    SERVICE_WAS_RUNNING=false
    
    if [ "$OS" = "linux" ]; then
        if systemctl list-unit-files | grep -q "auto-beeper.service"; then
            SERVICE_EXISTS=true
            if systemctl is-active --quiet auto-beeper.service; then
                SERVICE_WAS_RUNNING=true
                print_info "Existing service detected and running"
            else
                print_info "Existing service detected but not running"
            fi
        fi
    elif [ "$OS" = "macos" ]; then
        local plist_file="$HOME/Library/LaunchAgents/com.beeper.automations.plist"
        if [ -f "$plist_file" ]; then
            SERVICE_EXISTS=true
            if launchctl list | grep -q com.beeper.automations; then
                SERVICE_WAS_RUNNING=true
                print_info "Existing service detected and running"
            else
                print_info "Existing service detected but not running"
            fi
        fi
    fi
    
    # Check if binaries exist
    if [ -f "$INSTALL_DIR/$SERVICE_NAME" ]; then
        print_info "Existing installation found - this will be an update"
    fi
}

# Get latest release URL
get_latest_release() {
    print_info "Fetching latest release information..."
    
    local api_url="https://api.github.com/repos/${GITHUB_REPO}/releases"
    
    local release_data=$(curl -s "$api_url")
    local curl_exit_code=$?
    
    if [ $curl_exit_code -ne 0 ]; then
        print_error "Failed to fetch release information (curl error: $curl_exit_code)"
        exit 1
    fi
    
    if [ -z "$release_data" ]; then
        print_error "Failed to fetch release information (empty response)"
        exit 1
    fi
    
    # Check if response is an error message
    if echo "$release_data" | grep -q '"message"'; then
        print_error "API returned an error:"
        echo "$release_data" | grep '"message"'
        exit 1
    fi
    
    # Check if jq is available for better JSON parsing
    if command -v jq &> /dev/null; then
        TAG=$(echo "$release_data" | jq -r '.[0].tag_name' 2>/dev/null)
    else
        # Fallback: Use grep with more robust pattern
        TAG=$(echo "$release_data" | grep -m 1 '"tag_name":' | sed -E 's/.*"tag_name":[[:space:]]*"([^"]+)".*/\1/')
    fi
    
    if [ -z "$TAG" ] || [ "$TAG" = "null" ]; then
        print_error "Could not determine latest release tag"
        print_error "Please check if releases exist at: https://github.com/${GITHUB_REPO}/releases"
        exit 1
    fi
    
    print_info "Latest release: $TAG"
}

# Download binaries
download_binaries() {
    print_info "Downloading binaries for $TARGET..."
    
    local tmp_dir=$(mktemp -d)
    cd "$tmp_dir"
    
    local base_url="https://github.com/${GITHUB_REPO}/releases/download/${TAG}"
    local service_binary="${SERVICE_NAME}-${TARGET}"
    local configurator_binary="${CONFIGURATOR_NAME}-${TARGET}"
    
    # Download service binary
    print_info "Downloading service binary..."
    if ! curl -f -L -o "${service_binary}" "${base_url}/${service_binary}"; then
        print_error "Failed to download service binary"
        print_error "URL: ${base_url}/${service_binary}"
        rm -rf "$tmp_dir"
        exit 1
    fi
    
    # Verify it's actually a binary
    if file "${service_binary}" | grep -q "text"; then
        print_error "Downloaded file is not a binary!"
        print_error "The binary for $TARGET may not be available in this release"
        rm -rf "$tmp_dir"
        exit 1
    fi
    
    mv "${service_binary}" "$SERVICE_NAME"
    
    # Download configurator binary
    print_info "Downloading configurator binary..."
    if ! curl -f -L -o "${configurator_binary}" "${base_url}/${configurator_binary}"; then
        print_error "Failed to download configurator binary"
        print_error "URL: ${base_url}/${configurator_binary}"
        rm -rf "$tmp_dir"
        exit 1
    fi
    
    # Verify configurator is also a binary
    if file "${configurator_binary}" | grep -q "text"; then
        print_error "Downloaded configurator is not a binary!"
        rm -rf "$tmp_dir"
        exit 1
    fi
    
    mv "${configurator_binary}" "$CONFIGURATOR_NAME"
    
    chmod +x "$SERVICE_NAME" "$CONFIGURATOR_NAME"
    
    DOWNLOAD_DIR="$tmp_dir"
    print_info "Binaries downloaded successfully"
}

# Stop service if running
stop_service() {
    if [ "$SERVICE_WAS_RUNNING" = true ]; then
        print_info "Stopping service for update..."
        
        if [ "$OS" = "linux" ]; then
            sudo systemctl stop auto-beeper.service || true
            sleep 2
        elif [ "$OS" = "macos" ]; then
            local plist_file="$HOME/Library/LaunchAgents/com.beeper.automations.plist"
            launchctl unload "$plist_file" 2>/dev/null || true
            sleep 2
        fi
        
        print_info "Service stopped"
    fi
}

# Install binaries
install_binaries() {
    print_info "Installing binaries to $INSTALL_DIR..."
    
    # Stop service before replacing binaries
    stop_service
    
    # Check if we need sudo
    if [ ! -w "$INSTALL_DIR" ]; then
        sudo mkdir -p "$INSTALL_DIR"
        
        # Copy with error handling
        if ! sudo cp "$DOWNLOAD_DIR/$SERVICE_NAME" "$INSTALL_DIR/" 2>/dev/null; then
            print_error "Failed to copy service binary"
            # Try to restart service if it was running
            if [ "$SERVICE_WAS_RUNNING" = true ]; then
                start_service
            fi
            exit 1
        fi
        
        if ! sudo cp "$DOWNLOAD_DIR/$CONFIGURATOR_NAME" "$INSTALL_DIR/" 2>/dev/null; then
            print_error "Failed to copy configurator binary"
            if [ "$SERVICE_WAS_RUNNING" = true ]; then
                start_service
            fi
            exit 1
        fi
        
        sudo chmod +x "$INSTALL_DIR/$SERVICE_NAME"
        sudo chmod +x "$INSTALL_DIR/$CONFIGURATOR_NAME"
    else
        mkdir -p "$INSTALL_DIR"
        
        if ! cp "$DOWNLOAD_DIR/$SERVICE_NAME" "$INSTALL_DIR/" 2>/dev/null; then
            print_error "Failed to copy service binary"
            if [ "$SERVICE_WAS_RUNNING" = true ]; then
                start_service
            fi
            exit 1
        fi
        
        if ! cp "$DOWNLOAD_DIR/$CONFIGURATOR_NAME" "$INSTALL_DIR/" 2>/dev/null; then
            print_error "Failed to copy configurator binary"
            if [ "$SERVICE_WAS_RUNNING" = true ]; then
                start_service
            fi
            exit 1
        fi
        
        chmod +x "$INSTALL_DIR/$SERVICE_NAME"
        chmod +x "$INSTALL_DIR/$CONFIGURATOR_NAME"
    fi
    
    print_info "Binaries installed successfully"
}

# Start service
start_service() {
    if [ "$OS" = "linux" ]; then
        print_info "Starting service..."
        sudo systemctl start auto-beeper.service
    elif [ "$OS" = "macos" ]; then
        print_info "Starting service..."
        local plist_file="$HOME/Library/LaunchAgents/com.beeper.automations.plist"
        launchctl load "$plist_file" 2>/dev/null || true
    fi
}

# Setup systemd service for Linux
setup_systemd_service() {
    print_info "Setting up systemd service..."
    
    local service_file="/etc/systemd/system/auto-beeper.service"
    
    # Create or update service file
    sudo tee "$service_file" > /dev/null <<EOF
[Unit]
Description=Beeper Automations Service
After=network.target

[Service]
Type=simple
User=$SERVICE_USER
ExecStart=$INSTALL_DIR/$SERVICE_NAME
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF
    
    # Reload systemd
    print_info "Reloading systemd configuration..."
    sudo systemctl daemon-reload
    
    # Enable service if not already enabled
    if ! systemctl is-enabled --quiet auto-beeper.service 2>/dev/null; then
        print_info "Enabling service..."
        sudo systemctl enable auto-beeper.service
    fi
    
    # Start the service
    start_service
    
    # Verify service started
    sleep 2
    if systemctl is-active --quiet auto-beeper.service; then
        print_info "Service started successfully"
    else
        print_warn "Service may have failed to start. Check status with: systemctl status auto-beeper"
    fi
    
    print_info "Systemd service configured"
}

# Setup launchd service for macOS
setup_launchd_service() {
    print_info "Setting up launchd service..."
    
    local plist_file="$HOME/Library/LaunchAgents/com.beeper.automations.plist"
    
    mkdir -p "$HOME/Library/LaunchAgents"
    mkdir -p "$HOME/Library/Logs"
    
    # Create or update plist file
    cat > "$plist_file" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.beeper.automations</string>
    <key>ProgramArguments</key>
    <array>
        <string>$INSTALL_DIR/$SERVICE_NAME</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$HOME/Library/Logs/beeper-automations.log</string>
    <key>StandardErrorPath</key>
    <string>$HOME/Library/Logs/beeper-automations.error.log</string>
</dict>
</plist>
EOF
    
    # Load the service
    start_service
    
    # Verify service started
    sleep 2
    if launchctl list | grep -q com.beeper.automations; then
        print_info "Service started successfully"
    else
        print_warn "Service may have failed to start. Check logs at: $HOME/Library/Logs/beeper-automations.error.log"
    fi
    
    print_info "Launchd service configured"
}

# Add installation directory to PATH
add_to_path() {
    # Skip if installing to a directory already in standard PATH
    case "$INSTALL_DIR" in
        /usr/local/bin|/usr/bin|/bin)
            print_info "Installation directory is already in system PATH"
            return 0
            ;;
    esac
    
    print_info "Adding $INSTALL_DIR to PATH..."
    
    local path_added=false
    
    # Add to bash/zsh via .bashrc or .zshrc
    for shell_rc in "$HOME/.bashrc" "$HOME/.zshrc"; do
        if [ -f "$shell_rc" ]; then
            # Check if path is already in the rc file
            if ! grep -q "export PATH=\"$INSTALL_DIR:\$PATH\"" "$shell_rc" && \
               ! grep -q "export PATH='$INSTALL_DIR:\$PATH'" "$shell_rc" && \
               ! grep -q "PATH=\"$INSTALL_DIR:\$PATH\"" "$shell_rc" && \
               ! grep -q "PATH='$INSTALL_DIR:\$PATH'" "$shell_rc"; then
                
                echo "" >> "$shell_rc"
                echo "# Added by Beeper Automations installer" >> "$shell_rc"
                echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$shell_rc"
                print_info "Added to $(basename $shell_rc)"
                path_added=true
            else
                print_info "Already present in $(basename $shell_rc)"
            fi
        fi
    done
    
    # Add to fish if available
    if command -v fish &> /dev/null; then
        print_info "Fish shell detected, adding to fish path..."
        
        # Check if already in fish path
        if fish -c "contains $INSTALL_DIR \$fish_user_paths" 2>/dev/null; then
            print_info "Already present in fish path"
        else
            if fish -c "fish_add_path -U $INSTALL_DIR" 2>/dev/null; then
                print_info "Added to fish path"
                path_added=true
            else
                print_warn "Failed to add to fish path automatically"
                print_info "You can manually add it by running: fish_add_path -U $INSTALL_DIR"
            fi
        fi
    fi
    
    # Update current session PATH
    export PATH="$INSTALL_DIR:$PATH"
    
    if [ "$path_added" = true ]; then
        print_info "PATH updated for future shell sessions"
        print_warn "Note: You may need to restart your terminal or run 'source ~/.bashrc' (or ~/.zshrc) to use '$CONFIGURATOR_NAME' in this session"
    fi
}

# Print service management instructions
print_instructions() {
    echo ""
    echo -e "${CYAN}╔════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║          Service Management            ║${NC}"
    echo -e "${CYAN}╚════════════════════════════════════════╝${NC}"
    echo ""
    
    if [ "$OS" = "linux" ]; then
        print_info "Manage the service using systemctl:"
        echo "  • Check status:  systemctl status auto-beeper"
        echo "  • Start service: sudo systemctl start auto-beeper"
        echo "  • Stop service:  sudo systemctl stop auto-beeper"
        echo "  • Restart:       sudo systemctl restart auto-beeper"
        echo "  • View logs:     journalctl -u auto-beeper -f"
    elif [ "$OS" = "macos" ]; then
        print_info "Manage the service using launchctl:"
        echo "  • Check status:  launchctl list | grep beeper"
        echo "  • Stop service:  launchctl unload ~/Library/LaunchAgents/com.beeper.automations.plist"
        echo "  • Start service: launchctl load ~/Library/LaunchAgents/com.beeper.automations.plist"
        echo "  • View logs:     tail -f ~/Library/Logs/beeper-automations.log"
    fi
    
    echo ""
    print_info "Configuration:"
    echo "  • Run '$CONFIGURATOR_NAME' to configure automations"
    echo "  • The service will automatically pick up configuration changes"
    echo ""
}

# Main installation flow
main() {
    echo ""
    echo -e "${CYAN}╔════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║  Beeper Automations Installer          ║${NC}"
    echo -e "${CYAN}║      Linux & macOS Edition             ║${NC}"
    echo -e "${CYAN}╚════════════════════════════════════════╝${NC}"
    echo ""
    
    detect_platform
    check_privileges
    check_existing_installation
    get_latest_release
    download_binaries
    install_binaries
    
    # Add to PATH
    add_to_path
    
    # Setup service based on OS
    case "$OS" in
        linux)
            setup_systemd_service
            ;;
        macos)
            setup_launchd_service
            ;;
    esac
    
    # Cleanup
    rm -rf "$DOWNLOAD_DIR"
    
    echo ""
    if [ "$SERVICE_EXISTS" = true ]; then
        print_info "✓ Update complete!"
    else
        print_info "✓ Installation complete!"
    fi
    print_info "Service binary: $INSTALL_DIR/$SERVICE_NAME"
    print_info "Configurator:   $INSTALL_DIR/$CONFIGURATOR_NAME"
    
    print_instructions
}

# Run main function
main