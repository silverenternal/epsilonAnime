use crate::layout::Rect;
use crate::render::{Buffer, Rgb, readable_text};
use crate::state_store;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

const MAX_HISTORY: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationLevel {
    Warning,
    Error,
}

impl NotificationLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Warning => "WARN",
            Self::Error => "ERROR",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub level: NotificationLevel,
    pub scope: String,
    #[serde(default)]
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Default, Serialize, Deserialize)]
pub struct NotificationCenter {
    history: VecDeque<Notification>,
}

impl NotificationCenter {
    pub fn new() -> Self {
        if cfg!(test) {
            return Self::default();
        }
        state_store::load("notifications.json", 2)
    }

    pub fn push(&mut self, notification: Notification) {
        self.history.push_back(notification);
        while self.history.len() > MAX_HISTORY {
            self.history.pop_front();
        }
        self.save();
    }

    pub fn classify(scope: &str, message: &str) -> Notification {
        let lower = message.to_ascii_lowercase();
        let level =
            if lower.contains("error") || lower.contains("failed") || lower.contains("unavailable")
            {
                NotificationLevel::Error
            } else {
                NotificationLevel::Warning
            };
        let retryable = lower.contains("network")
            || lower.contains("audio")
            || lower.contains("http")
            || lower.contains("unavailable");
        let category = if lower.contains("network") || lower.contains("http") {
            "transport"
        } else if lower.contains("audio") || lower.contains("device") {
            "playback"
        } else if lower.contains("login") || lower.contains("auth") {
            "auth"
        } else {
            "runtime"
        };
        let code = format!("{}.{}", scope.replace([' ', ':'], "_"), category);
        Notification {
            level,
            scope: scope.into(),
            code,
            message: message.chars().take(180).collect(),
            retryable,
        }
    }

    pub fn latest_retryable_playback(&self) -> Option<&Notification> {
        self.history.iter().rev().find(|item| {
            item.retryable
                && !matches!(item.scope.as_str(), "auth" | "netease")
                && !item.code.starts_with("auth.")
        })
    }

    pub fn save(&self) {
        state_store::save("notifications.json", self, 2);
    }

    pub fn draw(&self, buffer: &mut Buffer, rect: Rect, palette: &[[u8; 3]]) {
        let inner = rect.inner();
        if inner.w < 8 || inner.h < 2 {
            return;
        }
        let bg = Rgb::from_slice(palette[0]).scale(0.50);
        let rows = self.history.iter().rev().take(inner.h).collect::<Vec<_>>();
        for (row, item) in rows.into_iter().rev().enumerate() {
            let color = match item.level {
                NotificationLevel::Warning => Rgb::new(255, 206, 120),
                NotificationLevel::Error => Rgb::new(255, 126, 132),
            };
            let retry = if item.retryable { " ↻" } else { "" };
            let line = format!(
                "[{}:{}] {}{}",
                item.scope,
                item.level.label(),
                item.message,
                retry
            );
            buffer.put_str_clipped(
                inner.x,
                inner.y + row,
                inner.w,
                &line,
                readable_text(bg, color),
                bg,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_are_classified_and_retryable_when_transient() {
        let notice = NotificationCenter::classify("audio", "network request failed");
        assert_eq!(notice.level, NotificationLevel::Error);
        assert!(notice.retryable);
        assert_eq!(notice.code, "audio.transport");
    }

    #[test]
    fn auth_errors_do_not_select_the_playback_retry_path() {
        let mut center = NotificationCenter::default();
        center.push(NotificationCenter::classify(
            "audio",
            "network request failed",
        ));
        center.push(NotificationCenter::classify("auth", "login unavailable"));
        assert_eq!(
            center
                .latest_retryable_playback()
                .map(|item| item.scope.as_str()),
            Some("audio")
        );
    }
}
