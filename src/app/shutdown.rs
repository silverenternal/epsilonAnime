use super::App;

impl Drop for App {
    fn drop(&mut self) {
        self.layout.save_state();
        self.playback.save();
    }
}
