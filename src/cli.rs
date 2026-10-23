use crate::config::Settings;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "epsilonAnime",
    version,
    about = "Terminal ASCII music visualizer powered by NetEase Cloud Music"
)]
pub struct Cli {
    /// Initial NetEase song URL; links can also be pasted after launch
    pub url: Option<String>,

    /// Run without network/audio using synthetic data
    #[arg(long)]
    pub demo: bool,

    /// Animation kernel (see --list-animations)
    #[arg(long)]
    pub animation: Option<String>,

    /// Palette name (see --list-palettes)
    #[arg(long)]
    pub palette: Option<String>,

    /// Glyph profile: ascii, blocks, quadrants, braille, mixed
    #[arg(long)]
    pub charset: Option<String>,

    /// Frames per second
    #[arg(long)]
    pub fps: Option<u32>,

    /// TOML config path
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Cover mix strength 0..1
    #[arg(long)]
    pub cover_blend: Option<f32>,

    /// Cover renderer: braille, hybrid, halfblock
    #[arg(long)]
    pub cover_style: Option<String>,

    /// Cover backend: auto, cells, kitty
    #[arg(long)]
    pub cover_backend: Option<String>,

    /// Hide the lyric bar
    #[arg(long)]
    pub no_lyrics: bool,

    /// Hide the title bar
    #[arg(long)]
    pub no_meta: bool,

    /// Show FPS and dirty-tile renderer statistics
    #[arg(long)]
    pub stats: bool,

    /// Hide the dedicated 2D spectrum history panel
    #[arg(long, alias = "no-audio3d")]
    pub no_audio_scope: bool,

    /// Hide the live log panel
    #[arg(long)]
    pub no_logs: bool,

    /// Print animation names and exit
    #[arg(long)]
    pub list_animations: bool,

    /// Print palette names and exit
    #[arg(long)]
    pub list_palettes: bool,

    /// Print glyph profiles and exit
    #[arg(long)]
    pub list_charsets: bool,

    /// Benchmark all kernels at 160x48 and exit
    #[arg(long, value_name = "FRAMES")]
    pub bench: Option<u32>,
}

impl Cli {
    pub fn apply_overrides(&self, settings: &mut Settings) {
        if let Some(animation) = &self.animation {
            settings.animation = animation.clone();
        }
        if let Some(palette) = &self.palette {
            settings.palette = palette.clone();
        }
        if let Some(charset) = &self.charset {
            settings.charset = charset.clone();
        }
        if let Some(fps) = self.fps {
            settings.fps = fps.clamp(90, 240);
        }
        if let Some(blend) = self.cover_blend {
            settings.cover_blend = blend.clamp(0.0, 1.0);
        }
        if let Some(style) = &self.cover_style {
            settings.cover_style = style.clone();
        }
        if let Some(backend) = &self.cover_backend {
            settings.cover_backend = backend.clone();
        }
        settings.show_lyrics &= !self.no_lyrics;
        settings.show_meta &= !self.no_meta;
        settings.show_stats |= self.stats;
        settings.show_audio_scope &= !self.no_audio_scope;
        settings.show_logs &= !self.no_logs;
    }
}
