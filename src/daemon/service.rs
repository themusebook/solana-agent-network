//! Service install (adapted from Reason Network's setup): launchd (macOS) / systemd --user (Linux).
//! Units never embed secrets; only SAN_ENDPOINT / DEVNET_RPC_URL style public config.
use anyhow::Result;
use std::path::{Path, PathBuf};

pub const LABEL: &str = "trade.musebook.san-daemon";

pub fn systemd_unit(bin: &Path, bind: &str) -> String {
    format!("[Unit]\nDescription=Solana Agent Network daemon (devnet)\nAfter=network-online.target\n\n[Service]\nExecStart={} daemon --bind {}\nRestart=on-failure\nRestartSec=5\n\n[Install]\nWantedBy=default.target\n", bin.display(), bind)
}

pub fn launchd_plist(bin: &Path, bind: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>{LABEL}</string>
  <key>ProgramArguments</key><array><string>{}</string><string>daemon</string><string>--bind</string><string>{}</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
</dict></plist>
"#, bin.display(), bind)
}

pub fn unit_path() -> PathBuf {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    if cfg!(target_os = "macos") { home.join("Library/LaunchAgents").join(format!("{LABEL}.plist")) }
    else { home.join(".config/systemd/user/san-daemon.service") }
}

/// Write the unit file. Returns (path, the command the user should run to start it).
pub fn install(bind: &str) -> Result<(PathBuf, String)> {
    let bin = std::env::current_exe()?;
    let p = unit_path();
    std::fs::create_dir_all(p.parent().unwrap())?;
    let (body, cmd) = if cfg!(target_os = "macos") {
        (launchd_plist(&bin, bind), format!("launchctl load -w {}", p.display()))
    } else {
        (systemd_unit(&bin, bind), "systemctl --user daemon-reload && systemctl --user enable --now san-daemon".into())
    };
    std::fs::write(&p, body)?;
    Ok((p, cmd))
}

pub fn uninstall() -> Result<PathBuf> {
    let p = unit_path();
    if p.exists() { std::fs::remove_file(&p)?; }
    Ok(p)
}
