use crate::anim::{Animation, FieldSample, FrameCtx};
use crate::glyphs::{self, Charset};
use crate::render::{BlendMode, Buffer, Rgb, blend, color_for, value_percentile};
use rayon::prelude::*;
use std::cell::RefCell;

#[derive(Clone, Copy, Default)]
struct StyledCell {
    glyph: char,
    fg: Rgb,
    bg: Rgb,
}

struct Scratch {
    values: Vec<FieldSample>,
    luminance: Vec<f32>,
    styled: Vec<StyledCell>,
}

impl Scratch {
    fn new() -> Self {
        Self {
            values: Vec::new(),
            luminance: Vec::new(),
            styled: Vec::new(),
        }
    }
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::new());
}

pub struct RenderOptions<'a> {
    pub palette: &'a [[u8; 3]],
    pub charset: Charset,
    pub exposure: f32,
    pub occluded: &'a [bool],
    pub low_quality: bool,
}

pub fn render_background(
    engine: &dyn Animation,
    buffer: &mut Buffer,
    ctx: &FrameCtx<'_>,
    options: RenderOptions<'_>,
) -> f32 {
    SCRATCH.with(|scratch| {
        render_with_scratch(engine, buffer, ctx, options, &mut scratch.borrow_mut())
    })
}

fn render_with_scratch(
    engine: &dyn Animation,
    buffer: &mut Buffer,
    ctx: &FrameCtx<'_>,
    options: RenderOptions<'_>,
    scratch: &mut Scratch,
) -> f32 {
    let RenderOptions {
        palette,
        charset,
        exposure,
        occluded,
        low_quality,
    } = options;
    let width = buffer.width;
    let height = buffer.height;
    let sub_w = width * 2;
    let sub_h = height * 4;
    let sub_count = sub_w * sub_h;
    scratch.values.resize(sub_count, FieldSample::default());
    scratch.luminance.resize(sub_count, 0.0);
    scratch.styled.resize(
        width * height,
        StyledCell {
            glyph: ' ',
            fg: Rgb::default(),
            bg: Rgb::default(),
        },
    );
    let Scratch {
        values,
        luminance,
        styled,
    } = scratch;
    let sample = |sx: usize, sy: usize| {
        let x = (sx as f32 + 0.5) / sub_w.max(1) as f32;
        let y = (sy as f32 + 0.5) / sub_h.max(1) as f32;
        let mut field = engine.field(x, y, ctx);
        field.intensity = if field.intensity.is_finite() {
            field.intensity.clamp(0.0, 1.0)
        } else {
            0.0
        };
        field.depth = if field.depth.is_finite() {
            field.depth.clamp(0.0, 1.0)
        } else {
            0.5
        };
        field.light = if field.light.is_finite() {
            field.light.clamp(0.0, 1.25)
        } else {
            1.0
        };
        field
    };
    if low_quality && sub_count >= 4096 {
        values
            .par_chunks_mut(sub_w.saturating_mul(2))
            .enumerate()
            .for_each(|(row_pair, rows)| {
                let sy = (row_pair * 2).min(sub_h.saturating_sub(1));
                for sx in (0..sub_w).step_by(2) {
                    let field = sample(sx, sy);
                    for dy in 0..2 {
                        let row = dy * sub_w;
                        if row >= rows.len() {
                            continue;
                        }
                        for dx in 0..2 {
                            if sx + dx < sub_w && row + sx + dx < rows.len() {
                                rows[row + sx + dx] = field;
                            }
                        }
                    }
                }
            });
    } else if sub_count >= 4096 {
        values
            .par_iter_mut()
            .enumerate()
            .for_each(|(index, field)| *field = sample(index % sub_w, index / sub_w));
    } else {
        values
            .iter_mut()
            .enumerate()
            .for_each(|(index, field)| *field = sample(index % sub_w, index / sub_w));
    }
    for (index, field) in values.iter().enumerate() {
        luminance[index] = field.intensity;
    }
    let p90 = value_percentile(luminance, 0.90);
    let target_exposure = (0.70 / p90.max(0.055)).clamp(0.55, 2.6);
    let exposure = exposure + (target_exposure - exposure) * 0.10;
    let dark = Rgb::from_slice(palette[0]).scale(0.55);
    let style = |cell: usize| -> StyledCell {
        if occluded.get(cell).copied().unwrap_or(false) {
            return StyledCell {
                glyph: ' ',
                fg: dark,
                bg: dark,
            };
        }
        let y = cell / width;
        let x = cell % width;
        let mut mask = 0u8;
        let mut sum = 0.0;
        let mut max: f32 = 0.0;
        let mut min: f32 = 1.0;
        let mut depth_sum = 0.0;
        let focus_dx = x as f32 / width.max(1) as f32 - 0.68;
        let focus_dy = y as f32 / height.max(1) as f32 - 0.48;
        let composition =
            0.42 + 0.58 * (-(focus_dx * focus_dx * 3.2 + focus_dy * focus_dy * 5.0)).exp();
        for sy in 0..4 {
            for sx in 0..2 {
                let index = (y * 4 + sy) * sub_w + x * 2 + sx;
                let sample = values[index];
                depth_sum += sample.depth;
                let dx = (x * 2 + sx) as f32 / sub_w.max(1) as f32 - 0.5;
                let dy = (y * 4 + sy) as f32 / sub_h.max(1) as f32 - 0.5;
                let vignette = 1.0 - 0.17 * (2.0 * (dx * dx + dy * dy)).min(1.0);
                let shaped = glyphs::tone_map(
                    sample.intensity
                        * sample.light
                        * vignette
                        * composition
                        * exposure
                        * ctx.params.brightness,
                );
                sum += shaped;
                max = max.max(shaped);
                min = min.min(shaped);
                let edge = max - min;
                let signal = edge * 0.90 + shaped * 0.16 + (shaped - 0.72).max(0.0) * 1.15;
                let threshold = 0.08 + (glyphs::bayer(x * 2 + sx, y * 4 + sy) + 0.5) * 0.42;
                if signal > threshold {
                    mask |= glyphs::BRAILLE_BITS[sy][sx];
                }
            }
        }
        let average = sum / 8.0;
        let depth = depth_sum / 8.0;
        let subcell = matches!(charset, Charset::Braille | Charset::Mixed);
        let glyph = if subcell {
            if mask == 0 {
                ' '
            } else {
                char::from_u32(0x2800 + mask as u32).unwrap_or(' ')
            }
        } else {
            glyphs::glyph_for(average, x, y, ctx.t, charset)
        };
        let mut fg = color_for(palette, (average * 1.08).min(1.0));
        fg = fg.lerp(color_for(palette, 0.28 + depth * 0.62), 0.16);
        let highlight =
            ((max - 0.80) / 0.20).clamp(0.0, 1.0).powi(2) * (0.22 + ctx.params.pulse * 0.45);
        fg = fg.lerp(Rgb::new(255, 255, 255), highlight);
        let bg = dark.lerp(fg, average * 0.07);
        StyledCell {
            glyph,
            fg,
            bg: blend(bg, fg, BlendMode::Add, ctx.params.pulse * 0.06),
        }
    };
    if low_quality && width * height >= 4096 {
        styled
            .par_chunks_mut(width.saturating_mul(2))
            .enumerate()
            .for_each(|(row_pair, rows)| {
                let y = (row_pair * 2).min(height.saturating_sub(1));
                for x in (0..width).step_by(2) {
                    let cell = style(y * width + x);
                    for dy in 0..2 {
                        let row = dy * width;
                        if row >= rows.len() {
                            continue;
                        }
                        for dx in 0..2 {
                            if x + dx < width && row + x + dx < rows.len() {
                                rows[row + x + dx] = cell;
                            }
                        }
                    }
                }
            });
    } else if width * height >= 4096 {
        styled
            .par_iter_mut()
            .enumerate()
            .for_each(|(cell, output)| *output = style(cell));
    } else {
        for (cell, output) in styled.iter_mut().enumerate() {
            *output = style(cell);
        }
    }
    for (index, styled) in styled.iter().enumerate() {
        buffer.set(
            index % width,
            index / width,
            styled.glyph,
            styled.fg,
            styled.bg,
        );
    }
    exposure
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::AudioParams;
    use crate::anim::bind::AnimParams;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingAnimation {
        calls: AtomicUsize,
    }

    impl Animation for CountingAnimation {
        fn sample(&self, x: f32, y: f32, _ctx: &FrameCtx<'_>) -> f32 {
            self.calls.fetch_add(1, Ordering::Relaxed);
            (x + y).fract()
        }
    }

    #[test]
    fn low_quality_reduces_math_sampling_work() {
        let audio = AudioParams::default();
        let params = AnimParams::default();
        let ctx = FrameCtx {
            width: 120,
            height: 40,
            t: 1.0,
            dt: 1.0 / 30.0,
            audio: &audio,
            params: &params,
            cover: None,
            lyric_index: 0,
            lyric_progress: 0.0,
            lyric_change: 0.0,
            lyric_seed: 1,
            low_quality: true,
        };
        let animation = CountingAnimation {
            calls: AtomicUsize::new(0),
        };
        let mut buffer = Buffer::new(120, 40);
        render_background(
            &animation,
            &mut buffer,
            &ctx,
            RenderOptions {
                palette: &[[0, 0, 0], [255, 255, 255]],
                charset: Charset::Braille,
                exposure: 1.0,
                occluded: &[],
                low_quality: true,
            },
        );
        let full_samples = 120 * 2 * 40 * 4;
        assert!(animation.calls.load(Ordering::Relaxed) < full_samples / 2);
    }
}
