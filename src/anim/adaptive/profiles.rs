use crate::anim::policy::{ModeProfile, MotionFamily};

pub(super) const MODE_NAMES: [&str; 18] = [
    "spectral",
    "julia",
    "superformula",
    "metaballs",
    "harmonics",
    "complex-flow",
    "newton",
    "torus-knot",
    "fourier",
    "interference",
    "reaction",
    "attractor",
    "lorenz3d",
    "mobius3d",
    "spherical3d",
    "surface3d",
    "hopf3d",
    "klein3d",
];

pub(super) const MODE_PROFILES: [ModeProfile; 18] = [
    profile("spectral", MotionFamily::Flow, 55.0, 175.0, 1),
    profile("julia", MotionFamily::Detailed, 45.0, 135.0, 2),
    profile("superformula", MotionFamily::Rhythmic, 70.0, 180.0, 1),
    profile("metaballs", MotionFamily::Rhythmic, 70.0, 190.0, 2),
    profile("harmonics", MotionFamily::Spatial, 50.0, 150.0, 1),
    profile("complex-flow", MotionFamily::Flow, 60.0, 170.0, 2),
    profile("newton", MotionFamily::Detailed, 45.0, 145.0, 2),
    profile("torus-knot", MotionFamily::Spatial, 55.0, 170.0, 2),
    profile("fourier", MotionFamily::Rhythmic, 75.0, 190.0, 1),
    profile("interference", MotionFamily::Rhythmic, 65.0, 190.0, 1),
    profile("reaction", MotionFamily::Calm, 40.0, 125.0, 3),
    profile("attractor", MotionFamily::Flow, 55.0, 175.0, 2),
    profile("lorenz3d", MotionFamily::Spatial, 55.0, 185.0, 2),
    profile("mobius3d", MotionFamily::Spatial, 55.0, 175.0, 2),
    profile("spherical3d", MotionFamily::Spatial, 45.0, 160.0, 2),
    profile("surface3d", MotionFamily::Spatial, 65.0, 190.0, 2),
    profile("hopf3d", MotionFamily::Spatial, 70.0, 200.0, 3),
    profile("klein3d", MotionFamily::Spatial, 55.0, 180.0, 2),
];

const fn profile(
    id: &'static str,
    family: MotionFamily,
    min_bpm: f32,
    max_bpm: f32,
    cost: u8,
) -> ModeProfile {
    ModeProfile {
        id,
        family,
        min_bpm,
        max_bpm,
        cost,
    }
}
