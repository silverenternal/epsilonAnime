use super::App;
use crate::frame::{self, SurfaceStyle};
use crate::layout::Rect;
use crate::render::{Rgb, color_for, readable_text};
use crate::shortcuts;
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    pub fn help_open(&self) -> bool {
        self.help_page.is_some()
    }

    pub fn toggle_help(&mut self) {
        self.help_page = self.help_page.is_none().then_some(0);
    }

    pub fn help_key(&mut self, key: &KeyEvent) {
        let pages = shortcuts::pages(&self.opts.hotkeys).len();
        let page = self.help_page.get_or_insert(0);
        match key.code {
            KeyCode::F(1) | KeyCode::Esc => self.help_page = None,
            KeyCode::Char('h' | 'H')
                if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) =>
            {
                self.help_page = None;
            }
            KeyCode::Left | KeyCode::Up | KeyCode::BackTab => {
                *page = page.saturating_sub(1);
            }
            KeyCode::Right | KeyCode::Down | KeyCode::Tab => {
                *page = (*page + 1) % pages;
            }
            _ => {}
        }
    }

    pub(super) fn draw_help(&mut self, palette: &[[u8; 3]]) {
        let Some(page_index) = self.help_page else {
            return;
        };
        let pages = shortcuts::pages(&self.opts.hotkeys);
        if pages.is_empty() || self.buffer.width < 34 || self.buffer.height < 10 {
            return;
        }
        let page = &pages[page_index.min(pages.len() - 1)];
        let width = self.buffer.width.saturating_sub(6).min(92);
        let height = self
            .buffer
            .height
            .saturating_sub(4)
            .min(page.entries.len() + 5)
            .max(8);
        let rect = Rect::new(
            (self.buffer.width - width) / 2,
            (self.buffer.height - height) / 2,
            width,
            height,
        );
        let title = format!(
            "快捷键 · {} · {}/{}",
            page.title,
            page_index.min(pages.len() - 1) + 1,
            pages.len()
        );
        frame::draw_frame(
            &mut self.buffer,
            rect,
            &title,
            palette,
            false,
            SurfaceStyle::Opaque,
        );
        let inner = rect.inner();
        let bg = Rgb::from_slice(palette[0]).scale(0.42);
        let key_color = readable_text(bg, color_for(palette, 0.94));
        let text_color = readable_text(bg, color_for(palette, 0.76));
        let key_width = 19.min(inner.w.saturating_sub(12));
        for (row, (keys, action)) in page
            .entries
            .iter()
            .take(inner.h.saturating_sub(2))
            .enumerate()
        {
            let y = inner.y + row;
            self.buffer
                .put_str_clipped(inner.x + 1, y, key_width, keys, key_color, bg);
            self.buffer.put_str_clipped(
                inner.x + 1 + key_width,
                y,
                inner.w.saturating_sub(key_width + 1),
                action,
                text_color,
                bg,
            );
        }
        self.buffer.put_str_clipped(
            inner.x + 1,
            inner.y + inner.h.saturating_sub(1),
            inner.w.saturating_sub(2),
            "←/→ 或 Tab 翻页 · F1/Esc 关闭",
            text_color.scale(0.78),
            bg,
        );
    }
}
