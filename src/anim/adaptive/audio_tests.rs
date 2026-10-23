use super::*;
use crate::anim::AudioParams;
use crate::anim::bind::AnimParams;

#[test]
fn generated_macros_receive_runtime_audio_parameters() {
    let quiet = generated_live_weave(0.37, 0.61, 1.2, 0.0, 0.0, 0.0);
    let loud = generated_live_weave(0.37, 0.61, 1.2, 1.0, 1.0, 1.0);
    assert!((quiet - loud).abs() > 0.01);
    let dark = generated_live_light(0.4, 0.7, 0.2, 0.0, 0.0, 0.0);
    let bright = generated_live_light(0.4, 0.7, 0.2, 1.0, 1.0, 1.0);
    assert!((dark - bright).abs() > 0.01);
}

#[test]
fn musical_descriptor_changes_scene_selection() {
    let params = AnimParams::default();
    let mut modes = std::collections::BTreeSet::new();
    for step in 0..16 {
        let audio = AudioParams {
            bpm: 62.0 + step as f32 * 8.0,
            rms: 0.08 + step as f32 * 0.055,
            bass: 0.1 + (step % 4) as f32 * 0.2,
            spectrum: [step as f32 / 15.0; 16],
            centroid: step as f32 / 15.0,
            ..Default::default()
        };
        let ctx = FrameCtx {
            width: 40,
            height: 20,
            t: 2.0,
            dt: 1.0 / 30.0,
            audio: &audio,
            params: &params,
            cover: None,
            lyric_index: 2,
            lyric_progress: 0.4,
            lyric_change: 0.0,
            lyric_seed: 7,
            low_quality: false,
        };
        let mut scene = Adaptive::new(40, 20);
        scene.update(&ctx);
        modes.insert(scene.mode);
    }
    assert!(modes.len() > 1, "audio descriptors selected {modes:?}");
}

#[test]
fn stable_music_rotates_within_its_audio_family() {
    let audio = AudioParams {
        bpm: 118.0,
        rms: 0.42,
        bass: 0.35,
        spectrum: [0.35; 16],
        centroid: 0.4,
        flatness: 0.2,
        stereo_width: 0.25,
        ..Default::default()
    };
    let params = AnimParams::default();
    let ctx = FrameCtx {
        width: 40,
        height: 20,
        t: 2.0,
        dt: 2.1,
        audio: &audio,
        params: &params,
        cover: None,
        lyric_index: 2,
        lyric_progress: 0.4,
        lyric_change: 0.0,
        lyric_seed: 7,
        low_quality: false,
    };
    let mut scene = Adaptive::new(40, 20);
    let mut modes = std::collections::BTreeSet::new();
    for _ in 0..12 {
        scene.update(&ctx);
        modes.insert(scene.mode);
    }
    assert!(modes.len() > 1, "stable audio selected {modes:?}");
}

#[test]
fn rotation_regularly_enters_a_detailed_fractal_chapter() {
    let audio = AudioParams {
        bpm: 118.0,
        rms: 0.42,
        bass: 0.35,
        spectrum: [0.35; 16],
        centroid: 0.4,
        flatness: 0.2,
        stereo_width: 0.25,
        ..Default::default()
    };
    let params = AnimParams::default();
    let ctx = FrameCtx {
        width: 40,
        height: 20,
        t: 2.0,
        dt: 2.1,
        audio: &audio,
        params: &params,
        cover: None,
        lyric_index: 2,
        lyric_progress: 0.4,
        lyric_change: 0.0,
        lyric_seed: 7,
        low_quality: false,
    };
    let mut scene = Adaptive::new(40, 20);
    let mut saw_fractal = false;
    for _ in 0..30 {
        scene.update(&ctx);
        saw_fractal |= matches!(scene.mode, 1 | 6 | 10);
    }
    assert!(saw_fractal, "fractal chapter never entered");
}
