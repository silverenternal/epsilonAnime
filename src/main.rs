#[macro_use]
mod macros;
mod mathlib;

mod anim;
mod app;
mod audio;
mod audio_scope;
mod auth;
mod background;
mod bench;
mod catalog;
mod cli;
mod config;
mod controls;
mod cover;
mod cover_art;
mod cover_loader;
mod danmaku;
mod events;
mod frame;
mod generated;
mod glyphs;
mod hotkeys;
mod i18n;
mod kitty;
mod layout;
mod library;
mod logs;
mod lyrics;
mod metrics;
mod notifications;
#[cfg(unix)]
mod pcm_analyzer;
mod playback;
mod player_actions;
mod prefetch;
mod render;
mod seed;
mod session;
mod session_actions;
mod shell;
mod shortcuts;
mod signals;
mod startup;
mod state_store;
mod terminal;
mod ui;

use anyhow::Result;
use app::{App, AppOptions, frame_interval};
use auth::{AuthClient, AuthEvent};
use clap::Parser;
use cli::Cli;
use controls::ControlAction;
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use layout::PointerAction;
use player_actions as action;
use prefetch::{Prefetcher, parse_links};
use session::SessionManager;
use session_actions::{
    is_logs_toggle, is_quit_key, is_shell_toggle, play_next, play_previous, play_song, start_auth,
    start_link, submit_input, submit_prompt,
};
use std::collections::VecDeque;
use std::thread;
use std::time::Instant;
use terminal::{InputEvent, TerminalGuard};
fn main() -> Result<()> {
    logs::install_panic_hook();
    signals::install();
    let cli = Cli::parse();
    if catalog::print_requested(&cli)? {
        return Ok(());
    }
    let opts = startup::options(&cli)?;
    run_interactive(cli.url.as_deref(), opts)
}

fn run_interactive(url: Option<&str>, opts: AppOptions) -> Result<()> {
    let (width, height) = TerminalGuard::size()?;
    let demo = opts.demo;
    let guard = TerminalGuard::enter()?;
    let mut app = App::new(opts, width, height);
    let session = SessionManager::new();
    let prefetcher = Prefetcher::new();
    let mut queue = VecDeque::new();
    let mut generation = 0u64;
    let mut auth: Option<AuthClient> = None;
    let mut consecutive_failures = 0u32;
    let mut preloaded_url: Option<String> = None;
    if !demo {
        if let Some(url) = url {
            submit_input(
                &mut app,
                &session,
                &prefetcher,
                &mut generation,
                &mut queue,
                url,
            );
        } else if let Some(song) = app.resume_song() {
            let index = app.resume_index();
            let position = app.resume_position();
            play_song(
                &mut app,
                &session,
                &prefetcher,
                &mut generation,
                index,
                &song,
            );
            if position > 0.0 {
                session.control_current(serde_json::json!({
                    "cmd": "seek",
                    "seconds": position
                }));
            }
        } else {
            app.status = "paste a NetEase song link or press i".into();
        }
    }
    loop {
        if signals::shutdown_requested() {
            break;
        }
        let mut quit = false;
        while let Some(input) = TerminalGuard::poll_input()? {
            match input {
                InputEvent::Key(key) => {
                    if app.help_open() {
                        let ctrl_c = key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL);
                        let plain_q = matches!(key.code, KeyCode::Char('q' | 'Q'))
                            && !key
                                .modifiers
                                .intersects(KeyModifiers::ALT | KeyModifiers::CONTROL);
                        if ctrl_c || plain_q {
                            quit = true;
                            break;
                        }
                        app.help_key(&key);
                        continue;
                    }
                    let question_mark_available = key.code != KeyCode::Char('?')
                        || (!app.shell_open() && !app.prompt_active());
                    if shortcuts::is_help_key(&key) && question_mark_available {
                        app.toggle_help();
                        continue;
                    }
                    if app.shell_open() {
                        if is_shell_toggle(&key) {
                            app.toggle_shell();
                        } else if app.shell_search_active() {
                            app.shell_search_key(&key);
                        } else if matches!(key.code, KeyCode::Char('f' | 'F'))
                            && key.modifiers.contains(KeyModifiers::ALT)
                        {
                            app.shell_begin_search();
                        } else if hotkeys::binding_matches(&app.opts.hotkeys.shell_next_tab, &key) {
                            app.shell_tab(1);
                        } else if hotkeys::binding_matches(
                            &app.opts.hotkeys.shell_previous_tab,
                            &key,
                        ) {
                            app.shell_tab(-1);
                        } else if matches!(key.code, KeyCode::Char('t' | 'T'))
                            && key.modifiers.contains(KeyModifiers::ALT)
                        {
                            app.shell_new_tab();
                        } else if matches!(key.code, KeyCode::Char('w' | 'W'))
                            && key.modifiers.contains(KeyModifiers::ALT)
                        {
                            app.shell_close_tab();
                        } else if matches!(key.code, KeyCode::Char('o' | 'O'))
                            && key.modifiers.contains(KeyModifiers::ALT)
                        {
                            app.shell_open_latest_link();
                        } else {
                            app.shell_key(&key);
                        }
                        continue;
                    }
                    if matches!(key.code, KeyCode::Char('f' | 'F'))
                        && key.modifiers.contains(KeyModifiers::ALT)
                        && !app.prompt_active()
                    {
                        app.begin_lyric_search();
                        continue;
                    }
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        quit = true;
                        break;
                    }
                    if is_logs_toggle(&key) {
                        app.toggle_logs();
                        continue;
                    }
                    if hotkeys::binding_matches(&app.opts.hotkeys.reload, &key) {
                        if let Err(err) = app.reload_config() {
                            app.status = format!("config reload failed: {err}");
                        }
                        continue;
                    }
                    if hotkeys::binding_matches(&app.opts.hotkeys.cycle_locale, &key) {
                        app.cycle_locale();
                        continue;
                    }
                    if matches!(key.code, KeyCode::Char('d' | 'D'))
                        && key.modifiers.contains(KeyModifiers::ALT)
                    {
                        app.animation_command(if key.modifiers.contains(KeyModifiers::SHIFT) {
                            crate::anim::AnimationCommand::ResetPolicy
                        } else {
                            crate::anim::AnimationCommand::ToggleDisabled
                        });
                        continue;
                    }
                    if is_shell_toggle(&key) {
                        app.toggle_shell();
                        continue;
                    }
                    if matches!(key.code, KeyCode::Char('n' | 'N'))
                        && key.modifiers.contains(KeyModifiers::ALT)
                    {
                        app.toggle_notifications();
                        continue;
                    }
                    if matches!(key.code, KeyCode::Char('r' | 'R'))
                        && key.modifiers.contains(KeyModifiers::ALT)
                    {
                        if let Some(link) = app.retry_link() {
                            start_link(
                                &mut app,
                                &session,
                                &mut generation,
                                &link,
                                Some(prefetch::cache_path(&link)),
                            );
                            app.status = "retrying current track".into();
                        } else {
                            app.status = "no retryable playback error".into();
                        }
                        continue;
                    }
                    if matches!(key.code, KeyCode::Char('x' | 'X'))
                        && key.modifiers.contains(KeyModifiers::ALT)
                    {
                        start_auth(&mut app, &mut auth, "logout", None);
                        continue;
                    }
                    if key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::CONTROL) {
                        app.focus_next_window();
                        continue;
                    }
                    if matches!(key.code, KeyCode::Char('M'))
                        && key.modifiers.contains(KeyModifiers::ALT)
                        && key.modifiers.contains(KeyModifiers::SHIFT)
                    {
                        app.restore_next_minimized();
                        continue;
                    }
                    if matches!(key.code, KeyCode::Char('m' | 'M'))
                        && key.modifiers.contains(KeyModifiers::ALT)
                    {
                        app.minimize_focused_window();
                        continue;
                    }
                    if key.code == KeyCode::PageUp && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        app.restack_focused_window(true);
                        continue;
                    }
                    if key.code == KeyCode::PageDown
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        app.restack_focused_window(false);
                        continue;
                    }
                    if app.prompt_active() {
                        match key.code {
                            KeyCode::Esc => app.cancel_prompt(),
                            KeyCode::Enter => {
                                let mode = app.take_prompt_mode();
                                if let Some(text) = app.take_prompt() {
                                    submit_prompt(
                                        &mut app,
                                        &mut auth,
                                        &session,
                                        &prefetcher,
                                        &mut generation,
                                        &mut queue,
                                        &text,
                                        mode,
                                    );
                                }
                            }
                            KeyCode::Backspace => app.backspace_prompt(),
                            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.push_prompt(&ch.to_string());
                            }
                            _ => {}
                        }
                    } else if app.library_open() {
                        match key.code {
                            KeyCode::Esc => app.close_library(),
                            KeyCode::Char('q' | 'Q') if is_quit_key(&key) => {
                                quit = true;
                                break;
                            }
                            KeyCode::Up => app.move_library_selection(-1),
                            KeyCode::Down => app.move_library_selection(1),
                            KeyCode::Backspace => app.library_back(),
                            KeyCode::Enter if app.library_has_songs() => {
                                if let Some((index, song)) = app.selected_song() {
                                    play_song(
                                        &mut app,
                                        &session,
                                        &prefetcher,
                                        &mut generation,
                                        index,
                                        &song,
                                    );
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(playlist) = app.selected_playlist() {
                                    session_actions::open_library_item(
                                        &mut app,
                                        &mut auth,
                                        &playlist.kind,
                                        playlist.id,
                                    );
                                }
                            }
                            KeyCode::PageDown if app.library_has_more() => {
                                let value = format!(
                                    "{}:{}:{}",
                                    app.collection_kind(),
                                    app.active_playlist(),
                                    app.collection_offset()
                                );
                                start_auth(&mut app, &mut auth, "songs", Some(&value));
                            }
                            KeyCode::Char('s' | 'S') => app.begin_search(),
                            KeyCode::Char('d' | 'D') => {
                                start_auth(&mut app, &mut auth, "browse", Some("daily"))
                            }
                            KeyCode::Char('h' | 'H') => {
                                app.prepare_library_browse();
                                start_auth(&mut app, &mut auth, "browse", Some("charts"))
                            }
                            KeyCode::Char('a' | 'A') => {
                                app.prepare_library_browse();
                                start_auth(&mut app, &mut auth, "browse", Some("albums"))
                            }
                            KeyCode::Char('r' | 'R') => {
                                app.prepare_library_browse();
                                start_auth(&mut app, &mut auth, "browse", Some("artists"))
                            }
                            KeyCode::Char('i' | 'I') => {
                                start_auth(&mut app, &mut auth, "browse", Some("liked"))
                            }
                            KeyCode::Char('m' | 'M') => {
                                start_auth(&mut app, &mut auth, "browse", Some("radio"))
                            }
                            KeyCode::Char('u' | 'U') => {
                                start_auth(&mut app, &mut auth, "browse", Some("recent"))
                            }
                            KeyCode::Char('f' | 'F') => {
                                if let Some(song_id) = app.current_song_id() {
                                    start_auth(
                                        &mut app,
                                        &mut auth,
                                        "like",
                                        Some(&song_id.to_string()),
                                    );
                                }
                            }
                            KeyCode::Char('v' | 'V') => {
                                if let Some(value) = app.library_collection_value() {
                                    start_auth(&mut app, &mut auth, "detail", Some(&value));
                                }
                            }
                            KeyCode::Char('x' | 'X') => {
                                start_auth(&mut app, &mut auth, "logout", None)
                            }
                            KeyCode::Char('g' | 'G') => {
                                start_auth(&mut app, &mut auth, "qr", None);
                            }
                            _ => {}
                        }
                    } else if is_quit_key(&key) {
                        quit = true;
                        break;
                    } else {
                        match key.code {
                            KeyCode::Char('n' | 'N') => {
                                play_next(
                                    &mut app,
                                    &session,
                                    &prefetcher,
                                    &mut generation,
                                    &mut queue,
                                );
                            }
                            KeyCode::Char('b' | 'B') => {
                                play_previous(&mut app, &session, &prefetcher, &mut generation);
                            }
                            KeyCode::Char('l' | 'L') => {
                                app.begin_library_home();
                                start_auth(&mut app, &mut auth, "playlists", None);
                            }
                            KeyCode::Char('g' | 'G') => {
                                app.open_library();
                                start_auth(&mut app, &mut auth, "qr", None);
                            }
                            _ => {
                                hotkeys::handle_playback_key(&mut app, &session, &key);
                            }
                        }
                    }
                }
                InputEvent::Paste(text) => {
                    if app.shell_open() {
                        app.shell_paste(&text);
                        continue;
                    }
                    if !parse_links(&text).is_empty() {
                        app.cancel_prompt();
                        submit_input(
                            &mut app,
                            &session,
                            &prefetcher,
                            &mut generation,
                            &mut queue,
                            &text,
                        );
                    } else if app.prompt_active() {
                        app.push_prompt(&text);
                    } else {
                        app.status = "pasted text does not contain a NetEase song link".into();
                    }
                }
                InputEvent::Mouse(mouse) => {
                    if app.shell_open() {
                        if !mouse.modifiers.contains(KeyModifiers::SHIFT) && app.shell_mouse(mouse)
                        {
                            continue;
                        }
                        match mouse.kind {
                            MouseEventKind::ScrollUp => {
                                app.shell_scroll(3);
                                continue;
                            }
                            MouseEventKind::ScrollDown => {
                                app.shell_scroll(-3);
                                continue;
                            }
                            _ => {}
                        }
                        if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                            let action = match mouse.kind {
                                MouseEventKind::Down(MouseButton::Left) => PointerAction::Down,
                                MouseEventKind::Drag(MouseButton::Left) => PointerAction::Drag,
                                MouseEventKind::Up(MouseButton::Left) => PointerAction::Up,
                                _ => continue,
                            };
                            if let Some(text) = app.shell_selection(
                                action,
                                mouse.column as usize,
                                mouse.row as usize,
                            ) {
                                guard.copy(&text)?;
                                app.status = format!("copied {} chars", text.chars().count());
                            }
                            continue;
                        }
                    }
                    let action = match mouse.kind {
                        MouseEventKind::Down(MouseButton::Left) => PointerAction::Down,
                        MouseEventKind::Drag(MouseButton::Left) => PointerAction::Drag,
                        MouseEventKind::Up(MouseButton::Left) => PointerAction::Up,
                        _ => continue,
                    };
                    if let Some(control) =
                        app.handle_pointer(action, mouse.column as usize, mouse.row as usize)
                    {
                        match control {
                            ControlAction::TogglePause => {
                                action::toggle_pause(&mut app, &session);
                            }
                            ControlAction::Next => {
                                play_next(
                                    &mut app,
                                    &session,
                                    &prefetcher,
                                    &mut generation,
                                    &mut queue,
                                );
                            }
                            ControlAction::Previous => {
                                play_previous(&mut app, &session, &prefetcher, &mut generation);
                            }
                            ControlAction::Seek(ratio) => {
                                let seconds = ratio * app.meta.duration_ms as f32 / 1000.0;
                                action::seek(&mut app, &session, seconds);
                            }
                        }
                    }
                }
            }
        }
        if quit {
            break;
        }
        let mut ended = false;
        let mut failed = false;
        for update in session.drain() {
            if update.generation == generation {
                ended |= matches!(
                    &update.event,
                    crate::events::BackendEvent::State { state } if state == "ended"
                );
                failed |= matches!(&update.event, crate::events::BackendEvent::Error { .. });
                app.apply_event(update.event);
            }
        }
        let mut auth_done = false;
        if let Some(client) = auth.as_ref() {
            for event in client.drain() {
                auth_done |= matches!(event, AuthEvent::Done);
                app.apply_auth_event(event);
            }
        }
        if auth_done {
            auth = None;
        }
        if ended {
            consecutive_failures = 0;
            play_next(&mut app, &session, &prefetcher, &mut generation, &mut queue);
        } else if failed {
            consecutive_failures += 1;
            let has_alternative = app.playlist_len() > 1 || !queue.is_empty();
            if has_alternative && consecutive_failures < 3 {
                app.status = format!("track unavailable · skipping ({consecutive_failures}/3)");
                play_next(&mut app, &session, &prefetcher, &mut generation, &mut queue);
            } else if !has_alternative {
                consecutive_failures = 0;
            }
        }
        session_actions::preload_next_if_needed(&app, &session, &mut preloaded_url);
        let (new_w, new_h) = TerminalGuard::size()?;
        if new_w != app.buffer.width || new_h != app.buffer.height {
            app.resize(new_w, new_h);
        }
        let frame_start = Instant::now();
        app.tick()?;
        let graphics = app.take_terminal_graphics();
        let report = guard.draw_with_graphics(&mut app.buffer, graphics)?;
        app.observe_terminal(report);
        let elapsed = frame_start.elapsed();
        app.observe_frame(elapsed);
        let interval = frame_interval(app.target_fps());
        if elapsed < interval {
            thread::sleep(interval - elapsed);
        }
    }

    let status = app.status.clone();
    if app.opts.show_stats {
        eprintln!("metrics: {}", app.metrics().to_json());
    }
    drop(guard);
    drop(session);
    if !status.is_empty() {
        eprintln!("backend: {status}");
    }
    if generation > 0 {
        eprintln!("backend log: target/backend.log");
    }
    if auth.is_some() {
        eprintln!("auth log: target/auth.log");
    }
    Ok(())
}
