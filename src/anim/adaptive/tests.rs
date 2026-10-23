use super::*;
use crate::anim::AudioParams;
use crate::anim::bind::AnimParams;

#[test]
fn seed_selects_one_mode_and_stays_bounded() {
    let mut scene = Adaptive::new(60, 24);
    let audio = AudioParams {
        bpm: 120.0,
        spectrum: [0.5; 16],
        ..Default::default()
    };
    let params = AnimParams::default();
    let ctx = FrameCtx {
        width: 60,
        height: 24,
        t: 2.0,
        dt: 1.0 / 30.0,
        audio: &audio,
        params: &params,
        cover: None,
        lyric_index: 1,
        lyric_progress: 0.3,
        lyric_change: 0.0,
        lyric_seed: 0x1234_5678,
        low_quality: false,
    };
    scene.update(&ctx);
    assert!(scene.mode < MODE_NAMES.len());
    let selected = scene.mode;
    scene.update(&ctx);
    assert_eq!(scene.mode, selected);
    for (x, y) in [(0.0, 0.0), (0.5, 0.5), (1.0, 1.0)] {
        assert!((0.0..=1.0).contains(&scene.sample(x, y, &ctx)));
    }
}

#[test]
fn spatial_modes_fill_a_multilevel_field() {
    let audio = AudioParams {
        bpm: 126.0,
        bass: 0.6,
        mid: 0.5,
        crest: 0.4,
        stereo_width: 0.5,
        spectrum: [0.6; 16],
        ..Default::default()
    };
    let params = AnimParams::default();
    for (seed, id) in MODE_NAMES.iter().enumerate().skip(12) {
        let mut scene = Adaptive::new(60, 24);
        scene.policy.favorites.clear();
        scene.policy.favorites.insert((*id).into());
        let ctx = FrameCtx {
            width: 60,
            height: 24,
            t: 2.0,
            dt: 1.0 / 30.0,
            audio: &audio,
            params: &params,
            cover: None,
            lyric_index: 0,
            lyric_progress: 0.4,
            lyric_change: 0.0,
            lyric_seed: seed as u64,
            low_quality: false,
        };
        for _ in 0..5 {
            scene.update(&ctx);
        }
        let mut samples = Vec::new();
        for y in 0..24 {
            for x in 0..60 {
                samples.push(scene.field(x as f32 / 60.0, y as f32 / 24.0, &ctx));
            }
        }
        assert!(samples.iter().any(|sample| sample.intensity > 0.01));
        assert!(
            samples
                .iter()
                .map(|sample| sample.depth)
                .fold(0.0f32, f32::max)
                > 0.2
        );
        if seed >= 12 {
            let min_light = samples
                .iter()
                .map(|sample| sample.light)
                .fold(1.0f32, f32::min);
            let max_light = samples
                .iter()
                .map(|sample| sample.light)
                .fold(0.0f32, f32::max);
            assert!(
                max_light - min_light > 0.05,
                "seed {seed} light range {min_light}..{max_light}"
            );
        }
    }
}

#[test]
fn scene_skeleton_changes_only_at_phrase_boundaries() {
    let audio = AudioParams::default();
    let params = AnimParams::default();
    let mut scene = Adaptive::new(40, 20);
    scene.policy.favorites.insert(MODE_NAMES[5].into());
    let make_ctx = |seed, index| FrameCtx {
        width: 40,
        height: 20,
        t: 1.0,
        dt: 1.0 / 30.0,
        audio: &audio,
        params: &params,
        cover: None,
        lyric_index: index,
        lyric_progress: 0.0,
        lyric_change: 0.0,
        lyric_seed: seed,
        low_quality: false,
    };
    scene.update(&make_ctx(1, 0));
    assert_eq!(scene.mode, 5);
    let mode = scene.mode;
    scene.update(&make_ctx(2, 2));
    assert_eq!(scene.mode, mode);
    scene.update(&make_ctx(5, 4));
    assert_eq!(scene.mode, 5 % MODE_NAMES.len());
}
