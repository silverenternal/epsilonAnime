use super::{Animation, FrameCtx};
use noise::{Fbm, NoiseFn, Perlin};
use std::sync::OnceLock;

fn idx(x: usize, y: usize, w: usize, h: usize) -> usize {
    y.min(h - 1) * w + x.min(w - 1)
}

fn sample_grid(grid: &[f32], x: f32, y: f32, w: usize, h: usize) -> f32 {
    let px = (x.clamp(0.0, 0.999) * w as f32) as usize;
    let py = (y.clamp(0.0, 0.999) * h as f32) as usize;
    grid[idx(px, py, w, h)]
}

pub fn clifford(w: usize, h: usize) -> Box<dyn Animation> {
    Box::new(Clifford::new(w, h))
}

struct Clifford {
    w: usize,
    h: usize,
    grid: Vec<f32>,
    px: f32,
    py: f32,
}

impl Clifford {
    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            grid: vec![0.0; w * h],
            px: 0.1,
            py: 0.2,
        }
    }
}

impl Animation for Clifford {
    fn update(&mut self, ctx: &FrameCtx<'_>) {
        for v in &mut self.grid {
            *v *= 0.965;
        }
        let t = ctx.t;
        let a = -1.4 + 0.03 * t.sin();
        let b = 1.6 + 0.02 * (t * 0.7).cos();
        let c = 1.0 + ctx.audio.bass * 0.08;
        let d = 0.7 + ctx.params.pulse * 0.1;
        for _ in 0..2400 {
            let nx = (a * self.py).sin() + c * (a * self.px).cos();
            let ny = (b * self.px).sin() + d * (b * self.py).cos();
            self.px = nx.clamp(-3.0, 3.0);
            self.py = ny.clamp(-3.0, 3.0);
            if fastrand::f32() > 0.06 {
                let x = ((self.px + 2.0) / 4.0 * ctx.width as f32) as isize;
                let y = ((self.py + 2.0) / 4.0 * ctx.height as f32) as isize;
                if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
                    self.grid[y as usize * self.w + x as usize] += 0.16;
                }
            }
        }
        for v in &mut self.grid {
            *v = v.min(1.0);
        }
    }

    fn sample(&self, x: f32, y: f32, _ctx: &FrameCtx<'_>) -> f32 {
        sample_grid(&self.grid, x, y, self.w, self.h)
            .sqrt()
            .clamp(0.0, 1.0)
    }
}

pub fn ifs(w: usize, h: usize) -> Box<dyn Animation> {
    Box::new(Ifs::new(w, h))
}

struct Ifs {
    w: usize,
    h: usize,
    grid: Vec<f32>,
    x: f32,
    y: f32,
}

impl Ifs {
    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            grid: vec![0.0; w * h],
            x: 0.1,
            y: 0.1,
        }
    }
}

impl Animation for Ifs {
    fn update(&mut self, ctx: &FrameCtx<'_>) {
        for v in &mut self.grid {
            *v *= 0.97;
        }
        for _ in 0..3600 {
            let p = fastrand::f32();
            let (nx, ny) = if p < 0.01 {
                (0.0, 0.16 * self.y)
            } else if p < 0.86 {
                (
                    0.85 * self.x + 0.04 * self.y,
                    -0.04 * self.x + 0.85 * self.y + 1.6,
                )
            } else if p < 0.93 {
                (
                    0.2 * self.x - 0.26 * self.y,
                    0.23 * self.x + 0.22 * self.y + 1.6,
                )
            } else {
                (
                    -0.15 * self.x + 0.28 * self.y,
                    0.26 * self.x + 0.24 * self.y + 0.44,
                )
            };
            self.x = nx;
            self.y = ny;
            let breathe = 1.0 + ctx.params.pulse * 0.05;
            let u = (self.x * 0.23 * breathe + 0.5).clamp(0.0, 0.999);
            let v = (1.0 - self.y * 0.09 * breathe).clamp(0.0, 0.999);
            let gx = (u * self.w as f32) as usize;
            let gy = (v * self.h as f32) as usize;
            self.grid[gy * self.w + gx] += 0.17;
        }
        for v in &mut self.grid {
            *v = v.min(1.0);
        }
    }

    fn sample(&self, x: f32, y: f32, _ctx: &FrameCtx<'_>) -> f32 {
        sample_grid(&self.grid, x, y, self.w, self.h)
            .sqrt()
            .clamp(0.0, 1.0)
    }
}

pub fn gray_scott(w: usize, h: usize) -> Box<dyn Animation> {
    Box::new(GrayScott::new(w, h))
}

struct GrayScott {
    w: usize,
    h: usize,
    u: Vec<f32>,
    v: Vec<f32>,
}

impl GrayScott {
    fn new(w: usize, h: usize) -> Self {
        let mut u = vec![1.0; w * h];
        let mut v = vec![0.0; w * h];
        for oy in 0..7 {
            for ox in 0..7 {
                let x = w / 3 + ox * w / 28;
                let y = h / 3 + oy * h / 28;
                if x < w && y < h {
                    v[y * w + x] = 1.0;
                    u[y * w + x] = 0.2;
                }
            }
        }
        Self { w, h, u, v }
    }
}

impl Animation for GrayScott {
    fn update(&mut self, ctx: &FrameCtx<'_>) {
        let (w, h) = (self.w, self.h);
        let du = 0.16;
        let dv = 0.08;
        let f = 0.035 + ctx.params.warp * 0.01;
        let k = 0.061 + ctx.audio.bass * 0.002;
        for _ in 0..7 {
            let old_u = self.u.clone();
            let old_v = self.v.clone();
            for y in 0..h {
                for x in 0..w {
                    let l = idx((x + w - 1) % w, y, w, h);
                    let r = idx((x + 1) % w, y, w, h);
                    let up = idx(x, (y + h - 1) % h, w, h);
                    let dn = idx(x, (y + 1) % h, w, h);
                    let c = y * w + x;
                    let lu = old_u[l] + old_u[r] + old_u[up] + old_u[dn] - 4.0 * old_u[c];
                    let lv = old_v[l] + old_v[r] + old_v[up] + old_v[dn] - 4.0 * old_v[c];
                    let uvv = old_u[c] * old_v[c] * old_v[c];
                    self.u[c] = (old_u[c] + du * lu - uvv + f * (1.0 - old_u[c])).clamp(0.0, 1.0);
                    self.v[c] = (old_v[c] + dv * lv + uvv - (f + k) * old_v[c]).clamp(0.0, 1.0);
                }
            }
        }
    }

    fn sample(&self, x: f32, y: f32, _ctx: &FrameCtx<'_>) -> f32 {
        (sample_grid(&self.v, x, y, self.w, self.h) * 3.8).clamp(0.0, 1.0)
    }
}

pub fn flow_field(w: usize, h: usize) -> Box<dyn Animation> {
    Box::new(FlowField::new(w, h))
}

struct FlowField {
    w: usize,
    h: usize,
    trail: Vec<f32>,
    particles: Vec<(f32, f32)>,
}

fn flow_noise() -> &'static Fbm<Perlin> {
    static NOISE: OnceLock<Fbm<Perlin>> = OnceLock::new();
    NOISE.get_or_init(|| Fbm::<Perlin>::new(0x0f10))
}

impl FlowField {
    fn new(w: usize, h: usize) -> Self {
        let particles = (0..650)
            .map(|_| (fastrand::f32(), fastrand::f32()))
            .collect();
        Self {
            w,
            h,
            trail: vec![0.0; w * h],
            particles,
        }
    }
}

impl Animation for FlowField {
    fn update(&mut self, ctx: &FrameCtx<'_>) {
        for v in &mut self.trail {
            *v *= 0.955;
        }
        let n = flow_noise();
        let scale = 3.2 + ctx.audio.mid * 1.5;
        let t = ctx.t * 0.12;
        for p in &mut self.particles {
            let e = 0.006;
            let y1 = n.get([(p.0 * scale) as f64, ((p.1 + e) * scale) as f64, t as f64]) as f32;
            let y0 = n.get([(p.0 * scale) as f64, ((p.1 - e) * scale) as f64, t as f64]) as f32;
            let x1 = n.get([((p.0 + e) * scale) as f64, (p.1 * scale) as f64, t as f64]) as f32;
            let x0 = n.get([((p.0 - e) * scale) as f64, (p.1 * scale) as f64, t as f64]) as f32;
            let vx = (y1 - y0) / (2.0 * e);
            let vy = -(x1 - x0) / (2.0 * e);
            let step = 0.0018 * (ctx.dt * 30.0).clamp(0.2, 3.0);
            p.0 += vx * step * (1.0 + ctx.params.pulse);
            p.1 += vy * step * (1.0 + ctx.params.pulse);
            if !(0.0..1.0).contains(&p.0) || !(0.0..1.0).contains(&p.1) || fastrand::f32() < 0.004 {
                p.0 = fastrand::f32();
                p.1 = fastrand::f32();
            }
            let x = (p.0 * self.w as f32) as usize;
            let y = (p.1 * self.h as f32) as usize;
            self.trail[y * self.w + x] += 0.45;
        }
    }

    fn sample(&self, x: f32, y: f32, _ctx: &FrameCtx<'_>) -> f32 {
        sample_grid(&self.trail, x, y, self.w, self.h)
            .sqrt()
            .clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::{AnimParams, AudioParams};

    fn ctx<'a>(audio: &'a AudioParams, params: &'a AnimParams) -> FrameCtx<'a> {
        FrameCtx {
            width: 40,
            height: 24,
            t: 1.0,
            dt: 1.0 / 30.0,
            audio,
            params,
            cover: None,
            lyric_index: 0,
            lyric_progress: 0.0,
            lyric_change: 0.0,
            lyric_seed: 0,
            low_quality: false,
        }
    }

    #[test]
    fn stateful_engines_update() {
        let audio = AudioParams::default();
        let params = AnimParams::default();
        let c = ctx(&audio, &params);
        for mut anim in [
            clifford(40, 24),
            ifs(40, 24),
            gray_scott(40, 24),
            flow_field(40, 24),
        ] {
            anim.update(&c);
            let v = anim.sample(0.5, 0.5, &c);
            assert!(v.is_finite());
        }
    }
}
