use super::App;
use crate::session::SessionManager;

impl App {
    pub fn load_lyric_timeline(&mut self, lines: Vec<crate::events::LyricLineData>) {
        self.lyric.load_timeline(lines);
    }

    pub fn scroll_lyrics(&mut self, delta: isize) {
        self.lyric.scroll(delta);
        self.status = format!("lyric scroll {:+#}", self.lyric.scroll_offset);
    }

    pub fn lyric_offset(&self) -> f32 {
        self.lyric.offset
    }

    pub fn toggle_translation(&mut self) {
        let visible = self.lyric.toggle_translation();
        self.status = format!("translation: {}", if visible { "on" } else { "off" });
    }

    pub fn toggle_romanization(&mut self) {
        let visible = self.lyric.toggle_romanization();
        self.status = format!("romanization: {}", if visible { "on" } else { "off" });
    }

    pub fn shift_lyric_offset(&mut self, session: &SessionManager, delta: f32) {
        self.lyric.offset = (self.lyric.offset + delta).clamp(-10.0, 10.0);
        self.lyric.save_preferences();
        session.control_current(serde_json::json!({
            "cmd": "lyric_offset",
            "seconds": self.lyric.offset
        }));
        self.status = format!("lyric offset {:+.1}s", self.lyric.offset);
    }
}
