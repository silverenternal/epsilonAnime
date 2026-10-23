use crate::frame::{self, SurfaceStyle};
use crate::layout::Rect;
use crate::render::{Buffer, Rgb, color_for, readable_text};
use std::collections::VecDeque;
use std::fs::{File, OpenOptions, metadata};
use std::io::{Read, Seek, SeekFrom, Write};
use std::time::{Duration, Instant};

const MAX_LINES: usize = 80;

pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|value| format!("{}:{}:{}", value.file(), value.line(), value.column()))
            .unwrap_or_else(|| "unknown".into());
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("non-string panic payload");
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("target/crash.log")
        {
            let _ = writeln!(file, "{location}: {payload}");
        }
        previous(info);
    }));
}

struct Tail {
    path: &'static str,
    offset: u64,
    pending: String,
}

impl Tail {
    fn new(path: &'static str) -> Self {
        let offset = metadata(path).map(|entry| entry.len()).unwrap_or(0);
        Self {
            path,
            offset,
            pending: String::new(),
        }
    }

    fn poll(&mut self, sink: &mut Vec<String>) {
        let Ok(meta) = metadata(self.path) else {
            return;
        };
        if meta.len() < self.offset {
            self.offset = 0;
            self.pending.clear();
        }
        if meta.len() == self.offset {
            return;
        }
        let Ok(mut file) = File::open(self.path) else {
            return;
        };
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return;
        }
        let mut bytes = Vec::new();
        if file.take(32 * 1024).read_to_end(&mut bytes).is_err() {
            return;
        }
        self.offset += bytes.len() as u64;
        let text = format!("{}{}", self.pending, String::from_utf8_lossy(&bytes));
        let mut parts = text.split('\n').peekable();
        while let Some(line) = parts.next() {
            if parts.peek().is_some() && !line.is_empty() {
                sink.push(format!("{}: {line}", self.path));
            }
        }
        self.pending = text.rsplit('\n').next().unwrap_or("").to_string();
    }
}

pub struct LogStore {
    lines: VecDeque<String>,
    tails: Vec<Tail>,
    next_poll: Instant,
}

impl LogStore {
    pub fn new() -> Self {
        Self {
            lines: VecDeque::from(["ui: F1 shortcuts · F3 logs · F4 shell · Tab layout".into()]),
            tails: vec![
                Tail::new("target/backend.log"),
                Tail::new("target/auth.log"),
                Tail::new("target/prefetch.log"),
            ],
            next_poll: Instant::now(),
        }
    }

    pub fn push(&mut self, line: impl Into<String>) {
        let line = redact_secrets(&line.into())
            .chars()
            .filter(|ch| !ch.is_control())
            .take(180)
            .collect::<String>();
        if line.trim().is_empty() || self.lines.back() == Some(&line) {
            return;
        }
        self.lines.push_back(line);
        while self.lines.len() > MAX_LINES {
            self.lines.pop_front();
        }
    }

    pub fn poll(&mut self) {
        let now = Instant::now();
        if now < self.next_poll {
            return;
        }
        self.next_poll = now + Duration::from_millis(300);
        let mut incoming = Vec::new();
        for tail in &mut self.tails {
            tail.poll(&mut incoming);
        }
        for line in incoming {
            self.push(line);
        }
    }

    pub fn draw(
        &self,
        buffer: &mut Buffer,
        rect: Rect,
        palette: &[[u8; 3]],
        glass: bool,
        focused: bool,
    ) {
        if rect.w < 8 || rect.h < 3 {
            return;
        }
        frame::draw_frame(
            buffer,
            rect,
            "LOGS · F3",
            palette,
            focused,
            if glass {
                SurfaceStyle::Glass
            } else {
                SurfaceStyle::Opaque
            },
        );
        let inner = rect.inner();
        let bg = Rgb::from_slice(palette[0]).scale(0.48);
        let normal = readable_text(bg, color_for(palette, 0.70));
        let warning = readable_text(bg, Rgb::new(255, 186, 120));
        let lines = self.lines.iter().rev().take(inner.h).collect::<Vec<_>>();
        for (row, line) in lines.into_iter().rev().enumerate() {
            let lower = line.to_ascii_lowercase();
            let color = if lower.contains("error") || lower.contains("failed") {
                warning
            } else {
                normal
            };
            buffer.put_str_clipped(inner.x, inner.y + row, inner.w, line, color, bg);
        }
    }
}

fn redact_secrets(line: &str) -> String {
    let mut redacted = redact_url_queries(line);
    redacted = redacted
        .split_whitespace()
        .map(|token| {
            if token.contains("MUSIC_U=") {
                return "MUSIC_U=<redacted>".into();
            }
            token.to_string()
        })
        .collect::<Vec<_>>()
        .join(" ");
    redacted
}

fn redact_url_queries(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(relative) = input[cursor..].find("http") {
        let start = cursor + relative;
        output.push_str(&input[cursor..start]);
        let end = input[start..]
            .find(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '\'' | ',' | '}' | ']'))
            .map_or(input.len(), |offset| start + offset);
        let token = &input[start..end];
        if let Some((base, _)) = token.split_once('?') {
            output.push_str(base);
            output.push_str("?<redacted>");
        } else {
            output.push_str(token);
        }
        cursor = end;
    }
    output.push_str(&input[cursor..]);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_buffer_deduplicates_and_keeps_latest_lines() {
        let mut logs = LogStore::new();
        logs.push("first");
        logs.push("first");
        for index in 0..MAX_LINES + 10 {
            logs.push(format!("line {index}"));
        }
        assert_eq!(logs.lines.len(), MAX_LINES);
        assert_eq!(logs.lines.back().unwrap(), "line 89");
        assert!(!logs.lines.iter().any(|line| line == "first"));
    }

    #[test]
    fn urls_are_redacted_before_logging() {
        let mut logs = LogStore::new();
        logs.push("error https://example.test/a.mp3?token=secret&x=1");
        assert_eq!(
            logs.lines.back().unwrap(),
            "error https://example.test/a.mp3?<redacted>"
        );
    }

    #[test]
    fn nested_json_urls_are_redacted_without_losing_structure() {
        let mut logs = LogStore::new();
        logs.push(
            r#"error {"stream":{"url":"https://example.test/a.mp3?token=secret"},"ok":true}"#,
        );
        let line = logs.lines.back().unwrap();
        assert!(line.contains(r#""url":"https://example.test/a.mp3?<redacted>""#));
        assert!(line.contains(r#""ok":true"#));
        assert!(!line.contains("secret"));
    }
}
