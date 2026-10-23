use crate::state_store;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionFamily {
    Calm,
    Flow,
    Rhythmic,
    Spatial,
    Detailed,
}

#[derive(Debug, Clone, Copy)]
pub struct ModeProfile {
    pub id: &'static str,
    pub family: MotionFamily,
    pub min_bpm: f32,
    pub max_bpm: f32,
    pub cost: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct SceneDescriptor {
    pub bpm: f32,
    pub energy: f32,
    pub spectral: f32,
    pub centroid: f32,
    pub flatness: f32,
    pub stereo_width: f32,
}

impl SceneDescriptor {
    pub fn from_audio(audio: &crate::anim::AudioParams) -> Self {
        Self {
            bpm: audio.bpm,
            energy: audio.rms.max(audio.bass).clamp(0.0, 1.0),
            spectral: audio.spectrum[..8].iter().sum::<f32>() / 8.0,
            centroid: audio.centroid.clamp(0.0, 1.0),
            flatness: audio.flatness.clamp(0.0, 1.0),
            stereo_width: audio.stereo_width.clamp(0.0, 1.0),
        }
    }

    pub fn family(self) -> MotionFamily {
        if self.energy > 0.72 && self.bpm > 124.0 {
            MotionFamily::Rhythmic
        } else if self.flatness > 0.40 || self.centroid > 0.62 {
            MotionFamily::Detailed
        } else if self.stereo_width > 0.48 || self.spectral > 0.62 {
            MotionFamily::Spatial
        } else if self.bpm < 92.0 || self.energy < 0.24 {
            MotionFamily::Calm
        } else {
            MotionFamily::Flow
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnimationPolicy {
    #[serde(default)]
    pub favorites: BTreeSet<String>,
    #[serde(default)]
    pub disabled: BTreeSet<String>,
    #[serde(default)]
    pub weights: BTreeMap<String, f32>,
}

impl AnimationPolicy {
    pub fn load() -> Self {
        if cfg!(test) {
            return Self::default();
        }
        state_store::load("animation-policy.json", 3)
    }

    pub fn save(&self) {
        state_store::save("animation-policy.json", self, 3);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn choose(
        &self,
        seed: u64,
        modes: &[ModeProfile],
        current_id: &str,
        descriptor: SceneDescriptor,
        target_family: MotionFamily,
        low_quality: bool,
        recent: &[usize],
    ) -> usize {
        let preferred: BTreeSet<&str> = if self.favorites.is_empty() {
            modes.iter().map(|mode| mode.id).collect()
        } else {
            self.favorites.iter().map(String::as_str).collect()
        };
        let available: Vec<(usize, f32)> = modes
            .iter()
            .enumerate()
            .filter(|(_, mode)| {
                preferred.contains(mode.id)
                    && !self.disabled.contains(mode.id)
                    && (!low_quality || mode.cost <= 1)
            })
            .map(|(index, mode)| {
                let weight = self.weights.get(mode.id).copied().unwrap_or(1.0);
                (index, weight * profile_affinity(*mode, descriptor))
            })
            .filter(|(_, weight)| *weight > 0.0)
            .collect();
        if available.is_empty() {
            return modes
                .iter()
                .position(|mode| mode.id == current_id)
                .unwrap_or(0);
        }
        let family_only: Vec<(usize, f32)> = available
            .iter()
            .copied()
            .filter(|(index, _)| modes[*index].family == target_family)
            .collect();
        let mut selected = if family_only.is_empty() {
            available
        } else {
            family_only
        };
        if selected.len() > 1 {
            let fresh: Vec<(usize, f32)> = selected
                .iter()
                .copied()
                .filter(|(index, _)| !recent.contains(index))
                .collect();
            if !fresh.is_empty() {
                selected = fresh;
            }
        }
        let total: f32 = selected.iter().map(|(_, weight)| *weight).sum();
        let mut point = deterministic_unit(seed) * total;
        for (index, weight) in selected {
            point -= weight;
            if point <= 0.0 {
                return index;
            }
        }
        modes.len().saturating_sub(1)
    }

    pub fn adjust_weight(&mut self, mode: &str, delta: f32) -> f32 {
        let weight = (self.weights.get(mode).copied().unwrap_or(1.0) + delta).clamp(0.1, 5.0);
        self.weights.insert(mode.into(), weight);
        weight
    }

    pub fn repair_enabled(&mut self, modes: &[&str], minimum: usize) -> bool {
        let enabled = modes
            .iter()
            .filter(|mode| !self.disabled.contains(**mode))
            .count();
        if enabled >= minimum {
            return false;
        }
        self.disabled.clear();
        true
    }

    pub fn reset(&mut self) {
        self.favorites.clear();
        self.disabled.clear();
        self.weights.clear();
        self.save();
    }

    pub fn step(&self, current_id: &str, modes: &[ModeProfile], direction: isize) -> usize {
        if modes.is_empty() {
            return 0;
        }
        let current = modes
            .iter()
            .position(|mode| mode.id == current_id)
            .unwrap_or(0);
        let direction = direction.signum();
        for offset in 1..=modes.len() {
            let index = (current as isize + offset as isize * direction)
                .rem_euclid(modes.len() as isize) as usize;
            if !self.disabled.contains(modes[index].id) {
                return index;
            }
        }
        current
    }
}

fn deterministic_unit(seed: u64) -> f32 {
    let value = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(29);
    (value >> 40) as f32 / (1u64 << 24) as f32
}

fn profile_affinity(mode: ModeProfile, descriptor: SceneDescriptor) -> f32 {
    let bpm = if descriptor.bpm > 20.0 {
        descriptor.bpm
    } else {
        112.0
    };
    let outside = if bpm < mode.min_bpm {
        mode.min_bpm - bpm
    } else if bpm > mode.max_bpm {
        bpm - mode.max_bpm
    } else {
        0.0
    };
    let tempo_fit = (1.0 - outside / 90.0).clamp(0.18, 1.0);
    let family_fit = match mode.family {
        MotionFamily::Calm => 1.35 - descriptor.energy * 0.55,
        MotionFamily::Flow => 1.0 + (1.0 - (descriptor.energy - 0.45).abs()) * 0.28,
        MotionFamily::Rhythmic => 0.78 + descriptor.energy * 0.62,
        MotionFamily::Spatial => 0.88 + descriptor.energy * 0.32,
        MotionFamily::Detailed => 0.92 + (1.0 - descriptor.energy) * 0.35,
    };
    (tempo_fit * family_fit).max(0.08)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODES: [ModeProfile; 3] = [
        ModeProfile {
            id: "calm",
            family: MotionFamily::Calm,
            min_bpm: 60.0,
            max_bpm: 96.0,
            cost: 0,
        },
        ModeProfile {
            id: "drive",
            family: MotionFamily::Rhythmic,
            min_bpm: 100.0,
            max_bpm: 150.0,
            cost: 1,
        },
        ModeProfile {
            id: "expensive",
            family: MotionFamily::Spatial,
            min_bpm: 90.0,
            max_bpm: 180.0,
            cost: 2,
        },
    ];

    fn descriptor(bpm: f32, energy: f32) -> SceneDescriptor {
        SceneDescriptor {
            bpm,
            energy,
            spectral: 0.4,
            centroid: 0.3,
            flatness: 0.2,
            stereo_width: 0.3,
        }
    }

    #[test]
    fn favorites_survive_mode_reordering() {
        let mut policy = AnimationPolicy::default();
        policy.favorites.insert("drive".into());
        for seed in 0..20 {
            let selected = policy.choose(
                seed,
                &MODES,
                "calm",
                descriptor(128.0, 0.7),
                MotionFamily::Rhythmic,
                false,
                &[],
            );
            assert_eq!(MODES[selected].id, "drive");
        }
    }

    #[test]
    fn low_end_profile_skips_costly_modes() {
        let policy = AnimationPolicy::default();
        for seed in 0..30 {
            let selected = policy.choose(
                seed,
                &MODES,
                "calm",
                descriptor(128.0, 0.7),
                MotionFamily::Rhythmic,
                true,
                &[],
            );
            assert!(MODES[selected].cost <= 1);
        }
    }

    #[test]
    fn steps_skip_disabled_stable_ids() {
        let mut policy = AnimationPolicy::default();
        policy.disabled.insert("drive".into());
        let selected = policy.step("calm", &MODES, 1);
        assert_eq!(MODES[selected].id, "expensive");
    }

    #[test]
    fn repairs_policy_that_accidentally_disabled_almost_every_mode() {
        let mut policy = AnimationPolicy::default();
        policy.disabled.extend(["calm".into(), "drive".into()]);
        assert!(policy.repair_enabled(&["calm", "drive", "expensive"], 3));
        assert!(policy.disabled.is_empty());
    }
}
