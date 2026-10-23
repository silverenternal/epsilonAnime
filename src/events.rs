use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BackendEvent {
    Meta {
        title: String,
        artist: String,
        duration_ms: u64,
        cover_path: Option<String>,
        cover_cols: Option<usize>,
        cover_rows: Option<usize>,
        #[serde(default)]
        particle_path: Option<String>,
        #[serde(default)]
        particle_count: usize,
    },
    CoverReady {
        path: String,
        cols: usize,
        rows: usize,
        #[serde(default)]
        particle_path: Option<String>,
        #[serde(default)]
        particle_count: usize,
    },
    Comments {
        comments: Vec<CommentData>,
    },
    Features {
        t: f32,
        rms: f32,
        bass: f32,
        mid: f32,
        treble: f32,
        flux: f32,
        onset: f32,
        #[serde(default)]
        bpm: f32,
        #[serde(default)]
        spectrum: Vec<f32>,
        #[serde(default)]
        centroid: f32,
        #[serde(default)]
        spread: f32,
        #[serde(default)]
        flatness: f32,
        #[serde(default)]
        rolloff: f32,
        #[serde(default)]
        zcr: f32,
        #[serde(default)]
        crest: f32,
        #[serde(default)]
        stereo_width: f32,
    },
    Lyric {
        index: usize,
        text: String,
        #[serde(default)]
        translation: String,
        #[serde(default)]
        romaji: String,
        #[serde(default)]
        segments: Vec<LyricSegment>,
        progress: f32,
        #[serde(default)]
        offset: f32,
    },
    LyricsReady {
        lines: Vec<LyricLineData>,
    },
    State {
        state: String,
    },
    Playback {
        #[serde(default)]
        position: f32,
        #[serde(default)]
        volume: f32,
        #[serde(default)]
        muted: bool,
        #[serde(default)]
        paused: bool,
    },
    Error {
        message: String,
        #[serde(default)]
        id: String,
        #[serde(default)]
        scope: String,
        #[serde(default)]
        level: String,
        #[serde(default)]
        retryable: bool,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommentData {
    pub text: String,
    pub user: String,
    pub likes: u64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LyricSegment {
    pub start: f32,
    pub end: f32,
    pub text: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LyricLineData {
    pub time: f32,
    pub text: String,
    #[serde(default)]
    pub translation: String,
    #[serde(default)]
    pub romaji: String,
    #[serde(default)]
    pub segments: Vec<LyricSegment>,
}

#[macro_export]
macro_rules! event_kinds {
    ($($variant:ident),+ $(,)?) => {
        impl BackendEvent {
            pub fn kind(&self) -> &'static str {
                match self {
                    $(Self::$variant { .. } => stringify!($variant)),+
                }
            }
        }
    };
}

crate::event_kinds!(
    Meta,
    CoverReady,
    Comments,
    Features,
    Lyric,
    LyricsReady,
    State,
    Playback,
    Error
);

impl BackendEvent {
    pub fn parse(line: &str) -> serde_json::Result<Self> {
        serde_json::from_str(line)
    }
}

#[cfg(test)]
mod tests {
    use super::BackendEvent;
    use proptest::prelude::*;

    #[test]
    fn parses_features() {
        let line = r#"{"type":"features","t":1.2,"rms":0.2,"bass":0.1,
            "mid":0.3,"treble":0.2,"flux":0.4,"onset":0.0}"#;
        assert!(matches!(
            BackendEvent::parse(line).unwrap(),
            BackendEvent::Features { .. }
        ));
    }

    #[test]
    fn round_trips_and_dispatches() {
        crossterm::style::Colored::set_ansi_color_disabled(false);
        let event = BackendEvent::State {
            state: "ended".into(),
        };
        let line = serde_json::to_string(&event).unwrap();
        let decoded = BackendEvent::parse(&line).unwrap();
        assert_eq!(decoded.kind(), "State");
    }

    proptest! {
        #[test]
        fn malformed_ndjson_only_returns_an_error(input in ".{0,512}") {
            let _ = BackendEvent::parse(&input);
        }

        #[test]
        fn state_events_round_trip(value in ".{0,160}") {
            let event = BackendEvent::State { state: value.clone() };
            let encoded = serde_json::to_string(&event).unwrap();
            let decoded = BackendEvent::parse(&encoded).unwrap();
            let round_tripped = match decoded {
                BackendEvent::State { state } => state == value,
                _ => false,
            };
            prop_assert!(round_tripped);
        }
    }
}
