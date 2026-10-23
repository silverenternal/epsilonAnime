use super::{ShellPane, strip_terminal_sequences};
use crate::layout::Rect;
use crate::render::{Buffer, Rgb, color_for, readable_text};
use crate::state_store;
use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const MAX_HISTORY_LINES: usize = 400;

#[derive(Serialize, Deserialize)]
struct ShellSnapshot {
    history: Vec<String>,
    tabs: Vec<String>,
}

pub struct ShellWorkspace {
    panes: Vec<ShellPane>,
    active: usize,
    open: bool,
    search: Option<String>,
    search_miss: bool,
    history: VecDeque<String>,
    next_save: Instant,
    tab_hotkeys: (String, String),
}

impl ShellWorkspace {
    pub fn new() -> Self {
        let history = state_store::load_optional::<ShellSnapshot>("shell-history.json", 1)
            .map(|snapshot| {
                let mut lines: VecDeque<String> = snapshot.history.into();
                while lines.len() > MAX_HISTORY_LINES {
                    lines.pop_front();
                }
                lines
            })
            .unwrap_or_default();
        Self {
            panes: vec![ShellPane::new()],
            active: 0,
            open: false,
            search: None,
            search_miss: false,
            history,
            next_save: Instant::now() + Duration::from_secs(3),
            tab_hotkeys: ("alt-[".into(), "alt-]".into()),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self, cols: usize, rows: usize) -> Result<(), String> {
        self.panes[self.active].open(cols, rows)?;
        self.open = true;
        Ok(())
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn drain(&mut self) {
        for pane in &mut self.panes {
            pane.drain();
        }
        self.capture_history();
        if Instant::now() >= self.next_save {
            self.save();
            self.next_save = Instant::now() + Duration::from_secs(3);
        }
    }

    pub fn send_key(&mut self, key: &KeyEvent) -> Result<(), String> {
        self.panes[self.active].send_key(key)
    }

    pub fn send_text(&mut self, text: &str) -> Result<(), String> {
        self.panes[self.active].send_text(text)
    }

    pub fn scroll(&mut self, lines: i32) {
        self.panes[self.active].scroll(lines);
    }

    pub fn begin_selection(&mut self, row: u16, col: u16) {
        self.panes[self.active].begin_selection(row, col);
    }

    pub fn update_selection(&mut self, row: u16, col: u16) {
        self.panes[self.active].update_selection(row, col);
    }

    pub fn finish_selection(&mut self) -> Option<String> {
        self.panes[self.active].finish_selection()
    }

    pub fn latest_link(&self) -> Option<String> {
        self.panes[self.active].latest_link()
    }

    pub fn next_tab(&mut self, delta: isize, cols: usize, rows: usize) {
        let len = self.panes.len() as isize;
        self.active = (self.active as isize + delta).rem_euclid(len) as usize;
        let _ = self.panes[self.active].open(cols, rows);
        self.open = true;
    }

    pub fn new_tab(&mut self, cols: usize, rows: usize) {
        let mut pane = ShellPane::new();
        if pane.open(cols, rows).is_ok() {
            self.panes.push(pane);
            self.active = self.panes.len() - 1;
            self.open = true;
        }
    }

    pub fn close_tab(&mut self) {
        if self.panes.len() == 1 {
            self.panes[0].close();
            self.open = false;
            return;
        }
        self.panes.remove(self.active);
        self.active = self.active.min(self.panes.len() - 1);
    }

    pub fn tab_label(&self) -> String {
        format!("{}/{}", self.active + 1, self.panes.len())
    }

    pub fn set_tab_hotkeys(&mut self, previous: &str, next: &str) {
        self.tab_hotkeys = (previous.into(), next.into());
    }

    pub fn begin_search(&mut self) {
        self.search = Some(String::new());
        self.search_miss = false;
    }

    pub fn search_active(&self) -> bool {
        self.search.is_some()
    }

    pub fn push_search(&mut self, ch: char) {
        if let Some(query) = &mut self.search {
            query.push(ch);
            self.apply_search();
        }
    }

    pub fn pop_search(&mut self) {
        if let Some(query) = &mut self.search {
            query.pop();
            self.apply_search();
        }
    }

    pub fn finish_search(&mut self) {
        self.search = None;
        self.search_miss = false;
    }

    fn apply_search(&mut self) {
        if let Some(query) = &self.search {
            self.search_miss = !self.panes[self.active].find_next(query);
        }
    }

    pub fn search_status(&self) -> String {
        match &self.search {
            Some(query) => format!(
                "search: {}{}",
                query,
                if self.search_miss { " · no match" } else { "" }
            ),
            None => String::new(),
        }
    }

    pub fn send_mouse(&mut self, event: MouseEvent, rect: Rect) -> bool {
        let pane = &mut self.panes[self.active];
        if !pane.mouse_tracking() {
            return false;
        }
        let inner = rect.inner();
        if !inner.contains(event.column as usize, event.row as usize) {
            return false;
        }
        let col = event.column.saturating_sub(inner.x as u16) + 1;
        let row = event.row.saturating_sub(inner.y as u16) + 1;
        let (code, release) = match event.kind {
            MouseEventKind::Down(MouseButton::Left) => (0, false),
            MouseEventKind::Down(MouseButton::Middle) => (1, false),
            MouseEventKind::Down(MouseButton::Right) => (2, false),
            MouseEventKind::Up(_) => (0, true),
            MouseEventKind::Drag(MouseButton::Left) => (32, false),
            MouseEventKind::ScrollUp => (64, false),
            MouseEventKind::ScrollDown => (65, false),
            _ => return false,
        };
        let suffix = if release { 'm' } else { 'M' };
        let sequence = format!("\x1b[<{code};{col};{row}{suffix}");
        pane.send_mouse_bytes(sequence).is_ok()
    }

    pub fn draw(&mut self, buffer: &mut Buffer, rect: Rect, palette: &[[u8; 3]]) {
        if !self.open {
            return;
        }
        self.panes[self.active].draw(buffer, rect, palette);
        let label = if self.search.is_some() {
            self.search_status()
        } else {
            format!(
                "SHELL · tab {} · {} / {} switch · Alt-T new · Alt-O link",
                self.tab_label(),
                self.tab_hotkeys.0,
                self.tab_hotkeys.1
            )
        };
        let bg = Rgb::from_slice(palette[0]).scale(0.36);
        let fg = readable_text(bg, color_for(palette, 0.92));
        buffer.put_str_clipped(
            rect.x + 2,
            rect.y,
            rect.w.saturating_sub(3),
            &format!(" {label} "),
            fg,
            bg,
        );
    }

    fn capture_history(&mut self) {
        let text = self.panes[self.active].history_text();
        let incoming: Vec<String> = text
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.trim().is_empty())
            .map(str::to_string)
            .collect();
        for line in incoming.into_iter().rev().take(40).rev() {
            if self.history.back() != Some(&line) {
                self.history.push_back(line);
            }
        }
        while self.history.len() > MAX_HISTORY_LINES {
            self.history.pop_front();
        }
    }

    fn save(&self) {
        let snapshot = ShellSnapshot {
            history: self.history.iter().cloned().collect(),
            tabs: self
                .panes
                .iter()
                .map(|pane| strip_terminal_sequences(&pane.history_text()))
                .collect(),
        };
        state_store::save("shell-history.json", &snapshot, 1);
    }
}

impl Drop for ShellWorkspace {
    fn drop(&mut self) {
        self.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_switches_and_closes_tabs() {
        let mut shell = ShellWorkspace::new();
        shell.new_tab(32, 8);
        assert_eq!(shell.tab_label(), "2/2");
        shell.next_tab(-1, 32, 8);
        assert_eq!(shell.tab_label(), "1/2");
        shell.close_tab();
        assert_eq!(shell.tab_label(), "1/1");
    }

    #[test]
    fn search_state_reports_matches_without_panicking() {
        let mut shell = ShellWorkspace::new();
        shell.begin_search();
        shell.push_search('x');
        assert!(shell.search_status().starts_with("search: x"));
        shell.pop_search();
        shell.finish_search();
        assert!(!shell.search_active());
    }

    #[test]
    fn scrollback_search_finds_old_output() {
        let mut shell = ShellWorkspace::new();
        shell.panes[0].parser.process(b"needle in scrollback\r\n");
        for index in 0..40 {
            shell.panes[0]
                .parser
                .process(format!("line {index}\r\n").as_bytes());
        }
        assert!(shell.panes[0].find_next("needle"));
    }
}
