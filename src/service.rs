use std::path::PathBuf;
use std::process::Command;

/// Get the current executable path
fn get_executable_path() -> anyhow::Result<PathBuf> {
    std::env::current_exe().map_err(|e| anyhow::anyhow!("Failed to get executable path: {}", e))
}

/// Check if the service is installed
pub fn is_service_installed() -> bool {
    #[cfg(target_os = "macos")]
    {
        get_launchd_plist_path()
            .map(|path| path.exists())
            .unwrap_or(false)
    }

    #[cfg(target_os = "linux")]
    {
        get_systemd_unit_path()
            .map(|path| path.exists())
            .unwrap_or(false)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        false
    }
}

/// Check if the service is currently running
pub fn is_service_running() -> anyhow::Result<bool> {
    #[cfg(target_os = "macos")]
    {
        is_launchd_service_running()
    }

    #[cfg(target_os = "linux")]
    {
        is_systemd_service_running()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err(anyhow::anyhow!(
            "Service status checking is only supported on macOS and Linux"
        ))
    }
}

/// Get the service file path
pub fn get_service_path() -> anyhow::Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        get_launchd_plist_path()
    }

    #[cfg(target_os = "linux")]
    {
        get_systemd_unit_path()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err(anyhow::anyhow!(
            "Service paths are only available on macOS and Linux"
        ))
    }
}

/// Install the monitor as a system service
pub fn install_service() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        install_launchd_service()
    }

    #[cfg(target_os = "linux")]
    {
        install_systemd_service()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err(anyhow::anyhow!(
            "Service installation is only supported on macOS and Linux"
        ))
    }
}

/// Uninstall the system service
pub fn uninstall_service() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        uninstall_launchd_service()
    }

    #[cfg(target_os = "linux")]
    {
        uninstall_systemd_service()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err(anyhow::anyhow!(
            "Service uninstallation is only supported on macOS and Linux"
        ))
    }
}

/// Start the service
pub fn start_service() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        start_launchd_service()
    }

    #[cfg(target_os = "linux")]
    {
        start_systemd_service()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err(anyhow::anyhow!(
            "Service control is only supported on macOS and Linux"
        ))
    }
}

/// Stop the service
pub fn stop_service() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        stop_launchd_service()
    }

    #[cfg(target_os = "linux")]
    {
        stop_systemd_service()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err(anyhow::anyhow!(
            "Service control is only supported on macOS and Linux"
        ))
    }
}

#[cfg(target_os = "macos")]
fn get_launchd_plist_path() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))?;
    Ok(home.join("Library/LaunchAgents/com.claude-notify.plist"))
}

#[cfg(target_os = "macos")]
fn install_launchd_service() -> anyhow::Result<()> {
    let exe_path = get_executable_path()?;
    let plist_path = get_launchd_plist_path()?;

    // Create LaunchAgents directory if it doesn't exist
    if let Some(parent) = plist_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Create plist content
    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.claude-notify</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>run</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/claude-notify.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/claude-notify.error.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
    </dict>
</dict>
</plist>
"#,
        exe_path.display()
    );

    // Write plist file
    std::fs::write(&plist_path, plist_content)?;

    println!("✓ Service installed at {}", plist_path.display());
    println!("  Use 'claude-notify start' to start the service");

    Ok(())
}

#[cfg(target_os = "macos")]
fn uninstall_launchd_service() -> anyhow::Result<()> {
    let plist_path = get_launchd_plist_path()?;

    // Stop the service first if it's running
    let _ = stop_launchd_service();

    // Remove plist file
    if plist_path.exists() {
        std::fs::remove_file(&plist_path)?;
        println!("✓ Service uninstalled from {}", plist_path.display());
    } else {
        println!("Service is not installed");
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn start_launchd_service() -> anyhow::Result<()> {
    let plist_path = get_launchd_plist_path()?;

    if !plist_path.exists() {
        return Err(anyhow::anyhow!(
            "Service is not installed. Run 'claude-notify setup-service' first."
        ));
    }

    let output = Command::new("launchctl")
        .args(&["load", &plist_path.to_string_lossy()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Failed to start service: {}", stderr));
    }

    println!("✓ Service started");
    println!("  Logs: /tmp/claude-notify.log");
    println!("  Errors: /tmp/claude-notify.error.log");

    Ok(())
}

#[cfg(target_os = "macos")]
fn stop_launchd_service() -> anyhow::Result<()> {
    let plist_path = get_launchd_plist_path()?;

    if !plist_path.exists() {
        return Err(anyhow::anyhow!("Service is not installed"));
    }

    let output = Command::new("launchctl")
        .args(&["unload", &plist_path.to_string_lossy()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Don't error if the service wasn't running
        if !stderr.contains("Could not find specified service") {
            return Err(anyhow::anyhow!("Failed to stop service: {}", stderr));
        }
    }

    println!("✓ Service stopped");

    Ok(())
}

#[cfg(target_os = "macos")]
fn is_launchd_service_running() -> anyhow::Result<bool> {
    let output = Command::new("launchctl")
        .args(&["list"])
        .output()?;

    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.contains("com.claude-notify"))
}

#[cfg(target_os = "linux")]
fn get_systemd_unit_path() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))?;
    Ok(home.join(".config/systemd/user/claude-notify.service"))
}

#[cfg(target_os = "linux")]
fn install_systemd_service() -> anyhow::Result<()> {
    let exe_path = get_executable_path()?;
    let unit_path = get_systemd_unit_path()?;

    // Create systemd user directory if it doesn't exist
    if let Some(parent) = unit_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Create unit file content
    let unit_content = format!(
        r#"[Unit]
Description=Claude Usage Monitor
After=network.target

[Service]
Type=simple
ExecStart={} run
Restart=always
RestartSec=10

[Install]
WantedBy=default.target
"#,
        exe_path.display()
    );

    // Write unit file
    std::fs::write(&unit_path, unit_content)?;

    // Reload systemd daemon
    Command::new("systemctl")
        .args(&["--user", "daemon-reload"])
        .output()?;

    println!("✓ Service installed at {}", unit_path.display());
    println!("  Use 'claude-notify start' to start the service");
    println!("  Use 'systemctl --user enable claude-notify' to enable at boot");

    Ok(())
}

#[cfg(target_os = "linux")]
fn uninstall_systemd_service() -> anyhow::Result<()> {
    let unit_path = get_systemd_unit_path()?;

    // Stop and disable the service first
    let _ = stop_systemd_service();
    let _ = Command::new("systemctl")
        .args(&["--user", "disable", "claude-notify"])
        .output();

    // Remove unit file
    if unit_path.exists() {
        std::fs::remove_file(&unit_path)?;
        println!("✓ Service uninstalled from {}", unit_path.display());
    } else {
        println!("Service is not installed");
    }

    // Reload systemd daemon
    Command::new("systemctl")
        .args(&["--user", "daemon-reload"])
        .output()?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn start_systemd_service() -> anyhow::Result<()> {
    let unit_path = get_systemd_unit_path()?;

    if !unit_path.exists() {
        return Err(anyhow::anyhow!(
            "Service is not installed. Run 'claude-notify setup-service' first."
        ));
    }

    let output = Command::new("systemctl")
        .args(&["--user", "start", "claude-notify"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Failed to start service: {}", stderr));
    }

    println!("✓ Service started");
    println!("  Use 'systemctl --user status claude-notify' to check status");
    println!("  Use 'journalctl --user -u claude-notify -f' to view logs");

    Ok(())
}

#[cfg(target_os = "linux")]
fn stop_systemd_service() -> anyhow::Result<()> {
    let unit_path = get_systemd_unit_path()?;

    if !unit_path.exists() {
        return Err(anyhow::anyhow!("Service is not installed"));
    }

    let output = Command::new("systemctl")
        .args(&["--user", "stop", "claude-notify"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Failed to stop service: {}", stderr));
    }

    println!("✓ Service stopped");

    Ok(())
}

#[cfg(target_os = "linux")]
fn is_systemd_service_running() -> anyhow::Result<bool> {
    let output = Command::new("systemctl")
        .args(&["--user", "is-active", "claude-notify"])
        .output()?;

    // is-active returns 0 (success) if the service is active, non-zero otherwise
    Ok(output.status.success())
}
