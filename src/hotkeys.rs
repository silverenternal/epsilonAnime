use crate::anim::AnimationCommand;
use crate::app::App;
use crate::player_actions as action;
use crate::session::SessionManager;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn binding_matches(binding: &str, key: &KeyEvent) -> bool {
    let binding = binding.trim().to_ascii_lowercase();
    let code = match key.code {
        KeyCode::Char(ch) => ch.to_ascii_lowercase().to_string(),
        KeyCode::Enter => "enter".into(),
        KeyCode::Esc => "esc".into(),
        KeyCode::Tab => "tab".into(),
        KeyCode::Up => "up".into(),
        KeyCode::Down => "down".into(),
        KeyCode::Left => "left".into(),
        KeyCode::Right => "right".into(),
        KeyCode::PageUp => "pageup".into(),
        KeyCode::PageDown => "pagedown".into(),
        KeyCode::Home => "home".into(),
        KeyCode::End => "end".into(),
        KeyCode::Backspace => "backspace".into(),
        KeyCode::Delete => "delete".into(),
        KeyCode::F(number) => format!("f{number}"),
        _ => return false,
    };
    let mut parts = binding.rsplit('-');
    if parts.next() != Some(code.as_str()) {
        return false;
    }
    let mut required = KeyModifiers::NONE;
    for modifier in parts {
        required |= match modifier {
            "ctrl" | "control" => KeyModifiers::CONTROL,
            "alt" | "option" => KeyModifiers::ALT,
            "shift" => KeyModifiers::SHIFT,
            _ => return false,
        };
    }
    let relevant = KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT;
    key.modifiers & relevant == required
}

pub fn handle_playback_key(app: &mut App, session: &SessionManager, key: &KeyEvent) -> bool {
    if key
        .modifiers
        .intersects(KeyModifiers::ALT | KeyModifiers::CONTROL)
    {
        return false;
    }
    match key.code {
        KeyCode::Char(' ') => action::toggle_pause(app, session),
        KeyCode::Left => action::seek_relative(app, session, -5.0),
        KeyCode::Right => action::seek_relative(app, session, 5.0),
        KeyCode::Char('-' | '_') => action::adjust_volume(app, session, -0.05),
        KeyCode::Char('=' | '+') => action::adjust_volume(app, session, 0.05),
        KeyCode::Char('m' | 'M') => action::toggle_mute(app, session),
        KeyCode::Char('p' | 'P') => action::cycle_mode(app),
        KeyCode::Char('t' | 'T') => app.toggle_translation(),
        KeyCode::Char('o' | 'O') => app.toggle_romanization(),
        KeyCode::Char(',') => app.shift_lyric_offset(session, -0.1),
        KeyCode::Char('.') => app.shift_lyric_offset(session, 0.1),
        KeyCode::Char(']') => app.animation_command(AnimationCommand::Next),
        KeyCode::Char('[') => app.animation_command(AnimationCommand::Previous),
        KeyCode::Char('f' | 'F') => app.animation_command(AnimationCommand::ToggleFavorite),
        KeyCode::Char('j' | 'J') => app.animation_command(AnimationCommand::ToggleLock),
        KeyCode::Char('w') => app.animation_command(AnimationCommand::WeightUp),
        KeyCode::Char('W') => app.animation_command(AnimationCommand::WeightDown),
        KeyCode::Char('r' | 'R') => app.reset_layout(),
        KeyCode::Tab => app.cycle_layout(),
        KeyCode::Char('c' | 'C') => app.cycle_cover_style(),
        KeyCode::Char('i' | 'I') => app.begin_prompt(),
        KeyCode::Char('/') => app.begin_search(),
        KeyCode::Up
            if key
                .modifiers
                .contains(crossterm::event::KeyModifiers::SHIFT) =>
        {
            app.scroll_lyrics(-2)
        }
        KeyCode::Down
            if key
                .modifiers
                .contains(crossterm::event::KeyModifiers::SHIFT) =>
        {
            app.scroll_lyrics(2)
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_configurable_bindings() {
        let key = KeyEvent::new(KeyCode::Char('R'), KeyModifiers::CONTROL);
        assert!(binding_matches("ctrl-r", &key));
        assert!(!binding_matches("alt-r", &key));
        assert!(binding_matches(
            "shift-up",
            &KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT)
        ));
        assert!(binding_matches(
            "ctrl-pagedown",
            &KeyEvent::new(KeyCode::PageDown, KeyModifiers::CONTROL)
        ));
    }
}
