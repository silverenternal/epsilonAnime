use crate::anim::AnimationKind;
use crate::app::AppOptions;
use crate::cli::Cli;
use crate::config::Settings;
use crate::cover_art::CoverStyle;
use crate::generated::palette_by_name;
use crate::glyphs::Charset;
use crate::kitty::CoverBackend;
use anyhow::{Context, Result};

pub fn options(cli: &Cli) -> Result<AppOptions> {
    let mut settings = Settings::load(cli.config.as_deref())?.sanitize();
    cli.apply_overrides(&mut settings);
    let kind = AnimationKind::parse(&settings.animation).with_context(|| {
        format!(
            "unknown animation `{}`; try --list-animations",
            settings.animation
        )
    })?;
    anyhow::ensure!(
        palette_by_name(&settings.palette).is_some(),
        "unknown palette `{}`; try --list-palettes",
        settings.palette
    );
    let charset = Charset::parse(&settings.charset).with_context(|| {
        format!(
            "unknown charset `{}`; try --list-charsets",
            settings.charset
        )
    })?;
    let cover_style = CoverStyle::parse(&settings.cover_style).with_context(|| {
        format!(
            "unknown cover style `{}`; choose braille, hybrid or halfblock",
            settings.cover_style
        )
    })?;
    let cover_backend = CoverBackend::parse(&settings.cover_backend).with_context(|| {
        format!(
            "unknown cover backend `{}`; choose auto, cells or kitty",
            settings.cover_backend
        )
    })?;
    Ok(AppOptions {
        kind,
        palette: settings.palette,
        charset,
        fps: settings.fps,
        cover_blend: settings.cover_blend,
        show_lyrics: settings.show_lyrics,
        show_meta: settings.show_meta,
        cover_ratio: settings.cover_width_ratio,
        cover_style,
        cover_backend,
        demo: cli.demo,
        show_stats: settings.show_stats,
        show_audio_scope: settings.show_audio_scope,
        show_logs: settings.show_logs,
        locale: settings.locale,
        accessibility: settings.accessibility,
        hotkeys: settings.hotkeys,
        config_path: cli.config.clone(),
    })
}
