use super::AudioParams;

#[derive(Debug, Clone, Copy)]
pub enum Feature {
    Rms,
    Bass,
    Mid,
    Treble,
    Flux,
    Onset,
}

impl Feature {
    pub fn get(self, audio: &AudioParams) -> f32 {
        match self {
            Self::Rms => audio.rms,
            Self::Bass => audio.bass,
            Self::Mid => audio.mid,
            Self::Treble => audio.treble,
            Self::Flux => audio.flux,
            Self::Onset => audio.onset,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Target {
    Warp,
    Pulse,
    Brightness,
    Zoom,
}

#[derive(Debug, Clone, Copy)]
pub struct Binding {
    pub source: Feature,
    pub lo: f32,
    pub hi: f32,
    pub target: Target,
}

#[derive(Debug, Clone, Copy)]
pub struct AnimParams {
    pub warp: f32,
    pub pulse: f32,
    pub brightness: f32,
    pub zoom: f32,
}

impl Default for AnimParams {
    fn default() -> Self {
        Self {
            warp: 0.7,
            pulse: 0.0,
            brightness: 1.0,
            zoom: 1.0,
        }
    }
}

impl AnimParams {
    pub fn update(&mut self, audio: &AudioParams, dt: f32) {
        let decayed = (self.pulse - dt * 4.5).max(0.0);
        apply_bindings(audio, self);
        self.pulse = self.pulse.max(decayed).clamp(0.0, 1.0);
    }
}

crate::bind! {
    Bass: 0.0..=1.0 => Warp,
    Rms: 0.65..=1.25 => Brightness,
    Treble: 0.8..=1.2 => Zoom,
    Mid: 0.8..=1.2 => Zoom,
    Flux: 0.8..=1.25 => Brightness,
    Onset: 0.0..=1.0 => Pulse,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_changes_params() {
        let audio = AudioParams {
            bass: 0.8,
            onset: 1.0,
            ..Default::default()
        };
        let mut p = AnimParams::default();
        p.update(&audio, 0.016);
        assert!(p.warp > 0.7);
        assert!(p.pulse > 0.5);
    }
}
