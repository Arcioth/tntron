use serde::Serialize;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

pub const DAEBOARD_SOCK_PATH: &str = "/run/daeboard/daeboard.sock";

#[derive(Debug, Clone, Serialize)]
pub struct DaetronStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub rules_dir: String,
    pub rules: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum HardwareCmd {
    DaeboardColor(String),
    DaeboardBrightness(u8),
}

/// Sends a single command to daeboard (`/run/daeboard/daeboard.sock`, `SOCK_SEQPACKET`).
pub fn daeboard_send_sync(cmd: &str) -> Result<String, String> {
    let clean = cmd.trim();
    if clean.is_empty() || clean.len() > 60 {
        return Err("invalid daeboard command length".into());
    }
    let sock_c = CString::new(DAEBOARD_SOCK_PATH).map_err(|_| "invalid sock path".to_string())?;
    let bytes = sock_c.as_bytes_with_nul();

    unsafe {
        let fd = libc::socket(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
            0,
        );
        if fd < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }

        let tv = libc::timeval {
            tv_sec: 0,
            tv_usec: 120_000,
        };
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_SNDTIMEO,
            (&raw const tv).cast(),
            std::mem::size_of::<libc::timeval>() as libc::socklen_t,
        );
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            (&raw const tv).cast(),
            std::mem::size_of::<libc::timeval>() as libc::socklen_t,
        );

        let mut addr: libc::sockaddr_un = std::mem::zeroed();
        addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
        if bytes.len() > addr.sun_path.len() {
            libc::close(fd);
            return Err("socket path too long".into());
        }
        std::ptr::copy_nonoverlapping(
            bytes.as_ptr().cast(),
            addr.sun_path.as_mut_ptr(),
            bytes.len(),
        );

        let rc = libc::connect(
            fd,
            (&raw const addr).cast::<libc::sockaddr>(),
            std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
        );
        if rc != 0 {
            let err = std::io::Error::last_os_error().to_string();
            libc::close(fd);
            return Err(err);
        }

        let sent = libc::send(fd, clean.as_ptr().cast(), clean.len(), 0);
        if sent < 0 {
            let err = std::io::Error::last_os_error().to_string();
            libc::close(fd);
            return Err(err);
        }

        let mut buf = [0u8; 64];
        let recvd = libc::recv(fd, buf.as_mut_ptr().cast(), buf.len(), 0);
        libc::close(fd);
        if recvd <= 0 {
            return Ok(String::new());
        }
        let reply = String::from_utf8_lossy(&buf[..recvd as usize])
            .trim()
            .to_string();
        Ok(reply)
    }
}

pub fn daetron_default_rules_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join("Documents/daetron/rules.d");
        if p.is_dir() {
            return p;
        }
    }
    PathBuf::from("/etc/daetron/rules.d")
}

pub fn daetron_probe(rules_dir_override: Option<&Path>) -> DaetronStatus {
    let pid = find_daetron_pid();
    let dir = rules_dir_override
        .map(Path::to_path_buf)
        .unwrap_or_else(daetron_default_rules_dir);

    let mut rules = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("rule") {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    rules.push(name.to_string());
                }
            }
        }
    }
    rules.sort();

    DaetronStatus {
        running: pid.is_some(),
        pid,
        rules_dir: dir.display().to_string(),
        rules,
    }
}

fn find_binary_in_home_or_path(rel_doc_path: &str, name: &str) -> Option<PathBuf> {
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join(rel_doc_path);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let cand = dir.join(name);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

pub fn daeboard_binary_path() -> Option<PathBuf> {
    find_binary_in_home_or_path("Documents/daeboard/daeboard", "daeboard")
}

pub fn daetron_binary_path() -> Option<PathBuf> {
    find_binary_in_home_or_path("Documents/daetron/daetron", "daetron")
}

/// Starts `daeboard` as a root transient systemd unit via passwordless sudo or a terminal sudo prompt.
pub fn daeboard_start_sync() -> Result<String, String> {
    if Path::new(DAEBOARD_SOCK_PATH).exists()
        && daeboard_send_sync("ping").is_ok_and(|r| r.eq_ignore_ascii_case("pong"))
    {
        return Ok("Daeboard daemon is already running".into());
    }
    let Some(bin) = daeboard_binary_path() else {
        return Err("daeboard binary not found in ~/Documents/daeboard/daeboard or PATH".into());
    };
    let bin_str = bin.display().to_string();

    // Fast path if sudo NOPASSWD is configured for systemd-run
    if let Ok(st) = std::process::Command::new("sudo")
        .args([
            "-n",
            "systemd-run",
            "--unit=daeboard",
            "--collect",
            "--description=daeboard keyboard lights",
            &bin_str,
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        if st.success() {
            return Ok("Started daeboard service via sudo systemd-run".into());
        }
    }

    // No GUI polkit agent is guaranteed on minimal compositors (e.g. Hyprland);
    // spawn a terminal sudo prompt so the user can authenticate cleanly.
    let terminals: [(&str, &[&str]); 5] = [
        ("kitty", &["--title", "Start Daeboard (sudo)", "-e"]),
        ("foot", &["--title", "Start Daeboard (sudo)"]),
        ("alacritty", &["--title", "Start Daeboard (sudo)", "-e"]),
        ("wezterm", &["start", "--"]),
        ("xterm", &["-T", "Start Daeboard (sudo)", "-e"]),
    ];
    for (term, prefix) in terminals {
        if find_binary_in_home_or_path("", term).is_some() {
            let mut cmd = std::process::Command::new(term);
            cmd.args(prefix).args([
                "sudo",
                "systemd-run",
                "--unit=daeboard",
                "--collect",
                "--description=daeboard keyboard lights",
                &bin_str,
            ]);
            if let Ok(_child) = cmd
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                return Ok(format!(
                    "Opened {term} sudo prompt to start daeboard — enter password to connect"
                ));
            }
        }
    }

    Err(format!(
        "No terminal/polkit agent found. Run in terminal: sudo systemd-run --unit=daeboard --collect {bin_str}"
    ))
}

/// Requests clean shutdown of running `daeboard` over `/run/daeboard/daeboard.sock`.
pub fn daeboard_stop_sync() -> Result<String, String> {
    if !Path::new(DAEBOARD_SOCK_PATH).exists() {
        return Err("Daeboard socket not active".into());
    }
    let reply = daeboard_send_sync("quit")?;
    Ok(format!("Sent quit to daeboard ({reply})"))
}

/// Starts `daetron run -r <rules_dir>` in the background if not already active.
pub fn daetron_start_sync() -> Result<String, String> {
    if let Some(pid) = find_daetron_pid() {
        return Ok(format!("daetron already running (pid {pid})"));
    }
    let Some(bin) = daetron_binary_path() else {
        return Err("daetron binary not found in ~/Documents/daetron/daetron or PATH".into());
    };
    let dir = daetron_default_rules_dir();
    let child = std::process::Command::new(&bin)
        .args(["run", "-r"])
        .arg(&dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to launch daetron: {e}"))?;
    Ok(format!("Started daetron (pid {})", child.id()))
}

pub fn daetron_reload_sync() -> Result<String, String> {
    let Some(pid) = find_daetron_pid() else {
        return Err("daetron process is not running".into());
    };
    let rc = unsafe { libc::kill(pid as libc::pid_t, libc::SIGHUP) };
    if rc == 0 {
        Ok(format!("Sent SIGHUP to daetron (pid {pid})"))
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}

fn find_daetron_pid() -> Option<u32> {
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return None;
    };
    for entry in rd.filter_map(|e| e.ok()) {
        let fname = entry.file_name();
        let Ok(pid) = fname.to_string_lossy().parse::<u32>() else {
            continue;
        };
        let comm_path = entry.path().join("comm");
        if let Ok(comm) = std::fs::read_to_string(comm_path) {
            if comm.trim() == "daetron" {
                return Some(pid);
            }
        }
    }
    None
}

/// Spawns a non-blocking hardware output worker so keyboard LED updates never block the engine.
pub fn spawn_hardware_worker() -> mpsc::Sender<HardwareCmd> {
    let (tx, mut rx) = mpsc::channel::<HardwareCmd>(32);
    tokio::spawn(async move {
        let mut last_color: Option<String> = None;
        let mut last_bri: Option<u8> = None;
        while let Some(cmd) = rx.recv().await {
            let mut pending = Some(cmd);
            // Coalesce queued bursts so only the latest state hits sysfs.
            while let Ok(newer) = rx.try_recv() {
                pending = Some(newer);
            }
            if let Some(c) = pending {
                match c {
                    HardwareCmd::DaeboardColor(hex) => {
                        let clean = hex.trim().trim_start_matches('#').to_ascii_lowercase();
                        if clean.len() == 6 && last_color.as_deref() != Some(clean.as_str()) {
                            last_color = Some(clean.clone());
                            let _ = tokio::task::spawn_blocking(move || {
                                let _ = daeboard_send_sync(&format!("color {clean}"));
                            })
                            .await;
                        }
                    }
                    HardwareCmd::DaeboardBrightness(lvl) => {
                        let clamped = lvl.min(3);
                        if last_bri != Some(clamped) {
                            last_bri = Some(clamped);
                            let _ = tokio::task::spawn_blocking(move || {
                                let _ = daeboard_send_sync(&format!("brightness {clamped}"));
                            })
                            .await;
                        }
                    }
                }
            }
        }
    });
    tx
}
