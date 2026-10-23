use super::App;
use crate::notifications::{Notification, NotificationCenter};

impl App {
    pub fn notify(&mut self, scope: &str, message: &str) {
        self.logs.push(format!("{scope}: {message}"));
        self.notifications
            .push(NotificationCenter::classify(scope, message));
    }

    pub fn toggle_notifications(&mut self) {
        if !self.opts.show_logs {
            self.opts.show_logs = true;
            self.layout.set_logs_visible(true);
        }
        self.notifications_view = !self.notifications_view;
        self.status = if self.notifications_view {
            "notifications · Alt+R retries · Alt+N closes"
        } else {
            "live logs"
        }
        .into();
    }

    pub fn retry_link(&self) -> Option<String> {
        self.notifications
            .latest_retryable_playback()
            .and_then(|_| self.retry_link.clone())
    }

    pub fn record_retry_link(&mut self, link: &str) {
        self.retry_link = Some(link.to_string());
    }

    pub fn push_notification(&mut self, notification: Notification) {
        self.notifications.push(notification);
    }
}
