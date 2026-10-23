use crate::library::SongItem;
use crate::state_store;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayMode {
    #[default]
    Sequential,
    RepeatAll,
    RepeatOne,
    Shuffle,
}

impl PlayMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Sequential => "seq",
            Self::RepeatAll => "all",
            Self::RepeatOne => "one",
            Self::Shuffle => "shuffle",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Sequential => Self::RepeatAll,
            Self::RepeatAll => Self::RepeatOne,
            Self::RepeatOne => Self::Shuffle,
            Self::Shuffle => Self::Sequential,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackQueue {
    songs: Vec<SongItem>,
    pos: usize,
    history: VecDeque<SongItem>,
    #[serde(default)]
    shuffle_seen: VecDeque<usize>,
    pub mode: PlayMode,
    pub volume: f32,
    #[serde(default = "default_volume")]
    pub last_volume: f32,
    #[serde(skip)]
    pub muted: bool,
    pub position: f32,
}

impl Default for PlaybackQueue {
    fn default() -> Self {
        Self {
            songs: Vec::new(),
            pos: 0,
            history: VecDeque::new(),
            shuffle_seen: VecDeque::new(),
            mode: PlayMode::Sequential,
            volume: 1.0,
            last_volume: 1.0,
            muted: false,
            position: 0.0,
        }
    }
}

impl PlaybackQueue {
    pub fn load() -> Self {
        let state: Self = state_store::load("playback.json", 3);
        state.normalized()
    }

    fn normalized(mut self) -> Self {
        self.volume = self.volume.clamp(0.0, 1.5);
        self.last_volume = self.last_volume.clamp(0.0, 1.5);
        if self.volume <= 0.001 {
            self.volume = if self.last_volume > 0.001 {
                self.last_volume
            } else {
                1.0
            };
        }
        // Muting is a transient session control; stale muted state on startup
        // should never make the application appear broken.
        self.muted = false;
        self.pos = self.pos.min(self.songs.len().saturating_sub(1));
        self
    }

    pub fn save(&self) {
        state_store::save("playback.json", self, 3);
    }

    pub fn songs(&self) -> &[SongItem] {
        &self.songs
    }

    pub fn len(&self) -> usize {
        self.songs.len()
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn set_songs(&mut self, songs: Vec<SongItem>) {
        self.songs = songs;
        self.pos = 0;
        self.position = 0.0;
        self.history.clear();
        self.shuffle_seen.clear();
        self.save();
    }

    pub fn replace_songs(&mut self, songs: Vec<SongItem>) {
        self.songs = songs;
        self.pos = self.pos.min(self.songs.len().saturating_sub(1));
        self.save();
    }

    pub fn select(&mut self, index: usize) -> Option<(usize, SongItem)> {
        let song = self.songs.get(index)?.clone();
        if let Some(previous) = self.songs.get(self.pos).cloned() {
            self.history.push_back(previous);
            while self.history.len() > 32 {
                self.history.pop_front();
            }
        }
        self.pos = index;
        if self.mode == PlayMode::Shuffle && !self.shuffle_seen.contains(&index) {
            self.shuffle_seen.push_back(index);
        }
        self.position = 0.0;
        self.save();
        Some((index, song))
    }

    pub fn current(&self) -> Option<SongItem> {
        self.songs.get(self.pos).cloned()
    }

    pub fn neighbor(&self, delta: isize) -> Option<SongItem> {
        if self.songs.is_empty() {
            return None;
        }
        let index = (self.pos as isize + delta).rem_euclid(self.songs.len() as isize) as usize;
        self.songs.get(index).cloned()
    }

    pub fn step(&mut self, delta: isize) -> Option<(usize, SongItem)> {
        if self.songs.is_empty() {
            return None;
        }
        let next = if delta < 0 {
            if let Some(previous) = self.history.pop_back() {
                self.songs.iter().position(|song| song.id == previous.id)
            } else {
                Some((self.pos as isize - 1).rem_euclid(self.songs.len() as isize) as usize)
            }
        } else {
            self.advance_index()
        };
        next.and_then(|index| self.select(index))
    }

    fn advance_index(&mut self) -> Option<usize> {
        if self.songs.is_empty() {
            return None;
        }
        match self.mode {
            PlayMode::RepeatOne => Some(self.pos),
            PlayMode::Sequential if self.pos + 1 >= self.songs.len() => None,
            PlayMode::Sequential | PlayMode::RepeatAll => Some((self.pos + 1) % self.songs.len()),
            PlayMode::Shuffle => {
                if self.songs.len() == 1 {
                    return Some(0);
                }
                let candidates: Vec<usize> = (0..self.songs.len())
                    .filter(|index| *index != self.pos && !self.shuffle_seen.contains(index))
                    .collect();
                let pool = if candidates.is_empty() {
                    self.shuffle_seen.clear();
                    (0..self.songs.len())
                        .filter(|index| *index != self.pos)
                        .collect()
                } else {
                    candidates
                };
                pool.get(fastrand::usize(..pool.len())).copied()
            }
        }
    }

    pub fn cycle_mode(&mut self) -> PlayMode {
        self.mode = self.mode.next();
        self.save();
        self.mode
    }

    pub fn set_position(&mut self, seconds: f32) {
        self.position = seconds.max(0.0);
        self.save();
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.5);
        if self.volume > 0.001 {
            self.last_volume = self.volume;
        }
        self.muted = false;
        self.save();
    }

    pub fn toggle_mute(&mut self) -> bool {
        self.muted = !self.muted;
        self.save();
        self.muted
    }

    pub fn effective_volume(&self) -> f32 {
        if self.muted { 0.0 } else { self.volume }
    }
}

fn default_volume() -> f32 {
    1.0
}

impl Drop for PlaybackQueue {
    fn drop(&mut self) {
        self.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(id: u64) -> SongItem {
        SongItem {
            id,
            title: format!("song {id}"),
            artist: "artist".into(),
            duration_ms: 0,
        }
    }

    #[test]
    fn modes_and_history_are_consistent() {
        let mut queue = PlaybackQueue::default();
        queue.songs = vec![song(1), song(2), song(3)];
        assert_eq!(queue.step(1).unwrap().0, 1);
        assert_eq!(queue.step(-1).unwrap().0, 0);
        queue.mode = PlayMode::RepeatOne;
        assert_eq!(queue.step(1).unwrap().0, 0);
        queue.set_volume(0.4);
        assert!(queue.toggle_mute());
        assert_eq!(queue.effective_volume(), 0.0);
        assert_eq!(queue.volume, 0.4);
    }

    #[test]
    fn stale_zero_volume_and_mute_are_repaired_on_load() {
        let mut state = PlaybackQueue::default();
        state.volume = 0.0;
        state.last_volume = 0.7;
        state.muted = true;
        let repaired = PlaybackQueue::normalized(state);
        assert!((repaired.volume - 0.7).abs() < 1.0e-6);
        assert!(!repaired.muted);
    }

    #[test]
    fn mute_is_transient_and_never_persisted() {
        let mut state = PlaybackQueue::default();
        state.volume = 0.8;
        state.last_volume = 0.8;
        state.toggle_mute();
        let encoded = serde_json::to_value(&state).unwrap();
        assert!(encoded.get("muted").is_none());
        assert_eq!(state.effective_volume(), 0.0);
        assert!((encoded["volume"].as_f64().unwrap() - 0.8).abs() < 1.0e-6);
        // toggle_mute did not destroy the logical volume used on the next launch.
        state.muted = false;
        assert_eq!(state.effective_volume(), 0.8);
    }
}
