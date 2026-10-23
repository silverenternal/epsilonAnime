#![allow(clippy::approx_constant, clippy::excessive_precision)]

include!(concat!(env!("OUT_DIR"), "/math_tables.rs"));

pub fn sin_lut(x: f32) -> f32 {
    let turns = x * (1.0 / std::f32::consts::TAU);
    let idx = ((turns.fract() + 1.0) * SIN_LUT.len() as f32) as usize % SIN_LUT.len();
    SIN_LUT[idx]
}

pub fn cos_lut(x: f32) -> f32 {
    let turns = x * (1.0 / std::f32::consts::TAU);
    let idx = ((turns.fract() + 1.0) * COS_LUT.len() as f32) as usize % COS_LUT.len();
    COS_LUT[idx]
}

pub fn palette_by_name(name: &str) -> Option<&'static [[u8; 3]]> {
    GENERATED_PALETTES
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, colors)| *colors)
}

pub fn palette_names() -> Vec<&'static str> {
    GENERATED_PALETTES.iter().map(|(name, _)| *name).collect()
}

pub fn variant_specs() -> &'static [(&'static str, &'static str, &'static str, f32)] {
    GENERATED_VARIANTS
}
