use super::AudioParams;
use crate::generated::{cos_lut, sin_lut};

fn polar(x: f32, y: f32) -> (f32, f32) {
    let px = (x - 0.5) * 2.0;
    let py = (y - 0.5) * 2.0;
    (px.hypot(py), py.atan2(px))
}

pub fn superformula(x: f32, y: f32, t: f32, audio: &AudioParams) -> f32 {
    let (r, theta) = polar(x, y);
    let m = 6.0 + 2.5 * sin_lut(t * 0.21);
    let n1 = 0.35 + 0.12 * cos_lut(t * 0.17) + audio.bass * 0.08;
    let n2 = 0.55 + 0.2 * sin_lut(t * 0.13);
    let n3 = 0.55 + 0.2 * cos_lut(t * 0.11);
    let a = cos_lut(m * theta / 4.0).abs().powf(n2);
    let b = sin_lut(m * theta / 4.0).abs().powf(n3);
    let r0 = (a + b).powf(-1.0 / n1.max(0.05)) * 0.72;
    let edge = (-((r - r0).abs() * (18.0 + audio.treble * 12.0)).powi(2)).exp();
    let fill = if r < r0 {
        0.12 + 0.12 * (r / r0.max(0.01))
    } else {
        0.0
    };
    (edge * 0.92 + fill + audio.pulse * 0.12).clamp(0.0, 1.0)
}

pub fn metaballs(x: f32, y: f32, t: f32, audio: &AudioParams) -> f32 {
    let mut field = 0.0;
    for i in 0..6 {
        let f = i as f32;
        let cx = 0.5 + 0.36 * sin_lut(t * (0.31 + f * 0.07) + f * 0.9);
        let cy = 0.5 + 0.36 * cos_lut(t * (0.27 + f * 0.05) + f * 1.7);
        let radius = 0.055 + 0.018 * sin_lut(f * 2.1) + audio.bass * 0.035;
        let d2 = (x - cx).powi(2) + (y - cy).powi(2) + 0.002;
        field += radius * radius / d2;
    }
    let edge = (-((field - 1.0) * 8.0).powi(2)).exp();
    (edge * 0.75 + (field - 0.5).clamp(0.0, 1.0) * 0.35 + audio.pulse * 0.15).clamp(0.0, 1.0)
}

pub fn mandala(x: f32, y: f32, t: f32, audio: &AudioParams) -> f32 {
    let (r, theta) = polar(x, y);
    let mut rose = 0.0;
    for k in 1..=7 {
        let kf = k as f32;
        let amp = 1.0 / kf;
        rose += amp * cos_lut(kf * theta + t * (0.33 + kf * 0.11));
    }
    let target = (0.46 + rose * 0.12 + audio.bass * 0.07).max(0.05);
    let d = (r - target).abs();
    let ring = (-(d * 16.0).powi(2)).exp();
    let echo = (-(((r - target * 0.55).abs()) * 22.0).powi(2)).exp() * 0.35;
    (ring + echo + audio.pulse * 0.12).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advanced_kernels_finite() {
        let a = AudioParams {
            bass: 0.5,
            treble: 0.5,
            ..Default::default()
        };
        for v in [
            superformula(0.3, 0.6, 2.0, &a),
            metaballs(0.3, 0.6, 2.0, &a),
            mandala(0.3, 0.6, 2.0, &a),
        ] {
            assert!(v.is_finite());
            assert!((0.0..=1.0).contains(&v));
        }
    }
}
