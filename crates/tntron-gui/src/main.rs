mod app;
mod button_style;
mod gif_viz;
mod icons;
mod logo;
mod reactive_bg;
mod theme;
mod ui_widgets;
mod visualizer;

use std::path::PathBuf;
use iced::Size;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tntron_core::{default_socket_path, IpcCommand, IpcResponse, TntronStatus};

use app::TntronApp;

fn resolve_music_dir(arg: Option<&str>) -> PathBuf {
    if let Some(a) = arg {
        let p = PathBuf::from(a);
        if p.is_dir() {
            return p;
        }
    }
    if let Ok(env_dir) = std::env::var("TNTRON_MUSIC_DIR") {
        let p = PathBuf::from(env_dir);
        if p.is_dir() {
            return p;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let music = PathBuf::from(home).join("Music");
        if music.is_dir() {
            return music;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn is_cli_subcommand(arg: &str) -> bool {
    matches!(
        arg,
        "play"
            | "pause"
            | "toggle"
            | "next"
            | "prev"
            | "stop"
            | "mute"
            | "volume"
            | "seek"
            | "speed"
            | "dsp"
            | "spatial"
            | "sleep"
            | "loop"
            | "shuffle"
            | "status"
            | "waybar"
            | "help"
            | "--help"
            | "-h"
    )
}

fn run_cli_subcommand(args: &[String]) -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move {
        let waybar = args[0] == "waybar";
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
                let val = args.get(1).ok_or_else(|| anyhow::anyhow!("missing volume value"))?;
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
                let val = args.get(1).ok_or_else(|| anyhow::anyhow!("missing seek value"))?;
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
            "speed" => IpcCommand::Speed {
                value: args.get(1).ok_or_else(|| anyhow::anyhow!("missing speed"))?.parse()?,
            },
            "dsp" => IpcCommand::DspPreset {
                preset: args.get(1).ok_or_else(|| anyhow::anyhow!("missing preset (flat|bass|vocal|warm|rock|8d)"))?.clone(),
            },
            "spatial" => IpcCommand::ToggleSpatial,
            "sleep" => {
                let mins = args.get(1).and_then(|s| if s == "off" { None } else { s.parse::<u64>().ok() });
                IpcCommand::Sleep { minutes: mins }
            }
            "loop" => IpcCommand::CycleLoop,
            "shuffle" => IpcCommand::Shuffle,
            "status" | "waybar" => {
                if follow {
                    IpcCommand::Subscribe
                } else {
                    IpcCommand::Status
                }
            }
            _ => {
                println!("tntron [DIR] | tntron <play|pause|toggle|next|prev|stop|mute|volume|seek|speed|dsp <preset>|spatial|sleep <min|off>|loop|shuffle|status|waybar [--follow]>");
                return Ok(());
            }
        };

        let sock = default_socket_path();
        let stream = UnixStream::connect(&sock).await?;
        let (read_half, mut write_half) = stream.into_split();
        write_half
            .write_all(serde_json::to_string(&cmd)?.as_bytes())
            .await?;
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
                    println!("{}", json!(st));
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
            }
        }
        Ok(())
    })
}

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(first) = args.first() {
        if is_cli_subcommand(first) {
            if let Err(e) = run_cli_subcommand(&args) {
                eprintln!("tntron: {e}");
                std::process::exit(1);
            }
            return Ok(());
        }
    }

    let start_fullscreen = args.iter().any(|a| a == "--fullscreen" || a == "-f");
    let force_x11 = args.iter().any(|a| a == "--x11");
    let force_wayland = args.iter().any(|a| a == "--wayland");
    let force_opaque = args.iter().any(|a| a == "--opaque")
        || std::env::var("TNTRON_OPAQUE").is_ok_and(|v| v == "1");

    if force_x11 && !force_wayland {
        std::env::set_var("WINIT_UNIX_BACKEND", "x11");
    } else if force_wayland && !force_x11 {
        std::env::set_var("WINIT_UNIX_BACKEND", "wayland");
    }

    tracing_subscriber::fmt()
        .with_env_filter("tntron=info,tntron_core=info")
        .init();

    let dir_arg = args.iter().find(|a| !a.starts_with('-')).map(|s| s.as_str());
    let music_root = resolve_music_dir(dir_arg);

    iced::application(
        move || TntronApp::boot(music_root.clone(), start_fullscreen),
        TntronApp::update,
        TntronApp::view,
    )
    .font(icons::ICON_FONT_BYTES)
    .title(TntronApp::title)
    .theme(TntronApp::theme)
    .subscription(TntronApp::subscription)
    .window(iced::window::Settings {
        size: Size::new(1220.0, 780.0),
        transparent: !force_opaque,
        icon: logo::window_icon(),
        platform_specific: iced::window::settings::PlatformSpecific {
            application_id: "tntron".to_string(),
            ..Default::default()
        },
        ..Default::default()
    })
    .run()
}
