#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Charset {
    Ascii,
    Blocks,
    Quadrants,
    Braille,
    Mixed,
}

impl Charset {
    pub const ALL: [Charset; 5] = [
        Charset::Ascii,
        Charset::Blocks,
        Charset::Quadrants,
        Charset::Braille,
        Charset::Mixed,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Ascii => "ascii",
            Self::Blocks => "blocks",
            Self::Quadrants => "quadrants",
            Self::Braille => "braille",
            Self::Mixed => "mixed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ascii" => Some(Self::Ascii),
            "block" | "blocks" => Some(Self::Blocks),
            "quad" | "quadrants" => Some(Self::Quadrants),
            "braille" => Some(Self::Braille),
            "mixed" | "extended" | "unicode" => Some(Self::Mixed),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
struct Glyph {
    ch: char,
    coverage: f32,
}

macro_rules! glyphs {
    ($(($ch:expr, $coverage:expr)),+ $(,)?) => {
        &[$(Glyph { ch: $ch, coverage: $coverage }),+]
    };
}

crate::ascii_ramp!(' ', '.', ':', '-', '=', '+', '*', '#', '%', '@');

const BLOCKS: &[Glyph] = glyphs![
    (' ', 0.00),
    ('▁', 0.125),
    ('░', 0.25),
    ('▂', 0.25),
    ('▃', 0.375),
    ('▄', 0.50),
    ('▒', 0.50),
    ('▅', 0.625),
    ('▆', 0.75),
    ('▓', 0.75),
    ('▇', 0.875),
    ('█', 1.00),
];

const QUADRANTS: &[Glyph] = glyphs![
    (' ', 0.00),
    ('▘', 0.25),
    ('▝', 0.25),
    ('▖', 0.25),
    ('▗', 0.25),
    ('▚', 0.50),
    ('▞', 0.50),
    ('▄', 0.50),
    ('▙', 0.75),
    ('▛', 0.75),
    ('▜', 0.75),
    ('▟', 0.75),
    ('▓', 0.75),
    ('█', 1.00),
];

const MIXED: &[Glyph] = glyphs![
    (' ', 0.00),
    ('·', 0.04),
    ('.', 0.07),
    (':', 0.10),
    ('▁', 0.125),
    ('-', 0.14),
    ('=', 0.20),
    ('░', 0.25),
    ('▂', 0.25),
    ('▘', 0.25),
    ('▝', 0.25),
    ('▖', 0.25),
    ('▗', 0.25),
    ('+', 0.28),
    ('*', 0.34),
    ('▃', 0.375),
    ('#', 0.45),
    ('▄', 0.50),
    ('▒', 0.50),
    ('▚', 0.50),
    ('▞', 0.50),
    ('%', 0.58),
    ('▅', 0.625),
    ('▆', 0.75),
    ('▓', 0.75),
    ('▙', 0.75),
    ('▛', 0.75),
    ('▜', 0.75),
    ('▟', 0.75),
    ('▇', 0.875),
    ('█', 1.00),
];

const BAYER: [[f32; 4]; 4] = [
    [0.0, 8.0, 2.0, 10.0],
    [12.0, 4.0, 14.0, 6.0],
    [3.0, 11.0, 1.0, 9.0],
    [15.0, 7.0, 13.0, 5.0],
];

pub const BRAILLE_BITS: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

pub fn tone_map(v: f32) -> f32 {
    let lifted = ((v - 0.018) / 0.965).clamp(0.0, 1.0);
    lifted.powf(0.88)
}

pub fn bayer(x: usize, y: usize) -> f32 {
    (BAYER[y & 3][x & 3] + 0.5) / 16.0 - 0.5
}

pub fn glyph_for(v: f32, x: usize, y: usize, t: f32, charset: Charset) -> char {
    let jitter = bayer(x, y) * 0.03;
    let shaped = (v + jitter).clamp(0.0, 1.0);
    match charset {
        Charset::Ascii => {
            let idx = (shaped * (ASCII_RAMP.len() - 1) as f32).round() as usize;
            ASCII_RAMP[idx.min(ASCII_RAMP.len() - 1)]
        }
        Charset::Blocks => pick(BLOCKS, shaped),
        Charset::Quadrants => pick(QUADRANTS, shaped),
        Charset::Mixed => pick(MIXED, shaped),
        Charset::Braille => braille(shaped, x, y, t),
    }
}

fn pick(table: &[Glyph], v: f32) -> char {
    let mut best = table[0];
    let mut distance = f32::INFINITY;
    for glyph in table {
        let d = (glyph.coverage - v).abs();
        if d < distance {
            best = *glyph;
            distance = d;
        }
    }
    best.ch
}

fn braille(v: f32, x: usize, y: usize, t: f32) -> char {
    let level = (v * 8.0).round().clamp(0.0, 8.0) as u32;
    if level == 0 {
        return ' ';
    }
    let mut mask = if level >= 8 {
        0xFF
    } else {
        (1u32 << level) - 1
    };
    let phase = ((x / 2 + y / 3) as u32 + (t * 1.7) as u32) & 7;
    mask = mask.rotate_right(phase);
    char::from_u32(0x2800 + mask).unwrap_or(' ')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_cover_full_range() {
        for charset in Charset::ALL {
            assert_eq!(glyph_for(0.0, 0, 0, 0.0, charset), ' ');
            assert_ne!(glyph_for(1.0, 0, 0, 0.0, charset), ' ');
        }
    }

    #[test]
    fn mixed_glyphs_are_ordered() {
        assert!(
            MIXED
                .windows(2)
                .all(|pair| pair[0].coverage <= pair[1].coverage)
        );
    }

    #[test]
    fn dithered_edge_varies() {
        let chars: std::collections::HashSet<char> = (0..16)
            .map(|i| glyph_for(0.23, i, i / 4, 0.0, Charset::Mixed))
            .collect();
        assert!(chars.len() > 1);
    }
}
