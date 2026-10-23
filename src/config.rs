use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub animation: String,
    pub palette: String,
    pub charset: String,
    pub fps: u32,
    pub cover_blend: f32,
    pub cover_style: String,
    pub cover_backend: String,
    pub show_lyrics: bool,
    pub show_meta: bool,
    pub show_stats: bool,
    pub show_audio_scope: bool,
    pub show_logs: bool,
    pub cover_width_ratio: f32,
    pub locale: crate::i18n::Locale,
    pub accessibility: Accessibility,
    pub hotkeys: Hotkeys,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Accessibility {
    #[default]
    Standard,
    HighContrast,
    Deuteranopia,
    Tritanopia,
}

impl Accessibility {
    pub fn palette(self) -> Option<&'static str> {
        match self {
            Self::Standard => None,
            Self::HighContrast => Some("high_contrast"),
            Self::Deuteranopia => Some("deuteranopia"),
            Self::Tritanopia => Some("tritanopia"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Hotkeys {
    pub reload: String,
    pub cycle_locale: String,
    pub shell_next_tab: String,
    pub shell_previous_tab: String,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            reload: "ctrl-r".into(),
            cycle_locale: "alt-y".into(),
            shell_next_tab: "alt-]".into(),
            shell_previous_tab: "alt-[".into(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            animation: "adaptive".into(),
            palette: "afterglow".into(),
            charset: "mixed".into(),
            fps: 120,
            cover_blend: 0.62,
            cover_style: "braille".into(),
            cover_backend: "auto".into(),
            show_lyrics: true,
            show_meta: true,
            show_stats: false,
            show_audio_scope: true,
            show_logs: true,
            cover_width_ratio: 0.30,
            locale: crate::i18n::Locale::ZhCn,
            accessibility: Accessibility::Standard,
            hotkeys: Hotkeys::default(),
        }
    }
}

impl Settings {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let Some(path) = path else {
            return Ok(Self::default());
        };
        let text =
            fs::read_to_string(path).with_context(|| format!("read config {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parse config {}", path.display()))
    }

    pub fn sanitize(mut self) -> Self {
        self.fps = self.fps.clamp(90, 240);
        self.cover_blend = self.cover_blend.clamp(0.0, 1.0);
        self.cover_width_ratio = self.cover_width_ratio.clamp(0.15, 0.55);
        self.hotkeys.reload = normalize_binding(&self.hotkeys.reload, "ctrl-r");
        self.hotkeys.cycle_locale = normalize_binding(&self.hotkeys.cycle_locale, "alt-y");
        self.hotkeys.shell_next_tab = normalize_binding(&self.hotkeys.shell_next_tab, "alt-]");
        self.hotkeys.shell_previous_tab =
            normalize_binding(&self.hotkeys.shell_previous_tab, "alt-[");
        self
    }
}

fn normalize_binding(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.into()
    } else {
        value.trim().to_ascii_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let s = Settings::default().sanitize();
        assert!(s.fps >= 5);
        assert!((0.0..=1.0).contains(&s.cover_blend));
        assert!(s.show_logs);
        assert!((s.cover_width_ratio - 0.30).abs() < 1.0e-6);
        assert_eq!(s.cover_backend, "auto");
        assert_eq!(s.accessibility, Accessibility::Standard);
    }

    #[test]
    fn locale_accessibility_and_hotkeys_parse_from_toml() {
        let settings = toml::from_str::<Settings>(
            r#"
locale = "ja"
accessibility = "deuteranopia"
fps = 60
[hotkeys]
reload = "Ctrl-R"
cycle_locale = "Alt-Y"
shell_next_tab = "Ctrl-PageDown"
shell_previous_tab = "Ctrl-PageUp"
"#,
        )
        .unwrap()
        .sanitize();
        assert_eq!(settings.locale, crate::i18n::Locale::Ja);
        assert_eq!(settings.accessibility, Accessibility::Deuteranopia);
        assert_eq!(settings.hotkeys.reload, "ctrl-r");
        assert_eq!(settings.accessibility.palette(), Some("deuteranopia"));
    }
}
