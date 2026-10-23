use crate::render::Buffer;
use anyhow::Result;
use crossterm::clipboard::CopyToClipboard;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyEvent, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::queue;
use crossterm::style::Colored;
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use std::io::{Write, stdout};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub struct TerminalGuard {
    active: bool,
    writer: Option<FrameWriter>,
}

pub enum InputEvent {
    Key(KeyEvent),
    Paste(String),
    Mouse(MouseEvent),
}

pub struct FrameReport {
    pub encode: Duration,
    pub write_wait: Duration,
}

struct FrameWriter {
    tx: Option<SyncSender<Option<Vec<u8>>>>,
    join: Option<JoinHandle<()>>,
}

impl FrameWriter {
    fn new() -> Self {
        let (tx, rx) = sync_channel::<Option<Vec<u8>>>(4);
        let join = thread::spawn(move || {
            let mut out = stdout().lock();
            while let Ok(Some(frame)) = rx.recv() {
                if out.write_all(&frame).is_err() || out.flush().is_err() {
                    break;
                }
            }
        });
        Self {
            tx: Some(tx),
            join: Some(join),
        }
    }

    fn submit(&self, frame: Vec<u8>) -> Result<Duration> {
        let started = Instant::now();
        if let Some(tx) = &self.tx {
            tx.send(Some(frame))
                .map_err(|_| anyhow::anyhow!("terminal writer stopped"))?;
        }
        Ok(started.elapsed())
    }

    fn close(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(None);
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl TerminalGuard {
    pub fn enter() -> Result<Self> {
        Colored::set_ansi_color_disabled(false);
        enable_raw_mode()?;
        execute!(
            stdout(),
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste,
            Hide,
            Clear(ClearType::All)
        )?;
        Ok(Self {
            active: true,
            writer: Some(FrameWriter::new()),
        })
    }

    pub fn size() -> Result<(usize, usize)> {
        let (w, h) = terminal::size()?;
        Ok((w.max(20) as usize, h.max(8) as usize))
    }

    pub fn poll_input() -> Result<Option<InputEvent>> {
        if !event::poll(Duration::from_millis(0))? {
            return Ok(None);
        }
        match event::read()? {
            Event::Key(key) if key.kind == crossterm::event::KeyEventKind::Press => {
                Ok(Some(InputEvent::Key(key)))
            }
            Event::Paste(text) => Ok(Some(InputEvent::Paste(text))),
            Event::Mouse(mouse)
                if matches!(
                    mouse.kind,
                    MouseEventKind::Down(MouseButton::Left)
                        | MouseEventKind::Drag(MouseButton::Left)
                        | MouseEventKind::Up(MouseButton::Left)
                ) =>
            {
                Ok(Some(InputEvent::Mouse(mouse)))
            }
            _ => Ok(None),
        }
    }

    pub fn draw_with_graphics(
        &self,
        buffer: &mut Buffer,
        graphics: Vec<u8>,
    ) -> Result<FrameReport> {
        let encode_started = Instant::now();
        let body = buffer.encode()?;
        let encode = encode_started.elapsed();
        let mut frame = Vec::with_capacity(body.len() + graphics.len() + 16);
        frame.extend_from_slice(b"\x1b[?2026h");
        frame.extend_from_slice(&body);
        frame.extend_from_slice(&graphics);
        frame.extend_from_slice(b"\x1b[?2026l");
        let write_wait = if let Some(writer) = &self.writer {
            writer.submit(frame)?
        } else {
            Duration::ZERO
        };
        Ok(FrameReport { encode, write_wait })
    }

    pub fn copy(&self, text: &str) -> Result<()> {
        let mut frame = Vec::new();
        queue!(frame, CopyToClipboard::to_clipboard_from(text))?;
        if let Some(writer) = &self.writer {
            writer.submit(frame)?;
        }
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        if let Some(mut writer) = self.writer.take() {
            writer.close();
        }
        let _ = stdout().write_all(&crate::kitty::delete_image());
        let _ = stdout().flush();
        let _ = disable_raw_mode();
        let _ = execute!(
            stdout(),
            DisableBracketedPaste,
            DisableMouseCapture,
            Show,
            LeaveAlternateScreen,
            Clear(ClearType::All)
        );
        let _ = execute!(stdout(), MoveTo(0, 0));
        self.active = false;
    }
}
