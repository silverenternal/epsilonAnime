use super::AudioParams;
use crate::generated::{cos_lut, sin_lut};
use noise::{Fbm, NoiseFn, Perlin};
use num_complex::Complex32;
use std::sync::OnceLock;

fn fbm() -> &'static Fbm<Perlin> {
    static FBM: OnceLock<Fbm<Perlin>> = OnceLock::new();
    FBM.get_or_init(|| Fbm::<Perlin>::new(0x51a7))
}

pub fn plasma(x: f32, y: f32, t: f32, audio: &AudioParams) -> f32 {
    let a = sin_lut(x * 5.8 + t * 0.9 + audio.bass * 2.0);
    let b = cos_lut(y * 6.6 - t * 1.2 + audio.mid * 1.8);
    let c = sin_lut((x + y) * 3.4 + t * 0.55);
    let d = cos_lut((x * x + y * y).sqrt() * 9.0 - t * 2.2);
    (0.5 + (a + b + c + d) * 0.105 + audio.rms * 0.22).clamp(0.0, 1.0)
}

pub fn fbm_warp(x: f32, y: f32, t: f32, audio: &AudioParams) -> f32 {
    let n = fbm();
    let amp = 0.24 + audio.bass * 0.28;
    let qx = n.get([(x * 2.4) as f64, (y * 2.4) as f64, (t * 0.18) as f64]) as f32;
    let qy = n.get([
        (x * 2.4 + 5.2) as f64,
        (y * 2.4 + 1.7) as f64,
        (t * 0.16) as f64,
    ]) as f32;
    let rx = n.get([
        (x * 4.1 + amp * qx) as f64,
        (y * 4.1 + amp * qy) as f64,
        (t * 0.22) as f64,
    ]) as f32;
    let ry = n.get([
        (x * 4.1 - amp * qy + 3.1) as f64,
        (y * 4.1 + amp * qx - 2.0) as f64,
        (t * 0.2) as f64,
    ]) as f32;
    let v = n.get([
        (x * 3.0 + 2.2 * rx) as f64,
        (y * 3.0 + 2.2 * ry) as f64,
        (t * 0.3) as f64,
    ]) as f32;
    let glow = 0.5 + 0.5 * sin_lut(v * 1.7 + audio.pulse * 0.4);
    glow.clamp(0.0, 1.0)
}

pub fn waves(x: f32, y: f32, t: f32, audio: &AudioParams) -> f32 {
    let mut sum = 0.0;
    for i in 0..5 {
        let f = i as f32 + 1.0;
        let sx = 0.5 + 0.38 * sin_lut(t * (0.23 + f * 0.06) + i as f32 * 1.7);
        let sy = 0.5 + 0.38 * cos_lut(t * (0.19 + f * 0.04) + i as f32 * 2.3);
        let d = ((x - sx).powi(2) + (y - sy).powi(2)).sqrt();
        let k = 34.0 + audio.mid * 18.0;
        sum += sin_lut(d * k - t * (4.0 + f) + i as f32) / f;
    }
    (0.5 + sum * 0.34 + audio.rms * 0.15).clamp(0.0, 1.0)
}

pub fn julia(x: f32, y: f32, t: f32, audio: &AudioParams) -> f32 {
    let zx = (x - 0.5) * 3.0;
    let zy = (y - 0.5) * 3.0;
    let mut z = Complex32::new(zx, zy);
    let c = Complex32::new(
        -0.745 + 0.11 * sin_lut(t * 0.31) + audio.bass * 0.03,
        0.186 + 0.12 * cos_lut(t * 0.23),
    );
    let max_iter = 56;
    let mut iter = 0;
    while z.norm_sqr() <= 16.0 && iter < max_iter {
        z = z * z + c;
        iter += 1;
    }
    if iter >= max_iter {
        return 0.0;
    }
    let smooth = iter as f32 + 1.0 - z.norm().ln() / 2.0_f32.ln();
    (0.15 + smooth / max_iter as f32 * 0.95 + audio.pulse * 0.2).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernels_stay_in_range() {
        let a = AudioParams::default();
        for (x, y) in [(0.0, 0.0), (0.3, 0.8), (1.0, 1.0)] {
            for v in [
                plasma(x, y, 1.2, &a),
                fbm_warp(x, y, 1.2, &a),
                waves(x, y, 1.2, &a),
                julia(x, y, 1.2, &a),
            ] {
                assert!((0.0..=1.0).contains(&v));
            }
        }
    }
}
