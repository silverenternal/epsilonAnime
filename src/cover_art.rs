use crate::anim::FrameCtx;
use crate::cover::Cover;
use crate::glyphs::{self, Charset};
use crate::render::{BlendMode, Buffer, Rgb, blend, color_for};
use std::f32::consts::TAU;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverStyle {
    Braille,
    Hybrid,
    HalfBlock,
}

impl CoverStyle {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "braille" | "dots" => Some(Self::Braille),
            "hybrid" | "auto" => Some(Self::Hybrid),
            "half" | "halfblock" | "colour" | "color" => Some(Self::HalfBlock),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Braille => "braille",
            Self::Hybrid => "hybrid",
            Self::HalfBlock => "halfblock",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Braille => Self::Hybrid,
            Self::Hybrid => Self::HalfBlock,
            Self::HalfBlock => Self::Braille,
        }
    }
}

struct Particle {
    target: (f32, f32),
    pos: (f32, f32),
    color: Rgb,
    luma: f32,
    edge: f32,
    salience: f32,
    phase: f32,
}

#[derive(Clone, Copy, Default)]
struct DotStats {
    energy: f32,
    r: f32,
    g: f32,
    b: f32,
    luma: f32,
    edge: f32,
    salience: f32,
}

#[derive(Clone, Copy, Default)]
struct BrailleCell {
    dots: [DotStats; 8],
}

#[derive(Clone, Copy, Default)]
struct CoverSample {
    color: Rgb,
    luma: f32,
    edge: f32,
    chroma: f32,
}

pub struct CoverArt {
    particles: Vec<Particle>,
    age: f32,
    beat_phase: f32,
    bpm: f32,
    step_accum: f32,
    burst: f32,
    scratch: Vec<BrailleCell>,
    source_count: usize,
    local_gain: f32,
}

impl CoverArt {
    pub fn new(cover: &Cover) -> Self {
        let (gw, gh) = cover.particle_grid();
        let particles = cover
            .particles()
            .iter()
            .enumerate()
            .filter(|(i, particle)| i % 4 == 0 || particle.edge > 96 || particle.salience > 215)
            .map(|(_, particle)| particle)
            .map(|particle| {
                let tx = particle.x as f32 / gw.max(1) as f32;
                let ty = particle.y as f32 / gh.max(1) as f32;
                let h = hash2(particle.x, particle.y);
                let angle = h * TAU;
                let radius = 0.45 + ((h * 17.0).fract() * 0.55);
                Self::particle(
                    tx,
                    ty,
                    (tx + angle.cos() * radius * 2.0, ty + angle.sin() * radius),
                    particle.color,
                    particle.luma,
                    particle.edge,
                    particle.salience,
                    angle,
                )
            })
            .collect();
        let mean_luma = if cover.particles().is_empty() {
            0.35
        } else {
            cover
                .particles()
                .iter()
                .map(|particle| particle.luma as f32 / 255.0)
                .sum::<f32>()
                / cover.particles().len() as f32
        };
        Self {
            particles,
            age: 0.0,
            beat_phase: 0.0,
            bpm: 120.0,
            step_accum: 0.0,
            burst: 0.0,
            scratch: Vec::new(),
            source_count: cover.particles().len(),
            local_gain: (0.46 / mean_luma.max(0.08)).clamp(0.65, 1.20),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn particle(
        target_x: f32,
        target_y: f32,
        pos: (f32, f32),
        color: Rgb,
        luma: u8,
        edge: u8,
        salience: u8,
        phase: f32,
    ) -> Particle {
        Particle {
            target: (target_x, target_y),
            pos,
            color,
            luma: luma as f32 / 255.0,
            edge: edge as f32 / 255.0,
            salience: salience as f32 / 255.0,
            phase,
        }
    }

    pub fn update(&mut self, ctx: &FrameCtx<'_>) {
        let incoming_bpm = if ctx.audio.bpm > 20.0 {
            ctx.audio.bpm
        } else {
            120.0
        };
        self.bpm += (incoming_bpm - self.bpm) * ctx.dt.min(0.2) * 1.5;
        let beat_seconds = 60.0 / self.bpm.max(40.0);
        let sample_step = (beat_seconds / 8.0).clamp(0.018, 0.10);
        self.step_accum += ctx.dt;
        while self.step_accum >= sample_step {
            self.step_accum -= sample_step;
            self.beat_phase = (self.beat_phase + sample_step / beat_seconds).fract();
        }
        if ctx.audio.onset > 0.5 || ctx.lyric_change > 0.9 {
            self.burst = 1.0;
        }
        self.burst = (self.burst - ctx.dt * 1.8).max(0.0);
        self.age += ctx.dt;
        let assemble = smoothstep((self.age / 2.4).clamp(0.0, 1.0));
        let pulse = (1.0 - self.beat_phase).powi(6) * (0.3 + ctx.audio.bass);
        let response = (ctx.dt * 9.0).clamp(0.02, 0.35);
        for particle in &mut self.particles {
            let twist = particle.edge * (self.beat_phase * TAU + particle.phase).sin() * 0.012;
            let tx = particle.target.0
                + twist
                + particle.salience * (ctx.t * 0.7 + particle.phase).sin() * 0.008;
            let ty = particle.target.1
                + particle.edge * (self.beat_phase * TAU * 0.5 + particle.phase).cos() * 0.010;
            particle.pos.0 += (tx - particle.pos.0) * response * (0.35 + assemble * 0.65);
            particle.pos.1 += (ty - particle.pos.1) * response * (0.35 + assemble * 0.65);
            let dx = particle.pos.0 - 0.5;
            let dy = particle.pos.1 - 0.5;
            let kick = (pulse * 0.006 + self.burst * particle.salience * 0.018)
                * (particle.phase + ctx.t).sin();
            particle.pos.0 += dx * kick;
            particle.pos.1 += dy * kick;
        }
    }

    pub fn particle_count(&self) -> usize {
        self.source_count
    }

    pub fn reset_view(&mut self) {
        self.age = self.age.max(2.4);
        self.step_accum = 0.0;
        self.scratch.clear();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        buffer: &mut Buffer,
        rect: crate::layout::Rect,
        palette: &[[u8; 3]],
        charset: Charset,
        style: CoverStyle,
        t: f32,
        alpha: f32,
        exposure: f32,
        cover: &Cover,
    ) {
        let inner = rect.inner();
        if inner.w == 0 || inner.h == 0 {
            return;
        }
        let bg = Rgb::from_slice(palette[0]).scale(0.30);
        let base_strength = smoothstep((self.age / 2.2).clamp(0.0, 1.0));
        self.scratch
            .resize(inner.w * inner.h, BrailleCell::default());
        self.scratch.fill(BrailleCell::default());
        let stats = &mut self.scratch;
        let sub_w = inner.w * 2;
        let sub_h = inner.h * 4;
        for particle in &self.particles {
            let sx = (particle.pos.0.clamp(0.0, 0.9999) * sub_w as f32) as usize;
            let sy = (particle.pos.1.clamp(0.0, 0.9999) * sub_h as f32) as usize;
            let cell_x = sx / 2;
            let cell_y = sy / 4;
            let dot = (sy & 3) * 2 + (sx & 1);
            let cell = &mut stats[cell_y * inner.w + cell_x];
            let dot = &mut cell.dots[dot];
            let mass = 0.15 + particle.luma * 0.9 + particle.edge * 0.6;
            dot.energy += mass;
            dot.r += particle.color.r as f32 * mass;
            dot.g += particle.color.g as f32 * mass;
            dot.b += particle.color.b as f32 * mass;
            dot.luma = dot.luma.max(particle.luma);
            dot.edge = dot.edge.max(particle.edge);
            dot.salience = dot.salience.max(particle.salience);
        }
        let mut field = vec![CoverSample::default(); sub_w * sub_h];
        for y in 0..sub_h {
            for x in 0..sub_w {
                let u = (x as f32 + 0.5) / sub_w as f32;
                let v = (y as f32 + 0.5) / sub_h as f32;
                let color = cover.sample_linear(u, v);
                let dx = 1.35 / sub_w as f32;
                let dy = 1.35 / sub_h as f32;
                let ex = (cover.sample_linear((u + dx).min(1.0), v).luma()
                    - cover.sample_linear((u - dx).max(0.0), v).luma())
                .abs();
                let ey = (cover.sample_linear(u, (v + dy).min(1.0)).luma()
                    - cover.sample_linear(u, (v - dy).max(0.0)).luma())
                .abs();
                let hi = color.r.max(color.g).max(color.b) as f32;
                let lo = color.r.min(color.g).min(color.b) as f32;
                field[y * sub_w + x] = CoverSample {
                    color,
                    luma: color.luma(),
                    edge: ex + ey,
                    chroma: (hi - lo) / 255.0,
                };
            }
        }
        let expected = (self.particles.len() as f32 / (inner.w * inner.h * 8) as f32).max(1.0);
        for y in 0..inner.h {
            for x in 0..inner.w {
                let cell = stats[y * inner.w + x];
                let mut mask = 0u8;
                let mut ink = DotStats::default();
                let mut total = DotStats::default();
                for (dot_index, dot) in cell.dots.iter().enumerate() {
                    if dot.energy <= 0.0 {
                        continue;
                    }
                    total.energy += dot.energy;
                    total.r += dot.r;
                    total.g += dot.g;
                    total.b += dot.b;
                    total.luma = total.luma.max(dot.luma);
                    total.edge = total.edge.max(dot.edge);
                    let sx = dot_index % 2;
                    let sy = dot_index / 2;
                    let density = (dot.energy / (expected * 0.82)).clamp(0.0, 1.0);
                    let threshold = 0.18 + (glyphs::bayer(x * 2 + sx, y * 4 + sy) + 0.5) * 0.58;
                    let signal =
                        density * (0.30 + dot.luma * 0.78 + dot.edge * 0.55 + dot.salience * 0.25);
                    if signal > threshold {
                        mask |= glyphs::BRAILLE_BITS[sy][sx];
                        ink.energy += dot.energy;
                        ink.r += dot.r;
                        ink.g += dot.g;
                        ink.b += dot.b;
                        ink.luma = ink.luma.max(dot.luma);
                        ink.edge = ink.edge.max(dot.edge);
                    }
                }
                let cell_x = x * 2;
                let cell_y = y * 4;
                let mut lo = 1.0f32;
                let mut hi = 0.0f32;
                for oy in 0..4 {
                    for ox in 0..2 {
                        let px = cell_x + ox;
                        let py = cell_y + oy;
                        let sample = field[py * sub_w + px];
                        lo = lo.min(sample.luma);
                        hi = hi.max(sample.luma);
                    }
                }
                let mut samples = [CoverSample::default(); 8];
                for sy in 0..4 {
                    for sx in 0..2 {
                        let index = sy * 2 + sx;
                        let sample = field[(cell_y + sy) * sub_w + cell_x + sx];
                        samples[index] = sample;
                        let contrast = (hi - lo).max(0.012);
                        let local = (sample.luma - lo) / contrast;
                        let dither = glyphs::bayer(cell_x + sx, cell_y + sy) - 0.5;
                        let detail = glyphs::tone_map(
                            sample.luma
                                * (0.60 + sample.edge * 1.8 + sample.chroma * 0.7)
                                * base_strength,
                        );
                        let threshold = 0.10 + dither * 0.38 + (1.0 - local.clamp(0.0, 1.0)) * 0.15;
                        let mass = 0.18 + sample.luma + sample.edge * 0.5;
                        total.energy += mass;
                        total.r += sample.color.r as f32 * mass;
                        total.g += sample.color.g as f32 * mass;
                        total.b += sample.color.b as f32 * mass;
                        total.luma = total.luma.max(sample.luma);
                        total.edge = total.edge.max(sample.edge.min(1.0));
                        if detail > threshold || sample.edge > 0.34 {
                            mask |= glyphs::BRAILLE_BITS[sy][sx];
                            ink.energy += mass;
                            ink.r += sample.color.r as f32 * mass;
                            ink.g += sample.color.g as f32 * mass;
                            ink.b += sample.color.b as f32 * mass;
                            ink.luma = ink.luma.max(sample.luma);
                            ink.edge = ink.edge.max(sample.edge.min(1.0));
                        }
                    }
                }
                let visible = mask.count_ones() as f32 / 8.0;
                let intensity = glyphs::tone_map(
                    (visible * 0.55 + total.luma * 0.25 + total.edge * 0.20)
                        * (0.92 + self.burst * 0.20),
                ) * (0.58 + alpha.clamp(0.0, 1.0) * 0.34)
                    * (0.82 + exposure.clamp(0.55, 2.5).sqrt() * 0.08)
                    * self.local_gain
                    * 0.88;
                let color_stats = if ink.energy > 0.0 { ink } else { total };
                let source = Rgb::new(
                    (color_stats.r / color_stats.energy.max(0.001)) as u8,
                    (color_stats.g / color_stats.energy.max(0.001)) as u8,
                    (color_stats.b / color_stats.energy.max(0.001)) as u8,
                )
                .scale(0.54 + intensity * 0.40);
                let tint = color_for(palette, intensity * 0.65);
                let color = blend(source, tint, BlendMode::Screen, 0.08);
                let color = blend(color, bg, BlendMode::Multiply, (1.0 - intensity) * 0.18);
                let top = average_samples(&samples[..4]);
                let bottom = average_samples(&samples[4..]);
                let color_delta = color_delta(top, bottom);
                let use_half_block = style == CoverStyle::HalfBlock
                    || (style == CoverStyle::Hybrid
                        && charset != Charset::Ascii
                        && (color_delta > 0.055 || total.edge > 0.18 || total.luma > 0.045));
                if use_half_block {
                    let response = 0.78 + exposure.clamp(0.55, 2.5).sqrt() * 0.07;
                    let flash = self.burst * 0.035 + self.beat_phase.min(0.25) * 0.02;
                    let particle_heat =
                        (total.energy / (expected * 8.0)).clamp(0.0, 1.0) * self.burst * 0.08;
                    let top = blend(top.scale(response), tint, BlendMode::Screen, flash);
                    let bottom =
                        blend(bottom.scale(response), tint, BlendMode::Screen, flash * 0.6);
                    let top = blend(top, source, BlendMode::Screen, particle_heat);
                    let bottom = blend(bottom, source, BlendMode::Screen, particle_heat * 0.6);
                    buffer.set(inner.x + x, inner.y + y, '▀', top, bottom);
                    continue;
                }
                let glyph = if style == CoverStyle::Braille {
                    char::from_u32(0x2800 + mask as u32).unwrap_or(' ')
                } else if charset == Charset::Ascii {
                    particle_glyph(intensity, x, y, t, charset)
                } else {
                    char::from_u32(0x2800 + mask as u32).unwrap_or(' ')
                };
                if glyph == ' ' {
                    continue;
                }
                let glow = (1.0 - intensity).powi(2) * (0.04 + total.edge * 0.12);
                let bg = bg.lerp(color, glow);
                buffer.set(inner.x + x, inner.y + y, glyph, color, bg);
            }
        }
    }
}

fn average_samples(samples: &[CoverSample]) -> Rgb {
    if samples.is_empty() {
        return Rgb::new(0, 0, 0);
    }
    let count = samples.len() as f32;
    Rgb::new(
        (samples
            .iter()
            .map(|sample| sample.color.r as f32)
            .sum::<f32>()
            / count) as u8,
        (samples
            .iter()
            .map(|sample| sample.color.g as f32)
            .sum::<f32>()
            / count) as u8,
        (samples
            .iter()
            .map(|sample| sample.color.b as f32)
            .sum::<f32>()
            / count) as u8,
    )
}

fn color_delta(a: Rgb, b: Rgb) -> f32 {
    let dr = (a.r as f32 - b.r as f32) / 255.0;
    let dg = (a.g as f32 - b.g as f32) / 255.0;
    let db = (a.b as f32 - b.b as f32) / 255.0;
    (dr * dr + dg * dg + db * db).sqrt()
}

fn particle_glyph(intensity: f32, x: usize, y: usize, t: f32, charset: Charset) -> char {
    const RAMP: &[char] = &[' ', '·', '.', ':', '-', '+', '*', '#', '░', '▒', '▓'];
    const ASCII: &[char] = &[' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];
    let ramp = if charset == Charset::Ascii {
        ASCII
    } else {
        RAMP
    };
    let value = (intensity * 0.82 + glyphs::bayer(x, y) * 0.018).clamp(0.0, 1.0);
    let staggered = (value * (ramp.len() - 1) as f32 + (t * 0.35).sin() * 0.08).round();
    ramp[(staggered as usize).min(ramp.len() - 1)]
}

fn hash2(x: u16, y: u16) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9e37_79b9) ^ (y as u32).wrapping_mul(0x85eb_ca6b);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    (h & 0x00ff_ffff) as f32 / 0x0100_0000 as f32
}

fn smoothstep(v: f32) -> f32 {
    v * v * (3.0 - 2.0 * v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::AudioParams;
    use crate::anim::bind::AnimParams;

    #[test]
    fn particle_state_stays_on_cover() {
        let cover = Cover::demo(32, 32);
        let mut art = CoverArt::new(&cover);
        let audio = AudioParams {
            bpm: 120.0,
            spectrum: [0.5; 16],
            ..Default::default()
        };
        let params = AnimParams::default();
        let ctx = FrameCtx {
            width: 40,
            height: 20,
            t: 2.0,
            dt: 1.0 / 30.0,
            audio: &audio,
            params: &params,
            cover: None,
            lyric_index: 0,
            lyric_progress: 0.0,
            lyric_change: 0.0,
            lyric_seed: 0,
            low_quality: false,
        };
        for _ in 0..30 {
            art.update(&ctx);
        }
        assert!(
            art.particles
                .iter()
                .all(|p| p.pos.0.is_finite() && p.pos.1.is_finite())
        );
    }

    #[test]
    fn cover_particles_never_use_solid_block() {
        for i in 0..100 {
            let glyph = particle_glyph(i as f32 / 99.0, i, i / 10, 1.0, Charset::Mixed);
            assert_ne!(glyph, '█');
        }
    }

    #[test]
    fn photo_cells_preserve_truecolor_and_do_not_collapse_to_white() {
        let cover = Cover::demo(128, 128);
        let palette = cover.palette();
        let mut art = CoverArt::new(&cover);
        let mut buffer = Buffer::new(40, 20);
        art.draw(
            &mut buffer,
            crate::layout::Rect::new(0, 0, 40, 20),
            &palette,
            Charset::Mixed,
            CoverStyle::Hybrid,
            1.0,
            0.62,
            1.0,
            &cover,
        );
        let cells: Vec<_> = (1..19)
            .flat_map(|y| (1..39).map(move |x| (x, y)))
            .map(|(x, y)| buffer.cell(x, y))
            .filter(|cell| cell.ch != ' ')
            .collect();
        assert!(cells.len() > 500);
        assert!(cells.iter().filter(|cell| cell.ch == '▀').count() > 450);
        assert!(cells.iter().filter(|cell| cell.ch == '█').count() == 0);
        let unique: std::collections::HashSet<_> = cells
            .iter()
            .map(|cell| (cell.fg.r, cell.fg.g, cell.fg.b))
            .collect();
        assert!(unique.len() > 64);
        assert!(
            cells
                .iter()
                .any(|cell| cell.fg.r.max(cell.fg.g).max(cell.fg.b) < 230)
        );
    }

    #[test]
    fn braille_style_uses_no_half_blocks() {
        let cover = Cover::demo(64, 64);
        let palette = cover.palette();
        let mut art = CoverArt::new(&cover);
        let mut buffer = Buffer::new(32, 18);
        art.draw(
            &mut buffer,
            crate::layout::Rect::new(0, 0, 32, 18),
            &palette,
            Charset::Mixed,
            CoverStyle::Braille,
            1.0,
            0.62,
            1.0,
            &cover,
        );
        assert!((1..17).all(|y| {
            (1..31).all(|x| {
                let ch = buffer.cell(x, y).ch;
                ch == ' ' || ('\u{2800}'..='\u{28ff}').contains(&ch)
            })
        }));
    }
}
