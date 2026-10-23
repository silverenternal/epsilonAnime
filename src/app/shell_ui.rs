use super::App;
use crate::layout::PointerAction;
use crossterm::event::{KeyCode, KeyEvent, MouseEvent};

impl App {
    pub fn shell_scroll(&mut self, lines: i32) {
        self.shell.scroll(lines);
    }

    pub fn shell_selection(&mut self, action: PointerAction, x: usize, y: usize) -> Option<String> {
        let inner = self.layout.rect(crate::layout::Panel::Shell).inner();
        if x < inner.x || y < inner.y || x >= inner.x + inner.w || y >= inner.y + inner.h {
            return None;
        }
        let row = (y - inner.y) as u16;
        let col = (x - inner.x) as u16;
        match action {
            PointerAction::Down => {
                self.shell.begin_selection(row, col);
                None
            }
            PointerAction::Drag => {
                self.shell.update_selection(row, col);
                None
            }
            PointerAction::Up => self.shell.finish_selection(),
        }
    }

    pub fn shell_search_active(&self) -> bool {
        self.shell.search_active()
    }

    pub fn shell_search_key(&mut self, key: &KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => self.shell.finish_search(),
            KeyCode::Backspace => self.shell.pop_search(),
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.shell.push_search(ch)
            }
            _ => {}
        }
        self.status = self.shell.search_status();
    }

    pub fn shell_begin_search(&mut self) {
        self.shell.begin_search();
        self.status = "shell search: type text · Enter closes".into();
    }

    pub fn shell_tab(&mut self, delta: isize) {
        let inner = self.layout.rect(crate::layout::Panel::Shell).inner();
        self.shell.next_tab(delta, inner.w, inner.h);
        self.status = format!("shell tab {}", self.shell.tab_label());
    }

    pub fn shell_new_tab(&mut self) {
        let inner = self.layout.rect(crate::layout::Panel::Shell).inner();
        self.shell.new_tab(inner.w, inner.h);
        self.status = format!("shell tab {}", self.shell.tab_label());
    }

    pub fn shell_close_tab(&mut self) {
        self.shell.close_tab();
        self.status = format!("shell tab {}", self.shell.tab_label());
    }

    pub fn shell_mouse(&mut self, event: MouseEvent) -> bool {
        let rect = self.layout.rect(crate::layout::Panel::Shell);
        self.shell.send_mouse(event, rect)
    }

    pub fn shell_open_latest_link(&mut self) {
        let Some(url) = self.shell.latest_link() else {
            self.status = "no OSC8 link in shell history".into();
            return;
        };
        if !url.starts_with("https://") && !url.starts_with("http://") {
            self.status = "OSC8 link scheme blocked".into();
            return;
        }
        #[cfg(target_os = "macos")]
        let command = ("open", vec![url.clone()]);
        #[cfg(target_os = "windows")]
        let command = (
            "cmd",
            vec!["/C".into(), "start".into(), String::new(), url.clone()],
        );
        #[cfg(all(unix, not(target_os = "macos")))]
        let command = ("xdg-open", vec![url.clone()]);
        match std::process::Command::new(command.0)
            .args(command.1)
            .spawn()
        {
            Ok(_) => self.status = format!("opened {url}"),
            Err(err) => self.status = format!("open link: {err}"),
        }
    }
}
