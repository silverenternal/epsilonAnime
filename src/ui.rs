use crate::render::{Buffer, Rgb, color_for, readable_text};

pub fn draw_status(buffer: &mut Buffer, status: &str, palette: &[[u8; 3]]) {
    if buffer.height == 0 {
        return;
    }
    let status = if status.is_empty() {
        "快捷键 │ l 音乐库 │ i 输入链接"
    } else {
        status
    };
    let msg = format!(" F1 │ {status} ");
    let bg = color_for(palette, 0.42).scale(0.30);
    let fg = readable_text(bg, Rgb::new(255, 245, 220));
    let x = buffer.width.saturating_sub(msg.chars().count() + 2);
    buffer.put_str(x, buffer.height - 1, &msg, fg, bg);
}

pub struct Stats<'a> {
    pub fps: f32,
    pub rms: f32,
    pub bpm: f32,
    pub exposure: f32,
    pub danmaku_active: usize,
    pub danmaku_total: usize,
    pub label: &'a str,
    pub sample_ms: f32,
    pub encode_ms: f32,
    pub terminal_wait_ms: f32,
    pub palette: &'a [[u8; 3]],
}

pub fn draw_stats(buffer: &mut Buffer, stats: &Stats<'_>) {
    if buffer.height == 0 {
        return;
    }
    let bg = Rgb::from_slice(stats.palette[0]).scale(0.7);
    let msg = format!(
        " {} · {:.0} fps · dirty {}/{} · flush {} · s/e/w {:.1}/{:.1}/{:.1}ms · dm {}/{} · rms {:.2} · {:.0}bpm · ev {:.2} ",
        stats.label,
        stats.fps,
        buffer.dirty_tile_count(),
        buffer.tile_count(),
        buffer.last_dirty_tile_count(),
        stats.sample_ms,
        stats.encode_ms,
        stats.terminal_wait_ms,
        stats.danmaku_active,
        stats.danmaku_total,
        stats.rms,
        stats.bpm,
        stats.exposure
    );
    let fg = readable_text(bg, color_for(stats.palette, 0.75));
    buffer.put_str_clipped(1, 0, msg.chars().count(), &msg, fg, bg);
}

pub fn draw_prompt(buffer: &mut Buffer, prompt: &str, palette: &[[u8; 3]], label: &str) {
    if buffer.height == 0 {
        return;
    }
    let bg = color_for(palette, 0.65).scale(0.35);
    let fg = readable_text(bg, Rgb::new(255, 255, 255));
    let accent = readable_text(bg, color_for(palette, 0.9));
    let y = buffer.height - 1;
    buffer.fill_rect(0, y, buffer.width, 1, bg);
    let prefix = format!(" {label} ▸ ");
    buffer.put_str_clipped(1, y, buffer.width.saturating_sub(2), &prefix, accent, bg);
    let x = 1 + prefix.chars().count();
    let text = format!("{prompt}▌");
    buffer.put_str_clipped(x, y, buffer.width.saturating_sub(x + 1), &text, fg, bg);
}
