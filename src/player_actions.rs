use crate::app::App;
use crate::session::SessionManager;

pub fn toggle_pause(app: &mut App, session: &SessionManager) {
    let paused = app.toggle_pause();
    session.control_current(serde_json::json!({
        "cmd": if paused { "pause" } else { "resume" }
    }));
}

pub fn seek(app: &mut App, session: &SessionManager, seconds: f32) {
    let duration = app.meta.duration_ms as f32 / 1000.0;
    let seconds = seconds.clamp(0.0, duration.max(0.0));
    app.update_position(seconds);
    session.control_current(serde_json::json!({
        "cmd": "seek",
        "seconds": seconds
    }));
}

pub fn seek_relative(app: &mut App, session: &SessionManager, delta: f32) {
    seek(app, session, app.current_position() + delta);
}

pub fn adjust_volume(app: &mut App, session: &SessionManager, delta: f32) {
    let volume = app.adjust_volume(delta);
    session.control_current(serde_json::json!({
        "cmd": "volume",
        "value": volume
    }));
    app.status = format!("volume {:.0}%", volume * 100.0);
}

pub fn toggle_mute(app: &mut App, session: &SessionManager) {
    let muted = app.toggle_mute();
    session.control_current(serde_json::json!({
        "cmd": "mute",
        "value": muted
    }));
    app.status = if muted { "muted" } else { "unmuted" }.into();
}

pub fn cycle_mode(app: &mut App) {
    let mode = app.cycle_play_mode();
    app.status = format!("play mode: {mode}");
}
