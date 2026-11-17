use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct MpvRequest {
    pub command: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum MpvIncoming {
    Event(MpvEvent),
    Response(MpvResponse),
}

#[derive(Debug, Deserialize)]
pub struct MpvResponse {
    pub error: String,
    pub request_id: Option<u64>,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event")]
pub enum MpvEvent {
    #[serde(rename = "property-change")]
    PropertyChange {
        id: u64,
        name: String,
        data: Option<serde_json::Value>,
    },
    #[serde(rename = "end-file")]
    EndFile {
        reason: Option<String>,
        file_error: Option<String>,
        playlist_entry_id: Option<i64>,
    },
    #[serde(rename = "file-loaded")]
    FileLoaded,
    #[serde(rename = "playback-restart")]
    PlaybackRestart,
    #[serde(rename = "start-file")]
    StartFile { playlist_entry_id: Option<i64> },
    #[serde(rename = "idle")]
    Idle,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MpvCommand {
    LoadFile { path: String, replace: bool },
    TogglePause,
    SetPause(bool),
    Seek { seconds: f64, relative: bool },
    SetVolume(f64),
    SetSpeed(f64),
    SetPitchCorrection(bool),
    SetMute(bool),
    SetAudioFilter(String),
    Stop,
    Quit,
    GetTimePos,
}

impl MpvCommand {
    pub fn to_request(&self, req_id: Option<u64>) -> MpvRequest {
        match self {
            Self::LoadFile { path, replace } => MpvRequest {
                command: vec![
                    "loadfile".into(),
                    path.clone().into(),
                    if *replace { "replace".into() } else { "append".into() },
                ],
                request_id: req_id,
            },
            Self::TogglePause => MpvRequest {
                command: vec!["cycle".into(), "pause".into()],
                request_id: req_id,
            },
            Self::SetPause(paused) => MpvRequest {
                command: vec!["set_property".into(), "pause".into(), (*paused).into()],
                request_id: req_id,
            },
            Self::Seek { seconds, relative } => MpvRequest {
                command: vec![
                    "seek".into(),
                    (*seconds).into(),
                    if *relative { "relative".into() } else { "absolute+exact".into() },
                ],
                request_id: req_id,
            },
            Self::SetVolume(vol) => MpvRequest {
                command: vec!["set_property".into(), "volume".into(), (*vol).into()],
                request_id: req_id,
            },
            Self::SetSpeed(speed) => MpvRequest {
                command: vec!["set_property".into(), "speed".into(), (*speed).into()],
                request_id: req_id,
            },
            Self::SetPitchCorrection(enabled) => MpvRequest {
                command: vec![
                    "set_property".into(),
                    "audio-pitch-correction".into(),
                    (*enabled).into(),
                ],
                request_id: req_id,
            },
            Self::SetMute(mute) => MpvRequest {
                command: vec!["set_property".into(), "mute".into(), (*mute).into()],
                request_id: req_id,
            },
            Self::SetAudioFilter(filter) => MpvRequest {
                command: vec!["set_property".into(), "af".into(), filter.clone().into()],
                request_id: req_id,
            },
            Self::Stop => MpvRequest {
                command: vec!["stop".into()],
                request_id: req_id,
            },
            Self::Quit => MpvRequest {
                command: vec!["quit".into()],
                request_id: req_id,
            },
            Self::GetTimePos => MpvRequest {
                command: vec!["get_property".into(), "time-pos".into()],
                request_id: req_id,
            },
        }
    }
}
