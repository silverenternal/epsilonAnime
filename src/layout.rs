use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

impl Rect {
    pub const fn new(x: usize, y: usize, w: usize, h: usize) -> Self {
        Self { x, y, w, h }
    }

    pub fn inner(self) -> Self {
        Self {
            x: self.x.saturating_add(1),
            y: self.y.saturating_add(1),
            w: self.w.saturating_sub(2),
            h: self.h.saturating_sub(2),
        }
    }

    pub fn contains(self, x: usize, y: usize) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }

    pub fn clamp(mut self, width: usize, height: usize) -> Self {
        self.w = self.w.min(width).max(1);
        self.h = self.h.min(height).max(1);
        self.x = self.x.min(width.saturating_sub(self.w));
        self.y = self.y.min(height.saturating_sub(self.h));
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Panel {
    Meta,
    Cover,
    AudioScope,
    Lyrics,
    Logs,
    Shell,
}

impl Panel {
    pub fn name(self) -> &'static str {
        match self {
            Self::Meta => "now-playing",
            Self::Cover => "cover",
            Self::AudioScope => "spectrum",
            Self::Lyrics => "lyrics",
            Self::Logs => "logs",
            Self::Shell => "shell",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerAction {
    Down,
    Drag,
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutPreset {
    Balanced,
    Gallery,
    Instrument,
}

impl LayoutPreset {
    pub fn name(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Gallery => "gallery",
            Self::Instrument => "instrument",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Balanced => Self::Gallery,
            Self::Gallery => Self::Instrument,
            Self::Instrument => Self::Balanced,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Drag {
    panel: Panel,
    dx: usize,
    dy: usize,
    resize: bool,
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub width: usize,
    pub height: usize,
    meta: Rect,
    cover: Rect,
    audio_scope: Rect,
    lyrics: Rect,
    logs: Rect,
    shell: Rect,
    drag: Option<Drag>,
    drag_preview: Option<Rect>,
    preset: LayoutPreset,
    cover_ratio: f32,
    logs_visible: bool,
    shell_visible: bool,
    focus: Panel,
    minimized: [bool; 6],
    order: Vec<Panel>,
}

mod geometry;
mod state;

impl Layout {
    pub fn new(width: usize, height: usize, cover_ratio: f32) -> Self {
        let mut layout = Self {
            width,
            height,
            meta: Rect::new(0, 0, 1, 1),
            cover: Rect::new(0, 0, 1, 1),
            audio_scope: Rect::new(0, 0, 1, 1),
            lyrics: Rect::new(0, 0, 1, 1),
            logs: Rect::new(0, 0, 1, 1),
            shell: Rect::new(0, 0, 1, 1),
            drag: None,
            drag_preview: None,
            preset: LayoutPreset::Balanced,
            cover_ratio: 0.30,
            logs_visible: true,
            shell_visible: false,
            focus: Panel::Lyrics,
            minimized: [false; 6],
            order: vec![
                Panel::Meta,
                Panel::Cover,
                Panel::AudioScope,
                Panel::Logs,
                Panel::Lyrics,
            ],
        };
        layout.reset(width, height, cover_ratio);
        layout
    }

    pub fn reset(&mut self, width: usize, height: usize, cover_ratio: f32) {
        self.width = width;
        self.height = height;
        self.cover_ratio = cover_ratio;
        self.drag = None;
        let meta_w = (width.saturating_sub(4)).clamp(20, 56);
        self.meta = Rect::new(2, 1, meta_w, 3).clamp(width, height);

        let lyric_w = ((width as f32 * 0.74) as usize)
            .max(34)
            .min(width.saturating_sub(4));
        let lyric_h = 7.min(height.saturating_sub(2)).max(2);
        let lyric_x = (width.saturating_sub(lyric_w)) / 2;
        let lyric_y = height.saturating_sub(lyric_h + 1);
        self.lyrics = Rect::new(lyric_x, lyric_y, lyric_w, lyric_h).clamp(width, height);

        let cover_y = self.meta.y + 4;
        let cover_scale = match self.preset {
            LayoutPreset::Gallery => 1.26,
            LayoutPreset::Instrument => 0.82,
            LayoutPreset::Balanced => 1.0,
        };
        let requested_w = (width as f32 * cover_ratio.clamp(0.15, 0.55) * cover_scale) as usize;
        let vertical_w = lyric_y.saturating_sub(cover_y + 1).saturating_mul(2);
        let cover_w = requested_w
            .clamp(16, 72)
            .min(width.saturating_sub(4))
            .min(vertical_w.max(18))
            .max(1);
        let cover_h = (cover_w / 2 + 2)
            .min(lyric_y.saturating_sub(cover_y).max(2))
            .min(height.saturating_sub(4));
        self.cover = Rect::new(2, cover_y, cover_w, cover_h).clamp(width, height);

        let audio_x = self.cover.x + self.cover.w + 2;
        let audio_w = width.saturating_sub(audio_x + 2).max(1);
        let audio_y = self.cover.y;
        let available_h = self.lyrics.y.saturating_sub(audio_y + 1);
        let audio_share = match self.preset {
            LayoutPreset::Gallery => 2,
            LayoutPreset::Instrument => 3,
            LayoutPreset::Balanced => 2,
        };
        let audio_divisor = if self.preset == LayoutPreset::Instrument {
            3
        } else {
            5
        };
        let audio_max = if self.preset == LayoutPreset::Instrument {
            12
        } else {
            8
        };
        let audio_h = (available_h * audio_share / audio_divisor)
            .clamp(3, audio_max)
            .min(available_h.max(1));
        let audio_w = if self.preset == LayoutPreset::Instrument {
            audio_w.saturating_sub(4).min(56)
        } else {
            audio_w.min(42)
        };
        self.audio_scope = Rect::new(audio_x, audio_y, audio_w, audio_h).clamp(width, height);
        self.shell = shell_rect(width, height);
        self.place_logs();
    }

    pub fn cycle_preset(&mut self) -> &'static str {
        self.preset = self.preset.next();
        self.reset(self.width, self.height, self.cover_ratio);
        self.preset.name()
    }

    pub fn reset_balanced(&mut self) {
        self.preset = LayoutPreset::Balanced;
        self.reset(self.width, self.height, self.cover_ratio);
    }

    pub fn rect(&self, panel: Panel) -> Rect {
        match panel {
            Panel::Meta => self.meta,
            Panel::Cover => self.cover,
            Panel::AudioScope => self.audio_scope,
            Panel::Lyrics => self.lyrics,
            Panel::Logs => self.logs,
            Panel::Shell => self.shell,
        }
    }

    pub fn set_logs_visible(&mut self, visible: bool) {
        self.logs_visible = visible;
        self.place_logs();
    }

    pub fn logs_visible(&self) -> bool {
        self.logs_visible
    }

    pub fn shell_visible(&self) -> bool {
        self.shell_visible
    }

    pub fn set_shell_visible(&mut self, visible: bool) {
        self.shell_visible = visible;
    }

    pub fn glass_for(&self, panel: Panel) -> bool {
        !self.glass_regions(panel).is_empty()
    }

    pub fn glass_regions(&self, panel: Panel) -> Vec<Rect> {
        let Some(index) = self.order.iter().position(|candidate| *candidate == panel) else {
            return Vec::new();
        };
        let current = self.rect(panel);
        self.order[..index]
            .iter()
            .filter(|candidate| **candidate != Panel::Logs || self.logs_visible)
            .filter(|candidate| **candidate != Panel::Shell || self.shell_visible)
            .filter_map(|candidate| intersection(current, self.rect(*candidate)))
            .collect()
    }

    pub fn under_glass(&self, panel: Panel) -> bool {
        let current = self.rect(panel);
        [
            Panel::Meta,
            Panel::Cover,
            Panel::AudioScope,
            Panel::Logs,
            Panel::Lyrics,
            Panel::Shell,
        ]
        .into_iter()
        .filter(|candidate| *candidate != panel)
        .filter(|candidate| *candidate != Panel::Logs || self.logs_visible)
        .filter(|candidate| *candidate != Panel::Shell || self.shell_visible)
        .any(|candidate| {
            (candidate == Panel::Shell || self.glass_for(candidate))
                && intersects(current, self.rect(candidate))
        })
    }

    fn place_logs(&mut self) {
        if !self.logs_visible {
            self.logs = Rect::new(0, 0, 1, 1);
            return;
        }
        let top = self.audio_scope.y + self.audio_scope.h + 1;
        let available = self.lyrics.y.saturating_sub(top + 1);
        let max_h = if self.height < 30 { 3 } else { 6 };
        let h = available.min(max_h);
        if h < 3 || self.audio_scope.w < 16 {
            self.logs = Rect::new(0, 0, 1, 1);
            return;
        }
        self.logs = Rect::new(
            self.audio_scope.x,
            self.lyrics.y.saturating_sub(h),
            self.audio_scope.w,
            h,
        )
        .clamp(self.width, self.height);
    }

    pub fn is_resize_handle(&self, x: usize, y: usize) -> bool {
        self.hit_order()
            .into_iter()
            .filter(|panel| *panel != Panel::Logs || self.logs_visible)
            .filter(|panel| *panel != Panel::Shell || self.shell_visible)
            .any(|panel| {
                let rect = self.rect(panel);
                x == rect.x + rect.w.saturating_sub(1) && y == rect.y + rect.h.saturating_sub(1)
            })
    }
}

fn snap(value: usize, limit: usize) -> usize {
    if value <= 2 {
        2.min(limit.saturating_sub(1))
    } else if value + 2 >= limit {
        limit.saturating_sub(2)
    } else {
        value
    }
}

fn min_size(panel: Panel) -> (usize, usize) {
    match panel {
        Panel::Meta => (20, 3),
        Panel::Cover => (18, 7),
        Panel::AudioScope => (20, 4),
        Panel::Lyrics => (32, 5),
        Panel::Logs => (16, 3),
        Panel::Shell => (40, 12),
    }
}

fn set_rect(layout: &mut Layout, panel: Panel, rect: Rect) {
    match panel {
        Panel::Meta => layout.meta = rect,
        Panel::Cover => layout.cover = rect,
        Panel::AudioScope => layout.audio_scope = rect,
        Panel::Lyrics => layout.lyrics = rect,
        Panel::Logs => layout.logs = rect,
        Panel::Shell => layout.shell = rect,
    }
}

fn intersects(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

fn intersection(a: Rect, b: Rect) -> Option<Rect> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = (a.x + a.w).min(b.x + b.w);
    let bottom = (a.y + a.h).min(b.y + b.h);
    (right > x && bottom > y).then(|| Rect::new(x, y, right - x, bottom - y))
}

pub fn shell_rect(width: usize, height: usize) -> Rect {
    let w = (width.saturating_mul(4) / 5).clamp(40, width.saturating_sub(4).max(1));
    let h = (height.saturating_mul(3) / 4).clamp(12, height.saturating_sub(3).max(1));
    Rect::new(
        (width.saturating_sub(w)) / 2,
        (height.saturating_sub(h)) / 2,
        w,
        h,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_clamps_panel() {
        let mut layout = Layout::new(100, 40, 0.3);
        let before = layout.rect(Panel::Cover);
        layout.begin_drag(before.x + 1, before.y + 1);
        layout.drag_to(500, 500);
        let after = layout.rect(Panel::Cover);
        assert!(after.x + after.w <= 100);
        assert!(after.y + after.h <= 40);
        assert!(layout.is_dragging(Panel::Cover));
        layout.end_drag();
        assert!(!layout.is_dragging(Panel::Cover));
    }

    #[test]
    fn topmost_panel_wins_hit_test() {
        let mut layout = Layout::new(100, 40, 0.3);
        let lyrics = layout.rect(Panel::Lyrics);
        layout.begin_drag(lyrics.x + 1, lyrics.y);
        assert!(layout.is_dragging(Panel::Lyrics));
    }

    #[test]
    fn cover_workspace_compensates_for_terminal_cell_aspect_ratio() {
        let layout = Layout::new(160, 48, 0.40);
        let inner = layout.rect(Panel::Cover).inner();
        let dot_w = inner.w * 2;
        let dot_h = inner.h * 4;
        assert!(inner.w > 50);
        assert!(dot_w.abs_diff(dot_h) < 8);
    }

    #[test]
    fn presets_cycle_and_resize_round_trips() {
        let mut layout = Layout::new(120, 40, 0.40);
        assert_eq!(layout.cycle_preset(), "gallery");
        assert_eq!(layout.cycle_preset(), "instrument");
        assert_eq!(layout.cycle_preset(), "balanced");
        let cover = layout.rect(Panel::Cover);
        layout.begin_drag(cover.x + cover.w - 1, cover.y + cover.h - 1);
        layout.drag_to(cover.x + 30, cover.y + 20);
        let resized = layout.rect(Panel::Cover);
        assert!(resized.w >= 18 && resized.h >= 7);
    }

    #[test]
    fn default_layout_leaves_room_for_logs_and_shell() {
        let layout = Layout::new(120, 40, 0.30);
        let cover = layout.rect(Panel::Cover);
        let logs = layout.rect(Panel::Logs);
        assert!(cover.w <= 38);
        assert!(logs.w >= 8 && logs.h >= 3);
        assert!(layout.logs_visible());
        let shell = shell_rect(120, 40);
        assert!(shell.x + shell.w <= 120);
        assert!(shell.y + shell.h <= 40);
    }

    #[test]
    fn overlap_uses_glass_and_shell_is_resizable() {
        let mut layout = Layout::new(120, 40, 0.30);
        let meta = layout.rect(Panel::Meta);
        let cover = layout.rect(Panel::Cover);
        layout.begin_drag(cover.x + 1, cover.y + 1);
        layout.drag_to(meta.x, meta.y);
        layout.end_drag();
        assert!(layout.glass_for(Panel::Cover));
        let regions = layout.glass_regions(Panel::Cover);
        assert!(!regions.is_empty());
        assert!(
            regions
                .iter()
                .all(|region| region.h < layout.rect(Panel::Cover).h)
        );

        layout.set_shell_visible(true);
        let shell = layout.rect(Panel::Shell);
        layout.begin_drag(shell.x + shell.w - 1, shell.y + shell.h - 1);
        layout.drag_to(shell.x + shell.w + 5, shell.y + shell.h + 3);
        let resized = layout.rect(Panel::Shell);
        assert!(resized.w > shell.w);
        assert!(resized.h > shell.h);
    }

    #[test]
    fn focus_minimize_and_restack_change_window_order() {
        let mut layout = Layout::new(120, 40, 0.3);
        let first = layout.draw_order()[0];
        layout.focus = first;
        layout.restack_focused(true);
        assert_ne!(layout.draw_order()[0], first);
        layout.focus = first;
        assert!(layout.toggle_minimize_focused());
        assert!(!layout.draw_order().contains(&first));
        layout.focus = first;
        assert!(!layout.toggle_minimize_focused());
        assert!(layout.draw_order().contains(&first));
        layout.restore_all();
        assert_eq!(layout.draw_order().len(), 5);
    }

    #[test]
    fn drag_has_a_preview_and_restore_targets_any_minimized_window() {
        let mut layout = Layout::new(120, 40, 0.3);
        let cover = layout.rect(Panel::Cover);
        layout.begin_drag(cover.x + 1, cover.y + 1);
        layout.drag_to(cover.x + 6, cover.y + 4);
        assert!(layout.drag_preview().is_some());
        layout.end_drag();
        assert!(layout.drag_preview().is_none());

        layout.focus = Panel::Cover;
        layout.toggle_minimize_focused();
        layout.focus = Panel::Lyrics;
        layout.toggle_minimize_focused();
        assert_eq!(layout.minimized_panels().len(), 2);
        assert!(layout.restore(Panel::Lyrics));
        assert!(!layout.is_minimized(Panel::Lyrics));
        assert_eq!(layout.focused_panel(), Panel::Lyrics);
    }

    #[test]
    fn near_peer_drag_snaps_without_overlap() {
        let mut layout = Layout::new(120, 40, 0.3);
        layout.meta = Rect::new(2, 1, 30, 3);
        layout.cover = Rect::new(10, 10, 30, 10);
        layout.begin_drag(11, 11);
        layout.drag_to(3, 5);
        let cover = layout.rect(Panel::Cover);
        assert_eq!(
            cover.y,
            layout.rect(Panel::Meta).y + layout.rect(Panel::Meta).h
        );
        assert!(!intersects(cover, layout.rect(Panel::Meta)));
    }
}
