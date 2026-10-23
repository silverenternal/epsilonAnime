use crate::layout::Rect;
use crate::render::{Buffer, Rgb, color_for, readable_text};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControlAction {
    Previous,
    TogglePause,
    Next,
    Seek(f32),
}

pub struct ControlDisplay<'a> {
    pub paused: bool,
    pub playlist_line: &'a str,
    pub progress: f32,
    pub volume: f32,
    pub mode: &'a str,
}

pub fn draw(buffer: &mut Buffer, rect: Rect, palette: &[[u8; 3]], display: ControlDisplay<'_>) {
    let ControlDisplay {
        paused,
        playlist_line,
        progress,
        volume,
        mode,
    } = display;
    let inner = rect.inner();
    if inner.h < 2 || inner.w < 20 {
        return;
    }
    let bg = Rgb::from_slice(palette[0]).scale(0.48);
    let muted = readable_text(bg, color_for(palette, 0.65));
    let progress_y = inner.y + inner.h.saturating_sub(2);
    if inner.h >= 3 {
        let width = inner.w.saturating_sub(12);
        let filled = (width as f32 * progress.clamp(0.0, 1.0)) as usize;
        let track = color_for(palette, 0.42).scale(0.45);
        let fill = color_for(palette, 0.82);
        buffer.fill_rect(inner.x + 2, progress_y, width, 1, track);
        buffer.fill_rect(inner.x + 2, progress_y, filled, 1, fill);
        let info = if volume <= 0.001 {
            format!("MUTED · {mode}")
        } else {
            format!("vol {:>3.0}% · {mode}", volume * 100.0)
        };
        buffer.put_str_clipped(
            inner.x + width + 3,
            progress_y,
            inner.w.saturating_sub(width + 3),
            &info,
            muted,
            bg,
        );
    }
    if !playlist_line.is_empty() && inner.h >= 4 {
        buffer.put_str_clipped(
            inner.x,
            inner.y + inner.h - 3,
            inner.w,
            playlist_line,
            muted,
            bg,
        );
    }
    let y = inner.y + inner.h - 1;
    let third = inner.w / 3;
    let labels = [
        "  ◀ PREV  ",
        if paused {
            "  ▶ PLAY  "
        } else {
            "  ⏸ PAUSE "
        },
        "  NEXT ▶  ",
    ];
    for (index, label) in labels.iter().enumerate() {
        let x = inner.x + index * third;
        let width = if index == 2 {
            inner.w.saturating_sub(index * third)
        } else {
            third
        };
        buffer.put_str_clipped(
            x,
            y,
            width,
            label,
            readable_text(bg, color_for(palette, 0.9)),
            bg,
        );
    }
}

pub fn action_at(rect: Rect, x: usize, y: usize) -> Option<ControlAction> {
    let inner = rect.inner();
    if inner.h == 0 || x < inner.x || x >= inner.x + inner.w {
        return None;
    }
    if y == inner.y + inner.h.saturating_sub(2) {
        return Some(ControlAction::Seek(
            (x - inner.x) as f32 / inner.w.max(1) as f32,
        ));
    }
    if y != inner.y + inner.h - 1 {
        return None;
    }
    let third = (inner.w / 3).max(1);
    match (x - inner.x) / third {
        0 => Some(ControlAction::Previous),
        1 => Some(ControlAction::TogglePause),
        _ => Some(ControlAction::Next),
    }
}
