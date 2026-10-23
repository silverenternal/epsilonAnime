use crate::generated::palette_by_name;
use crate::layout::Rect;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::{Color, Print, SetBackgroundColor, SetForegroundColor};
use std::io::{self, Write};
use unicode_width::UnicodeWidthChar;

crate::palette!(
    FALLBACK_PALETTE,
    [5, 5, 16],
    [58, 12, 163],
    [114, 9, 183],
    [247, 37, 133],
    [76, 201, 240],
    [255, 255, 255]
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn from_slice(v: [u8; 3]) -> Self {
        Self::new(v[0], v[1], v[2])
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
        Self::new(
            mix(self.r, other.r),
            mix(self.g, other.g),
            mix(self.b, other.b),
        )
    }

    pub fn screen(self, other: Self) -> Self {
        let f = |a: u8, b: u8| 255 - ((255 - a as u16) * (255 - b as u16) / 255) as u8;
        Self::new(f(self.r, other.r), f(self.g, other.g), f(self.b, other.b))
    }

    pub fn add(self, other: Self) -> Self {
        Self::new(
            self.r.saturating_add(other.r),
            self.g.saturating_add(other.g),
            self.b.saturating_add(other.b),
        )
    }

    pub fn mul(self, other: Self) -> Self {
        Self::new(
            (self.r as u16 * other.r as u16 / 255) as u8,
            (self.g as u16 * other.g as u16 / 255) as u8,
            (self.b as u16 * other.b as u16 / 255) as u8,
        )
    }

    pub fn scale(self, k: f32) -> Self {
        let f = |v: u8| (v as f32 * k).clamp(0.0, 255.0) as u8;
        Self::new(f(self.r), f(self.g), f(self.b))
    }

    pub fn luma(self) -> f32 {
        0.2126 * linear_channel(self.r)
            + 0.7152 * linear_channel(self.g)
            + 0.0722 * linear_channel(self.b)
    }

    pub fn crossterm(self) -> Color {
        Color::Rgb {
            r: self.r,
            g: self.g,
            b: self.b,
        }
    }

    pub fn to_array(self) -> [u8; 3] {
        [self.r, self.g, self.b]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Rgb,
    pub hidden: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Rgb::new(220, 220, 230),
            bg: Rgb::new(0, 0, 0),
            hidden: false,
        }
    }
}

const TILE_W: usize = 8;
const TILE_H: usize = 3;
const COLOR_SLACK: u8 = 2;

#[derive(Debug)]
struct DirtyTiles {
    cols: usize,
    tiles: Vec<bool>,
    count: usize,
}

impl DirtyTiles {
    fn new(width: usize, height: usize) -> Self {
        let cols = width.div_ceil(TILE_W).max(1);
        Self {
            cols,
            tiles: vec![false; cols * height.div_ceil(TILE_H).max(1)],
            count: 0,
        }
    }

    fn mark(&mut self, x: usize, y: usize) {
        let tile = (y / TILE_H) * self.cols + x / TILE_W;
        if !self.tiles[tile] {
            self.tiles[tile] = true;
            self.count += 1;
        }
    }

    fn indices(&self) -> Vec<usize> {
        self.tiles
            .iter()
            .enumerate()
            .filter_map(|(i, dirty)| dirty.then_some(i))
            .collect()
    }

    fn bounds(&self, index: usize) -> (usize, usize, usize, usize) {
        let tx = index % self.cols;
        let ty = index / self.cols;
        (
            tx * TILE_W,
            ty * TILE_H,
            (tx + 1) * TILE_W,
            (ty + 1) * TILE_H,
        )
    }

    fn reset(&mut self) {
        self.tiles.fill(false);
        self.count = 0;
    }
}

pub struct Buffer {
    pub width: usize,
    pub height: usize,
    cells: Vec<Cell>,
    prev: Vec<Cell>,
    dirty: DirtyTiles,
    last_dirty: usize,
}

impl Buffer {
    pub fn new(width: usize, height: usize) -> Self {
        let count = width.saturating_mul(height);
        Self {
            width,
            height,
            cells: vec![Cell::default(); count],
            prev: vec![Cell::default(); count],
            dirty: DirtyTiles::new(width, height),
            last_dirty: 0,
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        if width == self.width && height == self.height {
            return;
        }
        *self = Self::new(width, height);
    }

    pub fn set(&mut self, x: usize, y: usize, ch: char, fg: Rgb, bg: Rgb) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = y * self.width + x;
        let next = Cell {
            ch,
            fg,
            bg,
            hidden: false,
        };
        if self.cells[idx] != next || self.prev[idx] != next {
            self.dirty.mark(x, y);
        }
        self.cells[idx] = next;
    }

    pub fn set_hidden(&mut self, x: usize, y: usize) {
        if x < self.width && y < self.height {
            let idx = y * self.width + x;
            let mut next = self.cells[idx];
            next.hidden = true;
            if self.cells[idx] != next || self.prev[idx] != next {
                self.dirty.mark(x, y);
            }
            self.cells[idx] = next;
        }
    }

    #[cfg(test)]
    pub fn cell(&self, x: usize, y: usize) -> Cell {
        self.cells[y * self.width + x]
    }

    pub fn put_str(&mut self, x: usize, y: usize, text: &str, fg: Rgb, bg: Rgb) -> usize {
        self.put_str_clipped(x, y, self.width.saturating_sub(x), text, fg, bg)
    }

    pub fn put_str_clipped(
        &mut self,
        x: usize,
        y: usize,
        max_width: usize,
        text: &str,
        fg: Rgb,
        bg: Rgb,
    ) -> usize {
        let mut col = x;
        let limit = x.saturating_add(max_width).min(self.width);
        for ch in text.chars() {
            let width = ch.width().unwrap_or(0);
            if width == 0 {
                continue;
            }
            if col + width > limit {
                break;
            }
            self.set(col, y, ch, fg, bg);
            for tail in 1..width {
                self.set_hidden(col + tail, y);
            }
            col += width;
        }
        col
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, bg: Rgb) {
        for row in y..(y + h).min(self.height) {
            for col in x..(x + w).min(self.width) {
                self.set(col, row, ' ', Rgb::new(255, 255, 255), bg);
            }
        }
    }

    pub fn fill_rect_excluding(&mut self, rect: Rect, bg: Rgb, excluded: &[Rect]) {
        for row in rect.y..(rect.y + rect.h).min(self.height) {
            for col in rect.x..(rect.x + rect.w).min(self.width) {
                if !excluded.iter().any(|region| region.contains(col, row)) {
                    self.set(col, row, ' ', Rgb::new(255, 255, 255), bg);
                }
            }
        }
    }

    pub fn glass_rect(&mut self, x: usize, y: usize, w: usize, h: usize, tint: Rgb, opacity: f32) {
        let opacity = opacity.clamp(0.0, 1.0);
        for row in y..(y + h).min(self.height) {
            for col in x..(x + w).min(self.width) {
                let idx = row * self.width + col;
                let under = self.cells[idx];
                let grain = (((col * 17 + row * 31) % 11) as f32 / 10.0 - 0.5) * 0.025;
                let frosted = tint.lerp(Rgb::new(255, 255, 255), grain.max(0.0));
                let bg = under.bg.lerp(frosted, opacity);
                let fg = under.fg.lerp(bg, opacity);
                self.set(col, row, under.ch, fg, bg);
            }
        }
    }

    pub fn set_translucent(
        &mut self,
        x: usize,
        y: usize,
        ch: char,
        fg: Rgb,
        bg: Rgb,
        opacity: f32,
    ) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = y * self.width + x;
        let under = self.cells[idx];
        let opacity = opacity.clamp(0.0, 1.0);
        self.set(
            x,
            y,
            ch,
            under.fg.lerp(fg, opacity),
            under.bg.lerp(bg, opacity),
        );
    }

    pub fn encode(&mut self) -> io::Result<Vec<u8>> {
        let mut out = Vec::new();
        self.encode_to(&mut out)?;
        Ok(out)
    }

    #[cfg(test)]
    pub fn flush<W: Write>(&mut self, out: &mut W) -> io::Result<()> {
        self.encode_to(out)?;
        out.flush()
    }

    fn encode_to<W: Write>(&mut self, out: &mut W) -> io::Result<()> {
        let tiles = self.dirty.indices();
        for tile in &tiles {
            let (x0, y0, x1, y1) = self.dirty.bounds(*tile);
            for y in y0..y1.min(self.height) {
                let mut run_open = false;
                let mut last_fg = None;
                let mut last_bg = None;
                for x in x0..x1.min(self.width) {
                    let idx = y * self.width + x;
                    let cell = self.cells[idx];
                    if cell == self.prev[idx] {
                        run_open = false;
                        last_fg = None;
                        last_bg = None;
                        continue;
                    }
                    self.prev[idx] = cell;
                    if cell.hidden {
                        continue;
                    }
                    if !run_open {
                        queue!(out, MoveTo(x as u16, y as u16))?;
                        run_open = true;
                    }
                    if last_fg.is_none_or(|color| !near(color, cell.fg, COLOR_SLACK)) {
                        queue!(out, SetForegroundColor(cell.fg.crossterm()))?;
                        last_fg = Some(cell.fg);
                    }
                    if last_bg.is_none_or(|color| !near(color, cell.bg, COLOR_SLACK)) {
                        queue!(out, SetBackgroundColor(cell.bg.crossterm()))?;
                        last_bg = Some(cell.bg);
                    }
                    queue!(out, Print(cell.ch))?;
                }
            }
        }
        self.last_dirty = tiles.len();
        self.dirty.reset();
        Ok(())
    }

    pub fn dirty_tile_count(&self) -> usize {
        self.dirty.count
    }

    pub fn last_dirty_tile_count(&self) -> usize {
        self.last_dirty
    }

    pub fn tile_count(&self) -> usize {
        self.dirty.tiles.len()
    }
}

fn near(a: Rgb, b: Rgb, slack: u8) -> bool {
    a.r.abs_diff(b.r) <= slack && a.g.abs_diff(b.g) <= slack && a.b.abs_diff(b.b) <= slack
}

fn linear_channel(value: u8) -> f32 {
    let s = value as f32 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

pub fn contrast_ratio(a: Rgb, b: Rgb) -> f32 {
    let (light, dark) = if a.luma() > b.luma() {
        (a.luma(), b.luma())
    } else {
        (b.luma(), a.luma())
    };
    (light + 0.05) / (dark + 0.05)
}

pub fn readable_text(bg: Rgb, preferred: Rgb) -> Rgb {
    if contrast_ratio(bg, preferred) >= 4.5 {
        return preferred;
    }
    let white = Rgb::new(255, 255, 255);
    let black = Rgb::new(0, 0, 0);
    if contrast_ratio(bg, white) >= contrast_ratio(bg, black) {
        white
    } else {
        black
    }
}

pub fn muted_text(bg: Rgb, preferred: Rgb) -> Rgb {
    let muted = preferred.lerp(bg, 0.28);
    if contrast_ratio(bg, muted) >= 3.0 {
        muted
    } else {
        readable_text(bg, preferred)
    }
}

pub fn value_percentile(values: &[f32], percentile: f32) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut histogram = [0u32; 64];
    for value in values {
        let bin = (value.clamp(0.0, 1.0) * 63.0).round() as usize;
        histogram[bin] += 1;
    }
    let target = (values.len() as f32 * percentile.clamp(0.0, 1.0)) as u32;
    let mut seen = 0;
    for (bin, count) in histogram.iter().enumerate() {
        seen += count;
        if seen >= target {
            return bin as f32 / 63.0;
        }
    }
    1.0
}

pub fn color_for(colors: &[[u8; 3]], v: f32) -> Rgb {
    if colors.is_empty() {
        return Rgb::new(255, 255, 255);
    }
    let t = v.clamp(0.0, 1.0) * (colors.len() - 1) as f32;
    let lo = t.floor() as usize;
    let hi = (lo + 1).min(colors.len() - 1);
    Rgb::from_slice(colors[lo]).lerp(Rgb::from_slice(colors[hi]), t - lo as f32)
}

pub fn find_palette(name: &str) -> &'static [[u8; 3]] {
    palette_by_name(name).unwrap_or(FALLBACK_PALETTE)
}

#[derive(Debug, Clone, Copy)]
pub enum BlendMode {
    Over,
    Add,
    Screen,
    Multiply,
    Lighten,
}

pub fn blend(base: Rgb, top: Rgb, mode: BlendMode, alpha: f32) -> Rgb {
    let alpha = alpha.clamp(0.0, 1.0);
    match mode {
        BlendMode::Over => base.lerp(top, alpha),
        BlendMode::Add => base.add(top).lerp(base, 1.0 - alpha),
        BlendMode::Screen => base.screen(top).lerp(base, 1.0 - alpha),
        BlendMode::Multiply => base.mul(top).lerp(base, 1.0 - alpha),
        BlendMode::Lighten => {
            let light = Rgb::new(base.r.max(top.r), base.g.max(top.g), base.b.max(top.b));
            light.lerp(base, 1.0 - alpha)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn screen_is_bright() {
        let a = Rgb::new(10, 20, 30);
        assert!(a.screen(a).luma() > a.luma());
    }

    #[test]
    fn text_contrast_repairs_low_contrast() {
        let bg = Rgb::new(20, 20, 20);
        let repaired = readable_text(bg, Rgb::new(30, 30, 30));
        assert!(contrast_ratio(bg, repaired) >= 4.5);
    }

    #[test]
    fn buffer_handles_wide_glyph() {
        let mut b = Buffer::new(10, 2);
        b.put_str(0, 0, "你好", Rgb::new(255, 255, 255), Rgb::new(0, 0, 0));
        assert!(b.cell(1, 0).hidden);
        assert!(!b.cell(2, 0).hidden);
    }

    #[test]
    fn flush_emits_truecolor() {
        crossterm::style::Colored::set_ansi_color_disabled(false);
        let mut b = Buffer::new(2, 1);
        b.set(0, 0, '@', Rgb::new(12, 34, 56), Rgb::new(1, 2, 3));
        let mut out = Vec::new();
        b.flush(&mut out).unwrap();
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("38;2;12;34;56"), "{text:?}");
    }

    #[test]
    fn dirty_tiles_track_changes() {
        let mut b = Buffer::new(32, 12);
        b.set(0, 0, 'a', Rgb::new(1, 2, 3), Rgb::new(0, 0, 0));
        assert_eq!(b.dirty_tile_count(), 1);
        b.flush(&mut Vec::new()).unwrap();
        assert_eq!(b.last_dirty_tile_count(), 1);
        b.set(0, 0, 'a', Rgb::new(1, 2, 3), Rgb::new(0, 0, 0));
        assert_eq!(b.dirty_tile_count(), 0);
    }

    #[test]
    fn glass_overlay_composites_at_eighty_percent() {
        let mut b = Buffer::new(8, 4);
        b.fill_rect(0, 0, 8, 4, Rgb::new(20, 40, 60));
        let under = b.cell(1, 1);
        let tint = Rgb::new(180, 160, 120);
        b.glass_rect(0, 0, 8, 4, tint, 0.8);
        let glass = b.cell(1, 1);
        assert_eq!(glass.bg, under.bg.lerp(tint, 0.8));
        assert_ne!(glass.bg, tint);
    }

    #[test]
    fn opaque_fill_can_preserve_glass_overlap_regions() {
        let mut buffer = Buffer::new(8, 4);
        buffer.fill_rect(0, 0, 8, 4, Rgb::new(20, 40, 60));
        buffer.fill_rect_excluding(
            Rect::new(0, 0, 8, 4),
            Rgb::new(200, 20, 20),
            &[Rect::new(2, 1, 2, 2)],
        );
        assert_eq!(buffer.cell(2, 1).bg, Rgb::new(20, 40, 60));
        assert_eq!(buffer.cell(0, 0).bg, Rgb::new(200, 20, 20));
    }

    #[test]
    fn tiny_frame_matches_snapshot() {
        let mut b = Buffer::new(2, 1);
        b.set(0, 0, '@', Rgb::new(12, 34, 56), Rgb::new(1, 2, 3));
        b.set(1, 0, 'x', Rgb::new(10, 20, 30), Rgb::new(4, 5, 6));
        let encoded = b.encode().unwrap();
        assert_eq!(
            String::from_utf8(encoded).unwrap(),
            include_str!("../tests/snapshots/tiny-frame.ansi").trim_end()
        );
    }

    proptest! {
        #[test]
        fn arbitrary_cells_never_panic(glyphs in prop::collection::vec(any::<char>(), 0..512)) {
            let mut buffer = Buffer::new(32, 8);
            for (index, glyph) in glyphs.into_iter().enumerate() {
                buffer.set(
                    index % 32,
                    (index / 32) % 8,
                    glyph,
                    Rgb::new(index as u8, (index >> 8) as u8, 17),
                    Rgb::new(0, 0, 0),
                );
            }
            let _ = buffer.encode();
        }
    }
}
