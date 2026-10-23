use crate::app::App;
use crate::app::search_prompt::PromptMode;
use crate::auth::AuthClient;
use crate::library::SongItem;
use crate::prefetch::{Prefetcher, cache_path, parse_links};
use crate::session::SessionManager;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::VecDeque;
use std::path::PathBuf;

pub fn submit_input(
    app: &mut App,
    session: &SessionManager,
    prefetcher: &Prefetcher,
    generation: &mut u64,
    queue: &mut VecDeque<String>,
    input: &str,
) {
    let mut links = parse_links(input);
    if links.is_empty() {
        app.status = "not a NetEase song link (expected music.163.com/song?id=...)".into();
        return;
    }
    queue.clear();
    let songs = links
        .iter()
        .filter_map(|link| song_id(link))
        .map(|id| SongItem {
            id,
            title: format!("Track {id}"),
            artist: String::new(),
            duration_ms: 0,
        })
        .collect();
    app.set_playlist(songs);
    let first = links.remove(0);
    start_link(app, session, generation, &first, Some(cache_path(&first)));
    if let Some(next) = app.playlist_neighbor(1) {
        prefetcher.prefetch(&format!("https://music.163.com/song?id={}", next.id));
    }
}

#[allow(clippy::too_many_arguments)]
pub fn submit_prompt(
    app: &mut App,
    auth: &mut Option<AuthClient>,
    session: &SessionManager,
    prefetcher: &Prefetcher,
    generation: &mut u64,
    queue: &mut VecDeque<String>,
    text: &str,
    mode: PromptMode,
) {
    match mode {
        PromptMode::SongSearch => {
            app.prepare_library_browse();
            start_auth(app, auth, "search", Some(text));
        }
        PromptMode::LyricSearch => app.search_lyrics(text),
        PromptMode::Url => submit_input(app, session, prefetcher, generation, queue, text),
    }
}

fn song_id(link: &str) -> Option<u64> {
    let trimmed = link.trim();
    if trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return trimmed.parse().ok();
    }
    let value = link.split("id=").nth(1)?;
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

pub fn play_song(
    app: &mut App,
    session: &SessionManager,
    prefetcher: &Prefetcher,
    generation: &mut u64,
    index: usize,
    song: &SongItem,
) {
    app.set_playlist_pos(index);
    app.close_library();
    let url = format!("https://music.163.com/song?id={}", song.id);
    start_link(app, session, generation, &url, Some(cache_path(&url)));
    if let Some(next) = app.playlist_neighbor(1) {
        prefetcher.prefetch(&format!("https://music.163.com/song?id={}", next.id));
    }
}

pub fn play_next(
    app: &mut App,
    session: &SessionManager,
    prefetcher: &Prefetcher,
    generation: &mut u64,
    queue: &mut VecDeque<String>,
) {
    if let Some((index, song)) = app.step_playlist(1) {
        play_song(app, session, prefetcher, generation, index, &song);
        return;
    }
    if let Some(url) = queue.pop_front() {
        let cache = cache_path(&url);
        start_link(app, session, generation, &url, Some(cache));
        if let Some(next) = queue.front() {
            prefetcher.prefetch(next);
        }
        return;
    }
    app.status = "end of queue".into();
}

pub fn play_previous(
    app: &mut App,
    session: &SessionManager,
    prefetcher: &Prefetcher,
    generation: &mut u64,
) {
    if let Some((index, song)) = app.step_playlist(-1) {
        play_song(app, session, prefetcher, generation, index, &song);
    } else {
        app.status = "previous is available inside a playlist".into();
    }
}

pub fn preload_next_if_needed(
    app: &App,
    session: &SessionManager,
    preloaded_url: &mut Option<String>,
) {
    if let Some(current) = app.resume_song() {
        let current_url = format!("https://music.163.com/song?id={}", current.id);
        if preloaded_url.as_deref() == Some(current_url.as_str()) {
            *preloaded_url = None;
        }
    }
    let duration = app.meta.duration_ms as f32 / 1000.0;
    if duration <= 0.0 || app.current_position() < (duration - 12.0).max(0.0) {
        return;
    }
    let Some(next) = app.playlist_neighbor(1) else {
        return;
    };
    let url = format!("https://music.163.com/song?id={}", next.id);
    if preloaded_url.as_deref() == Some(url.as_str()) {
        return;
    }
    session.preload(url.clone(), Some(cache_path(&url)));
    *preloaded_url = Some(url);
}

pub fn start_auth(
    app: &mut App,
    client: &mut Option<AuthClient>,
    command: &str,
    value: Option<&str>,
) {
    *client = None;
    match AuthClient::start(command, value) {
        Ok(started) => {
            *client = Some(started);
            app.status = if command == "qr" {
                "waiting for NetEase login QR…".into()
            } else {
                format!("loading NetEase {command}…")
            };
        }
        Err(err) => app.status = format!("auth backend: {err}"),
    }
}

pub fn open_library_item(app: &mut App, client: &mut Option<AuthClient>, kind: &str, id: u64) {
    if matches!(kind, "daily" | "radio" | "recent" | "liked") {
        start_auth(app, client, "browse", Some(kind));
        return;
    }
    if matches!(kind, "charts" | "albums" | "artists") {
        app.prepare_library_browse();
        start_auth(app, client, "browse", Some(kind));
        return;
    }
    let value = format!("{kind}:{id}:0");
    start_auth(app, client, "songs", Some(&value));
}

pub fn start_link(
    app: &mut App,
    session: &SessionManager,
    generation: &mut u64,
    input: &str,
    cache: Option<PathBuf>,
) {
    *generation = generation.saturating_add(1);
    app.start_session();
    app.record_retry_link(input);
    session.load(*generation, input.trim().to_string(), cache);
    session.control_current(serde_json::json!({
        "cmd": "volume",
        "value": app.effective_volume()
    }));
    session.control_current(serde_json::json!({
        "cmd": "mute",
        "value": app.is_muted()
    }));
    session.control_current(serde_json::json!({
        "cmd": "lyric_offset",
        "seconds": app.lyric_offset()
    }));
}

pub fn is_quit_key(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('q' | 'Q') | KeyCode::Esc)
        && !key
            .modifiers
            .intersects(KeyModifiers::ALT | KeyModifiers::CONTROL)
}

pub fn is_shell_toggle(key: &KeyEvent) -> bool {
    key.code == KeyCode::F(4)
        || (matches!(key.code, KeyCode::Char('s' | 'S'))
            && key.modifiers.contains(KeyModifiers::ALT))
}

pub fn is_logs_toggle(key: &KeyEvent) -> bool {
    key.code == KeyCode::F(3)
        || (matches!(key.code, KeyCode::Char('l' | 'L'))
            && key.modifiers.contains(KeyModifiers::ALT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_song_id_from_links() {
        assert_eq!(
            song_id("https://music.163.com/song?id=3347119767&uct2=x"),
            Some(3347119767)
        );
        assert_eq!(song_id("3347119767"), Some(3347119767));
    }

    #[test]
    fn modified_q_is_not_a_global_quit_shortcut() {
        assert!(is_quit_key(&KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE
        )));
        assert!(!is_quit_key(&KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::ALT
        )));
    }
}
