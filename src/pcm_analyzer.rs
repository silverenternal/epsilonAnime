use crate::events::BackendEvent;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex32;
use std::io::{self, Read};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use wide::f32x8;

const FFT_SIZE: usize = 2048;
const BANDS: usize = 16;
const MAGIC: &[u8; 4] = b"HBP1";

pub struct PcmAnalyzer {
    path: PathBuf,
    stop: Arc<AtomicBool>,
    stream: Arc<Mutex<Option<UnixStream>>>,
    join: Option<JoinHandle<()>>,
}

impl PcmAnalyzer {
    pub fn bind(path: PathBuf, tx: Sender<BackendEvent>) -> io::Result<Self> {
        let listener = UnixListener::bind(&path)?;
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Arc::new(Mutex::new(None));
        let thread_stop = Arc::clone(&stop);
        let thread_stream = Arc::clone(&stream);
        let join = thread::spawn(move || {
            listen_and_read(listener, tx, thread_stop, thread_stream);
        });
        Ok(Self {
            path,
            stop,
            stream,
            join: Some(join),
        })
    }
}

impl Drop for PcmAnalyzer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Ok(slot) = self.stream.lock()
            && let Some(stream) = slot.as_ref()
        {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        self.join.take();
        let _ = std::fs::remove_file(&self.path);
    }
}

fn listen_and_read(
    listener: UnixListener,
    tx: Sender<BackendEvent>,
    stop: Arc<AtomicBool>,
    stream_slot: Arc<Mutex<Option<UnixStream>>>,
) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                if let Ok(clone) = stream.try_clone()
                    && let Ok(mut slot) = stream_slot.lock()
                {
                    *slot = Some(clone);
                }
                read_stream(stream, &tx);
                return;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(8));
            }
            Err(_) => return,
        }
    }
}

fn read_stream(mut stream: UnixStream, tx: &Sender<BackendEvent>) {
    let mut header = [0u8; 12];
    if stream.read_exact(&mut header).is_err() || &header[..4] != MAGIC {
        return;
    }
    let sample_rate = u32::from_le_bytes(header[4..8].try_into().unwrap()).max(8000);
    let channels = u16::from_le_bytes(header[8..10].try_into().unwrap()).max(1) as usize;
    let mut engine = FeatureEngine::new(sample_rate, channels);
    loop {
        let mut packet = [0u8; 8];
        if stream.read_exact(&mut packet).is_err() {
            break;
        }
        let timestamp = f32::from_le_bytes(packet[0..4].try_into().unwrap()) as f64;
        let byte_len = u32::from_le_bytes(packet[4..8].try_into().unwrap()) as usize;
        if byte_len == 0 || !byte_len.is_multiple_of(4) || byte_len > 32 * 1024 * 1024 {
            break;
        }
        let mut raw = vec![0u8; byte_len];
        if stream.read_exact(&mut raw).is_err() {
            break;
        }
        let samples: Vec<f32> = raw
            .chunks_exact(4)
            .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
            .collect();
        if let Some(features) = engine.process(&samples, timestamp)
            && tx
                .send(BackendEvent::Features {
                    t: features.t,
                    rms: features.rms,
                    bass: features.bass,
                    mid: features.mid,
                    treble: features.treble,
                    flux: features.flux,
                    onset: features.onset,
                    bpm: features.bpm,
                    spectrum: features.spectrum.to_vec(),
                    centroid: features.centroid,
                    spread: features.spread,
                    flatness: features.flatness,
                    rolloff: features.rolloff,
                    zcr: features.zcr,
                    crest: features.crest,
                    stereo_width: features.stereo_width,
                })
                .is_err()
        {
            return;
        }
    }
}

struct FeatureEngine {
    sample_rate: f32,
    channels: usize,
    window: Vec<f32>,
    hann: Vec<f32>,
    prev_mag: Vec<f32>,
    flux_history: Vec<f32>,
    onsets: Vec<f64>,
    fft: Arc<dyn rustfft::Fft<f32>>,
    scratch: Vec<Complex32>,
}

struct Features {
    t: f32,
    rms: f32,
    bass: f32,
    mid: f32,
    treble: f32,
    flux: f32,
    onset: f32,
    bpm: f32,
    spectrum: [f32; BANDS],
    centroid: f32,
    spread: f32,
    flatness: f32,
    rolloff: f32,
    zcr: f32,
    crest: f32,
    stereo_width: f32,
}

impl FeatureEngine {
    fn new(sample_rate: u32, channels: usize) -> Self {
        let hann = (0..FFT_SIZE)
            .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (FFT_SIZE - 1) as f32).cos())
            .collect();
        let fft = FftPlanner::<f32>::new().plan_fft_forward(FFT_SIZE);
        Self {
            sample_rate: sample_rate as f32,
            channels,
            window: vec![0.0; FFT_SIZE],
            hann,
            prev_mag: vec![0.0; FFT_SIZE / 2 + 1],
            flux_history: Vec::with_capacity(28),
            onsets: Vec::with_capacity(8),
            fft,
            scratch: vec![Complex32::default(); FFT_SIZE],
        }
    }

    fn process(&mut self, samples: &[f32], timestamp: f64) -> Option<Features> {
        let frames = samples.len() / self.channels;
        if frames == 0 {
            return None;
        }
        let mono: Vec<f32> = samples
            .chunks_exact(self.channels)
            .map(|frame| frame.iter().sum::<f32>() / self.channels as f32)
            .collect();
        let zcr = mono
            .windows(2)
            .filter(|pair| pair[0].is_sign_positive() != pair[1].is_sign_positive())
            .count() as f32
            / (mono.len().saturating_sub(1).max(1)) as f32;
        let stereo_width = if self.channels >= 2 {
            let mut side = 0.0;
            let mut total = 0.0;
            for frame in samples.chunks_exact(self.channels) {
                let mid = frame.iter().sum::<f32>() / self.channels as f32;
                for value in frame {
                    let diff = value - mid;
                    side += diff * diff;
                    total += value * value;
                }
            }
            (side / total.max(1.0e-9)).sqrt().clamp(0.0, 1.0)
        } else {
            0.0
        };
        if mono.len() >= FFT_SIZE {
            self.window.copy_from_slice(&mono[mono.len() - FFT_SIZE..]);
        } else {
            self.window.copy_within(mono.len().., 0);
            self.window[FFT_SIZE - mono.len()..].copy_from_slice(&mono);
        }

        let mut energy = f32x8::ZERO;
        for i in (0..FFT_SIZE).step_by(8) {
            let s = f32x8::new(self.window[i..i + 8].try_into().unwrap());
            let h = f32x8::new(self.hann[i..i + 8].try_into().unwrap());
            let windowed = s * h;
            energy += windowed * windowed;
            for (j, value) in windowed.to_array().iter().enumerate() {
                self.scratch[i + j] = Complex32::new(*value, 0.0);
            }
        }
        let rms = (energy.reduce_add() / FFT_SIZE as f32).sqrt();
        self.fft.process(&mut self.scratch);
        let mag: Vec<f32> = self.scratch[..=FFT_SIZE / 2]
            .iter()
            .map(|v| v.norm())
            .collect();
        let sum_mag = mag.iter().sum::<f32>().max(1.0e-9);
        let centroid = mag
            .iter()
            .enumerate()
            .map(|(i, value)| i as f32 / (mag.len() - 1) as f32 * value)
            .sum::<f32>()
            / sum_mag;
        let spread = (mag
            .iter()
            .enumerate()
            .map(|(i, value)| {
                let f = i as f32 / (mag.len() - 1) as f32;
                (f - centroid).powi(2) * value
            })
            .sum::<f32>()
            / sum_mag)
            .sqrt();
        let flatness = ((mag.iter().map(|v| (v + 1.0e-9).ln()).sum::<f32>() / mag.len() as f32)
            .exp()
            / (sum_mag / mag.len() as f32))
            .clamp(0.0, 1.0);
        let mut cumulative = 0.0;
        let mut rolloff = 0.0;
        for (i, value) in mag.iter().enumerate() {
            cumulative += value;
            if cumulative >= sum_mag * 0.85 {
                rolloff = i as f32 / (mag.len() - 1) as f32;
                break;
            }
        }
        let peak = self.window.iter().map(|v| v.abs()).fold(0.0, f32::max);
        let crest = (peak / rms.max(1.0e-9)).clamp(0.0, 4.0) / 4.0;
        let flux_raw = mag
            .iter()
            .zip(&self.prev_mag)
            .map(|(a, b)| (a - b).max(0.0))
            .sum::<f32>()
            / mag.len() as f32;
        let mean = average(&self.flux_history);
        let stddev = stddev(&self.flux_history, mean);
        let onset = flux_raw > mean + (stddev * 1.4).max(0.012);
        if onset {
            self.onsets.push(timestamp);
            if self.onsets.len() > 8 {
                self.onsets.remove(0);
            }
        }
        self.flux_history.push(flux_raw);
        if self.flux_history.len() > 28 {
            self.flux_history.remove(0);
        }
        self.prev_mag.copy_from_slice(&mag);
        Some(Features {
            t: timestamp as f32,
            rms: (rms * 8.0).ln_1p() / 1.8,
            bass: band(&mag, self.sample_rate, 25.0, 250.0),
            mid: band(&mag, self.sample_rate, 250.0, 2200.0),
            treble: band(&mag, self.sample_rate, 2200.0, 10000.0),
            flux: (flux_raw / 1.5).clamp(0.0, 1.0),
            onset: if onset { 1.0 } else { 0.0 },
            bpm: self.bpm(),
            spectrum: self.spectrum(&mag),
            centroid,
            spread,
            flatness,
            rolloff,
            zcr,
            crest,
            stereo_width,
        })
    }

    fn spectrum(&self, mag: &[f32]) -> [f32; BANDS] {
        let mut out = [0.0; BANDS];
        let low = 40.0f32;
        let ratio = (12000.0 / low).powf(1.0 / BANDS as f32);
        for (i, value) in out.iter_mut().enumerate() {
            let lo = low * ratio.powi(i as i32);
            *value = band(mag, self.sample_rate, lo, lo * ratio);
        }
        out
    }

    fn bpm(&self) -> f32 {
        if self.onsets.len() < 3 {
            return 0.0;
        }
        let mut intervals: Vec<f64> = self
            .onsets
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .filter(|value| *value > 0.18)
            .collect();
        if intervals.is_empty() {
            return 0.0;
        }
        intervals.sort_by(f64::total_cmp);
        let mut bpm = 60.0 / intervals[intervals.len() / 2];
        while bpm < 60.0 {
            bpm *= 2.0;
        }
        while bpm > 190.0 {
            bpm /= 2.0;
        }
        bpm as f32
    }
}

fn band(mag: &[f32], sample_rate: f32, low: f32, high: f32) -> f32 {
    let scale = FFT_SIZE as f32 / sample_rate;
    let start = (low * scale).floor().max(1.0) as usize;
    let end = (high * scale).ceil().min((mag.len() - 1) as f32) as usize;
    if end <= start {
        return 0.0;
    }
    let energy = mag[start..=end].iter().map(|v| v * v).sum::<f32>() / (end - start + 1) as f32;
    (energy.sqrt() / 180.0).sqrt().clamp(0.0, 1.0)
}

fn average(values: &[f32]) -> f32 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f32>() / values.len() as f32
    }
}

fn stddev(values: &[f32], mean: f32) -> f32 {
    if values.is_empty() {
        0.0
    } else {
        (values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / values.len() as f32).sqrt()
    }
}

pub fn socket_path(counter: u64) -> PathBuf {
    std::env::temp_dir().join(format!(
        "happybirthday-pcm-{}-{counter}.sock",
        std::process::id()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::io::Write;
    use std::sync::mpsc;

    #[test]
    fn synthetic_tone_produces_spectrum() {
        let mut engine = FeatureEngine::new(44_100, 2);
        let mut samples = Vec::new();
        for i in 0..2048 {
            let v = (std::f32::consts::TAU * 440.0 * i as f32 / 44_100.0).sin();
            samples.extend([v, v]);
        }
        let features = engine.process(&samples, 1.0).unwrap();
        assert!(features.rms > 0.0);
        assert!(features.spectrum.iter().any(|v| *v > 0.1));
    }

    #[test]
    fn unix_stream_pipeline_emits_features() {
        let path = socket_path(999_999);
        let _ = std::fs::remove_file(&path);
        let (tx, rx) = mpsc::channel();
        let analyzer = PcmAnalyzer::bind(path.clone(), tx).unwrap();
        let mut sender = UnixStream::connect(&path).unwrap();
        sender
            .write_all(&[b'H', b'B', b'P', b'1', 0x44, 0xac, 0, 0, 2, 0, 0, 0])
            .unwrap();
        let mut samples = Vec::new();
        for i in 0..1024 {
            let value = (std::f32::consts::TAU * 440.0 * i as f32 / 44_100.0).sin();
            samples.extend_from_slice(&value.to_le_bytes());
            samples.extend_from_slice(&value.to_le_bytes());
        }
        sender.write_all(&1.0f32.to_le_bytes()).unwrap();
        sender
            .write_all(&(samples.len() as u32).to_le_bytes())
            .unwrap();
        sender.write_all(&samples).unwrap();
        let event = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        match event {
            BackendEvent::Features { spectrum, rms, .. } => {
                assert!(rms > 0.0);
                assert_eq!(spectrum.len(), BANDS);
                assert!(spectrum.iter().any(|v| *v > 0.1));
            }
            _ => panic!("expected features"),
        }
        drop(sender);
        drop(analyzer);
    }

    proptest! {
        #[test]
        fn arbitrary_pcm_frames_never_panic(samples in prop::collection::vec(-1.0f32..1.0, 0..8192)) {
            let mut engine = FeatureEngine::new(44_100, 2);
            let _ = engine.process(&samples, 1.0);
        }
    }
}
