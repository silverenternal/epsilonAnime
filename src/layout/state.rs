use super::{Layout, Panel, Rect};
use crate::state_store;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Snapshot {
    meta: Rect,
    cover: Rect,
    audio: Rect,
    lyrics: Rect,
    logs: Rect,
    shell: Rect,
    focus: Panel,
    minimized: [bool; 6],
    order: Vec<Panel>,
}

fn index(panel: Panel) -> usize {
    match panel {
        Panel::Meta => 0,
        Panel::Cover => 1,
        Panel::AudioScope => 2,
        Panel::Lyrics => 3,
        Panel::Logs => 4,
        Panel::Shell => 5,
    }
}

impl Layout {
    pub fn is_minimized(&self, panel: Panel) -> bool {
        self.minimized[index(panel)]
    }

    pub fn is_focused(&self, panel: Panel) -> bool {
        self.focus == panel && !self.is_minimized(panel)
    }

    pub fn focused_panel(&self) -> Panel {
        self.focus
    }

    pub fn focus_next(&mut self) {
        if let Some(position) = self.order.iter().position(|panel| *panel == self.focus) {
            self.focus = self.order[(position + 1) % self.order.len()];
        }
        self.save_state();
    }

    pub fn toggle_minimize_focused(&mut self) -> bool {
        let focus = self.focus;
        let slot = index(focus);
        self.minimized[slot] = !self.minimized[slot];
        if self.minimized[slot]
            && let Some(next) = self
                .order
                .iter()
                .copied()
                .find(|panel| *panel != focus && !self.minimized[index(*panel)])
        {
            self.focus = next;
        }
        let result = self.minimized[slot];
        self.save_state();
        result
    }

    pub fn restore_all(&mut self) {
        self.minimized.fill(false);
        self.save_state();
    }

    pub fn minimized_panels(&self) -> Vec<Panel> {
        self.order
            .iter()
            .copied()
            .filter(|panel| self.is_minimized(*panel))
            .collect()
    }

    pub fn restore(&mut self, panel: Panel) -> bool {
        let slot = index(panel);
        let was_minimized = self.minimized[slot];
        self.minimized[slot] = false;
        self.focus = panel;
        self.save_state();
        was_minimized
    }

    pub fn restack_focused(&mut self, forward: bool) {
        let Some(position) = self.order.iter().position(|panel| *panel == self.focus) else {
            return;
        };
        if forward && position + 1 < self.order.len() {
            self.order.swap(position, position + 1);
        } else if !forward && position > 0 {
            self.order.swap(position, position - 1);
        }
        self.save_state();
    }

    pub fn draw_order(&self) -> Vec<Panel> {
        self.order
            .iter()
            .copied()
            .filter(|panel| !self.is_minimized(*panel))
            .filter(|panel| *panel != Panel::Logs || self.logs_visible)
            .collect()
    }

    pub fn save_state(&self) {
        let snapshot = Snapshot {
            meta: self.meta,
            cover: self.cover,
            audio: self.audio_scope,
            lyrics: self.lyrics,
            logs: self.logs,
            shell: self.shell,
            focus: self.focus,
            minimized: self.minimized,
            order: self.order.clone(),
        };
        state_store::save("layout.json", &snapshot, 2);
    }

    pub(crate) fn load_state(&mut self) {
        let Some(snapshot) = state_store::load_optional::<Snapshot>("layout.json", 2) else {
            return;
        };
        self.meta = snapshot.meta.clamp(self.width, self.height);
        self.cover = snapshot.cover.clamp(self.width, self.height);
        self.audio_scope = snapshot.audio.clamp(self.width, self.height);
        self.lyrics = snapshot.lyrics.clamp(self.width, self.height);
        if self.logs_visible {
            self.logs = snapshot.logs.clamp(self.width, self.height);
        }
        self.shell = snapshot.shell.clamp(self.width, self.height);
        self.focus = snapshot.focus;
        self.minimized = snapshot.minimized;
        if snapshot.order.len() == 5 {
            self.order = snapshot.order;
        }
    }
}
