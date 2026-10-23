use crate::layout::{PointerAction, Rect};
use crate::render::{Buffer, Rgb, color_for, readable_text};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct PlaylistItem {
    pub id: u64,
    pub name: String,
    pub count: usize,
    #[serde(default = "default_kind")]
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SongItem {
    pub id: u64,
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub duration_ms: u64,
}

fn default_kind() -> String {
    "playlist".into()
}

pub fn home_menu() -> Vec<PlaylistItem> {
    [
        ("每日推荐", "daily"),
        ("私人 FM", "radio"),
        ("最近播放", "recent"),
        ("我喜欢的音乐", "liked"),
        ("排行榜", "charts"),
        ("收藏专辑", "albums"),
        ("收藏歌手", "artists"),
    ]
    .into_iter()
    .map(|(name, kind)| PlaylistItem {
        id: 0,
        name: name.into(),
        count: 0,
        kind: kind.into(),
    })
    .collect()
}

#[derive(Default)]
pub struct LibraryState {
    pub open: bool,
    pub qr_url: String,
    pub qr_lines: Vec<String>,
    pub user_uid: u64,
    pub active_playlist: u64,
    pub collection_kind: String,
    pub collection_total: usize,
    pub collection_offset: usize,
    pub nickname: String,
    pub signature: String,
    pub collection_title: String,
    pub collection_description: String,
    pub cached: bool,
    pub section_view: bool,
    pub notice: String,
    pub playlists: Vec<PlaylistItem>,
    pub songs: Vec<SongItem>,
    pub selected: usize,
    pub song_selected: usize,
    pub window: Option<Rect>,
    pub(crate) drag: Option<LibraryDrag>,
    pub(crate) nav_stack: Vec<LibraryNav>,
}

#[derive(Clone, Default)]
pub(crate) struct LibraryNav {
    playlists: Vec<PlaylistItem>,
    songs: Vec<SongItem>,
    selected: usize,
    song_selected: usize,
    active_playlist: u64,
    collection_kind: String,
    collection_title: String,
    collection_description: String,
    section_view: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct LibraryDrag {
    dx: usize,
    dy: usize,
    resize: bool,
}

impl LibraryState {
    pub fn selected_playlist(&self) -> Option<&PlaylistItem> {
        self.playlists.get(self.selected)
    }

    pub fn selected_song(&self) -> Option<&SongItem> {
        self.songs.get(self.song_selected)
    }

    pub fn move_selection(&mut self, delta: isize) {
        let len = if self.songs.is_empty() {
            self.playlists.len()
        } else {
            self.songs.len()
        };
        if len == 0 {
            return;
        }
        let target = if self.songs.is_empty() {
            &mut self.selected
        } else {
            &mut self.song_selected
        };
        *target = (*target as isize + delta).rem_euclid(len as isize) as usize;
    }

    pub fn open_collection(&mut self, title: String, description: String) {
        if !self.section_view {
            self.push_view();
        }
        self.section_view = false;
        self.collection_title = title;
        self.collection_description = description;
    }

    pub fn push_view(&mut self) {
        self.nav_stack.push(LibraryNav {
            playlists: self.playlists.clone(),
            songs: self.songs.clone(),
            selected: self.selected,
            song_selected: self.song_selected,
            active_playlist: self.active_playlist,
            collection_kind: self.collection_kind.clone(),
            collection_title: self.collection_title.clone(),
            collection_description: self.collection_description.clone(),
            section_view: self.section_view,
        });
    }

    pub fn back(&mut self) -> bool {
        let Some(previous) = self.nav_stack.pop() else {
            return false;
        };
        self.playlists = previous.playlists;
        self.songs = previous.songs;
        self.selected = previous.selected;
        self.song_selected = previous.song_selected;
        self.active_playlist = previous.active_playlist;
        self.collection_kind = previous.collection_kind;
        self.collection_title = previous.collection_title;
        self.collection_description = previous.collection_description;
        self.section_view = previous.section_view;
        true
    }

    pub fn handle_pointer(
        &mut self,
        action: PointerAction,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    ) {
        let Some(rect) = self.window else {
            return;
        };
        match action {
            PointerAction::Down if rect.contains(x, y) => {
                let resize = x == rect.x + rect.w.saturating_sub(1)
                    && y == rect.y + rect.h.saturating_sub(1);
                self.drag = Some(LibraryDrag {
                    dx: x.saturating_sub(rect.x),
                    dy: y.saturating_sub(rect.y),
                    resize,
                });
            }
            PointerAction::Drag => {
                let Some(drag) = self.drag else {
                    return;
                };
                let mut next = self.window.unwrap_or(rect);
                if drag.resize {
                    next.w = x
                        .saturating_sub(next.x)
                        .saturating_add(1)
                        .clamp(30, width.saturating_sub(next.x).max(30));
                    next.h = y
                        .saturating_sub(next.y)
                        .saturating_add(1)
                        .clamp(10, height.saturating_sub(next.y).max(10));
                } else {
                    next.x = snap_axis(x.saturating_sub(drag.dx), next.w, width);
                    next.y = snap_axis(y.saturating_sub(drag.dy), next.h, height);
                }
                self.window = Some(next.clamp(width, height));
            }
            PointerAction::Up => self.drag = None,
            PointerAction::Down => {}
        }
    }
}

fn snap_axis(value: usize, size: usize, limit: usize) -> usize {
    if value <= 2 {
        0
    } else if value + size + 2 >= limit {
        limit.saturating_sub(size)
    } else {
        value
    }
}

pub fn draw(
    buffer: &mut Buffer,
    state: &mut LibraryState,
    palette: &[[u8; 3]],
    locale: crate::i18n::Locale,
) {
    if !state.open || buffer.width < 20 || buffer.height < 8 {
        return;
    }
    let qr_w = state
        .qr_lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let w = (qr_w + 6).max(44).min(buffer.width.saturating_sub(4));
    let h = if state.qr_lines.is_empty() {
        buffer.height.saturating_sub(4).max(12)
    } else {
        (state.qr_lines.len() + 7)
            .max(12)
            .min(buffer.height.saturating_sub(4))
    };
    let x = (buffer.width - w) / 2;
    let y = (buffer.height - h) / 2;
    let rect = state
        .window
        .unwrap_or(Rect::new(x, y, w, h))
        .clamp(buffer.width, buffer.height);
    state.window = Some(rect);
    let title = if state.user_uid > 0 {
        format!(
            "{} · UID {}",
            crate::i18n::tr(locale, "library").to_uppercase(),
            state.user_uid
        )
    } else {
        crate::i18n::tr(locale, "library").to_uppercase()
    };
    crate::frame::draw_frame(
        buffer,
        rect,
        &title,
        palette,
        false,
        crate::frame::SurfaceStyle::Glass,
    );
    let inner = rect.inner();
    let bg = Rgb::from_slice(palette[0]).scale(0.45);
    let fg = readable_text(bg, color_for(palette, 0.9));
    let muted = readable_text(bg, color_for(palette, 0.62));
    let accent = Rgb::new(255, 224, 160);
    let mut row = inner.y;
    if !state.qr_lines.is_empty() {
        let qr_limit = inner.y + inner.h.saturating_sub(1);
        for line in &state.qr_lines {
            if row >= qr_limit {
                break;
            }
            let x = inner.x + inner.w.saturating_sub(line.chars().count()) / 2;
            buffer.put_str_clipped(x, row, inner.w, line, fg, bg);
            row += 1;
        }
        if row < inner.y + inner.h {
            let hint = "Scan with the NetEase Cloud Music app";
            buffer.put_str_clipped(inner.x, row, inner.w, hint, accent, bg);
        }
        return;
    }
    if !state.nickname.is_empty() {
        let title = format!("{}  {}", state.nickname, state.signature);
        buffer.put_str_clipped(inner.x, row, inner.w, &title, fg, bg);
        row += 1;
    }
    if !state.notice.is_empty() && row < inner.y + inner.h.saturating_sub(1) {
        buffer.put_str_clipped(inner.x, row, inner.w, &state.notice, accent, bg);
        row += 1;
    }
    let list = if state.songs.is_empty() {
        &state.playlists
    } else {
        return draw_songs(buffer, state, inner, row, palette);
    };
    if list.is_empty() {
        buffer.put_str_clipped(
            inner.x,
            row,
            inner.w,
            "g login · s search[a:artist:kw] · d daily · h charts · a albums · r artists · i liked · m radio · u recent",
            muted,
            bg,
        );
        return;
    }
    let footer_y = inner.y + inner.h.saturating_sub(1);
    let visible = footer_y.saturating_sub(row);
    let start = state
        .selected
        .saturating_sub(visible.saturating_sub(1) / 2)
        .min(list.len().saturating_sub(visible));
    for (index, playlist) in list.iter().enumerate().skip(start).take(visible) {
        if row >= footer_y {
            break;
        }
        let selected = index == state.selected;
        let line = if playlist.count > 0 {
            format!("{}  {:>3}", playlist.name, playlist.count)
        } else {
            playlist.name.clone()
        };
        let line_bg = if selected {
            color_for(palette, 0.55).scale(0.42)
        } else {
            bg
        };
        buffer.put_str_clipped(
            inner.x,
            row,
            inner.w,
            &line,
            if selected { accent } else { fg },
            line_bg,
        );
        row += 1;
    }
    if footer_y >= inner.y {
        buffer.put_str_clipped(
            inner.x,
            footer_y,
            inner.w,
            "↑/↓ · Enter · d daily · i liked · F1 help · Backspace · Esc · q quit",
            muted,
            bg,
        );
    }
}

fn draw_songs(
    buffer: &mut Buffer,
    state: &LibraryState,
    inner: Rect,
    mut row: usize,
    palette: &[[u8; 3]],
) {
    let bg = Rgb::from_slice(palette[0]).scale(0.45);
    let fg = readable_text(bg, color_for(palette, 0.88));
    let muted = readable_text(bg, color_for(palette, 0.62));
    let accent = Rgb::new(255, 224, 160);
    if !state.collection_title.is_empty() && row < inner.y + inner.h.saturating_sub(1) {
        buffer.put_str_clipped(
            inner.x,
            row,
            inner.w,
            &format!(
                "{} · #{} · {}",
                state.collection_title,
                state.active_playlist,
                if state.cached { "CACHED" } else { "LIVE" }
            ),
            muted,
            bg,
        );
        row += 1;
    }
    let visible = inner.h.saturating_sub(row.saturating_sub(inner.y) + 1);
    let start = state
        .song_selected
        .saturating_sub(visible.saturating_sub(1) / 2)
        .min(state.songs.len().saturating_sub(visible));
    for (index, song) in state.songs.iter().enumerate().skip(start).take(visible) {
        if row >= inner.y + inner.h.saturating_sub(1) {
            break;
        }
        let selected = index == state.song_selected;
        let line = format!("{:02}. {} — {}", index + 1, song.title, song.artist);
        let line_bg = if selected {
            color_for(palette, 0.55).scale(0.42)
        } else {
            bg
        };
        buffer.put_str_clipped(
            inner.x,
            row,
            inner.w,
            &line,
            if selected { accent } else { fg },
            line_bg,
        );
        row += 1;
    }
    if row < inner.y + inner.h {
        buffer.put_str_clipped(
            inner.x,
            inner.y + inner.h - 1,
            inner.w,
            "↑/↓ select · Enter play · f favorite · PgDn more · Backspace · Esc",
            muted,
            bg,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_window_moves_and_resizes() {
        let mut state = LibraryState {
            window: Some(Rect::new(10, 5, 44, 16)),
            ..Default::default()
        };
        let before = state.window.unwrap();
        state.handle_pointer(
            PointerAction::Down,
            before.x + before.w - 1,
            before.y + before.h - 1,
            100,
            40,
        );
        state.handle_pointer(PointerAction::Drag, 90, 32, 100, 40);
        state.handle_pointer(PointerAction::Up, 90, 32, 100, 40);
        let resized = state.window.unwrap();
        assert!(resized.w > before.w);
        assert!(resized.h > before.h);
    }

    #[test]
    fn compact_braille_qr_fits_a_twenty_four_row_terminal() {
        let mut buffer = Buffer::new(80, 24);
        let mut state = LibraryState {
            open: true,
            qr_lines: (0..13).map(|_| "⠿".repeat(25)).collect(),
            playlists: vec![PlaylistItem {
                id: 1,
                name: "stale playlist".into(),
                count: 1,
                kind: "playlist".into(),
            }],
            ..Default::default()
        };
        draw(
            &mut buffer,
            &mut state,
            &[[0, 0, 0], [255, 255, 255]],
            crate::i18n::Locale::En,
        );
        let window = state.window.unwrap();
        assert_eq!(window.h, 20);
        let last_qr_row = window.inner().y + 12;
        assert!(
            (window.inner().x..window.inner().x + window.inner().w)
                .any(|x| buffer.cell(x, last_qr_row).ch == '⠿')
        );
    }

    #[test]
    fn long_playlist_menu_scrolls_to_selected_item() {
        let mut buffer = Buffer::new(100, 30);
        let mut state = LibraryState {
            open: true,
            selected: 19,
            playlists: (0..20)
                .map(|index| PlaylistItem {
                    id: index,
                    name: format!("playlist-{index:02}"),
                    count: index as usize,
                    kind: "playlist".into(),
                })
                .collect(),
            ..Default::default()
        };
        draw(
            &mut buffer,
            &mut state,
            &[[0, 0, 0], [255, 255, 255]],
            crate::i18n::Locale::En,
        );
        let window = state.window.unwrap();
        let visible_rows: Vec<String> = (window.inner().y..window.inner().y + window.inner().h)
            .map(|y| {
                (window.inner().x..window.inner().x + window.inner().w)
                    .map(|x| buffer.cell(x, y).ch)
                    .collect()
            })
            .collect();
        assert!(visible_rows.iter().any(|line| line.contains("playlist-19")));
    }
}
