use crate::frame::{self, SurfaceStyle};
use crate::layout::Rect;
use crate::render::{Buffer, Rgb, color_for};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::num::NonZeroU16;
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread;

mod workspace;
pub use workspace::ShellWorkspace;

#[derive(Debug, Clone, Copy)]
struct Selection {
    anchor: (u16, u16),
    cursor: (u16, u16),
}

pub struct ShellPane {
    parser: vt100::Parser<ShellCallbacks>,
    master: Option<Box<dyn MasterPty + Send>>,
    writer: Option<Box<dyn Write + Send>>,
    child: Option<Box<dyn Child + Send + Sync>>,
    rx: Option<Receiver<Vec<u8>>>,
    rows: u16,
    cols: u16,
    open: bool,
    status: String,
    selection: Option<Selection>,
    links: Arc<Mutex<VecDeque<String>>>,
}

#[derive(Default)]
struct ShellCallbacks {
    links: Arc<Mutex<VecDeque<String>>>,
}

impl vt100::Callbacks for ShellCallbacks {
    fn unhandled_osc(&mut self, _: &mut vt100::Screen, params: &[&[u8]]) {
        if params.first() != Some(&b"8".as_slice()) {
            return;
        }
        let Some(uri) = params.get(2).filter(|uri| !uri.is_empty()) else {
            return;
        };
        if let Ok(mut links) = self.links.lock() {
            links.push_back(String::from_utf8_lossy(uri).into_owned());
            while links.len() > 32 {
                links.pop_front();
            }
        }
    }
}

impl ShellPane {
    pub fn new() -> Self {
        let links = Arc::new(Mutex::new(VecDeque::new()));
        let parser = vt100::Parser::new_with_callbacks(
            nonzero(24),
            nonzero(80),
            2000,
            ShellCallbacks {
                links: Arc::clone(&links),
            },
        );
        Self {
            parser,
            master: None,
            writer: None,
            child: None,
            rx: None,
            rows: 24,
            cols: 80,
            open: false,
            status: "ready".into(),
            selection: None,
            links,
        }
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn open(&mut self, cols: usize, rows: usize) -> Result<(), String> {
        self.ensure_started(cols, rows)?;
        self.open = true;
        Ok(())
    }

    fn ensure_started(&mut self, cols: usize, rows: usize) -> Result<(), String> {
        if let Some(child) = &mut self.child {
            if child.try_wait().ok().flatten().is_none() {
                self.resize(cols, rows);
                return Ok(());
            }
            self.child = None;
        }
        let cols = cols.clamp(8, u16::MAX as usize) as u16;
        let rows = rows.clamp(3, u16::MAX as usize) as u16;
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| err.to_string())?;
        let shell = if cfg!(windows) {
            std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into())
        } else {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
        };
        let mut command = CommandBuilder::new(shell);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        if let Ok(cwd) = std::env::var("EPSILON_ANIME_SHELL_CWD")
            .or_else(|_| std::env::var("HAPPYBIRTHDAY_SHELL_CWD"))
        {
            command.cwd(cwd);
        }
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|err| err.to_string())?;
        drop(pair.slave);
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|err| err.to_string())?;
        let writer = pair.master.take_writer().map_err(|err| err.to_string())?;
        let (tx, rx) = sync_channel::<Vec<u8>>(64);
        thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => {
                        if tx.send(buffer[..count].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        self.parser.set_size(nonzero(rows), nonzero(cols));
        self.rows = rows;
        self.cols = cols;
        self.master = Some(pair.master);
        self.writer = Some(writer);
        self.child = Some(child);
        self.rx = Some(rx);
        self.status = format!("{}x{}", cols, rows);
        Ok(())
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        let cols = cols.clamp(1, u16::MAX as usize) as u16;
        let rows = rows.clamp(1, u16::MAX as usize) as u16;
        if cols == self.cols && rows == self.rows {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        self.parser.set_size(nonzero(rows), nonzero(cols));
        if let Some(master) = &self.master {
            let _ = master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
    }

    pub fn drain(&mut self) {
        let Some(rx) = &self.rx else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(bytes) => self.parser.process(&bytes),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.status = "process exited · F4 reopen".into();
                    break;
                }
            }
        }
    }

    pub fn send_key(&mut self, key: &KeyEvent) -> Result<(), String> {
        self.parser.screen_mut().set_scrollback(0);
        self.selection = None;
        let bytes = encode_key(key);
        if bytes.is_empty() {
            return Ok(());
        }
        let writer = self.writer.as_mut().ok_or("shell is not running")?;
        writer.write_all(&bytes).map_err(|err| err.to_string())?;
        writer.flush().map_err(|err| err.to_string())
    }

    pub fn send_text(&mut self, text: &str) -> Result<(), String> {
        self.parser.screen_mut().set_scrollback(0);
        let writer = self.writer.as_mut().ok_or("shell is not running")?;
        writer
            .write_all(text.as_bytes())
            .map_err(|err| err.to_string())
    }

    pub fn scroll(&mut self, delta: i32) {
        let current = self.parser.screen().scrollback() as i32;
        self.parser
            .screen_mut()
            .set_scrollback((current + delta).max(0) as usize);
    }

    pub fn history_text(&self) -> String {
        let mut output = String::new();
        let mut state = vt100::capture::BasicFormattedCaptureState::new();
        let _ = self.parser.screen().write_contents_formatted_basic(
            &mut output,
            vt100::capture::BasicFormattedCaptureRange::Full(&mut state),
        );
        strip_terminal_sequences(&output)
    }

    pub fn find_next(&mut self, query: &str) -> bool {
        let query = query.to_lowercase();
        if query.is_empty() {
            return false;
        }
        let current = self.parser.screen().scrollback();
        let max = self.parser.screen().scrollback_len();
        let start = current.saturating_sub(1);
        for step in 0..=max {
            let offset = (start + step) % (max + 1);
            self.parser.screen_mut().set_scrollback(offset);
            if self
                .parser
                .screen()
                .contents()
                .to_lowercase()
                .contains(&query)
            {
                return true;
            }
        }
        self.parser.screen_mut().set_scrollback(current);
        false
    }

    pub fn mouse_tracking(&self) -> bool {
        self.parser.screen().mouse_protocol_mode() != vt100::MouseProtocolMode::None
    }

    pub fn latest_link(&self) -> Option<String> {
        self.links.lock().ok()?.back().cloned()
    }

    pub fn send_mouse_bytes(&mut self, bytes: String) -> Result<(), String> {
        let writer = self.writer.as_mut().ok_or("shell is not running")?;
        writer
            .write_all(bytes.as_bytes())
            .and_then(|()| writer.flush())
            .map_err(|err| err.to_string())
    }

    pub fn begin_selection(&mut self, row: u16, col: u16) {
        self.selection = Some(Selection {
            anchor: (row, col),
            cursor: (row, col),
        });
    }

    pub fn update_selection(&mut self, row: u16, col: u16) {
        if let Some(selection) = &mut self.selection {
            selection.cursor = (row, col);
        }
    }

    pub fn finish_selection(&mut self) -> Option<String> {
        let selection = self.selection.take()?;
        let (mut row0, mut col0) = selection.anchor;
        let (mut row1, mut col1) = selection.cursor;
        if (row0, col0) > (row1, col1) {
            std::mem::swap(&mut row0, &mut row1);
            std::mem::swap(&mut col0, &mut col1);
        }
        let screen = self.parser.screen();
        let mut output = String::new();
        for row in row0..=row1 {
            let start = if row == row0 { col0 } else { 0 };
            let end = if row == row1 {
                col1.min(self.cols.saturating_sub(1))
            } else {
                self.cols.saturating_sub(1)
            };
            for col in start..=end {
                if let Some(cell) = screen.cell(row, col)
                    && !cell.is_wide_continuation()
                {
                    output.push_str(cell.contents());
                }
            }
            if row != row1 {
                output.push('\n');
            }
        }
        let trimmed = output.trim_end().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    }

    pub fn draw(&mut self, buffer: &mut Buffer, rect: Rect, palette: &[[u8; 3]]) {
        if !self.open {
            return;
        }
        let inner = rect.inner();
        self.resize(inner.w, inner.h);
        frame::draw_frame(
            buffer,
            rect,
            &format!("SHELL · F4 CLOSE · {}", self.status),
            palette,
            false,
            SurfaceStyle::Transparent,
        );
        if inner.w == 0 || inner.h == 0 {
            return;
        }
        let bg = Rgb::from_slice(palette[0]).scale(0.26);
        let default_fg = color_for(palette, 0.90);
        let screen = self.parser.screen();
        for row in 0..inner.h.min(self.rows as usize) {
            for col in 0..inner.w.min(self.cols as usize) {
                let Some(cell) = screen.cell(row as u16, col as u16) else {
                    continue;
                };
                if cell.is_wide_continuation() {
                    continue;
                }
                let ch = cell.contents().chars().next().unwrap_or(' ');
                let mut fg = shell_color(cell.fgcolor(), default_fg);
                let mut cell_bg = shell_color(cell.bgcolor(), bg);
                if cell.dim() {
                    fg = fg.scale(0.62);
                }
                if cell.bold() {
                    fg = fg.lerp(Rgb::new(255, 255, 255), 0.14);
                }
                if ch == ' ' && cell.bgcolor() == vt100::Color::Default {
                    cell_bg = bg;
                }
                if self
                    .selection
                    .is_some_and(|selection| selected(selection, row as u16, col as u16))
                {
                    std::mem::swap(&mut fg, &mut cell_bg);
                }
                buffer.set_translucent(inner.x + col, inner.y + row, ch, fg, cell_bg, 0.8);
            }
        }
        let (cursor_row, cursor_col) = screen.cursor_position();
        if !screen.hide_cursor()
            && (cursor_row as usize) < inner.h
            && (cursor_col as usize) < inner.w
        {
            let x = inner.x + cursor_col as usize;
            let y = inner.y + cursor_row as usize;
            let (fg, cell_bg) = screen
                .cell(cursor_row, cursor_col)
                .map(|cell| {
                    (
                        shell_color(cell.bgcolor(), bg),
                        shell_color(cell.fgcolor(), default_fg),
                    )
                })
                .unwrap_or((default_fg, bg));
            buffer.set(x, y, '▏', fg, cell_bg);
        }
    }
}

fn selected(selection: Selection, row: u16, col: u16) -> bool {
    let (mut r0, mut c0) = selection.anchor;
    let (mut r1, mut c1) = selection.cursor;
    if (r0, c0) > (r1, c1) {
        std::mem::swap(&mut r0, &mut r1);
        std::mem::swap(&mut c0, &mut c1);
    }
    (row > r0 || (row == r0 && col >= c0)) && (row < r1 || (row == r1 && col <= c1))
}

impl Drop for ShellPane {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.writer.take();
    }
}

fn nonzero(value: u16) -> NonZeroU16 {
    NonZeroU16::new(value.max(1)).unwrap()
}

fn strip_terminal_sequences(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            } else if chars.peek() == Some(&']') {
                chars.next();
                for next in chars.by_ref() {
                    if next == '\x07' {
                        break;
                    }
                }
            }
        } else if ch == '\n' || ch == '\t' || !ch.is_control() {
            output.push(ch);
        }
    }
    output
}

fn encode_key(key: &KeyEvent) -> Vec<u8> {
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let mut bytes = match key.code {
        KeyCode::Char(ch) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let ch = ch.to_ascii_uppercase();
            if ('@'..='_').contains(&ch) {
                vec![(ch as u8) & 0x1f]
            } else {
                Vec::new()
            }
        }
        KeyCode::Char(ch) => ch.to_string().into_bytes(),
        KeyCode::Enter => b"\r".to_vec(),
        KeyCode::Tab => b"\t".to_vec(),
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Backspace => b"\x7f".to_vec(),
        KeyCode::Esc => b"\x1b".to_vec(),
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::F(n) => format!("\x1b[{}~", 10 + n).into_bytes(),
        _ => Vec::new(),
    };
    if alt && !bytes.is_empty() {
        bytes.insert(0, 0x1b);
    }
    bytes
}

fn shell_color(color: vt100::Color, fallback: Rgb) -> Rgb {
    match color {
        vt100::Color::Default => fallback,
        vt100::Color::Rgb(r, g, b) => Rgb::new(r, g, b),
        vt100::Color::Idx(index) => indexed(index),
    }
}

fn indexed(index: u8) -> Rgb {
    const BASIC: [[u8; 3]; 16] = [
        [8, 10, 14],
        [195, 82, 92],
        [92, 184, 120],
        [210, 174, 80],
        [83, 132, 210],
        [176, 112, 196],
        [78, 180, 190],
        [218, 218, 224],
        [82, 88, 102],
        [255, 112, 122],
        [126, 222, 150],
        [245, 211, 110],
        [112, 166, 245],
        [215, 145, 238],
        [112, 218, 228],
        [255, 255, 255],
    ];
    if index < 16 {
        Rgb::from_slice(BASIC[index as usize])
    } else if index < 232 {
        let value = index - 16;
        let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
        Rgb::new(level(value / 36), level((value / 6) % 6), level(value % 6))
    } else {
        let level = 8 + (index - 232) * 10;
        Rgb::new(level, level, level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn encodes_shell_navigation_and_control_keys() {
        let arrow = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(encode_key(&arrow), b"\x1b[A");
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(encode_key(&ctrl_c), vec![3]);
    }

    #[cfg(unix)]
    #[test]
    fn embedded_shell_executes_a_command() {
        let mut shell = ShellPane::new();
        shell.open(48, 12).unwrap();
        shell.send_text("printf 'shell-ok\\n'\r").unwrap();
        for _ in 0..40 {
            shell.drain();
            if shell.parser.screen().contents().contains("shell-ok") {
                return;
            }
            thread::sleep(std::time::Duration::from_millis(25));
        }
        panic!("shell output did not reach the vt100 screen");
    }

    #[test]
    fn visible_selection_returns_cell_text() {
        let mut shell = ShellPane::new();
        shell.parser.process(b"hello");
        shell.begin_selection(0, 0);
        shell.update_selection(0, 4);
        assert_eq!(shell.finish_selection().as_deref(), Some("hello"));
    }

    #[test]
    fn captures_osc8_hyperlink_targets() {
        let mut shell = ShellPane::new();
        shell
            .parser
            .process(b"\x1b]8;;https://example.test/page\x1b\\link\x1b]8;;\x1b\\");
        assert_eq!(
            shell.latest_link().as_deref(),
            Some("https://example.test/page")
        );
    }

    proptest! {
        #[test]
        fn arbitrary_key_events_never_panic(ch in any::<char>(), bits in any::<u8>()) {
            let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::from_bits_truncate(bits));
            let _ = encode_key(&key);
        }
    }
}
