use super::App;
use super::search_prompt::PromptMode;
use crate::anim::FrameCtx;
use crate::background::{RenderOptions, render_background};
use crate::controls;
use crate::frame::{self, SurfaceStyle};
use crate::layout::{self, Panel};
use crate::library;
use crate::lyrics::fmt_time;
use crate::render::{Rgb, color_for, find_palette, muted_text, readable_text};
use crate::ui;
use crossterm::event::KeyEvent;
use std::time::Instant;

impl App {
    pub fn toggle_logs(&mut self) {
        self.opts.show_logs = !self.opts.show_logs;
        self.layout.set_logs_visible(self.opts.show_logs);
        self.layout.save_state();
        let state = if self.opts.show_logs {
            "shown"
        } else {
            "hidden"
        };
        self.status = format!("logs: {state} · F3");
        self.logs.push(format!("ui: logs {state}"));
    }

    pub fn toggle_shell(&mut self) {
        if self.shell.is_open() {
            self.shell.close();
            self.layout.set_shell_visible(false);
            self.layout.save_state();
            self.status = "shell hidden · F4 to resume".into();
            self.logs.push("shell: hidden (process kept alive)");
            return;
        }
        let rect = self.layout.rect(Panel::Shell);
        let inner = rect.inner();
        self.shell.set_tab_hotkeys(
            &self.opts.hotkeys.shell_previous_tab,
            &self.opts.hotkeys.shell_next_tab,
        );
        match self.shell.open(inner.w, inner.h) {
            Ok(()) => {
                self.layout.set_shell_visible(true);
                self.layout.save_state();
                self.status = "shell active · F4 to hide · Ctrl-D exits".into();
                self.logs.push("shell: opened PTY session");
            }
            Err(err) => {
                self.status = format!("shell: {err}");
                self.logs.push(format!("shell: open failed: {err}"));
            }
        }
    }

    pub fn shell_open(&self) -> bool {
        self.shell.is_open()
    }

    pub fn shell_key(&mut self, key: &KeyEvent) {
        if let Err(err) = self.shell.send_key(key) {
            self.logs.push(format!("shell: {err}"));
            self.status = err;
        }
    }

    pub fn shell_paste(&mut self, text: &str) {
        if let Err(err) = self.shell.send_text(text) {
            self.logs.push(format!("shell: {err}"));
            self.status = err;
        }
    }

    pub fn cycle_layout(&mut self) {
        let preset = self.layout.cycle_preset();
        self.status = format!("layout: {preset} · Tab to cycle");
    }

    pub fn cycle_cover_style(&mut self) {
        self.opts.cover_style = self.opts.cover_style.next();
        self.status = format!("cover: {} · c to cycle", self.opts.cover_style.name());
    }

    pub(super) fn render(&mut self, ctx: &FrameCtx<'_>, t: f32) {
        let palette_name = self
            .opts
            .accessibility
            .palette()
            .unwrap_or(&self.opts.palette);
        let fallback = find_palette(palette_name);
        let palette_vec = if self.opts.accessibility == crate::config::Accessibility::Standard
            && self.derived_palette.len() >= 3
        {
            let count = self.derived_palette.len().max(fallback.len());
            (0..count)
                .map(|i| {
                    Rgb::from_slice(self.derived_palette[i % self.derived_palette.len()])
                        .lerp(Rgb::from_slice(fallback[i % fallback.len()]), 0.30)
                        .to_array()
                })
                .collect()
        } else {
            fallback.to_vec()
        };
        let palette = palette_vec.as_slice();
        let occluded = self.occlusion_mask();
        let sample_started = Instant::now();
        self.exposure = render_background(
            self.engine.as_ref(),
            &mut self.buffer,
            ctx,
            RenderOptions {
                palette,
                charset: self.opts.charset,
                exposure: self.exposure,
                occluded: &occluded,
                low_quality: self.low_quality,
            },
        );
        self.sample_ms = self.sample_ms * 0.85 + sample_started.elapsed().as_secs_f32() * 150.0;
        let exposure = self.exposure;

        let danmaku_bounds = self.danmaku_bounds();
        self.danmaku
            .draw(&mut self.buffer, palette, t, danmaku_bounds);
        let draw_order = self.layout.draw_order();
        if !draw_order.contains(&Panel::Cover) {
            self.clear_kitty_cover();
        }
        for panel in draw_order {
            match panel {
                Panel::Meta if self.opts.show_meta => self.draw_meta(t, palette),
                Panel::Cover => self.draw_cover_panel(palette, t, exposure),
                Panel::AudioScope if self.opts.show_audio_scope => {
                    self.draw_audio_scope_panel(palette)
                }
                Panel::Logs if self.opts.show_logs => self.draw_logs_panel(palette),
                Panel::Lyrics if self.opts.show_lyrics => self.draw_lyrics_panel(palette),
                _ => {}
            }
        }
        if self.opts.show_stats {
            let (danmaku_active, danmaku_total) = self.danmaku.counts();
            let stats = ui::Stats {
                fps: self.fps,
                rms: self.audio.rms,
                bpm: self.audio.bpm,
                exposure,
                danmaku_active,
                danmaku_total,
                label: self.engine.label(),
                sample_ms: self.sample_ms,
                encode_ms: self.encode_ms,
                terminal_wait_ms: self.terminal_wait_ms,
                palette,
            };
            ui::draw_stats(&mut self.buffer, &stats);
        }
        let live = if self.audio.bpm > 20.0 {
            format!(
                "MATH {} · {:.0}bpm · rms {:.2}",
                self.engine.label(),
                self.audio.bpm,
                self.audio.rms
            )
        } else {
            format!("MATH {}", self.engine.label())
        };
        let status = if self.status.is_empty() {
            format!("{live} │ ] 下一场")
        } else {
            format!("{live} │ {}", self.status)
        };
        ui::draw_status(&mut self.buffer, &status, palette);
        if let Some(prompt) = self.prompt.clone() {
            let label = match self.prompt_mode {
                PromptMode::Url => "LINK",
                PromptMode::SongSearch => "SEARCH",
                PromptMode::LyricSearch => "LYRIC",
            };
            ui::draw_prompt(&mut self.buffer, &prompt, palette, label);
        }
        if self.library.open {
            library::draw(
                &mut self.buffer,
                &mut self.library,
                palette,
                self.opts.locale,
            );
        }
        if self.shell.is_open() {
            let rect = self.layout.rect(Panel::Shell);
            self.shell.draw(&mut self.buffer, rect, palette);
        }
        self.layout.draw_drag_preview(&mut self.buffer, palette);
        self.draw_help(palette);
    }

    fn occlusion_mask(&self) -> Vec<bool> {
        let mut mask = vec![false; self.buffer.width * self.buffer.height];
        let panels = [
            (Panel::Meta, self.opts.show_meta),
            (Panel::Cover, true),
            (Panel::AudioScope, self.opts.show_audio_scope),
            (Panel::Logs, self.opts.show_logs),
            (Panel::Lyrics, self.opts.show_lyrics),
        ];
        for (panel, visible) in panels {
            if !visible
                || self.layout.is_minimized(panel)
                || self.layout.glass_for(panel)
                || self.layout.under_glass(panel)
            {
                continue;
            }
            let rect = self.layout.rect(panel);
            for y in rect.y..(rect.y + rect.h).min(self.buffer.height) {
                for x in rect.x..(rect.x + rect.w).min(self.buffer.width) {
                    mask[y * self.buffer.width + x] = true;
                }
            }
        }
        mask
    }

    fn draw_cover_panel(&mut self, palette: &[[u8; 3]], t: f32, exposure: f32) {
        let rect = self.layout.rect(Panel::Cover);
        if self.cover_render_rect != Some(rect) {
            self.cover_render_rect = Some(rect);
            self.kitty_signature = None;
            self.kitty_retried = false;
            if let Some(art) = &mut self.cover_art {
                art.reset_view();
            }
        }
        let dragging =
            self.layout.is_dragging(Panel::Cover) || self.layout.is_focused(Panel::Cover);
        let glass_regions = self.layout.glass_regions(Panel::Cover);
        let surface = surface_style(!glass_regions.is_empty());
        let cover = self.cover.clone();
        let title = self
            .cover_art
            .as_ref()
            .map(|art| {
                format!(
                    "{} · {} · {} PARTICLES",
                    crate::i18n::tr(self.opts.locale, "cover").to_uppercase(),
                    self.opts.cover_style.name().to_uppercase(),
                    art.particle_count()
                )
            })
            .unwrap_or_else(|| crate::i18n::tr(self.opts.locale, "cover").to_uppercase());
        frame::draw_frame_with_regions(
            &mut self.buffer,
            rect,
            &title,
            palette,
            dragging,
            surface,
            &glass_regions,
        );
        let kitty = self.opts.cover_backend == crate::kitty::CoverBackend::Kitty
            && crate::kitty::CoverBackend::supported()
            && !self.library.open
            && !self.shell.is_open()
            && !self.layout.is_minimized(Panel::Cover)
            && !self.layout.glass_for(Panel::Cover)
            && !self.layout.under_glass(Panel::Cover)
            && !self.kitty_failed;
        if kitty {
            let signature = (self.cover_request, rect);
            if let Some(cover) = cover.as_ref()
                && self.kitty_signature != Some(signature)
            {
                self.pending_graphics.extend(self.kitty_cache.transfer(
                    self.cover_request,
                    rect,
                    cover.width,
                    cover.height,
                    &cover.rgb_bytes(),
                ));
                self.kitty_signature = Some(signature);
            }
            if self
                .kitty_cache
                .should_retransmit(std::time::Duration::from_millis(350))
                && !self.kitty_retried
            {
                self.kitty_retried = true;
                if let Some(cover) = cover.as_ref() {
                    self.pending_graphics.extend(self.kitty_cache.retransmit(
                        rect,
                        cover.width,
                        cover.height,
                    ));
                }
            }
            if self
                .kitty_cache
                .should_fallback(std::time::Duration::from_millis(900))
            {
                self.kitty_failed = true;
                self.clear_kitty_cover();
                self.status = "kitty graphics timeout; using braille".into();
            } else {
                return;
            }
        }
        self.clear_kitty_cover();
        let (Some(art), Some(cover)) = (&mut self.cover_art, cover) else {
            let inner = rect.inner();
            self.buffer.put_str_clipped(
                inner.x,
                inner.y,
                inner.w,
                " waiting for cover…",
                Rgb::new(180, 180, 200),
                Rgb::from_slice(palette[0]).scale(0.52),
            );
            return;
        };
        art.draw(
            &mut self.buffer,
            rect,
            palette,
            self.opts.charset,
            self.opts.cover_style,
            t,
            self.opts.cover_blend,
            exposure,
            &cover,
        );
    }

    fn clear_kitty_cover(&mut self) {
        if self.kitty_signature.take().is_some() {
            self.pending_graphics.extend(crate::kitty::delete_image());
        }
        self.kitty_cache.invalidate();
    }

    fn draw_lyrics_panel(&mut self, palette: &[[u8; 3]]) {
        let rect = self.layout.rect(Panel::Lyrics);
        let dragging =
            self.layout.is_dragging(Panel::Lyrics) || self.layout.is_focused(Panel::Lyrics);
        let glass_regions = self.layout.glass_regions(Panel::Lyrics);
        let surface = surface_style(!glass_regions.is_empty());
        frame::draw_frame_with_regions(
            &mut self.buffer,
            rect,
            crate::i18n::tr(self.opts.locale, "lyrics"),
            palette,
            dragging,
            surface,
            &glass_regions,
        );
        self.lyric.draw(&mut self.buffer, rect, palette);
        let playlist_line = self
            .library
            .songs
            .get(self.playlist_pos)
            .map(|song| {
                format!(
                    " PLAY {}/{} · {} · {}",
                    self.playlist_pos + 1,
                    self.library.songs.len(),
                    song.title,
                    song.artist
                )
            })
            .unwrap_or_default();
        let volume = self.effective_volume();
        let mode = self.play_mode_name();
        controls::draw(
            &mut self.buffer,
            rect,
            palette,
            controls::ControlDisplay {
                paused: self.paused,
                playlist_line: &playlist_line,
                progress: (self.audio_t / (self.meta.duration_ms as f32 / 1000.0).max(0.1))
                    .clamp(0.0, 1.0),
                volume,
                mode,
            },
        );
    }

    fn draw_audio_scope_panel(&mut self, palette: &[[u8; 3]]) {
        let rect = self.layout.rect(Panel::AudioScope);
        let dragging =
            self.layout.is_dragging(Panel::AudioScope) || self.layout.is_focused(Panel::AudioScope);
        let glass_regions = self.layout.glass_regions(Panel::AudioScope);
        let surface = surface_style(!glass_regions.is_empty());
        frame::draw_frame_with_regions(
            &mut self.buffer,
            rect,
            crate::i18n::tr(self.opts.locale, "spectrum"),
            palette,
            dragging,
            surface,
            &glass_regions,
        );
        self.audio_scope.draw(&mut self.buffer, rect, palette);
    }

    pub(super) fn danmaku_bounds(&self) -> layout::Rect {
        let audio = self.layout.rect(Panel::AudioScope);
        let lyrics = self.layout.rect(Panel::Lyrics);
        let logs = self.layout.rect(Panel::Logs);
        let x = audio.x;
        let y = (audio.y + audio.h + 1).min(self.buffer.height.saturating_sub(2));
        let bottom = if self.layout.logs_visible() && logs.w >= 8 {
            logs.y
        } else {
            lyrics.y
        };
        let available = bottom.saturating_sub(y).max(1);
        let h = available.min(7);
        layout::Rect::new(x, y, self.buffer.width.saturating_sub(x + 2), h)
    }

    fn draw_meta(&mut self, t: f32, palette: &[[u8; 3]]) {
        let rect = self.layout.rect(Panel::Meta);
        let dragging = self.layout.is_dragging(Panel::Meta) || self.layout.is_focused(Panel::Meta);
        let glass_regions = self.layout.glass_regions(Panel::Meta);
        let surface = surface_style(!glass_regions.is_empty());
        frame::draw_frame_with_regions(
            &mut self.buffer,
            rect,
            crate::i18n::tr(self.opts.locale, "now_playing"),
            palette,
            dragging,
            surface,
            &glass_regions,
        );
        let inner = rect.inner();
        let bg = Rgb::from_slice(palette[0]).scale(0.52);
        let fg = readable_text(
            bg,
            Rgb::from_slice(*palette.last().unwrap_or(&[255, 255, 255])),
        );
        let clock_fg = muted_text(bg, color_for(palette, 0.82));
        let total = self.meta.duration_ms as f32 / 1000.0;
        let shown = if self.opts.demo || self.audio_t <= 0.0 {
            t
        } else {
            self.audio_t
        };
        let clock = format!(
            "{} / {}{}",
            fmt_time(shown),
            fmt_time(total),
            if self.audio.bpm > 0.0 {
                format!("  {:.0}bpm", self.audio.bpm)
            } else {
                String::new()
            }
        );
        let clock_w = clock.chars().count();
        let title_w = inner.w.saturating_sub(clock_w + 1);
        let title = format!("{}/{}", self.meta.title, self.meta.artist);
        self.buffer
            .put_str_clipped(inner.x, inner.y, title_w, &title, fg, bg);
        let x = inner.x + inner.w.saturating_sub(clock_w);
        self.buffer
            .put_str_clipped(x, inner.y, clock_w, &clock, clock_fg, bg);
    }

    fn draw_logs_panel(&mut self, palette: &[[u8; 3]]) {
        let rect = self.layout.rect(Panel::Logs);
        let focused = self.layout.is_focused(Panel::Logs);
        if self.notifications_view {
            let surface = surface_style(self.layout.glass_for(Panel::Logs));
            frame::draw_frame(
                &mut self.buffer,
                rect,
                "NOTIFICATIONS · Alt+N · Alt+R RETRY",
                palette,
                focused,
                surface,
            );
            self.notifications.draw(&mut self.buffer, rect, palette);
        } else {
            self.logs.draw(
                &mut self.buffer,
                rect,
                palette,
                self.layout.glass_for(Panel::Logs),
                focused,
            );
        }
    }
}

fn surface_style(glass: bool) -> SurfaceStyle {
    if glass {
        SurfaceStyle::Glass
    } else {
        SurfaceStyle::Opaque
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::AnimationKind;
    use crate::app::{App, AppOptions};
    use crate::config::{Accessibility, Hotkeys};
    use crate::cover_art::CoverStyle;
    use crate::glyphs::Charset;
    use crate::i18n::Locale;
    use crate::layout::PointerAction;

    fn app() -> App {
        App::new(
            AppOptions {
                kind: AnimationKind::Adaptive,
                palette: "aurora".into(),
                charset: Charset::Mixed,
                fps: 120,
                cover_blend: 0.62,
                show_lyrics: true,
                show_meta: true,
                cover_ratio: 0.30,
                cover_style: CoverStyle::Braille,
                cover_backend: crate::kitty::CoverBackend::Cells,
                demo: true,
                show_stats: false,
                show_audio_scope: true,
                show_logs: true,
                locale: Locale::En,
                accessibility: Accessibility::Standard,
                hotkeys: Hotkeys::default(),
                config_path: None,
            },
            100,
            40,
        )
    }

    fn cover_pixels(app: &App) -> usize {
        let rect = app.layout.rect(Panel::Cover).inner();
        (rect.y..rect.y + rect.h)
            .flat_map(|y| (rect.x..rect.x + rect.w).map(move |x| (x, y)))
            .filter(|(x, y)| app.buffer.cell(*x, *y).ch != ' ')
            .count()
    }

    #[test]
    fn cover_survives_drag_resize_and_style_changes() {
        let mut app = app();
        for _ in 0..3 {
            app.tick().unwrap();
        }
        assert!(cover_pixels(&app) > 10);
        let rect = app.layout.rect(Panel::Cover);
        app.handle_pointer(PointerAction::Down, rect.x + 2, rect.y + 2);
        app.handle_pointer(PointerAction::Drag, 12, 8);
        app.handle_pointer(PointerAction::Up, 12, 8);
        app.tick().unwrap();
        assert!(cover_pixels(&app) > 10);
        app.cycle_cover_style();
        app.tick().unwrap();
        assert!(cover_pixels(&app) > 10);
        app.resize(110, 42);
        app.tick().unwrap();
        assert!(cover_pixels(&app) > 10);
    }
}
