use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Metrics {
    pub schema: u32,
    pub animation: String,
    pub fps: f32,
    pub frame_ms: f32,
    pub sample_ms: f32,
    pub encode_ms: f32,
    pub terminal_wait_ms: f32,
    pub target_fps: f32,
    pub dirty_tiles: usize,
    pub flushed_tiles: usize,
    pub bpm: f32,
    pub exposure: f32,
    pub low_quality: bool,
}

impl Metrics {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_schema_has_stable_operational_fields() {
        let metrics = Metrics {
            schema: 1,
            animation: "adaptive".into(),
            fps: 30.0,
            frame_ms: 20.0,
            sample_ms: 5.0,
            encode_ms: 3.0,
            terminal_wait_ms: 1.0,
            target_fps: 30.0,
            dirty_tiles: 2,
            flushed_tiles: 2,
            bpm: 120.0,
            exposure: 1.0,
            low_quality: false,
        };
        let value: serde_json::Value = serde_json::from_str(&metrics.to_json()).unwrap();
        for key in [
            "schema",
            "fps",
            "sample_ms",
            "encode_ms",
            "terminal_wait_ms",
            "bpm",
        ] {
            assert!(value.get(key).is_some(), "missing {key}");
        }
    }
}
