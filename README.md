# Beeper Automations

A powerful automation framework for the Beeper Desktop messenger app using its local REST API.

## Quick Installation

### Linux / macOS

Run the following command in your terminal:

```bash
curl -fsSL https://raw.githubusercontent.com/ErdemGKSL/beeper-auotmations/main/scripts/install.sh | bash
```

Or download and run manually:

```bash
wget https://raw.githubusercontent.com/ErdemGKSL/beeper-auotmations/main/scripts/install.sh
chmod +x install.sh
./install.sh
```

This will:
- Download the latest binaries for your platform
- Install them to `/usr/local/bin`
- Set up and start the service automatically (systemd for Linux, launchd for macOS)

### Windows

Run the following command in PowerShell or CMD as **Administrator**:

```powershell
powershell -c "irm https://raw.githubusercontent.com/ErdemGKSL/beeper-auotmations/main/scripts/install.ps1 | iex"
```

Alternatively, you can run directly in PowerShell:

```powershell
irm https://raw.githubusercontent.com/ErdemGKSL/beeper-auotmations/main/scripts/install.ps1 | iex
```

Or download and run manually:

```powershell
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/ErdemGKSL/beeper-auotmations/main/scripts/install.ps1" -OutFile install.ps1
.\install.ps1
```

This will:
- Download the latest Windows binaries
- Install them to `C:\Program Files\BeeperAutomations`
- Create and start a Windows service
- Add the installation directory to your PATH

### Manual Installation

Download the latest release binaries from the [releases page](https://github.com/ErdemGKSL/beeper-auotmations/releases) for your platform.

## Overview

Beeper Automations is a Rust-based project that provides automated functionality and management tools for the Beeper Desktop messenger application. It leverages the Beeper Desktop API (local API running at `localhost:23373`) to enable sophisticated automation workflows and user-friendly configuration management.

## Project Structure

This project consists of multiple components working together:

### Core Architecture

- **Service Executable**: The main automation service that runs continuously, monitoring and executing configured automations. Features hot-reloading of configuration from `data_dir/config.toml`, allowing real-time updates without restarting the service.
  
- **Configurator Executable**: A terminal user interface (TUI) application built with [Ratatui](https://github.com/ratatui-org/ratatui) that provides an interactive way to create, edit, and manage your `config.toml` configuration file.

## Modules

The project is organized into modular components, each providing specific automation capabilities:
- **Notification Manager**: Handle and manage Beeper notifications with custom rules and actions (It works, but not properly)

### Planned Modules

- **Auto Response**: Automatically respond to messages based on configurable patterns and conditions
- *(More modules to be added)*

## Technologies

- **Language**: Rust
- **Runtime**: Tokio (async runtime)
- **API Client**: `beeper-desktop-api` crate
- **UI Framework**: Ratatui (for configurator TUI)
- **Configuration**: TOML format

## Getting Started

### Prerequisites

- Rust 1.70+ (or edition 2024)
- Beeper Desktop application running with local API enabled

### Building

```bash
cargo build --release
```

### Running

#### Service
```bash
cargo run --release --bin auto-beeper-service
```

#### Configurator
```bash
cargo run --release --bin auto-beeper-configurator
```

## Configuration

Configuration is stored in `config.toml` at your data directory. The service continuously monitors this file for changes and hot-reloads when updates are detected.

Example structure:
```toml
[notifications]
enabled = true
# notification manager settings

[auto_response]
enabled = true
# auto response settings
```

## API Reference

The project uses the `beeper-desktop-api` crate which provides:

- Chat management (`list_chats`, `get_chat`)
- Message operations (`send_message`, `get_messages`)
- Application control (`focus_app`)
- Search functionality
- Account and profile operations

## License

*(Add your license information)*

## Contributing

*(Add contribution guidelines)*
