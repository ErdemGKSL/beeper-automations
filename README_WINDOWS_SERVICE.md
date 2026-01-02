# Windows Service Installation

## Building

To build the Windows service binary:

```bash
cargo build --release --features windows-service --bin auto-beeper-windows-service
```

## Installation

1. **Build the service** (as shown above)

2. **Install the service** using `sc` command (run as Administrator):

```cmd
sc create BeeperAutomations binPath= "C:\path\to\auto-beeper-windows-service.exe" start= auto
sc description BeeperAutomations "Beeper Desktop Automation Service"
```

3. **Start the service**:

```cmd
sc start BeeperAutomations
```

## Management

**Check status**:
```cmd
sc query BeeperAutomations
```

**Stop the service**:
```cmd
sc stop BeeperAutomations
```

**Remove the service**:
```cmd
sc delete BeeperAutomations
```

**View logs**: Check Windows Event Viewer under "Windows Logs > Application"

## Configuration

The service uses the same configuration file as the standalone service:
- `%APPDATA%\beeper-automations\config.toml`

Use `auto-beeper-configurator.exe` to configure the service before starting it.

## Notes

- The service runs under the Local System account by default
- For security, consider running under a dedicated service account
- The service supports hot-reload of configuration files
- Ctrl+C signal handling is replaced with Windows Service Control Manager signals
