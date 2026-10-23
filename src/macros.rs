#[macro_export]
macro_rules! ascii_ramp {
    ($($ch:expr),+ $(,)?) => {
        pub const ASCII_RAMP: &[char] = &[$($ch),+];
    };
}

#[macro_export]
macro_rules! palette {
    ($name:ident, $([$r:expr, $g:expr, $b:expr]),+ $(,)?) => {
        pub const $name: &[[u8; 3]] = &[$([$r, $g, $b]),+];
    };
}

#[macro_export]
macro_rules! bind {
    ($($source:ident : $lo:literal ..= $hi:literal => $target:ident),+ $(,)?) => {
        pub const BINDINGS: &[Binding] = &[
            $(Binding { source: Feature::$source, lo: $lo, hi: $hi, target: Target::$target }),+
        ];

        pub fn apply_bindings(audio: &AudioParams, params: &mut AnimParams) {
            for b in BINDINGS {
                let value = b.source.get(audio);
                let mapped = (value.clamp(0.0, 1.0) * (b.hi - b.lo) + b.lo).clamp(0.0, 1.5);
                match b.target {
                    Target::Warp => params.warp = mapped,
                    Target::Pulse => params.pulse = mapped,
                    Target::Brightness => params.brightness = mapped,
                    Target::Zoom => params.zoom = mapped,
                }
            }
        }
    };
}

#[macro_export]
macro_rules! animations {
    ($($variant:ident => $kind:ident($ctor:path)),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
        pub enum AnimationKind {
            $($variant),+
        }

        impl AnimationKind {
            pub fn all() -> &'static [AnimationKind] {
                &[$(AnimationKind::$variant),+]
            }

            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => stringify!($variant)),+
                }
            }

            pub fn parse(value: &str) -> Option<Self> {
                use clap::ValueEnum;
                let normalized = value.to_ascii_lowercase().replace('_', "-");
                Self::from_str(&normalized, true).ok()
            }

            pub fn create(self, width: usize, height: usize) -> Box<dyn Animation> {
                match self {
                    $(Self::$variant => $crate::animations!(@ctor width, height, $kind, $ctor)),+
                }
            }
        }
    };
    (@ctor $w:ident, $h:ident, stateless, $func:path) => {
        Box::new(FnAnimation::new($func))
    };
    (@ctor $w:ident, $h:ident, stateful, $ctor:path) => {
        $ctor($w, $h)
    };
}

#[macro_export]
macro_rules! spectral_engine {
    ($name:ident) => {
        pub struct $name;

        impl $name {
            pub fn sample(&self, x: f32, y: f32, beat_phase: f32, bands: &[f32; 16]) -> f32 {
                let mut sum = 0.0f32;
                for (i, &(kx, ky, phase, speed, weight)) in
                    $crate::generated::SPECTRAL_MODES.iter().enumerate()
                {
                    let energy = bands[i % bands.len()];
                    sum += weight
                        * energy
                        * $crate::generated::sin_lut(
                            std::f32::consts::TAU * (kx * x + ky * y + beat_phase * speed) + phase,
                        );
                }
                (0.5 + sum * 0.45).clamp(0.0, 1.0)
            }
        }
    };
}
