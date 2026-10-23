#![allow(dead_code)]

use sphrs::{Coordinates, RealSH, SHEval};

pub fn bessel_j(n: f32, x: f32) -> f32 {
    puruspe::Jn(n.max(0.0).round() as u32, x.max(0.0) as f64) as f32
}

pub fn bessel_y(n: f32, x: f32) -> f32 {
    puruspe::Yn(n.max(0.0).round() as u32, x.max(1.0e-6) as f64) as f32
}

pub fn bessel_i(n: f32, x: f32) -> f32 {
    puruspe::In(n.max(0.0).round() as u32, x.max(0.0) as f64) as f32
}

pub fn bessel_k(n: f32, x: f32) -> f32 {
    puruspe::Kn(n.max(0.0).round() as u32, x.max(1.0e-6) as f64) as f32
}

pub fn erf(x: f32) -> f32 {
    puruspe::erf(x as f64) as f32
}

pub fn erfc(x: f32) -> f32 {
    puruspe::erfc(x as f64) as f32
}

pub fn gamma(x: f32) -> f32 {
    puruspe::gamma(x as f64) as f32
}

pub fn ln_gamma(x: f32) -> f32 {
    puruspe::ln_gamma(x as f64) as f32
}

pub fn spherical_harmonic(degree: f32, order: f32, theta: f32, phi: f32) -> f32 {
    let coordinate = Coordinates::spherical(1.0, theta as f64, phi as f64);
    RealSH::Spherical.eval(
        degree.max(0.0).round() as i64,
        order.round() as i64,
        &coordinate,
    ) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_functions_are_finite() {
        assert!(bessel_j(0.0, 1.0).is_finite());
        assert!(erf(0.5).is_finite());
        assert!(gamma(2.5).is_finite());
        assert!(spherical_harmonic(2.0, 1.0, 0.8, 0.4).is_finite());
    }
}
