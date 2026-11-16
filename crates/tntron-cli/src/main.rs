mod popup;

use anyhow::{Context, Result};
use tntron_core::{default_socket_path, IpcCommand, IpcResponse, TntronStatus};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        print_usage();
        return Ok(());
    }

    if args[0] == "popup" {
        return popup::run_popup_tui().await;
    }

    let waybar = args.first().map(|s| s.as_str()) == Some("waybar");
    let follow = args.iter().any(|a| a == "--follow");

    let cmd = match args[0].as_str() {
        "play" => IpcCommand::Play,
        "pause" => IpcCommand::Pause,
        "toggle" => IpcCommand::Toggle,
        "next" => IpcCommand::Next,
        "prev" => IpcCommand::Prev,
        "stop" => IpcCommand::Stop,
        "mute" => IpcCommand::Mute { value: None },
        "volume" => {
            let val = args.get(1).context("missing volume value (e.g. +5 or 80)")?;
            if val.starts_with('+') || val.starts_with('-') {
                IpcCommand::Volume {
                    delta: Some(val.parse()?),
                    value: None,
                }
            } else {
                IpcCommand::Volume {
                    delta: None,
                    value: Some(val.parse()?),
                }
            }
        }
        "seek" => {
            let val = args.get(1).context("missing seek value (e.g. +10 or 60)")?;
            if val.starts_with('+') || val.starts_with('-') {
                IpcCommand::Seek {
                    delta: Some(val.parse()?),
                    value: None,
                }
            } else {
                IpcCommand::Seek {
                    delta: None,
                    value: Some(val.parse()?),
                }
            }
        }
        "speed" => {
            let val: f64 = args.get(1).context("missing speed value")?.parse()?;
            IpcCommand::Speed { value: val }
        }
        "waybar" | "status" => {
            if follow {
                IpcCommand::Subscribe
            } else {
                IpcCommand::Status
            }
        }
        _ => {
            print_usage();
            return Ok(());
        }
    };

    let sock = default_socket_path();
    let stream = UnixStream::connect(&sock)
        .await
        .with_context(|| format!("Could not connect to tntron at {}", sock.display()))?;

    let (read_half, mut write_half) = stream.into_split();
    let payload = serde_json::to_string(&cmd)?;
    write_half.write_all(payload.as_bytes()).await?;
    write_half.write_all(b"\n").await?;

    let mut lines = BufReader::new(read_half).lines();
    if matches!(cmd, IpcCommand::Subscribe) {
        while let Some(line) = lines.next_line().await? {
            if waybar {
                println!("{line}");
            } else if let Ok(st) = serde_json::from_str::<TntronStatus>(&line) {
                println!("{} | {} ({}%)", st.state, st.text, st.percentage);
            }
        }
    } else if let Some(line) = lines.next_line().await? {
        let resp: IpcResponse = serde_json::from_str(&line)?;
        if let Some(st) = resp.status {
            if waybar {
                println!("{}", serde_json::to_string(&st)?);
            } else {
                println!(
                    "{} — {} [{:.0}s / {:.0}s] (Vol: {:.0}%, Loop: {}, Shuffle: {})",
                    st.state,
                    st.text,
                    st.position,
                    st.duration,
                    st.volume,
                    st.loop_mode,
                    if st.shuffle { "On" } else { "Off" }
                );
            }
        } else if let Some(err) = resp.error {
            anyhow::bail!("{err}");
        }
    }

    Ok(())
}

fn print_usage() {
    eprintln!(
        "tntron-ctl <play|pause|toggle|next|prev|stop|mute|volume <v|+d>|seek <s|+d>|speed <x>|status|waybar [--follow]|popup>"
    );
}
