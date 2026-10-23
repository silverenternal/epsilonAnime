use super::policy::{AnimationPolicy, MotionFamily, SceneDescriptor};
use super::{Animation, AnimationCommand, FieldSample, FrameCtx, spatial};
use crate::anim::{advanced, basic, stateful};
use num_complex::Complex32;
use std::f32::consts::TAU;

mod animation_impl;
#[cfg(test)]
mod audio_tests;
mod helpers;
mod profiles;
use helpers::{
    complex_velocity, fourier_point, live_scene_signature, live_surface_adapter, newton_root,
};
use profiles::{MODE_NAMES, MODE_PROFILES};

crate::spectral_engine!(SpectralField);

anim_macro::math_kernel! {
    name = generated_bessel,
    params = { freq = 8.0 },
    expr = "0.5 + 0.24 * sin(bessel_j(2.0, 1.0 + sqrt((x-0.5)^2 + (y-0.5)^2)*freq) + t*0.8)"
}

anim_macro::math_kernel! {
    name = generated_live_weave,
    dynamic = { a, b, c },
    expr = "0.5 + 0.22 * (sin(x*(5.5+a*7.0) + t*(0.45+b*1.8)) + cos(y*(7.0+c*9.0) - t*0.5) + sin((x+y)*(3.2+a*2.0) + t*(0.3+b*0.8)))"
}

anim_macro::math_kernel! {
    name = generated_live_rosette,
    dynamic = { a, b, c },
    expr = "0.5 + 0.28 * (cos((4.0+a*8.0)*atan2(y-0.5, x-0.5) + t*(0.4+b*0.8)) * sin(sqrt((x-0.5)^2 + (y-0.5)^2)*(14.0+c*10.0) - t*(2.0+a)))"
}

anim_macro::math_kernel! {
    name = generated_live_surface,
    dynamic = { a, b, c },
    expr = "0.5 + 0.42*sin(x*(5.0+a*9.0) + t*(0.9+b*1.1)) * cos(y*(7.0+c*8.0) - t*(0.6+a*0.5))"
}

anim_macro::math_kernel! {
    name = generated_live_light,
    dynamic = { a, b, c },
    expr = "0.25 + 0.58*x + (0.18+0.32*a)*sin(y*TAU*(0.5+b) + t*TAU*(0.6+c)) * (0.30 + 0.70*x)"
}

pub fn adaptive(w: usize, h: usize) -> Box<dyn Animation> {
    Box::new(Adaptive::new(w, h))
}

struct Adaptive {
    w: usize,
    h: usize,
    seed: u64,
    mode: usize,
    beat_phase: f32,
    bpm: f32,
    sample_accum: f32,
    spectral: [f32; 16],
    trail: Vec<f32>,
    depth: Vec<f32>,
    field_w: usize,
    field_h: usize,
    particles: Vec<[f32; 5]>,
    flash: f32,
    sub: Option<Box<dyn Animation>>,
    scene_bucket: usize,
    policy: AnimationPolicy,
    locked: bool,
    previous_mode: Option<usize>,
    transition: f32,
    scene_signature: u64,
    scene_accum: f32,
    scene_low_quality: bool,
    scene_epoch: u64,
    recent_modes: Vec<usize>,
}

impl Adaptive {
    fn new(w: usize, h: usize) -> Self {
        let mut policy = AnimationPolicy::load();
        if policy.repair_enabled(&MODE_NAMES, 4) {
            policy.save();
        }
        Self {
            w,
            h,
            seed: 0,
            mode: 0,
            beat_phase: 0.0,
            bpm: 120.0,
            sample_accum: 0.0,
            spectral: [0.0; 16],
            trail: vec![0.0; w * 2 * h * 4],
            depth: vec![0.0; w * 2 * h * 4],
            field_w: w * 2,
            field_h: h * 4,
            particles: Vec::new(),
            flash: 0.0,
            sub: None,
            scene_bucket: usize::MAX,
            policy,
            locked: false,
            previous_mode: None,
            transition: 0.0,
            scene_signature: 0,
            scene_accum: 0.0,
            scene_low_quality: false,
            scene_epoch: 0,
            recent_modes: Vec::new(),
        }
    }

    fn select(
        &mut self,
        ctx: &FrameCtx<'_>,
        seed: u64,
        lyric_index: usize,
        target_family: MotionFamily,
    ) {
        self.seed = seed;
        let descriptor = SceneDescriptor::from_audio(ctx.audio);
        let next = self.policy.choose(
            seed,
            &MODE_PROFILES,
            MODE_NAMES[self.mode],
            descriptor,
            target_family,
            ctx.low_quality,
            &self.recent_modes,
        );
        self.scene_bucket = lyric_index / 2;
        self.scene_low_quality = ctx.low_quality;
        if next == self.mode {
            return;
        }
        self.recent_modes.insert(0, self.mode);
        self.recent_modes.truncate(3);
        self.previous_mode = Some(self.mode);
        self.transition = 1.0;
        self.mode = next;
        for value in &mut self.trail {
            *value *= 0.22;
        }
        for value in &mut self.depth {
            *value *= 0.22;
        }
        self.prepare_mode();
        self.flash = 1.0;
    }

    fn update_scene_selection(&mut self, ctx: &FrameCtx<'_>) {
        if self.locked {
            return;
        }
        let signature = live_scene_signature(ctx);
        let bpm = if ctx.audio.bpm > 20.0 {
            ctx.audio.bpm
        } else {
            112.0
        };
        self.scene_accum += ctx.dt;
        let beat = (60.0 / bpm).max(0.25);
        let phrase_changed = self.seed == 0
            || (ctx.lyric_index / 2 != self.scene_bucket && self.scene_accum >= beat);
        let quality_changed = ctx.low_quality != self.scene_low_quality;
        let audio_changed = signature != self.scene_signature;
        let next_epoch = self.scene_epoch.wrapping_add(1);
        let beats = if self.scene_epoch.is_multiple_of(3) {
            8.0
        } else if audio_changed {
            2.0
        } else {
            4.0
        };
        let interval = beat * beats;
        if phrase_changed || quality_changed || self.scene_accum >= interval {
            self.scene_epoch = self.scene_epoch.wrapping_add(1);
            let target_family = if next_epoch.is_multiple_of(3) {
                MotionFamily::Detailed
            } else {
                SceneDescriptor::from_audio(ctx.audio).family()
            };
            let seed = ctx
                .lyric_seed
                .wrapping_add(signature.rotate_left(17))
                .wrapping_add(self.scene_epoch.wrapping_mul(0x9E37_79B9))
                .wrapping_mul(0x9E37_79B9_7F4A_7C15);
            self.scene_signature = signature;
            self.scene_accum = 0.0;
            self.select(ctx, seed, ctx.lyric_index, target_family);
        }
    }

    fn prepare_mode(&mut self) {
        self.particles.clear();
        self.sub = match self.mode {
            10 => Some(stateful::gray_scott(self.w, self.h)),
            11 => Some(stateful::clifford(self.w, self.h)),
            _ => None,
        };
        if self.mode == 5 {
            for _ in 0..420 {
                self.particles.push([
                    fastrand::f32(),
                    fastrand::f32(),
                    fastrand::f32() * 0.006 - 0.003,
                    fastrand::f32() * 0.006 - 0.003,
                    -100.0,
                ]);
            }
        } else if self.mode == 12 {
            for _ in 0..72 {
                self.particles.push([
                    0.1 + fastrand::f32() * 0.4,
                    0.1 + fastrand::f32() * 0.4,
                    20.0 + fastrand::f32() * 8.0,
                    -100.0,
                    -100.0,
                ]);
            }
        }
    }

    fn mode_field_at(&self, mode: usize, x: f32, y: f32, ctx: &FrameCtx<'_>) -> FieldSample {
        if mode == self.mode
            && let Some(sub) = &self.sub
        {
            return sub.field(x, y, ctx);
        }
        let intensity = self.sample_mode_at(mode, x, y, ctx);
        let depth = if matches!(mode, 5 | 8 | 12..=17) {
            self.trail_depth(x, y).max(0.18)
        } else {
            0.42 + intensity * 0.16
        };
        let light = if matches!(mode, 12..=17) {
            let angle = x * 0.8 + y * 1.7 + (self.seed & 0xff) as f32 * 0.01;
            0.50 + generated_live_light(
                depth * 0.72 + x * 0.28,
                angle,
                self.beat_phase,
                ctx.audio.rms,
                ctx.audio.bass,
                ctx.audio.stereo_width,
            ) * 0.50
        } else {
            1.0
        };
        FieldSample {
            intensity,
            depth,
            light,
        }
    }

    fn mixed_field(&self, x: f32, y: f32, ctx: &FrameCtx<'_>) -> FieldSample {
        let current = self.mode_field_at(self.mode, x, y, ctx);
        let blended = self.previous_mode.map_or(current, |previous| {
            let previous = self.mode_field_at(previous, x, y, ctx);
            let amount = self.transition.powi(2) * 0.55;
            FieldSample {
                intensity: current.intensity + (previous.intensity - current.intensity) * amount,
                depth: current.depth + (previous.depth - current.depth) * amount,
                light: current.light + (previous.light - current.light) * amount,
            }
        });
        FieldSample {
            intensity: (blended.intensity * (1.0 + self.flash * 0.18)).clamp(0.0, 1.0),
            depth: blended.depth.clamp(0.0, 1.0),
            light: blended.light.clamp(0.0, 1.25),
        }
    }

    fn update_clock(&mut self, ctx: &FrameCtx<'_>) {
        let incoming_bpm = if ctx.audio.bpm > 20.0 {
            ctx.audio.bpm
        } else {
            120.0
        };
        self.bpm += (incoming_bpm - self.bpm) * ctx.dt.min(0.2) * 1.4;
        let beat_seconds = 60.0 / self.bpm.max(40.0);
        let step = (beat_seconds / 8.0).clamp(0.016, 0.10);
        self.sample_accum += ctx.dt;
        while self.sample_accum >= step {
            self.sample_accum -= step;
            self.beat_phase = (self.beat_phase + step / beat_seconds).fract();
            for (current, incoming) in self.spectral.iter_mut().zip(ctx.audio.spectrum) {
                *current += (incoming - *current) * 0.74;
            }
        }
        self.flash = (self.flash - ctx.dt * 3.2).max(0.0);
    }

    fn update_trail(&mut self, ctx: &FrameCtx<'_>) {
        if !matches!(self.mode, 5 | 8 | 12..=17) {
            return;
        }
        let decay = (-ctx.dt * 2.1).exp();
        for value in &mut self.trail {
            *value *= decay;
        }
        for value in &mut self.depth {
            *value *= decay;
        }
        let yaw = self.beat_phase * TAU * (0.18 + ctx.audio.stereo_width * 0.12)
            + (self.seed & 0xff) as f32 * 0.01
            + ctx.audio.flux * 0.18;
        let pitch = 0.30 + ctx.lyric_progress * 0.18 + ctx.audio.centroid * 0.14;
        let zoom = 0.70 + ctx.audio.bass * 0.13 + ctx.audio.crest * 0.06;
        let camera = spatial::Camera::new(yaw, pitch, zoom);
        let mut canvas =
            spatial::Canvas::new(&mut self.trail, &mut self.depth, self.field_w, self.field_h);
        match self.mode {
            5 => {
                for particle in &mut self.particles {
                    let (vx, vy) =
                        complex_velocity(particle[0], particle[1], &self.spectral, self.beat_phase);
                    particle[2] += vx * ctx.dt * 8.0;
                    particle[3] += vy * ctx.dt * 8.0;
                    particle[2] *= 0.985;
                    particle[3] *= 0.985;
                    particle[0] += particle[2] * ctx.dt * 60.0;
                    particle[1] += particle[3] * ctx.dt * 60.0;
                    if !(0.0..1.0).contains(&particle[0]) || !(0.0..1.0).contains(&particle[1]) {
                        particle[0] = fastrand::f32();
                        particle[1] = fastrand::f32();
                    }
                    canvas.deposit(particle[0], particle[1], 0.5, 0.7);
                }
            }
            8 => {
                let offset = (self.seed & 0xffff) as f32 / 65535.0 * TAU;
                let mut previous = None;
                for i in 0..=280 {
                    let s = i as f32 / 280.0;
                    let z = fourier_point(s, &self.spectral, self.beat_phase, offset, ctx.audio);
                    let point = (0.5 + z.re * 0.55, 0.5 + z.im * 0.55, 0.55);
                    if let Some(last) = previous {
                        canvas.line(last, point, 0.85);
                    }
                    previous = Some(point);
                }
            }
            12 => spatial::lorenz(
                &mut canvas,
                &mut self.particles,
                &camera,
                0.35 + ctx.audio.bass * 0.18,
                self.beat_phase,
            ),
            13 => spatial::mobius(&mut canvas, &camera, self.beat_phase * 0.35, 0.56),
            14 => spatial::spherical(
                &mut canvas,
                &camera,
                self.beat_phase,
                0.12 + ctx.audio.crest * 0.18,
                0.42,
            ),
            15 => spatial::surface(
                &mut canvas,
                &camera,
                ctx.t,
                ctx.audio,
                0.45 + ctx.audio.mid * 0.15,
                live_surface_adapter,
            ),
            16 => spatial::hopf(&mut canvas, &camera, self.beat_phase * 0.5, 0.42),
            _ => spatial::klein(&mut canvas, &camera, self.beat_phase, 0.36),
        }
    }

    fn sample_mode_at(&self, mode: usize, x: f32, y: f32, ctx: &FrameCtx<'_>) -> f32 {
        match mode {
            0 => {
                let field = SpectralField.sample(
                    x + (ctx.audio.centroid - 0.5) * 0.08,
                    y + ctx.audio.onset * 0.02,
                    self.beat_phase,
                    &self.spectral,
                );
                let frequency = 6.0 + ctx.audio.spread * 10.0;
                let ribbon = (TAU * (y * frequency + field * 0.7) - self.beat_phase * TAU).sin();
                let weave = generated_live_weave(
                    x + ctx.audio.centroid * 0.05,
                    y,
                    ctx.t + self.beat_phase * 0.4,
                    ctx.audio.rms,
                    ctx.audio.onset,
                    ctx.audio.centroid,
                );
                let bessel = generated_bessel(x, y, ctx.t + self.beat_phase);
                ((field * 0.32 + weave * 0.20 + bessel * 0.12 + ribbon.abs().powf(2.2) * 0.36)
                    * (0.8 + ctx.audio.flatness * 0.4)
                    * (1.0 + self.flash * 0.2))
                    .clamp(0.0, 1.0)
            }
            1 => basic::julia(x, y, ctx.t + self.phase(), ctx.audio),
            2 => advanced::superformula(x, y, ctx.t + self.phase(), ctx.audio),
            3 => advanced::metaballs(x, y, ctx.t + self.phase(), ctx.audio),
            4 => self.harmonics(x, y, ctx),
            5 | 8 | 12..=17 => self.trail_value(x, y),
            6 => self.newton(x, y, ctx),
            7 => self.torus(x, y, ctx),
            9 => basic::waves(x, y, ctx.t + self.phase(), ctx.audio),
            _ => 0.0,
        }
    }

    fn phase(&self) -> f32 {
        (self.seed & 0xff) as f32 * 0.031
    }

    fn harmonics(&self, x: f32, y: f32, ctx: &FrameCtx<'_>) -> f32 {
        let dx = x - 0.50;
        let dy = (y - 0.48) * 1.8;
        let r = dx.hypot(dy);
        let theta = dy.atan2(dx);
        let mut sum = 0.0;
        for k in 0..12 {
            sum += self.spectral[k % 16]
                * (((k + 2) as f32 + ctx.audio.spread * 2.0) * theta
                    + self.beat_phase * TAU * (k as f32 + 1.0) * 0.06
                    + ctx.audio.centroid)
                    .cos();
        }
        let radius = 0.18 + ctx.audio.centroid * 0.08 + sum * 0.035;
        let rosette = generated_live_rosette(
            x,
            y,
            ctx.t + self.beat_phase * 0.5,
            ctx.audio.rms,
            ctx.audio.spread,
            ctx.audio.centroid,
        );
        ((-((r - radius) * 22.0).powi(2)).exp() * (0.72 + rosette * 0.36)).clamp(0.0, 1.0)
    }

    fn newton(&self, x: f32, y: f32, ctx: &FrameCtx<'_>) -> f32 {
        let z = Complex32::new(
            (x - 0.5) * 2.8 + (ctx.audio.centroid - 0.5) * 0.35,
            (y - 0.5) * 2.8 + ctx.audio.stereo_width * 0.18,
        );
        let (root, steps) = newton_root(z);
        let boundary = ((steps as f32 - 4.0) / 22.0).clamp(0.0, 1.0);
        let root_edge = (1.0 - (root.norm() - 1.0).abs() * 0.4).clamp(0.0, 1.0);
        (boundary * 0.82 + root_edge * 0.10).clamp(0.0, 1.0)
    }

    fn torus(&self, x: f32, y: f32, ctx: &FrameCtx<'_>) -> f32 {
        let mut best = f32::INFINITY;
        let phase = self.beat_phase * TAU * 0.4;
        for i in 0..64 {
            let theta = TAU * i as f32 / 64.0 + phase;
            let ring = 2.0 + (3.0 * theta).cos();
            let px = ring * (2.0 * theta).cos();
            let py = ring * (2.0 * theta).sin();
            let pz = (3.0 * theta).sin();
            let perspective = 4.2 + pz * (0.25 + ctx.audio.stereo_width * 0.35);
            let sx = 0.5 + px / perspective * (0.30 + ctx.audio.spread * 0.10);
            let sy = 0.5 + py / perspective * (0.26 + ctx.audio.crest * 0.08);
            best = best.min((x - sx).powi(2) + (y - sy).powi(2));
        }
        (-best * (1800.0 + ctx.audio.crest * 1800.0)).exp()
    }

    fn trail_value(&self, x: f32, y: f32) -> f32 {
        let px = (x.clamp(0.0, 0.999) * self.field_w as f32) as usize;
        let py = (y.clamp(0.0, 0.999) * self.field_h as f32) as usize;
        self.trail[py.min(self.field_h - 1) * self.field_w + px.min(self.field_w - 1)]
            .sqrt()
            .clamp(0.0, 1.0)
    }

    fn trail_depth(&self, x: f32, y: f32) -> f32 {
        let px = (x.clamp(0.0, 0.999) * self.field_w as f32) as usize;
        let py = (y.clamp(0.0, 0.999) * self.field_h as f32) as usize;
        self.depth[py.min(self.field_h - 1) * self.field_w + px.min(self.field_w - 1)]
    }
}

#[cfg(test)]
mod tests;
