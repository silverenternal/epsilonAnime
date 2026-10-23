use self::search_prompt::PromptMode;
use crate::anim::bind::AnimParams;
use crate::anim::{AnimationKind, AudioParams, FrameCtx, make_engine};
use crate::audio_scope::AudioScope;
use crate::config::{Accessibility, Hotkeys};
use crate::controls::{self, ControlAction};
use crate::cover::Cover;
use crate::cover_art::{CoverArt, CoverStyle};
use crate::cover_loader::CoverLoader;
use crate::danmaku::Danmaku;
use crate::events::BackendEvent;
use crate::glyphs::Charset;
use crate::i18n::Locale;
use crate::kitty::CoverBackend;
use crate::layout::{Layout, Panel, PointerAction};
use crate::library::LibraryState;
use crate::logs::LogStore;
use crate::lyrics::LyricState;
use crate::notifications::NotificationCenter;
use crate::playback::PlaybackQueue;
use crate::render::Buffer;
use crate::seed::visual_seed;
use crate::shell::ShellWorkspace;
use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod animation_ui;
mod auth_ui;
mod help_ui;
mod lyrics_ui;
mod notifications_ui;
mod playlist;
mod preferences_ui;
mod quality;
pub(crate) mod search_prompt;
mod shell_ui;
mod shutdown;
mod ui_draw;
mod window_ui;

#[derive(Debug, Clone)]
pub struct AppOptions {
    pub kind: AnimationKind,
    pub palette: String,
    pub charset: Charset,
    pub fps: u32,
    pub cover_blend: f32,
    pub show_lyrics: bool,
    pub show_meta: bool,
    pub cover_ratio: f32,
    pub cover_style: CoverStyle,
    pub cover_backend: CoverBackend,
    pub demo: bool,
    pub show_stats: bool,
    pub show_audio_scope: bool,
    pub show_logs: bool,
    pub locale: Locale,
    pub accessibility: Accessibility,
    pub hotkeys: Hotkeys,
    pub config_path: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct Meta {
    pub title: String,
    pub artist: String,
    pub duration_ms: u64,
}

pub struct App {
    pub opts: AppOptions,
    pub buffer: Buffer,
    engine: Box<dyn crate::anim::Animation>,
    audio: AudioParams,
    audio_t: f32,
    params: AnimParams,
    pub meta: Meta,
    pub lyric: LyricState,
    cover: Option<Arc<Cover>>,
    cover_art: Option<CoverArt>,
    derived_palette: Vec<[u8; 3]>,
    visual_t: f32,
    last: Instant,
    pub status: String,
    pub finished: bool,
    layout: Layout,
    fps: f32,
    fps_window: Instant,
    fps_frames: u32,
    prompt: Option<String>,
    prompt_mode: PromptMode,
    cover_loader: CoverLoader,
    cover_request: u64,
    cover_next_id: u64,
    cover_render_rect: Option<crate::layout::Rect>,
    last_lyric_index: usize,
    lyric_change: f32,
    exposure: f32,
    lyric_seed: u64,
    bpm_bucket: i32,
    danmaku: Danmaku,
    audio_scope: AudioScope,
    library: LibraryState,
    logs: LogStore,
    notifications: NotificationCenter,
    notifications_view: bool,
    retry_link: Option<String>,
    pending_graphics: Vec<u8>,
    kitty_signature: Option<(u64, crate::layout::Rect)>,
    kitty_cache: crate::kitty::KittyCache,
    kitty_failed: bool,
    kitty_retried: bool,
    frame_ms: f32,
    sample_ms: f32,
    encode_ms: f32,
    terminal_wait_ms: f32,
    low_quality: bool,
    target_fps: f32,
    shell: ShellWorkspace,
    playback: PlaybackQueue,
    paused: bool,
    playlist_pos: usize,
    help_page: Option<usize>,
}

impl App {
    pub fn new(opts: AppOptions, width: usize, height: usize) -> Self {
        let demo = opts.demo;
        let cover_ratio = opts.cover_ratio;
        let target_fps = opts.fps as f32;
        let engine = make_engine(opts.kind, width, height);
        let mut meta = Meta::default();
        let mut lyric = LyricState::load();
        if demo {
            meta.title = "happybirthday".into();
            meta.artist = "terminal math lab".into();
            meta.duration_ms = 180_000;
            lyric.set(
                0,
                "Synthetic demo — press q to quit".into(),
                "no network backend required".into(),
                0.0,
            );
        }
        let (initial_cover, initial_palette) = if demo {
            let cover = Arc::new(Cover::demo(240, 240));
            let palette = cover.palette();
            (Some(cover), palette)
        } else {
            (None, Vec::new())
        };
        let initial_art = initial_cover.as_ref().map(|cover| CoverArt::new(cover));
        let playback = PlaybackQueue::load();
        let playlist_pos = playback.position();
        let library = LibraryState {
            songs: playback.songs().to_vec(),
            song_selected: playlist_pos,
            ..Default::default()
        };
        let mut layout = Layout::new(width, height, cover_ratio);
        layout.set_logs_visible(opts.show_logs);
        layout.load_state();
        Self {
            opts,
            buffer: Buffer::new(width, height),
            engine,
            audio: AudioParams::default(),
            audio_t: 0.0,
            params: AnimParams::default(),
            meta,
            lyric,
            cover: initial_cover,
            cover_art: initial_art,
            derived_palette: initial_palette,
            visual_t: 0.0,
            last: Instant::now(),
            status: String::new(),
            finished: false,
            layout,
            fps: 0.0,
            fps_window: Instant::now(),
            fps_frames: 0,
            prompt: None,
            prompt_mode: PromptMode::Url,
            cover_loader: CoverLoader::new(),
            cover_request: 0,
            cover_next_id: 1,
            cover_render_rect: None,
            last_lyric_index: 0,
            lyric_change: 0.0,
            exposure: 1.0,
            lyric_seed: 0,
            bpm_bucket: 0,
            danmaku: Danmaku::new(),
            audio_scope: AudioScope::new(),
            library,
            logs: LogStore::new(),
            notifications: NotificationCenter::new(),
            notifications_view: false,
            retry_link: None,
            pending_graphics: Vec::new(),
            kitty_signature: None,
            kitty_cache: crate::kitty::KittyCache::new(),
            kitty_failed: false,
            kitty_retried: false,
            frame_ms: 33.0,
            sample_ms: 0.0,
            encode_ms: 0.0,
            terminal_wait_ms: 0.0,
            low_quality: false,
            target_fps,
            shell: ShellWorkspace::new(),
            playback,
            paused: false,
            playlist_pos,
            help_page: None,
        }
    }

    pub fn take_terminal_graphics(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pending_graphics)
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.buffer.resize(width, height);
        self.engine = make_engine(self.opts.kind, width, height);
        self.layout.reset(width, height, self.opts.cover_ratio);
        self.layout.set_logs_visible(self.opts.show_logs);
    }

    pub fn handle_pointer(
        &mut self,
        action: PointerAction,
        x: usize,
        y: usize,
    ) -> Option<ControlAction> {
        if self.layout.shell_visible() {
            match action {
                PointerAction::Down if self.layout.rect(Panel::Shell).contains(x, y) => {
                    self.layout.begin_drag(x, y);
                }
                PointerAction::Drag => self.layout.drag_to(x, y),
                PointerAction::Up => self.layout.end_drag(),
                PointerAction::Down => {}
            }
            return None;
        }
        if self.library.open {
            self.library
                .handle_pointer(action, x, y, self.buffer.width, self.buffer.height);
            return None;
        }
        if action == PointerAction::Down && self.layout.is_resize_handle(x, y) {
            self.layout.begin_drag(x, y);
            return None;
        }
        if action == PointerAction::Down
            && let Some(control) = controls::action_at(self.layout.rect(Panel::Lyrics), x, y)
        {
            return Some(control);
        }
        match action {
            PointerAction::Down => self.layout.begin_drag(x, y),
            PointerAction::Drag => self.layout.drag_to(x, y),
            PointerAction::Up => self.layout.end_drag(),
        }
        None
    }

    pub fn toggle_pause(&mut self) -> bool {
        self.paused = !self.paused;
        self.paused
    }

    pub fn start_session(&mut self) {
        self.opts.demo = false;
        self.meta = Meta::default();
        self.lyric = LyricState::load();
        self.cover = None;
        self.cover_art = None;
        self.derived_palette.clear();
        self.audio = AudioParams::default();
        self.audio_t = 0.0;
        self.params = AnimParams::default();
        self.visual_t = 0.0;
        self.last = Instant::now();
        self.finished = false;
        self.status = "loading song…".into();
        self.cover_request = 0;
        self.cover_render_rect = None;
        self.last_lyric_index = 0;
        self.lyric_change = 0.0;
        self.exposure = 1.0;
        self.lyric_seed = 0;
        self.bpm_bucket = 0;
        self.danmaku.clear();
        self.audio_scope.reset();
        self.paused = false;
    }

    pub fn apply_event(&mut self, event: BackendEvent) {
        let event_kind = event.kind();
        match event {
            BackendEvent::Meta {
                title,
                artist,
                duration_ms,
                cover_path: _,
                cover_cols: _,
                cover_rows: _,
                ..
            } => {
                self.status.clear();
                self.logs.push(format!("track: {title} / {artist}"));
                self.update_current_song_meta(&title, &artist, duration_ms);
                self.meta = Meta {
                    title,
                    artist,
                    duration_ms,
                };
            }
            BackendEvent::CoverReady {
                path,
                cols,
                rows,
                particle_path,
                particle_count: _,
            } => {
                self.logs.push(format!("cover: {cols}x{rows}"));
                self.cover_next_id = self.cover_next_id.wrapping_add(1).max(1);
                self.cover_request = self.cover_next_id;
                self.kitty_failed = false;
                self.kitty_retried = false;
                self.cover_loader
                    .request(self.cover_request, path, cols, rows, particle_path);
            }
            BackendEvent::Comments { comments } => self.danmaku.set_comments(comments),
            BackendEvent::Features {
                t,
                rms,
                bass,
                mid,
                treble,
                flux,
                onset,
                bpm,
                spectrum,
                centroid,
                spread,
                flatness,
                rolloff,
                zcr,
                crest,
                stereo_width,
            } => {
                self.audio_t = t;
                let mut bins = [0.0; 16];
                for (slot, value) in bins.iter_mut().zip(spectrum) {
                    *slot = value.clamp(0.0, 1.0);
                }
                self.audio = AudioParams {
                    rms,
                    bass,
                    mid,
                    treble,
                    flux,
                    onset,
                    pulse: self.audio.pulse,
                    bpm,
                    spectrum: bins,
                    centroid,
                    spread,
                    flatness,
                    rolloff,
                    zcr,
                    crest,
                    stereo_width,
                };
                let bucket = (bpm / 4.0).round() as i32;
                if bucket != self.bpm_bucket {
                    self.bpm_bucket = bucket;
                    if !self.lyric.text.is_empty() {
                        self.lyric_seed =
                            visual_seed(&self.lyric.text, &self.meta.title, &self.meta.artist, bpm);
                    }
                }
            }
            BackendEvent::Lyric {
                index,
                text,
                translation,
                romaji,
                segments,
                offset,
                progress,
            } => {
                self.lyric_seed =
                    visual_seed(&text, &self.meta.title, &self.meta.artist, self.audio.bpm);
                if index != self.last_lyric_index {
                    self.last_lyric_index = index;
                    self.lyric_change = 1.0;
                }
                self.lyric
                    .set_rich(index, text, translation, romaji, segments, offset, progress);
            }
            BackendEvent::LyricsReady { lines } => self.load_lyric_timeline(lines),
            BackendEvent::State { state } => {
                if state == "paused" {
                    self.paused = true;
                    self.status = "paused".into();
                } else if state == "playing" {
                    self.paused = false;
                    self.status.clear();
                } else if state == "loading" {
                    self.status = "loading song…".into();
                    self.finished = false;
                } else if state == "seeking" {
                    self.status = "seeking…".into();
                } else if matches!(state.as_str(), "ended" | "backend_eof") {
                    self.logs.push(format!("backend: {state}"));
                    self.finished = true;
                    if self.status.is_empty() {
                        self.status = state;
                    }
                }
            }
            BackendEvent::Playback {
                position,
                volume,
                muted,
                paused,
            } => self.apply_playback_state(position, volume, muted, paused),
            BackendEvent::Error {
                message,
                id,
                scope,
                level: _,
                retryable,
                ..
            } => {
                let scope = if scope.is_empty() { event_kind } else { &scope };
                if retryable {
                    let mut notification =
                        crate::notifications::NotificationCenter::classify(scope, &message);
                    if !id.is_empty() {
                        notification.code = id;
                    }
                    notification.retryable = true;
                    self.logs.push(format!("{scope}: {message}"));
                    self.push_notification(notification);
                } else {
                    self.notify(scope, &message);
                }
                self.status = format!("{event_kind}: {message}");
            }
        }
    }

    pub fn update_demo_audio(&mut self, t: f32) {
        self.audio_t = t;
        let beat = (t * std::f32::consts::TAU / 1.7).sin().max(0.0);
        self.audio = AudioParams {
            rms: 0.36 + beat * 0.22,
            bass: beat,
            mid: 0.4 + 0.2 * (t * 0.7).sin(),
            treble: 0.3 + 0.3 * (t * 1.3).cos().abs(),
            flux: beat,
            onset: if beat > 0.985 { 1.0 } else { 0.0 },
            pulse: self.audio.pulse,
            bpm: 118.0,
            spectrum: std::array::from_fn(|i| {
                (0.15 + 0.75 * (t * (0.7 + i as f32 * 0.11) + i as f32).sin().abs())
                    * (1.0 - i as f32 / 20.0)
            }),
            centroid: 0.4 + 0.15 * (t * 0.2).sin(),
            spread: 0.35,
            flatness: 0.28,
            rolloff: 0.62,
            zcr: 0.18,
            crest: 0.35,
            stereo_width: 0.45,
        };
        let line = ((t / 4.0) as usize) % 4;
        let lines = [
            (
                "Synthetic demo — press q to quit",
                "no network backend required",
            ),
            (
                "Math is the animation engine",
                "superformula · attractors · PDE",
            ),
            (
                "Brightness uses glyph density + palette",
                "each cell is a tiny computation",
            ),
            (
                "Macros generated this registry",
                "kernel DSL normalized at compile time",
            ),
        ];
        self.lyric.set(
            line,
            lines[line].0.into(),
            lines[line].1.into(),
            (t % 4.0) / 4.0,
        );
        self.lyric_seed = visual_seed(
            lines[line].0,
            &self.meta.title,
            &self.meta.artist,
            self.audio.bpm,
        );
    }
    pub fn tick(&mut self) -> Result<()> {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().clamp(0.0, 0.1);
        self.last = now;
        if !self.paused {
            self.visual_t += dt;
        }
        let t = self.visual_t;
        self.fps_frames += 1;
        let window = now.duration_since(self.fps_window).as_secs_f32();
        if window >= 0.5 {
            self.fps = self.fps_frames as f32 / window;
            self.fps_frames = 0;
            self.fps_window = now;
        }
        if self.opts.demo {
            self.update_demo_audio(t);
        }
        self.params.update(&self.audio, dt);
        if let Some(segment) = self.lyric.update_active(self.audio_t) {
            self.lyric_seed =
                visual_seed(segment, &self.meta.title, &self.meta.artist, self.audio.bpm);
        }
        self.logs.poll();
        self.shell.drain();
        self.audio.pulse = self.params.pulse;
        self.lyric_change = (self.lyric_change - dt * 0.65).max(0.0);
        for loaded in self.cover_loader.drain() {
            if loaded.id != self.cover_request {
                continue;
            }
            match loaded.result {
                Ok(cover) => {
                    let cover = Arc::new(cover);
                    self.derived_palette = cover.palette();
                    self.cover_art = Some(CoverArt::new(&cover));
                    self.cover_render_rect = None;
                    self.cover = Some(cover);
                }
                Err(err) => self.status = format!("cover: {err}"),
            }
        }
        let audio = self.audio;
        let params = self.params;
        let cover = self.cover.clone();
        let ctx = FrameCtx {
            width: self.buffer.width,
            height: self.buffer.height,
            t,
            dt,
            audio: &audio,
            params: &params,
            cover,
            lyric_index: self.last_lyric_index,
            lyric_progress: self.lyric.progress,
            lyric_change: self.lyric_change,
            lyric_seed: self.lyric_seed,
            low_quality: self.low_quality,
        };
        self.engine.update(&ctx);
        if let Some(art) = &mut self.cover_art {
            art.update(&ctx);
        }
        let danmaku_bounds = self.danmaku_bounds();
        self.danmaku.update(&ctx, &audio, danmaku_bounds);
        self.audio_scope.update(&audio, dt);
        self.render(&ctx, t);
        Ok(())
    }
}

pub fn frame_interval(fps: u32) -> Duration {
    Duration::from_secs_f32(1.0 / fps.clamp(90, 240) as f32)
}
