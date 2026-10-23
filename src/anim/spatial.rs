use glam::{Mat3, Vec3};
use std::f32::consts::{PI, TAU};

pub(crate) struct Camera {
    yaw: f32,
    pitch: f32,
    zoom: f32,
}

impl Camera {
    pub(crate) fn new(yaw: f32, pitch: f32, zoom: f32) -> Self {
        Self { yaw, pitch, zoom }
    }

    fn project(&self, point: [f32; 3]) -> (f32, f32, f32) {
        let rotation = Mat3::from_rotation_y(self.yaw) * Mat3::from_rotation_x(self.pitch);
        let projected = rotation * Vec3::from_array(point);
        let distance = 3.0 + projected.z;
        (
            0.5 + projected.x / distance * self.zoom,
            0.5 + projected.y / distance * self.zoom * 0.82,
            ((projected.z + 1.5) / 3.0).clamp(0.0, 1.0),
        )
    }
}

pub(crate) struct Canvas<'a> {
    intensity: &'a mut [f32],
    depth: &'a mut [f32],
    w: usize,
    h: usize,
}

impl<'a> Canvas<'a> {
    pub(crate) fn new(intensity: &'a mut [f32], depth: &'a mut [f32], w: usize, h: usize) -> Self {
        Self {
            intensity,
            depth,
            w,
            h,
        }
    }

    pub(crate) fn deposit(&mut self, x: f32, y: f32, depth: f32, energy: f32) {
        if !(0.0..1.0).contains(&x) || !(0.0..1.0).contains(&y) {
            return;
        }
        let px = (x * self.w as f32) as isize;
        let py = (y * self.h as f32) as isize;
        let radius = if depth > 0.72 { 1 } else { 0 };
        for oy in -radius..=radius {
            for ox in -radius..=radius {
                let tx = px + ox;
                let ty = py + oy;
                if tx < 0 || ty < 0 || tx >= self.w as isize || ty >= self.h as isize {
                    continue;
                }
                let falloff = 1.0 / (1.0 + (ox.abs() + oy.abs()) as f32 * 1.8);
                let idx = ty as usize * self.w + tx as usize;
                self.intensity[idx] = (self.intensity[idx] + energy * falloff).min(1.0);
                self.depth[idx] = self.depth[idx].max(depth);
            }
        }
    }

    pub(crate) fn line(&mut self, a: (f32, f32, f32), b: (f32, f32, f32), energy: f32) {
        let distance = ((a.0 - b.0) * self.w as f32).hypot((a.1 - b.1) * self.h as f32);
        let steps = (distance.ceil() as usize).clamp(1, 48);
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let depth = a.2 + (b.2 - a.2) * t;
            self.deposit(
                a.0 + (b.0 - a.0) * t,
                a.1 + (b.1 - a.1) * t,
                depth,
                energy / steps as f32 * (0.45 + depth * 0.55),
            );
        }
    }
}

pub(crate) fn lorenz(
    canvas: &mut Canvas<'_>,
    points: &mut [[f32; 5]],
    camera: &Camera,
    energy: f32,
    phase: f32,
) {
    for point in points {
        for _ in 0..18 {
            let dx = 10.0 * (point[1] - point[0]);
            let dy = point[0] * (28.0 - point[2]) - point[1];
            let dz = point[0] * point[1] - (8.0 / 3.0) * point[2];
            point[0] += dx * 0.0035;
            point[1] += dy * 0.0035;
            point[2] += dz * 0.0035;
        }
        let next = camera.project([
            point[0] / 22.0 + phase.sin() * 0.08,
            point[1] / 30.0,
            point[2] / 38.0,
        ]);
        if point[3] >= 0.0 {
            canvas.line((point[3], point[4], next.2), next, energy);
        }
        point[3] = next.0;
        point[4] = next.1;
    }
}

pub(crate) fn mobius(canvas: &mut Canvas<'_>, camera: &Camera, phase: f32, energy: f32) {
    const U_STEPS: usize = 52;
    const V_STEPS: usize = 10;
    let point = |ui: usize, vi: usize| {
        let u = TAU * ui as f32 / U_STEPS as f32 + phase;
        let v = vi as f32 / (V_STEPS - 1) as f32 * 2.0 - 1.0;
        let radius = 1.0 + v * 0.34 * (u * 0.5).cos();
        [
            radius * u.cos(),
            radius * u.sin(),
            v * 0.42 * (u * 0.5).sin() + (u * 3.0).sin() * 0.04,
        ]
    };
    for ui in 0..U_STEPS {
        for vi in 0..V_STEPS - 1 {
            canvas.line(
                camera.project(point(ui, vi)),
                camera.project(point(ui, vi + 1)),
                energy,
            );
        }
    }
    for vi in (0..V_STEPS).step_by(2) {
        for ui in 0..U_STEPS - 1 {
            canvas.line(
                camera.project(point(ui, vi)),
                camera.project(point(ui + 1, vi)),
                energy * 0.65,
            );
        }
    }
}

pub(crate) fn spherical(
    canvas: &mut Canvas<'_>,
    camera: &Camera,
    phase: f32,
    deformation: f32,
    energy: f32,
) {
    const THETA_STEPS: usize = 18;
    const PHI_STEPS: usize = 30;
    let point = |ti: usize, pi: usize| {
        let theta = PI * ti as f32 / (THETA_STEPS - 1) as f32;
        let phi = TAU * (pi as f32 / PHI_STEPS as f32 + phase * 0.03);
        let harmonic = crate::mathlib::spherical_harmonic(3.0, 2.0, theta, phi) * 0.8
            + crate::mathlib::spherical_harmonic(6.0, 3.0, theta, phi) * 0.2;
        let radius = 1.0 + harmonic * deformation;
        [
            radius * theta.sin() * phi.cos(),
            radius * theta.cos(),
            radius * theta.sin() * phi.sin(),
        ]
    };
    for ti in 0..THETA_STEPS {
        for pi in 0..PHI_STEPS {
            let next = (pi + 1) % PHI_STEPS;
            canvas.line(
                camera.project(point(ti, pi)),
                camera.project(point(ti, next)),
                energy,
            );
            if ti + 1 < THETA_STEPS && pi % 2 == 0 {
                canvas.line(
                    camera.project(point(ti, pi)),
                    camera.project(point(ti + 1, pi)),
                    energy * 0.45,
                );
            }
        }
    }
}

pub(crate) fn surface(
    canvas: &mut Canvas<'_>,
    camera: &Camera,
    t: f32,
    audio: &super::AudioParams,
    energy: f32,
    weave: fn(f32, f32, f32, &super::AudioParams) -> f32,
) {
    let point = |ix: usize, iy: usize| {
        let x = ix as f32 / 38.0 - 0.5;
        let y = iy as f32 / 22.0 - 0.5;
        let envelope = (-(x * x + y * y) * 7.0).exp();
        let z = ((weave(x + 0.5, y + 0.5, t, audio) - 0.5) * 0.42
            + (x * 18.0 - t * 1.7).sin() * 0.08)
            * (0.10 + audio.spectrum[4] * 0.16)
            * envelope;
        [x, y, z]
    };
    for iy in 0..23 {
        for ix in 0..39 {
            let p = camera.project(point(ix, iy));
            if ix + 1 < 39 {
                canvas.line(p, camera.project(point(ix + 1, iy)), energy);
            }
            if iy + 1 < 23 && ix % 2 == 0 {
                canvas.line(p, camera.project(point(ix, iy + 1)), energy * 0.48);
            }
        }
    }
}

pub(crate) fn hopf(canvas: &mut Canvas<'_>, camera: &Camera, phase: f32, energy: f32) {
    for fiber in 0..8 {
        let alpha = TAU * fiber as f32 / 8.0 + phase * 0.2;
        let mut previous = None;
        for i in 0..73 {
            let phi = TAU * i as f32 / 72.0 + phase;
            let theta = 0.65 + (alpha * 1.7).sin() * 0.48;
            let q0 = (
                (theta * 0.5).cos() * phi.cos(),
                (theta * 0.5).cos() * phi.sin(),
            );
            let q1 = (
                (theta * 0.5).sin() * (phi + alpha).cos(),
                (theta * 0.5).sin() * (phi + alpha).sin(),
            );
            let denom = (1.0 - q0.0).max(0.12);
            let p = camera.project([q0.1 / denom, q1.0 / denom, q1.1 / denom]);
            if let Some(last) = previous {
                canvas.line(last, p, energy);
            }
            previous = Some(p);
        }
    }
}

pub(crate) fn klein(canvas: &mut Canvas<'_>, camera: &Camera, phase: f32, energy: f32) {
    const U_STEPS: usize = 64;
    const V_STEPS: usize = 16;
    let point = |ui: usize, vi: usize| {
        let u = TAU * ui as f32 / U_STEPS as f32 + phase * 0.15;
        let v = TAU * vi as f32 / (V_STEPS - 1) as f32;
        let c = 2.0 + (u * 0.5).cos() * v.sin() - (u * 0.5).sin() * (2.0 * v).sin();
        [
            c * u.cos() * 0.48,
            c * u.sin() * 0.48,
            ((u * 0.5).sin() * v.sin() + (u * 0.5).cos() * (2.0 * v).sin()) * 0.42,
        ]
    };
    for ui in 0..U_STEPS {
        for vi in 0..V_STEPS {
            canvas.line(
                camera.project(point(ui, vi)),
                camera.project(point((ui + 1) % U_STEPS, vi)),
                energy,
            );
            if vi + 1 < V_STEPS {
                canvas.line(
                    camera.project(point(ui, vi)),
                    camera.project(point(ui, vi + 1)),
                    energy * 0.55,
                );
            }
        }
    }
}
