use super::App;
use crate::metrics::Metrics;
use crate::terminal::FrameReport;
use std::time::Duration;

impl App {
    pub fn observe_frame(&mut self, elapsed: Duration) {
        let millis = elapsed.as_secs_f32() * 1000.0;
        self.frame_ms = self.frame_ms * 0.85 + millis * 0.15;
        let budget = 1000.0 / self.opts.fps.max(5) as f32;
        if self.frame_ms > budget * 1.15 {
            self.low_quality = true;
        } else if self.frame_ms < budget * 0.70 {
            self.low_quality = false;
        }
        if self.frame_ms > budget * 1.40 {
            self.target_fps = (self.target_fps - 2.0).max(90.0);
        } else if self.frame_ms < budget * 0.60 {
            self.target_fps = (self.target_fps + 1.0).min(self.opts.fps as f32);
        }
    }

    pub fn target_fps(&self) -> u32 {
        self.target_fps.round().max(90.0) as u32
    }

    pub fn observe_terminal(&mut self, report: FrameReport) {
        self.encode_ms = self.encode_ms * 0.85 + report.encode.as_secs_f32() * 1000.0 * 0.15;
        self.terminal_wait_ms =
            self.terminal_wait_ms * 0.85 + report.write_wait.as_secs_f32() * 1000.0 * 0.15;
    }

    pub fn metrics(&self) -> Metrics {
        Metrics {
            schema: 1,
            animation: self.engine.label().to_string(),
            fps: self.fps,
            frame_ms: self.frame_ms,
            sample_ms: self.sample_ms,
            encode_ms: self.encode_ms,
            terminal_wait_ms: self.terminal_wait_ms,
            target_fps: self.target_fps,
            dirty_tiles: self.buffer.last_dirty_tile_count(),
            flushed_tiles: self.buffer.last_dirty_tile_count(),
            bpm: self.audio.bpm,
            exposure: self.exposure,
            low_quality: self.low_quality,
        }
    }
}
