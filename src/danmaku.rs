use crate::anim::{AudioParams, FrameCtx};
use crate::events::CommentData;
use crate::layout::Rect;
use crate::render::{Buffer, Rgb, readable_text};

struct Active {
    label: String,
    x: f32,
    row: usize,
    speed: f32,
    likes: u64,
    width: usize,
}

pub struct Danmaku {
    pool: Vec<CommentData>,
    active: Vec<Active>,
    cursor: usize,
    cooldown: f32,
}

impl Danmaku {
    pub fn new() -> Self {
        Self {
            pool: Vec::new(),
            active: Vec::new(),
            cursor: 0,
            cooldown: 0.5,
        }
    }

    pub fn set_comments(&mut self, comments: Vec<CommentData>) {
        self.pool = comments;
        self.cursor = 0;
        self.cooldown = 0.4;
    }

    pub fn clear(&mut self) {
        self.pool.clear();
        self.active.clear();
        self.cursor = 0;
    }

    pub fn counts(&self) -> (usize, usize) {
        (self.active.len(), self.pool.len())
    }

    pub fn update(&mut self, ctx: &FrameCtx<'_>, audio: &AudioParams, bounds: Rect) {
        self.cooldown -= ctx.dt;
        let bpm = if audio.bpm > 20.0 { audio.bpm } else { 110.0 };
        for item in &mut self.active {
            item.x -= item.speed * ctx.dt;
        }
        self.active
            .retain(|item| item.x + item.width as f32 > bounds.x as f32);
        let triggered = ctx.audio.onset > 0.5 || ctx.lyric_change > 0.8;
        if self.pool.is_empty() || (self.cooldown > 0.0 && !(triggered && self.cooldown < 0.25)) {
            return;
        }
        let max_active = (bounds.h / 2).clamp(1, 4);
        if self.active.len() >= max_active {
            return;
        }
        let seed = ctx.lyric_seed ^ (self.cursor as u64).wrapping_mul(0x9e37_79b9);
        let comment = &self.pool[seed as usize % self.pool.len()];
        let text = truncate_width(&comment.text, 72);
        let lane_count = (bounds.h / 2).max(1);
        let row = bounds.y + (seed as usize / 7) % lane_count * 2;
        let speed = 12.0 + bpm * 0.12 + audio.rms * 8.0;
        let label = format!(" {} ", text);
        let width = display_width(&label);
        self.active.push(Active {
            label,
            x: (bounds.x + bounds.w) as f32,
            row: row.min(bounds.y + bounds.h - 1),
            speed,
            likes: comment.likes,
            width,
        });
        self.cursor = self.cursor.wrapping_add(1);
        self.cooldown = (3.8 - audio.rms * 1.2).clamp(1.6, 3.8);
    }

    pub fn draw(&self, buffer: &mut Buffer, palette: &[[u8; 3]], t: f32, bounds: Rect) {
        let base_bg = Rgb::from_slice(palette[0]).scale(0.62);
        for item in &self.active {
            if item.row < bounds.y || item.row >= bounds.y + bounds.h || item.row >= buffer.height {
                continue;
            }
            let x = item.x.max(bounds.x as f32) as usize;
            if x >= bounds.x + bounds.w || x >= buffer.width {
                continue;
            }
            let accent = if item.likes > 500 {
                Rgb::new(255, 236, 180)
            } else {
                Rgb::from_slice(palette[palette.len().saturating_sub(2)])
            };
            let fg = readable_text(base_bg, accent);
            buffer.put_str_clipped(
                x,
                item.row,
                (bounds.x + bounds.w).saturating_sub(x),
                &item.label,
                fg,
                base_bg.lerp(accent, 0.06 + (t * 0.2).sin().abs() * 0.03),
            );
        }
    }
}

fn display_width(text: &str) -> usize {
    text.chars()
        .map(|ch| unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0))
        .sum()
}

fn truncate_width(text: &str, max_width: usize) -> String {
    let mut out = String::new();
    let mut width = 0;
    for ch in text.chars() {
        let next = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + next > max_width {
            break;
        }
        out.push(ch);
        width += next;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::bind::AnimParams;

    #[test]
    fn comments_spawn_and_move() {
        let mut danmaku = Danmaku::new();
        danmaku.set_comments(vec![CommentData {
            text: "前排".into(),
            user: "user".into(),
            likes: 12,
        }]);
        let audio = AudioParams::default();
        let params = AnimParams::default();
        let ctx = FrameCtx {
            width: 80,
            height: 24,
            t: 1.0,
            dt: 0.1,
            audio: &audio,
            params: &params,
            cover: None,
            lyric_index: 0,
            lyric_progress: 0.0,
            lyric_change: 1.0,
            lyric_seed: 7,
            low_quality: false,
        };
        danmaku.cooldown = 0.0;
        let bounds = Rect::new(0, 0, 80, 16);
        danmaku.update(&ctx, &audio, bounds);
        assert_eq!(danmaku.active.len(), 1);
    }
}
