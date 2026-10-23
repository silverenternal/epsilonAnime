use crate::layout::Rect;
use crate::render::{BlendMode, Buffer, Rgb, blend, color_for, readable_text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceStyle {
    Opaque,
    Glass,
    Transparent,
}

pub fn draw_frame(
    buf: &mut Buffer,
    rect: Rect,
    title: &str,
    palette: &[[u8; 3]],
    dragging: bool,
    surface_style: SurfaceStyle,
) {
    draw_frame_with_regions(buf, rect, title, palette, dragging, surface_style, &[]);
}

#[allow(clippy::too_many_arguments)]
pub fn draw_frame_with_regions(
    buf: &mut Buffer,
    rect: Rect,
    title: &str,
    palette: &[[u8; 3]],
    dragging: bool,
    surface_style: SurfaceStyle,
    glass_regions: &[Rect],
) {
    if rect.w < 2 || rect.h < 2 {
        return;
    }
    let shadow = Rgb::new(0, 0, 0).lerp(Rgb::from_slice(palette[0]), 0.35);
    if rect.x + rect.w < buf.width {
        for y in rect.y + 1..(rect.y + rect.h).min(buf.height) {
            buf.set(rect.x + rect.w, y, ' ', shadow, shadow);
        }
    }
    if rect.y + rect.h < buf.height {
        for x in rect.x + 2..(rect.x + rect.w).min(buf.width) {
            buf.set(x, rect.y + rect.h, ' ', shadow, shadow);
        }
    }
    let bg = blend(
        Rgb::new(0, 0, 0),
        Rgb::from_slice(palette[0]).scale(0.52),
        BlendMode::Over,
        1.0,
    );
    let border = blend(
        color_for(palette, if dragging { 0.9 } else { 0.58 }),
        color_for(palette, 0.95),
        BlendMode::Lighten,
        if dragging { 0.35 } else { 0.12 },
    );
    let side_border = border.scale(0.72);
    let top_border = border.lerp(Rgb::new(255, 255, 255), 0.08);
    let accent = Rgb::from_slice(*palette.last().unwrap_or(&[255, 255, 255]));
    let label_color = readable_text(bg, accent);
    match surface_style {
        SurfaceStyle::Opaque => buf.fill_rect(rect.x, rect.y, rect.w, rect.h, bg),
        SurfaceStyle::Glass if glass_regions.is_empty() => {
            buf.glass_rect(rect.x, rect.y, rect.w, rect.h, bg, 0.8);
        }
        SurfaceStyle::Glass => {
            for region in glass_regions {
                buf.glass_rect(region.x, region.y, region.w, region.h, bg, 0.8);
            }
            buf.fill_rect_excluding(rect, bg, glass_regions);
        }
        SurfaceStyle::Transparent => {}
    }
    let right = rect.x + rect.w - 1;
    let bottom = rect.y + rect.h - 1;
    buf.set(rect.x, rect.y, '┌', top_border, bg);
    buf.set(right, rect.y, '┐', top_border, bg);
    buf.set(rect.x, bottom, '└', side_border, bg);
    buf.set(right, bottom, '◢', side_border, bg);
    for x in rect.x + 1..right {
        buf.set(x, rect.y, '─', top_border, bg);
        buf.set(x, bottom, '─', side_border, bg);
    }
    for y in rect.y + 1..bottom {
        buf.set(rect.x, y, '│', side_border, bg);
        buf.set(right, y, '│', side_border, bg);
    }
    let label = format!(" {title} ");
    let title_bg = bg.lerp(border, 0.10);
    buf.put_str_clipped(
        rect.x + 2,
        rect.y,
        rect.w.saturating_sub(3),
        &label,
        label_color,
        title_bg,
    );
    if dragging {
        let handle = " ⇲ ";
        let x = right.saturating_sub(handle.chars().count() + 1);
        buf.put_str(x, rect.y, handle, label_color, bg);
    }
}
