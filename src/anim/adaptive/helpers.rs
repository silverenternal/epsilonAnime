use super::{FrameCtx, SceneDescriptor, generated_live_surface};
use num_complex::Complex32;
use std::f32::consts::TAU;

pub(super) fn complex_velocity(
    x: f32,
    y: f32,
    spectral: &[f32; 16],
    beat_phase: f32,
) -> (f32, f32) {
    let z = Complex32::new((x - 0.5) * 2.0, (y - 0.5) * 2.0);
    let mut d = Complex32::new(0.0, 0.0);
    for k in 1..=5 {
        let energy = spectral[(k - 1) * 3];
        d += Complex32::from_polar(
            0.005 + energy * 0.04 * k as f32,
            TAU * beat_phase * (0.25 + k as f32 * 0.19) + k as f32,
        ) * z.powi(k as i32 - 1);
    }
    (d.conj().re, d.conj().im)
}

pub(super) fn live_surface_adapter(
    x: f32,
    y: f32,
    t: f32,
    audio: &super::super::AudioParams,
) -> f32 {
    generated_live_surface(x, y, t, audio.bass, audio.mid, audio.centroid)
}

pub(super) fn newton_root(mut z: Complex32) -> (Complex32, usize) {
    for i in 0..28 {
        let z2 = z * z;
        let denom = 3.0 * z2;
        if denom.norm_sqr() < 1.0e-8 {
            return (z, i);
        }
        let next = z - (z * z2 - Complex32::new(1.0, 0.0)) / denom;
        if (next - z).norm_sqr() < 1.0e-8 {
            return (next, i);
        }
        z = next;
    }
    (z, 28)
}

pub(super) fn fourier_point(
    s: f32,
    spectral: &[f32; 16],
    beat_phase: f32,
    offset: f32,
    audio: &super::super::AudioParams,
) -> Complex32 {
    let mut z = Complex32::new(0.0, 0.0);
    let mut scale = 0.28 + audio.spread * 0.20;
    let harmonics = (4 + (audio.rolloff * 6.0) as usize).min(8);
    for k in 0..harmonics {
        let energy =
            spectral[k * 2] * scale * (0.65 + audio.flatness * 0.7) * (1.0 + audio.zcr * 0.25);
        z += Complex32::from_polar(
            energy,
            TAU * (k as f32 + 1.0) * s
                + beat_phase * TAU * 0.17 * k as f32
                + offset
                + audio.centroid * (k as f32 + 1.0)
                + audio.stereo_width * 0.8,
        );
        scale *= 0.72;
    }
    z
}

pub(super) fn live_scene_signature(ctx: &FrameCtx<'_>) -> u64 {
    let descriptor = SceneDescriptor::from_audio(ctx.audio);
    let bpm = if descriptor.bpm > 20.0 {
        descriptor.bpm
    } else {
        112.0
    };
    let lanes = [
        (bpm / 4.0).round() as u64 & 0xff,
        (descriptor.energy * 15.0).round() as u64,
        (descriptor.spectral * 15.0).round() as u64,
        (descriptor.centroid * 15.0).round() as u64,
        (descriptor.flatness * 15.0).round() as u64,
        (descriptor.stereo_width * 15.0).round() as u64,
    ];
    lanes.into_iter().fold(0xcbf2_9ce4_8422_2325, |hash, lane| {
        (hash ^ lane).wrapping_mul(0x100_0000_01b3)
    })
}
