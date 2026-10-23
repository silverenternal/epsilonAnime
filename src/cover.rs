use crate::render::Rgb;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub struct CoverParticle {
    pub x: u16,
    pub y: u16,
    pub color: Rgb,
    pub luma: u8,
    pub edge: u8,
    pub salience: u8,
}

#[derive(Clone)]
pub struct Cover {
    pub width: usize,
    pub height: usize,
    pixels: Vec<Rgb>,
    particles: Vec<CoverParticle>,
    particle_grid: (u16, u16),
}

impl Cover {
    pub fn load(
        path: &str,
        width: usize,
        height: usize,
        particle_path: Option<&str>,
    ) -> Result<Self> {
        let bytes = fs::read(Path::new(path)).with_context(|| format!("read cover {path}"))?;
        anyhow::ensure!(
            bytes.len() == width * height * 3,
            "cover size mismatch: {} != {}",
            bytes.len(),
            width * height * 3
        );
        let (channels, _) = bytes.as_chunks::<3>();
        let pixels = channels
            .iter()
            .map(|c| Rgb::new(c[0], c[1], c[2]))
            .collect();
        let mut cover = Self {
            width,
            height,
            pixels,
            particles: Vec::new(),
            particle_grid: (width as u16, height as u16),
        };
        if let Some(path) = particle_path {
            cover.particles = Self::load_particles(path)?;
            cover.particle_grid = Self::read_particle_grid(path)?;
        }
        if cover.particles.is_empty() {
            cover.generate_particles();
        }
        Ok(cover)
    }

    pub fn sample(&self, u: f32, v: f32) -> Rgb {
        let x = (u.clamp(0.0, 0.9999) * self.width as f32) as usize;
        let y = (v.clamp(0.0, 0.9999) * self.height as f32) as usize;
        self.pixels[y * self.width + x]
    }

    pub fn sample_linear(&self, u: f32, v: f32) -> Rgb {
        let x = (u.clamp(0.0, 1.0) * (self.width - 1) as f32).max(0.0);
        let y = (v.clamp(0.0, 1.0) * (self.height - 1) as f32).max(0.0);
        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let x1 = (x0 + 1).min(self.width.saturating_sub(1));
        let y1 = (y0 + 1).min(self.height.saturating_sub(1));
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;
        let top = self.pixels[y0 * self.width + x0].lerp(self.pixels[y0 * self.width + x1], fx);
        let bottom = self.pixels[y1 * self.width + x0].lerp(self.pixels[y1 * self.width + x1], fx);
        top.lerp(bottom, fy)
    }

    pub fn demo(width: usize, height: usize) -> Self {
        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let fx = x as f32 / width as f32;
                let fy = y as f32 / height as f32;
                let wave = ((fx * 14.0).sin() * (fy * 11.0).cos() * 0.5 + 0.5) * 255.0;
                let r = (fx * 210.0 + wave * 0.2) as u8;
                let g = (fy * 190.0 + wave * 0.28) as u8;
                let b = ((1.0 - fx) * 180.0 + wave * 0.35) as u8;
                pixels.push(Rgb::new(r, g, b));
            }
        }
        let mut cover = Self {
            width,
            height,
            pixels,
            particles: Vec::new(),
            particle_grid: (width as u16, height as u16),
        };
        cover.generate_particles();
        cover
    }

    pub fn particles(&self) -> &[CoverParticle] {
        &self.particles
    }

    pub fn rgb_bytes(&self) -> Vec<u8> {
        self.pixels
            .iter()
            .flat_map(|pixel| [pixel.r, pixel.g, pixel.b])
            .collect()
    }

    pub fn particle_grid(&self) -> (u16, u16) {
        self.particle_grid
    }

    fn load_particles(path: &str) -> Result<Vec<CoverParticle>> {
        let bytes =
            fs::read(Path::new(path)).with_context(|| format!("read cover particles {path}"))?;
        anyhow::ensure!(
            bytes.len() >= 16 && matches!(&bytes[..4], b"EAPC" | b"HBPC"),
            "bad particle file"
        );
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        anyhow::ensure!(bytes.len() >= 16 + count * 10, "truncated particle file");
        let mut particles = Vec::with_capacity(count);
        for i in 0..count {
            let at = 16 + i * 10;
            particles.push(CoverParticle {
                x: u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()),
                y: u16::from_le_bytes(bytes[at + 2..at + 4].try_into().unwrap()),
                color: Rgb::new(bytes[at + 4], bytes[at + 5], bytes[at + 6]),
                luma: bytes[at + 7],
                edge: bytes[at + 8],
                salience: bytes[at + 9],
            });
        }
        Ok(particles)
    }

    fn read_particle_grid(path: &str) -> Result<(u16, u16)> {
        let bytes = fs::read(Path::new(path))?;
        anyhow::ensure!(bytes.len() >= 16, "bad particle header");
        Ok((
            u16::from_le_bytes(bytes[12..14].try_into().unwrap()),
            u16::from_le_bytes(bytes[14..16].try_into().unwrap()),
        ))
    }

    fn generate_particles(&mut self) {
        let grid = 96usize;
        self.particle_grid = (grid as u16, grid as u16);
        let mut particles = Vec::with_capacity(grid * grid);
        for gy in 0..grid {
            for gx in 0..grid {
                let x = (gx as f32 + 0.5) / grid as f32;
                let y = (gy as f32 + 0.5) / grid as f32;
                let color = self.sample(x, y);
                let luma = (color.luma() * 255.0) as u8;
                let edge = 0;
                if luma > 4 {
                    particles.push(CoverParticle {
                        x: (gx + 1) as u16,
                        y: (gy + 1) as u16,
                        color,
                        luma,
                        edge,
                        salience: (30 + luma as u32 * 150 / 255) as u8,
                    });
                }
            }
        }
        self.particles = particles;
    }

    pub fn palette(&self) -> Vec<[u8; 3]> {
        let mut buckets = vec![(0u32, 0u32, 0u32, 0u32); 64];
        let step = (self.pixels.len() / 4096).max(1);
        for pixel in self.pixels.iter().step_by(step) {
            let key = ((pixel.r / 64) as usize) << 4
                | ((pixel.g / 64) as usize) << 2
                | (pixel.b / 64) as usize;
            let bucket = &mut buckets[key];
            bucket.0 += pixel.r as u32;
            bucket.1 += pixel.g as u32;
            bucket.2 += pixel.b as u32;
            bucket.3 += 1;
        }
        let mut colors: Vec<(u32, Rgb)> = buckets
            .into_iter()
            .filter(|b| b.3 > 0)
            .map(|(r, g, b, count)| {
                let color = Rgb::new((r / count) as u8, (g / count) as u8, (b / count) as u8);
                let saturation =
                    color.r.max(color.g).max(color.b) - color.r.min(color.g).min(color.b);
                (count * (32 + saturation as u32), color)
            })
            .collect();
        colors.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        let mut palette = Vec::new();
        for (_, color) in colors.into_iter().take(6) {
            let dark = color.scale(0.16);
            let bright = color.scale(1.25);
            if !palette.contains(&[dark.r, dark.g, dark.b]) {
                palette.push([dark.r, dark.g, dark.b]);
            }
            if !palette.contains(&[color.r, color.g, color.b]) {
                palette.push([color.r, color.g, color.b]);
            }
            if !palette.contains(&[bright.r, bright.g, bright.b]) {
                palette.push([bright.r, bright.g, bright.b]);
            }
            if palette.len() >= 8 {
                break;
            }
        }
        if palette.len() < 3 {
            return vec![[5, 5, 16], [90, 40, 170], [240, 80, 170], [255, 255, 255]];
        }
        palette.sort_by_key(|c| {
            let rgb = Rgb::from_slice(*c);
            (rgb.luma() * 1000.0) as u32
        });
        palette
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_has_expected_pixels() {
        let cover = Cover::demo(16, 16);
        assert_eq!(cover.pixels.len(), 256);
        assert!(cover.sample(0.5, 0.5).luma() >= 0.0);
        let linear = cover.sample_linear(0.25, 0.75);
        assert!(linear.r > 0 && linear.g > 0);
        assert!(cover.palette().len() >= 3);
    }
}
