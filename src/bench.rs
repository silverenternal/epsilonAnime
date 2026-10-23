use crate::anim::bind::AnimParams;
use crate::anim::{AnimationKind, AudioParams, FrameCtx, make_engine};
use anyhow::Result;
use std::hint::black_box;
use std::time::Instant;

pub fn run_bench(frames: u32) -> Result<()> {
    let frames = frames.max(1);
    for kind in AnimationKind::all() {
        let (w, h) = (160, 48);
        let audio = AudioParams {
            rms: 0.45,
            bass: 0.4,
            mid: 0.3,
            treble: 0.2,
            flux: 0.2,
            onset: 0.0,
            pulse: 0.2,
            bpm: 120.0,
            spectrum: std::array::from_fn(|i| 0.75 - i as f32 * 0.035),
            centroid: 0.42,
            spread: 0.36,
            flatness: 0.28,
            rolloff: 0.62,
            zcr: 0.18,
            crest: 0.35,
            stereo_width: 0.45,
        };
        let params = AnimParams::default();
        let seeds: Vec<u64> = if *kind == AnimationKind::Adaptive {
            (0..18).collect()
        } else {
            vec![0x1234_5678]
        };
        for seed in seeds {
            let mut engine = make_engine(*kind, w, h);
            let start = Instant::now();
            let mut checksum = 0.0f32;
            for frame in 0..frames {
                let ctx = FrameCtx {
                    width: w,
                    height: h,
                    t: frame as f32 / 30.0,
                    dt: 1.0 / 30.0,
                    audio: &audio,
                    params: &params,
                    cover: None,
                    lyric_index: 0,
                    lyric_progress: 0.0,
                    lyric_change: 0.0,
                    lyric_seed: seed,
                    low_quality: false,
                };
                engine.update(&ctx);
                for i in 0..w * h {
                    let field =
                        engine.field((i % w) as f32 / w as f32, (i / w) as f32 / h as f32, &ctx);
                    checksum += field.intensity + field.depth * 0.01;
                }
            }
            let elapsed = start.elapsed().as_secs_f32();
            let name = if *kind == AnimationKind::Adaptive {
                format!("adaptive/{}", engine.label())
            } else {
                kind.name().to_string()
            };
            println!(
                "{name:20} {:6.1} fps  (checksum {:.2})",
                frames as f32 / elapsed.max(1.0e-6),
                black_box(checksum)
            );
        }
    }
    Ok(())
}
