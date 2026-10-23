use crate::anim::AudioParams;
use crate::render::{Buffer, Rgb, color_for};
use std::collections::VecDeque;

const BANDS: usize = 16;
const HISTORY: usize = 24;

pub struct AudioScope {
    history: VecDeque<[f32; BANDS]>,
    accum: f32,
    bpm: f32,
}

impl AudioScope {
    pub fn new() -> Self {
        Self {
            history: VecDeque::from([[0.0; BANDS]; HISTORY]),
            accum: 0.0,
            bpm: 120.0,
        }
    }

    pub fn reset(&mut self) {
        self.history.clear();
        self.history.extend([[0.0; BANDS]; HISTORY]);
        self.accum = 0.0;
    }

    pub fn update(&mut self, audio: &AudioParams, dt: f32) {
        let bpm = if audio.bpm > 20.0 { audio.bpm } else { 120.0 };
        self.bpm += (bpm - self.bpm) * dt.min(0.2);
        let interval = (60.0 / self.bpm / 8.0).clamp(0.025, 0.08);
        self.accum += dt;
        if self.accum >= interval {
            self.accum = 0.0;
            self.history.push_back(audio.spectrum);
            if self.history.len() > HISTORY {
                self.history.pop_front();
            }
        }
    }

    pub fn draw(&mut self, buffer: &mut Buffer, rect: crate::layout::Rect, palette: &[[u8; 3]]) {
        let inner = rect.inner();
        if inner.w < 8 || inner.h < 3 {
            return;
        }
        let bg = Rgb::from_slice(palette[0]).scale(0.28);
        let dot_w = inner.w * 2;
        let dot_h = inner.h * 4;
        for cy in 0..inner.h {
            for cx in 0..inner.w {
                let mut mask = 0u8;
                let mut energy_sum: f32 = 0.0;
                let mut dots: f32 = 0.0;
                for sy in 0..4 {
                    for sx in 0..2 {
                        let fx = (cx * 2 + sx) as f32 / dot_w.max(1) as f32;
                        let fy = 1.0 - (cy * 4 + sy) as f32 / dot_h.max(1) as f32;
                        let band = fx * (BANDS - 1) as f32;
                        let time = fy * (HISTORY - 1) as f32;
                        let energy = sample_history(&self.history, band, time);
                        energy_sum += energy;
                        dots += 1.0;
                        let threshold =
                            0.035 + (crate::glyphs::bayer(cx * 2 + sx, cy * 4 + sy) + 0.5) * 0.52;
                        if energy > threshold {
                            mask |= crate::glyphs::BRAILLE_BITS[sy][sx];
                        }
                    }
                }
                if mask == 0 {
                    continue;
                }
                let energy = (energy_sum / dots.max(1.0)).clamp(0.0, 1.0);
                let color = color_for(palette, 0.28 + energy * 0.72);
                let glyph = char::from_u32(0x2800 + mask as u32).unwrap_or(' ');
                buffer.set(
                    inner.x + cx,
                    inner.y + cy,
                    glyph,
                    color,
                    bg.lerp(color, energy * 0.06),
                );
            }
        }
    }
}

fn sample_history(history: &VecDeque<[f32; BANDS]>, band: f32, time: f32) -> f32 {
    let a = band.floor().clamp(0.0, (BANDS - 1) as f32) as usize;
    let b = (a + 1).min(BANDS - 1);
    let bt = band.fract();
    let t0 = time.floor().clamp(0.0, (HISTORY - 1) as f32) as usize;
    let t1 = (t0 + 1).min(HISTORY - 1);
    let tt = time.fract();
    let row = |i: usize| history.get(i).copied().unwrap_or([0.0; BANDS]);
    let r0 = row(t0);
    let r1 = row(t1);
    let top = r0[a] * (1.0 - bt) + r0[b] * bt;
    let bottom = r1[a] * (1.0 - bt) + r1[b] * bt;
    top * (1.0 - tt) + bottom * tt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spectrum_history_advances() {
        let mut scope = AudioScope::new();
        let audio = AudioParams {
            bpm: 120.0,
            spectrum: [0.5; BANDS],
            ..Default::default()
        };
        for _ in 0..10 {
            scope.update(&audio, 0.05);
        }
        assert!(scope.history.back().unwrap()[0] > 0.0);
    }
}
