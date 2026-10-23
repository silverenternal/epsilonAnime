use super::{Animation, FrameCtx};
use noise::{Fbm, NoiseFn, Perlin};
use std::f32::consts::TAU;
use std::sync::OnceLock;

crate::spectral_engine!(SpectralField);

pub fn director(w: usize, h: usize) -> Box<dyn Animation> {
    Box::new(Director::new(w, h))
}

pub fn prism(w: usize, h: usize) -> Box<dyn Animation> {
    Box::new(Director::with_mode(w, h, 0, false))
}

pub fn orbital(w: usize, h: usize) -> Box<dyn Animation> {
    Box::new(Director::with_mode(w, h, 1, false))
}

pub fn ink(w: usize, h: usize) -> Box<dyn Animation> {
    Box::new(Director::with_mode(w, h, 2, false))
}

pub fn bloom(w: usize, h: usize) -> Box<dyn Animation> {
    Box::new(Director::with_mode(w, h, 3, false))
}

struct Star {
    x: f32,
    y: f32,
    depth: f32,
    phase: f32,
}

struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    energy: f32,
}

struct Director {
    w: usize,
    h: usize,
    stars: Vec<Star>,
    particles: Vec<Particle>,
    overlay: Vec<f32>,
    mode: u8,
    transition: f32,
    age: f32,
    mood: f32,
    last_lyric: usize,
    burst_cooldown: f32,
    spectral: [f32; 16],
    beat_phase: f32,
    bpm: f32,
    sample_accum: f32,
    beat_count: u32,
    cycle_modes: bool,
}

impl Director {
    fn new(w: usize, h: usize) -> Self {
        Self::with_mode(w, h, 0, true)
    }

    fn with_mode(w: usize, h: usize, mode: u8, cycle_modes: bool) -> Self {
        let stars = (0..170)
            .map(|_| Star {
                x: fastrand::f32(),
                y: fastrand::f32(),
                depth: 0.25 + fastrand::f32() * 0.75,
                phase: fastrand::f32() * TAU,
            })
            .collect();
        Self {
            w,
            h,
            stars,
            particles: Vec::new(),
            overlay: vec![0.0; w * h],
            mode,
            transition: 0.0,
            age: 0.0,
            mood: 0.5,
            last_lyric: 0,
            burst_cooldown: 0.0,
            spectral: [0.0; 16],
            beat_phase: 0.0,
            bpm: 120.0,
            sample_accum: 0.0,
            beat_count: 0,
            cycle_modes,
        }
    }

    fn switch_mode(&mut self) {
        self.mode = (self.mode + 1) % 4;
        self.transition = 1.0;
        self.age = 0.0;
        self.beat_count = 0;
    }

    fn burst(&mut self, energy: f32) {
        let cx = 0.62;
        let cy = 0.48;
        for i in 0..90 {
            let angle = fastrand::f32() * TAU;
            let speed = 0.015 + fastrand::f32() * 0.055 * (0.4 + energy);
            let offset = i as f32 / 90.0;
            self.particles.push(Particle {
                x: cx + angle.cos() * offset * 0.08,
                y: cy + angle.sin() * offset * 0.05,
                vx: angle.cos() * speed,
                vy: angle.sin() * speed * 0.55,
                life: 0.8 + fastrand::f32() * 1.4,
                energy: 0.35 + energy * 0.65,
            });
        }
    }

    fn update_overlay(&mut self, ctx: &FrameCtx<'_>) {
        for value in &mut self.overlay {
            *value *= 0.94;
        }
        for star in &mut self.stars {
            star.x += (0.0008 + ctx.audio.mid * 0.0012) * star.depth * ctx.dt * 60.0;
            if star.x > 1.0 {
                star.x -= 1.0;
            }
            star.phase += ctx.dt * (0.4 + star.depth * 1.3);
            let x = (star.x * self.w as f32) as usize;
            let y = (star.y * self.h as f32) as usize;
            if x < self.w && y < self.h {
                let twinkle = 0.45 + 0.55 * star.phase.sin();
                self.overlay[y * self.w + x] =
                    self.overlay[y * self.w + x].max(twinkle * star.depth * 0.55);
            }
        }
        let spectral = self.spectral;
        let beat_phase = self.beat_phase;
        for particle in &mut self.particles {
            let (fx, fy) = complex_velocity(particle.x, particle.y, &spectral, beat_phase);
            particle.vx += fx * ctx.dt * 0.8;
            particle.vy += fy * ctx.dt * 0.5;
            particle.x += particle.vx * ctx.dt * 60.0;
            particle.y += particle.vy * ctx.dt * 60.0;
            particle.vy += 0.00015;
            particle.life -= ctx.dt;
        }
        self.particles.retain(|p| p.life > 0.0);
        for particle in &self.particles {
            let x = (particle.x * self.w as f32) as isize;
            let y = (particle.y * self.h as f32) as isize;
            if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
                let idx = y as usize * self.w + x as usize;
                self.overlay[idx] = self.overlay[idx].max(particle.life.min(1.0) * particle.energy);
            }
        }
    }

    fn sample_mode(&self, mode: u8, x: f32, y: f32, ctx: &FrameCtx<'_>) -> f32 {
        match mode {
            0 => self.aurora(x, y, ctx),
            1 => self.orbit(x, y, ctx),
            2 => self.tide(x, y, ctx),
            _ => self.bloom(x, y, ctx),
        }
    }

    fn aurora(&self, x: f32, y: f32, ctx: &FrameCtx<'_>) -> f32 {
        let index = (x * 15.0) as usize;
        let energy = self.spectral[index.min(15)];
        let spectral = SpectralField.sample(x, y, self.beat_phase, &self.spectral);
        let center = 0.48
            + 0.19 * (TAU * x * 0.62 + ctx.t * 0.11 + index as f32 * 0.23).sin()
            + 0.05 * (TAU * x * 1.7 - ctx.t * 0.19).cos()
            + (ctx.lyric_progress - 0.5) * 0.025
            + (spectral - 0.5) * 0.10;
        let thickness = 0.025 + energy * 0.055 + ctx.audio.bass * 0.012;
        let d = (y - center) / thickness;
        let primary = (-d * d).exp() * (0.15 + energy * 1.1);
        let echo_center = 0.53 + 0.16 * (TAU * x * 0.35 - ctx.t * 0.07).sin();
        let echo = (-((y - echo_center) / 0.11).powi(2)).exp() * (0.04 + ctx.audio.treble * 0.13);
        let veil = flow_noise(x * 2.2, y * 3.5, ctx.t * 0.07) * 0.12;
        let cover = self.cover_luma(x, y, ctx) * primary * 0.18;
        (primary + echo + veil + cover).clamp(0.0, 1.0)
    }

    fn orbit(&self, x: f32, y: f32, ctx: &FrameCtx<'_>) -> f32 {
        let cx = 0.63;
        let cy = 0.49;
        let dx = x - cx;
        let dy = (y - cy) * 2.0;
        let r = (dx * dx + dy * dy).sqrt();
        let angle = dy.atan2(dx);
        let mut value: f32 = 0.0;
        for i in 0..4 {
            let radius = 0.16 + i as f32 * 0.105 + ctx.audio.bass * 0.012;
            let ring = (-((r - radius) / (0.008 + i as f32 * 0.003)).powi(2)).exp();
            let band = self.spectral[(i as usize * 3).min(15)];
            value = value.max(ring * (0.18 + band * 1.1));
            for phase in [0.0, 2.4 + i as f32] {
                let a = angle - ctx.t * (0.10 + i as f32 * 0.035) - phase;
                let spark = (-((a * 1.8).sin()).powi(2) * 420.0).exp();
                value = value.max(ring * spark * (0.4 + band));
            }
        }
        let lyric_wave = (-((r - (0.60 - ctx.lyric_change * 0.42)) * 16.0).powi(2)).exp()
            * ctx.lyric_change
            * 0.7;
        let core = if r < 0.17 {
            self.cover_luma(0.5 + dx * 2.8, 0.5 + dy * 1.4, ctx) * 0.23
        } else {
            0.0
        };
        (value + lyric_wave + core).clamp(0.0, 1.0)
    }

    fn tide(&self, x: f32, y: f32, ctx: &FrameCtx<'_>) -> f32 {
        let (vx, vy) = complex_velocity(x, y, &self.spectral, self.beat_phase);
        let spectral = SpectralField.sample(x, y, self.beat_phase, &self.spectral);
        let n = flow_noise(
            x * 1.7 + ctx.t * 0.03 + vx * 0.35 + (spectral - 0.5) * 0.18,
            y * 2.8 + vy * 0.20,
            ctx.t * 0.05 + self.beat_phase * 0.12,
        );
        let band = self.spectral[((1.0 - y) * 15.0) as usize] * (0.25 + ctx.audio.bass * 0.9);
        let horizon = 0.60 + n * 0.10 + ctx.lyric_change * 0.03;
        let shore = (-((y - horizon) * 17.0).powi(2)).exp();
        let ripple = (TAU * (y * 9.0 - ctx.t * 0.18) + n * 4.0).sin() * 0.5 + 0.5;
        let crest = shore * (0.15 + band) * (0.55 + ripple * 0.45);
        let cover =
            self.cover_luma(x + n * 0.02, y, ctx) * (-((y - 0.82) * 8.0).powi(2)).exp() * 0.18;
        (crest + cover).clamp(0.0, 1.0)
    }

    fn bloom(&self, x: f32, y: f32, ctx: &FrameCtx<'_>) -> f32 {
        let dx = x - 0.62;
        let dy = (y - 0.48) * 1.8;
        let r = (dx * dx + dy * dy).sqrt();
        let theta = dy.atan2(dx);
        let mut harmonics = 0.0;
        let mut weight_sum = 0.0;
        for k in 0..8 {
            let energy = self.spectral[k * 2];
            let order = (k + 3) as f32;
            harmonics += energy * (order * theta + TAU * self.beat_phase * (0.06 * order)).cos();
            weight_sum += energy;
        }
        let petals = harmonics / weight_sum.max(0.25);
        let target = 0.20 + petals * 0.10 + ctx.lyric_change * 0.035;
        let ring = (-((r - target) / (0.016 + ctx.audio.treble * 0.01)).powi(2)).exp();
        let aura = (-((r - target * 1.45) / 0.11).powi(2)).exp() * 0.18;
        let core = if r < 0.13 {
            self.cover_luma(0.5 + dx * 3.2, 0.5 + dy * 1.6, ctx) * 0.24
        } else {
            0.0
        };
        (ring * (0.35 + ctx.audio.treble) + aura + core).clamp(0.0, 1.0)
    }

    fn cover_luma(&self, x: f32, y: f32, ctx: &FrameCtx<'_>) -> f32 {
        ctx.cover
            .as_ref()
            .map(|cover| {
                cover
                    .sample(x.clamp(0.0, 0.999), y.clamp(0.0, 0.999))
                    .luma()
            })
            .unwrap_or(0.0)
    }
}

fn complex_velocity(x: f32, y: f32, spectral: &[f32; 16], beat_phase: f32) -> (f32, f32) {
    let z = num_complex::Complex32::new((x - 0.5) * 2.0, (y - 0.5) * 2.0);
    let mut derivative = num_complex::Complex32::new(0.0, 0.0);
    for k in 1..=5 {
        let energy = spectral[(k - 1) * 3];
        let magnitude = 0.006 + energy * 0.035;
        let phase = TAU * beat_phase * (0.35 + k as f32 * 0.17) + k as f32;
        derivative +=
            num_complex::Complex32::from_polar(magnitude * k as f32, phase) * z.powi(k as i32 - 1);
    }
    let velocity = derivative.conj();
    (velocity.re, velocity.im)
}

impl Animation for Director {
    fn update(&mut self, ctx: &FrameCtx<'_>) {
        self.burst_cooldown = (self.burst_cooldown - ctx.dt).max(0.0);
        self.age += ctx.dt;
        self.transition = (self.transition - ctx.dt * 0.72).max(0.0);
        self.mood += ((ctx.audio.bass * 0.7 + ctx.audio.treble * 0.3) - self.mood) * ctx.dt * 0.6;
        let incoming_bpm = if ctx.audio.bpm > 20.0 {
            ctx.audio.bpm
        } else {
            120.0
        };
        self.bpm += (incoming_bpm - self.bpm) * ctx.dt.min(0.2) * 1.2;
        let beat_seconds = 60.0 / self.bpm.max(40.0);
        let sample_step = (beat_seconds / 8.0).clamp(0.016, 0.10);
        self.sample_accum += ctx.dt;
        while self.sample_accum >= sample_step {
            self.sample_accum -= sample_step;
            let previous_phase = self.beat_phase;
            self.beat_phase = (self.beat_phase + sample_step / beat_seconds).fract();
            for (current, incoming) in self.spectral.iter_mut().zip(ctx.audio.spectrum) {
                *current += (incoming - *current) * 0.72;
            }
            if self.beat_phase < previous_phase {
                self.beat_count = self.beat_count.saturating_add(1);
            }
        }
        if ctx.lyric_index != self.last_lyric {
            self.last_lyric = ctx.lyric_index;
            if self.burst_cooldown <= 0.0 {
                self.burst(0.6 + ctx.audio.bass * 0.6);
                self.burst_cooldown = 1.8;
            }
        }
        if ctx.audio.onset > 0.5 && self.burst_cooldown <= 0.0 {
            self.burst(0.35 + ctx.audio.bass * 0.5);
            self.burst_cooldown = 0.8;
        }
        let switch_ready =
            self.beat_count >= 64 || (self.beat_count >= 16 && ctx.lyric_change > 0.9);
        if self.cycle_modes && switch_ready && self.transition <= 0.05 {
            self.switch_mode();
        }
        self.update_overlay(ctx);
    }

    fn sample(&self, x: f32, y: f32, ctx: &FrameCtx<'_>) -> f32 {
        let value = self.sample_mode(self.mode, x, y, ctx) * (1.0 + self.transition * 0.15);
        let px = (x.clamp(0.0, 0.999) * self.w as f32) as usize;
        let py = (y.clamp(0.0, 0.999) * self.h as f32) as usize;
        let overlay = self.overlay[py.min(self.h - 1) * self.w + px.min(self.w - 1)];
        (value + overlay * 0.65).clamp(0.0, 1.0)
    }
}

fn flow_noise(x: f32, y: f32, z: f32) -> f32 {
    static FBM: OnceLock<Fbm<Perlin>> = OnceLock::new();
    let fbm = FBM.get_or_init(|| Fbm::<Perlin>::new(0x1a2b_3c4d));
    fbm.get([x as f64, y as f64, z as f64]) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::{AnimParams, AudioParams};

    #[test]
    fn cinematic_scene_stays_bounded() {
        let mut scene = Director::new(80, 24);
        let audio = AudioParams {
            bass: 0.6,
            spectrum: [0.5; 16],
            ..Default::default()
        };
        let params = AnimParams::default();
        let ctx = FrameCtx {
            width: 80,
            height: 24,
            t: 2.0,
            dt: 1.0 / 30.0,
            audio: &audio,
            params: &params,
            cover: None,
            lyric_index: 1,
            lyric_progress: 0.3,
            lyric_change: 1.0,
            lyric_seed: 42,
            low_quality: false,
        };
        scene.update(&ctx);
        for (x, y) in [(0.0, 0.0), (0.3, 0.6), (0.9, 0.4), (1.0, 1.0)] {
            let value = scene.sample(x, y, &ctx);
            assert!((0.0..=1.0).contains(&value));
        }
    }
}
