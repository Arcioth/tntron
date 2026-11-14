use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};
use tracing::{error, info, warn};

use super::protocol::{MpvCommand, MpvEvent, MpvIncoming};

#[derive(Debug, Clone)]
pub enum MpvActorEvent {
    Event(MpvEvent),
    TimePos(f64),
    Closed,
}

pub struct MpvProcessGuard {
    child: Option<Child>,
    socket_path: PathBuf,
}

impl Drop for MpvProcessGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

pub struct MpvSupervisor {
    #[allow(dead_code)]
    pub guard: MpvProcessGuard,
    pub stream: UnixStream,
}

impl MpvSupervisor {
    pub async fn spawn() -> Result<Self> {
        let uid = unsafe { libc::getuid() };
        let pid = std::process::id();
        let socket_dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);

        let socket_path = socket_dir.join(format!("tntron-mpv-{}-{}.sock", uid, pid));
        let _ = std::fs::remove_file(&socket_path);

        let parent_pid = unsafe { libc::getpid() };
        let mut cmd = Command::new("mpv");
        cmd.args([
            "--no-video",
            "--audio-display=no",
            "--vo=null",
            "--no-terminal",
            "--input-terminal=no",
            "--idle=yes",
            "--keep-open=no",
            "--no-osc",
            "--no-osd-bar",
            "--load-scripts=no",
            "--no-config",
            "--volume=100",
            "--volume-max=100",
            "--ao=pipewire,pulse,alsa",
            "--msg-level=all=no",
        ]);
        cmd.arg(format!("--input-ipc-server={}", socket_path.display()));
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());
        cmd.kill_on_drop(true);

        unsafe {
            cmd.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::getppid() != parent_pid {
                    libc::_exit(1);
                }
                Ok(())
            });
        }

        let mut child = cmd.spawn().context("Failed to spawn headless mpv binary")?;
        let stream = Self::connect_with_retry(&socket_path, &mut child, Duration::from_secs(3))
            .await
            .context("Failed to connect to mpv IPC socket")?;

        Ok(Self {
            guard: MpvProcessGuard {
                child: Some(child),
                socket_path,
            },
            stream,
        })
    }

    async fn connect_with_retry(
        socket_path: &Path,
        child: &mut Child,
        timeout: Duration,
    ) -> Result<UnixStream> {
        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                anyhow::bail!("mpv process exited prematurely with status: {status}");
            }
            match UnixStream::connect(socket_path).await {
                Ok(stream) => return Ok(stream),
                Err(_) if start.elapsed() < timeout => {
                    tokio::time::sleep(Duration::from_millis(30)).await;
                }
                Err(e) => anyhow::bail!("Timeout connecting to mpv socket: {e}"),
            }
        }
    }
}

pub async fn run_mpv_actor(
    stream: UnixStream,
    mut cmd_rx: mpsc::Receiver<MpvCommand>,
    event_tx: mpsc::Sender<MpvActorEvent>,
) -> Result<()> {
    let (read_half, write_half) = stream.into_split();
    let mut reader = FramedRead::new(read_half, LinesCodec::new_with_max_length(64 * 1024));
    let mut writer = FramedWrite::new(write_half, LinesCodec::new_with_max_length(64 * 1024));

    for cmd in [
        r#"{"command":["observe_property",2,"pause"]}"#,
        r#"{"command":["observe_property",3,"duration"]}"#,
        r#"{"command":["observe_property",5,"volume"]}"#,
        r#"{"command":["observe_property",6,"audio-codec-name"]}"#,
        r#"{"command":["observe_property",7,"audio-params"]}"#,
    ] {
        writer.send(cmd.to_string()).await?;
    }

    let mut next_req_id = 100u64;
    let mut pending_time_pos_id: Option<u64> = None;

    loop {
        tokio::select! {
            maybe_line = reader.next() => {
                match maybe_line {
                    Some(Ok(line)) => {
                        if let Ok(incoming) = serde_json::from_str::<MpvIncoming>(&line) {
                            match incoming {
                                MpvIncoming::Event(event) => {
                                    let _ = event_tx.send(MpvActorEvent::Event(event)).await;
                                }
                                MpvIncoming::Response(resp) => {
                                    if resp.error == "success"
                                        && resp.request_id.is_some()
                                        && pending_time_pos_id == resp.request_id
                                    {
                                        pending_time_pos_id = None;
                                        if let Some(num) = resp.data.and_then(|v| v.as_f64()) {
                                            let _ = event_tx.send(MpvActorEvent::TimePos(num)).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Some(Err(e)) => warn!("mpv IPC line read error: {e}"),
                    None => {
                        info!("mpv IPC stream closed");
                        let _ = event_tx.send(MpvActorEvent::Closed).await;
                        break;
                    }
                }
            }
            Some(cmd) = cmd_rx.recv() => {
                next_req_id += 1;
                if matches!(cmd, MpvCommand::GetTimePos) {
                    pending_time_pos_id = Some(next_req_id);
                }
                let req = cmd.to_request(Some(next_req_id));
                if let Ok(json_str) = serde_json::to_string(&req) {
                    if let Err(e) = writer.send(json_str).await {
                        error!("Failed to write command to mpv: {e}");
                        break;
                    }
                }
            }
            else => break,
        }
    }

    Ok(())
}
