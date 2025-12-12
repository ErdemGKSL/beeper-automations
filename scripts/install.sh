#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
GITHUB_REPO="ErdemGKSL/beeper-auotmations"
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
            print_error "For Windows, please download binaries manually from GitHub releases"
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
            SUFFIX=""
            ;;
        macos-x86_64)
            TARGET="x86_64-apple-darwin"
            SUFFIX=""
            ;;
        macos-aarch64)
            TARGET="aarch64-apple-darwin"
            SUFFIX=""
            ;;
        *)
            print_error "Unsupported platform: $OS-$ARCH"
            exit 1
            ;;
    esac
    
    print_info "Detected platform: $OS-$ARCH (target: $TARGET)"
}

# Get latest release URL
get_latest_release() {
    print_info "Fetching latest release information..."
    
    # Fetch all releases (including pre-releases) and get the most recent one
    local api_url="https://api.github.com/repos/${GITHUB_REPO}/releases"
    print_info "API URL: $api_url"
    
    local release_data=$(curl -s "$api_url")
    local curl_exit_code=$?
    
    print_info "Curl exit code: $curl_exit_code"
    
    if [ $curl_exit_code -ne 0 ]; then
        print_error "Failed to fetch release information (curl error: $curl_exit_code)"
        exit 1
    fi
    
    if [ -z "$release_data" ]; then
        print_error "Failed to fetch release information (empty response)"
        exit 1
    fi
    
    print_info "Response length: ${#release_data} characters"
    print_info "First 500 characters of response:"
    echo "$release_data" | head -c 500
    echo ""
    echo ""
    
    # Check if response is an error message
    if echo "$release_data" | grep -q '"message"'; then
        print_error "API returned an error:"
        echo "$release_data" | grep '"message"'
        exit 1
    fi
    
    # Check if jq is available for better JSON parsing
    if command -v jq &> /dev/null; then
        print_info "Using jq for JSON parsing"
        TAG=$(echo "$release_data" | jq -r '.[0].tag_name' 2>/dev/null)
        local jq_exit_code=$?
        print_info "jq exit code: $jq_exit_code"
    else
        print_info "jq not found, using grep/sed fallback"
        # Fallback: Use grep with more robust pattern
        TAG=$(echo "$release_data" | grep -m 1 '"tag_name":' | sed -E 's/.*"tag_name":[[:space:]]*"([^"]+)".*/\1/')
    fi
    
    print_info "Extracted TAG: '$TAG'"
    
    if [ -z "$TAG" ] || [ "$TAG" = "null" ]; then
        print_error "Could not determine latest release tag"
        print_error "Please check if releases exist at: https://github.com/${GITHUB_REPO}/releases"
        print_error ""
        print_error "Full API response:"
        echo "$release_data"
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
    local service_binary="${SERVICE_NAME}-${TARGET}${SUFFIX}"
    local configurator_binary="${CONFIGURATOR_NAME}-${TARGET}${SUFFIX}"
    
    # Download service binary
    print_info "Downloading $service_binary..."
    print_info "URL: ${base_url}/${service_binary}"
    
    if ! curl -f -L -o "$SERVICE_NAME$SUFFIX" "${base_url}/${service_binary}"; then
        print_error "Failed to download service binary"
        print_error "URL: ${base_url}/${service_binary}"
        print_error "This usually means the binary for your platform doesn't exist in the release"
        rm -rf "$tmp_dir"
        exit 1
    fi
    
    # Verify it's actually a binary and not an error page
    if file "$SERVICE_NAME$SUFFIX" | grep -q "text"; then
        print_error "Downloaded file is not a binary!"
        print_error "Content:"
        head -n 5 "$SERVICE_NAME$SUFFIX"
        print_error ""
        print_error "The binary for $TARGET may not be available in this release."
        rm -rf "$tmp_dir"
        exit 1
    fi
    
    # Download configurator binary
    print_info "Downloading $configurator_binary..."
    print_info "URL: ${base_url}/${configurator_binary}"
    
    if ! curl -f -L -o "$CONFIGURATOR_NAME$SUFFIX" "${base_url}/${configurator_binary}"; then
        print_error "Failed to download configurator binary"
        print_error "URL: ${base_url}/${configurator_binary}"
        rm -rf "$tmp_dir"
        exit 1
    fi
    
    # Verify configurator is also a binary
    if file "$CONFIGURATOR_NAME$SUFFIX" | grep -q "text"; then
        print_error "Downloaded configurator is not a binary!"
        print_error "Content:"
        head -n 5 "$CONFIGURATOR_NAME$SUFFIX"
        rm -rf "$tmp_dir"
        exit 1
    fi
    
    chmod +x "$SERVICE_NAME$SUFFIX" "$CONFIGURATOR_NAME$SUFFIX"
    
    DOWNLOAD_DIR="$tmp_dir"
    print_info "Binaries downloaded successfully"
}

# Install binaries
install_binaries() {
    print_info "Installing binaries to $INSTALL_DIR..."
    
    # Check if we need sudo
    if [ ! -w "$INSTALL_DIR" ]; then
        print_info "Installing requires elevated privileges..."
        sudo mkdir -p "$INSTALL_DIR"
        sudo cp "$DOWNLOAD_DIR/$SERVICE_NAME$SUFFIX" "$INSTALL_DIR/"
        sudo cp "$DOWNLOAD_DIR/$CONFIGURATOR_NAME$SUFFIX" "$INSTALL_DIR/"
        sudo chmod +x "$INSTALL_DIR/$SERVICE_NAME$SUFFIX"
        sudo chmod +x "$INSTALL_DIR/$CONFIGURATOR_NAME$SUFFIX"
    else
        mkdir -p "$INSTALL_DIR"
        cp "$DOWNLOAD_DIR/$SERVICE_NAME$SUFFIX" "$INSTALL_DIR/"
        cp "$DOWNLOAD_DIR/$CONFIGURATOR_NAME$SUFFIX" "$INSTALL_DIR/"
        chmod +x "$INSTALL_DIR/$SERVICE_NAME$SUFFIX"
        chmod +x "$INSTALL_DIR/$CONFIGURATOR_NAME$SUFFIX"
    fi
    
    print_info "Binaries installed successfully"
}

# Setup systemd service for Linux
setup_systemd_service() {
    print_info "Setting up systemd service..."
    
    local service_file="/etc/systemd/system/auto-beeper.service"
    
    # Stop existing service if running
    if systemctl is-active --quiet auto-beeper.service; then
        print_info "Stopping existing service..."
        sudo systemctl stop auto-beeper.service
    fi
    
    # Create service file
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
    
    # Reload systemd and enable service
    print_info "Enabling and starting service..."
    sudo systemctl daemon-reload
    sudo systemctl enable auto-beeper.service
    sudo systemctl start auto-beeper.service
    
    print_info "Systemd service configured and started"
    print_info "Use 'systemctl status auto-beeper' to check service status"
    print_info "Use 'journalctl -u auto-beeper -f' to view logs"
}

# Setup launchd service for macOS
setup_launchd_service() {
    print_info "Setting up launchd service..."
    
    local plist_file="$HOME/Library/LaunchAgents/com.beeper.automations.plist"
    
    # Stop existing service if running
    if launchctl list | grep -q com.beeper.automations; then
        print_info "Stopping existing service..."
        launchctl unload "$plist_file" 2>/dev/null || true
    fi
    
    mkdir -p "$HOME/Library/LaunchAgents"
    
    # Create plist file
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
    launchctl load "$plist_file"
    
    print_info "Launchd service configured and started"
    print_info "Use 'launchctl list | grep beeper' to check service status"
    print_info "Logs are at $HOME/Library/Logs/beeper-automations.log"
}

# Main installation flow
main() {
    echo "╔════════════════════════════════════════╗"
    echo "║  Beeper Automations Installer         ║"
    echo "╚════════════════════════════════════════╝"
    echo ""
    
    detect_platform
    get_latest_release
    download_binaries
    install_binaries
    
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
    print_info "✓ Installation complete!"
    print_info "Service binary: $INSTALL_DIR/$SERVICE_NAME$SUFFIX"
    print_info "Configurator: $INSTALL_DIR/$CONFIGURATOR_NAME$SUFFIX"
    echo ""
}

# Run main function
main
