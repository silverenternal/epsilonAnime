use super::App;
use crate::anim::AnimationKind;
use crate::config::Settings;
use anyhow::{Result, anyhow};

impl App {
    pub fn cycle_locale(&mut self) {
        self.opts.locale = self.opts.locale.next();
        self.status = format!("locale: {}", self.opts.locale.name());
    }

    pub fn reload_config(&mut self) -> Result<()> {
        let path = self
            .opts
            .config_path
            .as_deref()
            .ok_or_else(|| anyhow!("no --config path was supplied"))?;
        let settings = Settings::load(Some(path))?.sanitize();
        let kind = AnimationKind::parse(&settings.animation)
            .ok_or_else(|| anyhow!("unknown animation `{}`", settings.animation))?;
        if kind != self.opts.kind {
            self.opts.kind = kind;
            self.engine = crate::anim::make_engine(kind, self.buffer.width, self.buffer.height);
        }
        self.opts.locale = settings.locale;
        self.opts.accessibility = settings.accessibility;
        self.opts.hotkeys = settings.hotkeys;
        self.opts.palette = settings.palette;
        self.opts.fps = settings.fps;
        self.opts.cover_blend = settings.cover_blend;
        self.opts.show_lyrics = settings.show_lyrics;
        self.opts.show_meta = settings.show_meta;
        self.opts.show_stats = settings.show_stats;
        self.opts.show_audio_scope = settings.show_audio_scope;
        self.opts.show_logs = settings.show_logs;
        self.opts.cover_ratio = settings.cover_width_ratio;
        if let Some(style) = crate::cover_art::CoverStyle::parse(&settings.cover_style) {
            self.opts.cover_style = style;
        }
        if let Some(backend) = crate::kitty::CoverBackend::parse(&settings.cover_backend) {
            self.opts.cover_backend = backend;
        }
        if let Some(charset) = crate::glyphs::Charset::parse(&settings.charset) {
            self.opts.charset = charset;
        }
        self.target_fps = settings.fps as f32;
        self.shell.set_tab_hotkeys(
            &self.opts.hotkeys.shell_previous_tab,
            &self.opts.hotkeys.shell_next_tab,
        );
        self.layout.set_logs_visible(settings.show_logs);
        self.layout
            .reset(self.buffer.width, self.buffer.height, self.opts.cover_ratio);
        self.status = "configuration reloaded".into();
        Ok(())
    }
}
