use super::App;
use crate::library::{PlaylistItem, SongItem};

impl App {
    pub fn open_library(&mut self) {
        self.library.open = true;
    }

    pub fn begin_library_home(&mut self) {
        self.library.open = true;
        self.library.nav_stack.clear();
        self.library.section_view = false;
        self.library.window = None;
        self.library.songs.clear();
        self.library.playlists = crate::library::home_menu();
        self.library.selected = 0;
        self.library.notice = "正在加载登录态和用户歌单…".into();
    }

    pub fn close_library(&mut self) {
        self.library.open = false;
    }

    pub fn library_open(&self) -> bool {
        self.library.open
    }

    pub fn move_library_selection(&mut self, delta: isize) {
        self.library.move_selection(delta);
    }

    pub fn selected_playlist(&self) -> Option<PlaylistItem> {
        self.library.selected_playlist().cloned()
    }

    pub fn selected_song(&self) -> Option<(usize, SongItem)> {
        self.library
            .selected_song()
            .cloned()
            .map(|song| (self.library.song_selected, song))
    }

    pub fn library_has_songs(&self) -> bool {
        !self.library.songs.is_empty()
    }

    pub fn library_has_more(&self) -> bool {
        self.library_has_songs() && self.library.collection_offset < self.library.collection_total
    }

    pub fn playlist_len(&self) -> usize {
        self.playback.len()
    }

    pub fn collection_kind(&self) -> &str {
        &self.library.collection_kind
    }

    pub fn active_playlist(&self) -> u64 {
        self.library.active_playlist
    }

    pub fn collection_offset(&self) -> usize {
        self.library.collection_offset
    }

    pub fn library_back(&mut self) {
        if !self.library.back() {
            self.close_library();
        }
    }

    pub fn prepare_library_browse(&mut self) {
        self.library.open = true;
        if self.library.playlists.is_empty() {
            self.library.playlists = crate::library::home_menu();
        }
        self.library.push_view();
        self.library.section_view = true;
    }

    pub fn library_collection_value(&self) -> Option<String> {
        let playlist = self.library.selected_playlist()?;
        Some(format!("{}:{}", playlist.kind, playlist.id))
    }

    pub fn current_song_id(&self) -> Option<u64> {
        self.selected_song().map(|(_, song)| song.id)
    }

    pub fn step_playlist(&mut self, delta: isize) -> Option<(usize, SongItem)> {
        let result = self.playback.step(delta);
        if let Some((index, _)) = &result {
            self.playlist_pos = *index;
            self.library.song_selected = *index;
        }
        result
    }

    pub fn playlist_neighbor(&self, delta: isize) -> Option<SongItem> {
        self.playback.neighbor(delta)
    }

    pub fn set_playlist(&mut self, songs: Vec<SongItem>) {
        self.playback.set_songs(songs.clone());
        self.library.songs = songs;
        self.playlist_pos = 0;
        self.library.song_selected = 0;
    }

    pub fn update_current_song_meta(&mut self, title: &str, artist: &str, duration_ms: u64) {
        if let Some(song) = self.library.songs.get_mut(self.playlist_pos) {
            song.title = title.into();
            song.artist = artist.into();
            song.duration_ms = duration_ms;
            self.playback.replace_songs(self.library.songs.clone());
        }
    }

    pub fn set_playlist_pos(&mut self, pos: usize) {
        if let Some((index, _)) = self.playback.select(pos) {
            self.playlist_pos = index;
            self.library.song_selected = index;
        }
    }

    pub fn cycle_play_mode(&mut self) -> &'static str {
        self.playback.cycle_mode().name()
    }

    pub fn play_mode_name(&self) -> &'static str {
        self.playback.mode.name()
    }

    pub fn adjust_volume(&mut self, delta: f32) -> f32 {
        self.playback.set_volume(self.playback.volume + delta);
        self.playback.effective_volume()
    }

    pub fn toggle_mute(&mut self) -> bool {
        self.playback.toggle_mute()
    }

    pub fn effective_volume(&self) -> f32 {
        self.playback.effective_volume()
    }

    pub fn is_muted(&self) -> bool {
        self.playback.muted
    }

    pub fn update_position(&mut self, seconds: f32) {
        self.playback.set_position(seconds);
        self.audio_t = seconds;
    }

    pub fn apply_playback_state(&mut self, position: f32, volume: f32, muted: bool, paused: bool) {
        self.audio_t = position;
        self.playback.position = position;
        if !muted && volume > 0.001 {
            self.playback.volume = volume.clamp(0.0, 1.5);
            self.playback.last_volume = self.playback.volume;
        }
        self.playback.muted = muted;
        self.paused = paused;
    }

    pub fn resume_song(&self) -> Option<SongItem> {
        self.playback.current()
    }

    pub fn resume_position(&self) -> f32 {
        self.playback.position
    }

    pub fn resume_index(&self) -> usize {
        self.playback.position()
    }

    pub fn current_position(&self) -> f32 {
        self.audio_t
    }
}
