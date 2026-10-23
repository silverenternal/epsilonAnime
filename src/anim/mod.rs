pub mod adaptive;
pub mod advanced;
pub mod basic;
pub mod bind;
pub mod cinematic;
pub mod dsl;
pub mod policy;
pub(crate) mod spatial;
pub mod stateful;

use bind::AnimParams;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default)]
pub struct AudioParams {
    pub rms: f32,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub flux: f32,
    pub onset: f32,
    pub pulse: f32,
    pub bpm: f32,
    pub spectrum: [f32; 16],
    pub centroid: f32,
    pub spread: f32,
    pub flatness: f32,
    pub rolloff: f32,
    pub zcr: f32,
    pub crest: f32,
    pub stereo_width: f32,
}

pub struct FrameCtx<'a> {
    pub width: usize,
    pub height: usize,
    pub t: f32,
    pub dt: f32,
    pub audio: &'a AudioParams,
    pub params: &'a AnimParams,
    pub cover: Option<Arc<crate::cover::Cover>>,
    pub lyric_index: usize,
    pub lyric_progress: f32,
    pub lyric_change: f32,
    pub lyric_seed: u64,
    pub low_quality: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FieldSample {
    pub intensity: f32,
    pub depth: f32,
    pub light: f32,
}

pub trait Animation: Send + Sync {
    fn update(&mut self, _ctx: &FrameCtx<'_>) {}
    fn sample(&self, x: f32, y: f32, ctx: &FrameCtx<'_>) -> f32;
    fn field(&self, x: f32, y: f32, ctx: &FrameCtx<'_>) -> FieldSample {
        FieldSample {
            intensity: self.sample(x, y, ctx),
            depth: 0.5,
            light: 1.0,
        }
    }
    fn label(&self) -> &'static str {
        "math"
    }
    fn command(&mut self, _command: AnimationCommand) {}
}

#[derive(Debug, Clone, Copy)]
pub enum AnimationCommand {
    Next,
    Previous,
    ToggleFavorite,
    ToggleDisabled,
    ResetPolicy,
    ToggleLock,
    WeightUp,
    WeightDown,
}

pub struct FnAnimation {
    func: fn(f32, f32, f32, &AudioParams) -> f32,
}

impl FnAnimation {
    pub fn new(func: fn(f32, f32, f32, &AudioParams) -> f32) -> Self {
        Self { func }
    }
}

impl Animation for FnAnimation {
    fn sample(&self, x: f32, y: f32, ctx: &FrameCtx<'_>) -> f32 {
        (self.func)(x, y, ctx.t, ctx.audio).clamp(0.0, 1.0)
    }
}

fn dsl_plasma_adapter(x: f32, y: f32, t: f32, _audio: &AudioParams) -> f32 {
    dsl::dsl_plasma(x, y, t)
}

crate::animations! {
    Plasma => stateless(basic::plasma),
    FbmWarp => stateless(basic::fbm_warp),
    Waves => stateless(basic::waves),
    Julia => stateless(basic::julia),
    Superformula => stateless(advanced::superformula),
    Metaballs => stateless(advanced::metaballs),
    Mandala => stateless(advanced::mandala),
    DslPlasma => stateless(dsl_plasma_adapter),
    Clifford => stateful(stateful::clifford),
    Ifs => stateful(stateful::ifs),
    GrayScott => stateful(stateful::gray_scott),
    FlowField => stateful(stateful::flow_field),
    Cinematic => stateful(cinematic::director),
    Prism => stateful(cinematic::prism),
    Orbital => stateful(cinematic::orbital),
    Ink => stateful(cinematic::ink),
    Bloom => stateful(cinematic::bloom),
    Adaptive => stateful(adaptive::adaptive),
}

pub fn make_engine(kind: AnimationKind, width: usize, height: usize) -> Box<dyn Animation> {
    kind.create(width.max(8), height.max(8))
}
