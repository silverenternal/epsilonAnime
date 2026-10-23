use super::App;

impl App {
    pub fn focus_next_window(&mut self) {
        self.layout.focus_next();
        self.status = format!("focus: {}", self.layout.focused_panel().name());
    }

    pub fn reset_layout(&mut self) {
        self.layout.reset_balanced();
        self.layout.restore_all();
    }

    pub fn minimize_focused_window(&mut self) {
        let minimized = self.layout.toggle_minimize_focused();
        self.status = format!(
            "{} {}",
            if minimized { "minimized" } else { "restored" },
            self.layout.focused_panel().name()
        );
    }

    pub fn restack_focused_window(&mut self, forward: bool) {
        self.layout.restack_focused(forward);
        self.status = format!("focus: {}", self.layout.focused_panel().name());
    }

    pub fn restore_next_minimized(&mut self) {
        let Some(panel) = self.layout.minimized_panels().first().copied() else {
            self.status = "no minimized windows".into();
            return;
        };
        self.layout.restore(panel);
        self.status = format!("restored {}", panel.name());
    }
}
