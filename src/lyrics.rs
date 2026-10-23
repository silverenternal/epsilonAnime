use crate::events::{LyricLineData, LyricSegment};
use crate::layout::Rect;
use crate::render::{Buffer, Rgb, muted_text, readable_text};
use crate::state_store;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct LyricPreferences {
    offset: f32,
    show_translation: bool,
    show_romanization: bool,
}

#[derive(Clone, Debug)]
pub struct LyricState {
    pub index: usize,
    pub text: String,
    pub translation: String,
    pub romaji: String,
    pub segments: Vec<LyricSegment>,
    pub offset: f32,
    pub active_segment: usize,
    pub show_translation: bool,
    pub show_romanization: bool,
    pub timeline: Vec<LyricLineData>,
    pub scroll_offset: isize,
    pub progress: f32,
    pub segment_progress: f32,
    pub current_time: f32,
}

impl Default for LyricState {
    fn default() -> Self {
        Self {
            index: 0,
            text: String::new(),
            translation: String::new(),
            romaji: String::new(),
            segments: Vec::new(),
            offset: 0.0,
            active_segment: 0,
            show_translation: true,
            show_romanization: false,
            timeline: Vec::new(),
            scroll_offset: 0,
            progress: 0.0,
            segment_progress: 0.0,
            current_time: 0.0,
        }
    }
}

impl LyricState {
    pub fn load() -> Self {
        let preferences: Option<LyricPreferences> = state_store::load_optional("lyrics.json", 1);
        let mut state = Self::default();
        if let Some(preferences) = preferences {
            state.offset = preferences.offset.clamp(-10.0, 10.0);
            state.show_translation = preferences.show_translation;
            state.show_romanization = preferences.show_romanization;
        }
        state
    }

    pub fn load_timeline(&mut self, lines: Vec<LyricLineData>) {
        self.timeline = lines;
        self.scroll_offset = 0;
    }

    pub fn scroll(&mut self, delta: isize) {
        let max_forward = self.timeline.len().saturating_sub(self.index + 1) as isize;
        let max_back = self.index as isize;
        self.scroll_offset = (self.scroll_offset + delta).clamp(-max_back, max_forward);
    }

    pub fn search(&mut self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return false;
        }
        let Some(target) = self.timeline.iter().position(|line| {
            line.text.to_lowercase().contains(&query)
                || line.translation.to_lowercase().contains(&query)
                || line.romaji.to_lowercase().contains(&query)
        }) else {
            return false;
        };
        self.scroll_offset = target as isize - self.index as isize;
        true
    }

    pub fn save_preferences(&self) {
        state_store::save(
            "lyrics.json",
            &LyricPreferences {
                offset: self.offset,
                show_translation: self.show_translation,
                show_romanization: self.show_romanization,
            },
            1,
        );
    }

    pub fn set(&mut self, index: usize, text: String, translation: String, progress: f32) {
        self.set_rich(
            index,
            text,
            translation,
            String::new(),
            Vec::new(),
            0.0,
            progress,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_rich(
        &mut self,
        index: usize,
        text: String,
        translation: String,
        romaji: String,
        segments: Vec<LyricSegment>,
        offset: f32,
        progress: f32,
    ) {
        if index != self.index {
            self.scroll_offset = 0;
        }
        self.index = index;
        self.text = text;
        self.translation = translation;
        self.romaji = romaji;
        self.segments = segments;
        self.offset = offset;
        self.active_segment = 0;
        self.progress = progress.clamp(0.0, 1.0);
    }

    pub fn update_active(&mut self, time: f32) -> Option<&str> {
        let time = time + self.offset;
        self.current_time = time;
        self.segment_progress = self
            .segments
            .iter()
            .find(|segment| time >= segment.start && time < segment.end)
            .map(|segment| {
                ((time - segment.start) / (segment.end - segment.start).max(0.001)).clamp(0.0, 1.0)
            })
            .unwrap_or_else(|| {
                if self
                    .segments
                    .last()
                    .is_some_and(|segment| time >= segment.end)
                {
                    1.0
                } else {
                    0.0
                }
            });
        let active = self
            .segments
            .iter()
            .position(|segment| time >= segment.start && time < segment.end)
            .unwrap_or_else(|| {
                self.segments
                    .iter()
                    .rposition(|segment| time >= segment.end)
                    .unwrap_or(0)
            });
        if active != self.active_segment {
            self.active_segment = active;
            return self
                .segments
                .get(active)
                .map(|segment| segment.text.as_str());
        }
        None
    }

    pub fn toggle_translation(&mut self) -> bool {
        self.show_translation = !self.show_translation;
        self.save_preferences();
        self.show_translation
    }

    pub fn toggle_romanization(&mut self) -> bool {
        self.show_romanization = !self.show_romanization;
        self.save_preferences();
        self.show_romanization
    }

    pub fn draw(&self, buf: &mut Buffer, rect: Rect, palette: &[[u8; 3]]) {
        let inner = rect.inner();
        if inner.w == 0 || inner.h == 0 {
            return;
        }
        let bg = Rgb::from_slice(palette[0]).scale(0.55);
        let fg = readable_text(
            bg,
            Rgb::from_slice(*palette.last().unwrap_or(&[255, 255, 255])),
        );
        let accent = muted_text(
            bg,
            Rgb::from_slice(palette[palette.len().saturating_sub(2)]),
        );
        let display_index = if self.timeline.is_empty() {
            0
        } else {
            (self.index as isize + self.scroll_offset)
                .clamp(0, self.timeline.len().saturating_sub(1) as isize) as usize
        };
        let display = self.timeline.get(display_index);
        let text = display.map_or(self.text.as_str(), |line| line.text.as_str());
        let segments = display.map_or(self.segments.as_slice(), |line| line.segments.as_slice());
        let active = if self.scroll_offset == 0 {
            self.active_segment
        } else {
            0
        };
        if text.is_empty() {
            center_line(
                buf,
                inner,
                inner.y,
                "♪ instrumental · no timed lyrics",
                accent,
                bg,
            );
            return;
        }
        if self.timeline.is_empty() || inner.h < 4 {
            if segments.is_empty() {
                center_line(buf, inner, inner.y, text, fg, bg);
            } else {
                center_segments(
                    buf,
                    inner,
                    inner.y,
                    segments,
                    active,
                    self.segment_progress,
                    fg,
                    accent,
                    bg,
                );
            }
            return;
        }
        let view_h = inner.h.saturating_sub(3);
        let line_y = inner.y + view_h.saturating_sub(1) / 2;
        let view_bottom = inner.y + view_h;
        for offset in -2isize..=2 {
            let index = display_index as isize + offset;
            if index < 0 || index >= self.timeline.len() as isize {
                continue;
            }
            let row = line_y as isize + offset;
            if row < inner.y as isize || row >= view_bottom as isize {
                continue;
            }
            let line = &self.timeline[index as usize];
            let selected = index == display_index as isize;
            let color = if selected {
                accent
            } else {
                fg.scale(if offset.abs() == 1 { 0.68 } else { 0.42 })
            };
            center_line(buf, inner, row as usize, &line.text, color, bg);
            if selected && (self.show_translation || self.show_romanization) && inner.h >= 6 {
                let secondary = if self.show_translation && !line.translation.is_empty() {
                    line.translation.as_str()
                } else if self.show_romanization {
                    line.romaji.as_str()
                } else {
                    ""
                };
                if !secondary.is_empty() && row as usize + 1 < inner.y + inner.h.saturating_sub(3) {
                    center_line(buf, inner, row as usize + 1, secondary, fg.scale(0.72), bg);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn center_segments(
    buf: &mut Buffer,
    inner: Rect,
    y: usize,
    segments: &[LyricSegment],
    active: usize,
    progress: f32,
    fg: Rgb,
    accent: Rgb,
    bg: Rgb,
) {
    let width: usize = segments
        .iter()
        .map(|segment| {
            segment
                .text
                .chars()
                .map(|ch| unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0))
                .sum::<usize>()
        })
        .sum();
    let mut x = inner.x + inner.w.saturating_sub(width) / 2;
    let limit = inner.x + inner.w;
    for (index, segment) in segments.iter().enumerate() {
        if x >= limit {
            break;
        }
        let color = if index < active {
            accent.scale(0.78)
        } else if index == active {
            fg
        } else {
            fg.scale(0.48)
        };
        if index != active {
            x = buf.put_str_clipped(x, y, limit.saturating_sub(x), &segment.text, color, bg);
            continue;
        }
        let glyphs: Vec<char> = segment.text.chars().filter(|ch| !ch.is_control()).collect();
        let count = glyphs.len().max(1);
        for (position, ch) in glyphs.into_iter().enumerate() {
            let local = (position + 1) as f32 / count as f32;
            let char_color = if local <= progress { accent } else { color };
            x = buf.put_str_clipped(
                x,
                y,
                limit.saturating_sub(x),
                &ch.to_string(),
                char_color,
                bg,
            );
        }
    }
}

fn center_line(buf: &mut Buffer, inner: Rect, y: usize, text: &str, fg: Rgb, bg: Rgb) {
    let width: usize = text
        .chars()
        .map(|c| unicode_width::UnicodeWidthChar::width(c).unwrap_or(0))
        .sum();
    let x = inner.x + inner.w.saturating_sub(width) / 2;
    let available = inner.x + inner.w - x;
    buf.put_str_clipped(x, y, available, text, fg, bg);
}

pub fn fmt_time(sec: f32) -> String {
    let sec = sec.max(0.0) as u64;
    format!("{:02}:{:02}", sec / 60, sec % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_format() {
        assert_eq!(fmt_time(65.9), "01:05");
    }

    #[test]
    fn lyric_draws() {
        let mut b = Buffer::new(40, 10);
        let mut l = LyricState::default();
        l.set(0, "hello".into(), String::new(), 0.5);
        l.draw(
            &mut b,
            Rect::new(0, 6, 40, 4),
            &[[0, 0, 0], [255, 255, 255]],
        );
        assert_eq!(b.cell(17, 7).ch, 'h');
    }

    #[test]
    fn japanese_lyric_uses_double_width_cells() {
        let mut b = Buffer::new(80, 24);
        let mut l = LyricState::default();
        l.set(
            0,
            "乱れた呼吸 慌ただしく".into(),
            "混乱的呼吸 匆忙不安".into(),
            0.42,
        );
        let rect = Rect::new(10, 18, 60, 5);
        l.draw(&mut b, rect, &[[5, 5, 16], [247, 37, 133], [255, 255, 255]]);

        let row: Vec<char> = (10..70).map(|x| b.cell(x, 19).ch).collect();
        let starts: Vec<(usize, char)> = row
            .iter()
            .enumerate()
            .filter_map(|(i, ch)| {
                (*ch != ' ' && !b.cell(10 + i, 19).hidden).then_some((10 + i, *ch))
            })
            .collect();
        assert_eq!(starts.first().map(|(_, ch)| *ch), Some('乱'));
        assert_eq!(
            starts.len(),
            "乱れた呼吸 慌ただしく"
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .count()
        );
        let first_x = starts.first().map(|(x, _)| *x).unwrap();
        assert!(b.cell(first_x + 1, 19).hidden);
        assert!(!b.cell(first_x + 2, 19).hidden);
    }

    #[test]
    fn long_japanese_lyric_is_clipped_to_panel() {
        let mut b = Buffer::new(40, 10);
        let mut l = LyricState::default();
        l.set(
            0,
            "乱れた呼吸 慌ただしく 鼓動急かす under the moon どこまでも続く".into(),
            String::new(),
            0.5,
        );
        let rect = Rect::new(2, 2, 20, 4);
        l.draw(&mut b, rect, &[[0, 0, 0], [255, 255, 255]]);
        assert_eq!(b.cell(21, 3).ch, ' ');
    }

    #[test]
    fn character_segments_advance_with_playback_time() {
        let mut lyric = LyricState::default();
        lyric.set_rich(
            0,
            "你好".into(),
            String::new(),
            "ni hao".into(),
            vec![
                LyricSegment {
                    start: 1.0,
                    end: 1.2,
                    text: "你".into(),
                },
                LyricSegment {
                    start: 1.2,
                    end: 1.5,
                    text: "好".into(),
                },
            ],
            0.0,
            0.0,
        );
        assert_eq!(lyric.update_active(1.3), Some("好"));
        assert_eq!(lyric.active_segment, 1);
        assert!((lyric.segment_progress - 1.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn timeline_scrolls_and_searches_every_language_field() {
        let mut lyric = LyricState::default();
        lyric.load_timeline(vec![
            LyricLineData {
                time: 0.0,
                text: "first".into(),
                translation: "第一".into(),
                ..Default::default()
            },
            LyricLineData {
                time: 3.0,
                text: "夜に駆ける".into(),
                romaji: "yoru ni kakeru".into(),
                ..Default::default()
            },
            LyricLineData {
                time: 6.0,
                text: "last".into(),
                translation: "最后".into(),
                ..Default::default()
            },
        ]);
        lyric.index = 1;
        lyric.scroll(-1);
        assert_eq!(lyric.scroll_offset, -1);
        assert!(lyric.search("最后"));
        assert_eq!(lyric.index as isize + lyric.scroll_offset, 2);
        lyric.index = 0;
        assert!(lyric.search("YORU"));
        assert_eq!(lyric.index as isize + lyric.scroll_offset, 1);
        assert!(!lyric.search("missing"));
    }
}
