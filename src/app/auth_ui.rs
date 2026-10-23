use super::App;
use crate::auth::AuthEvent;

impl App {
    pub fn apply_auth_event(&mut self, event: AuthEvent) {
        match event {
            AuthEvent::Qr { url, lines } => {
                let columns = lines
                    .iter()
                    .map(|line| line.chars().count())
                    .max()
                    .unwrap_or(0);
                self.logs.push(format!(
                    "auth: QR login waiting ({columns}x{} braille)",
                    lines.len()
                ));
                self.library.open = true;
                self.library.qr_url = url;
                self.library.qr_lines = lines;
                self.library.window = None;
                self.library.playlists.clear();
                self.status = "scan QR with NetEase Cloud Music".into();
            }
            AuthEvent::User {
                uid,
                nickname,
                signature,
            } => {
                self.logs
                    .push(format!("auth: logged in as {nickname} ({uid})"));
                self.library.user_uid = uid;
                self.library.nickname = nickname;
                self.library.signature = signature;
                self.library.qr_lines.clear();
                self.library.notice = "登录成功，正在加载用户歌单…".into();
                self.status = "logged in".into();
            }
            AuthEvent::Playlists {
                items: user_items,
                cached,
            } => {
                let user_count = user_items.len();
                let root_menu = !self.library.section_view;
                let mut items = if root_menu {
                    crate::library::home_menu()
                } else {
                    Vec::new()
                };
                items.extend(user_items);
                self.library.section_view = false;
                self.library.notice = if root_menu {
                    format!("用户歌单已加载：{user_count} 个")
                } else {
                    format!("列表已加载：{user_count} 项")
                };
                self.logs.push(format!("auth: {user_count} playlist rows"));
                if user_count == 0 {
                    self.status = "library home ready · g signs in for personal data".into();
                }
                self.library.playlists = items;
                self.library.selected = 0;
                self.library.cached = cached;
                if cached {
                    self.status = "offline playlist cache".into();
                }
                self.library.qr_lines.clear();
            }
            AuthEvent::Songs {
                playlist_id,
                kind,
                offset,
                total,
                title,
                description,
                cached,
                items,
            } => {
                self.logs
                    .push(format!("playlist #{playlist_id}: {} tracks", items.len()));
                self.library.active_playlist = playlist_id;
                self.library.collection_kind = kind;
                self.library.collection_total = total.max(items.len());
                self.library.collection_offset = offset + items.len();
                if offset == 0 {
                    self.library.open_collection(title, description);
                    self.set_playlist(items);
                } else {
                    self.library.songs.extend(items);
                    self.playback.replace_songs(self.library.songs.clone());
                }
                self.library.cached = cached;
                self.library.open = true;
                self.status = if cached {
                    "offline collection cache · Enter to play".into()
                } else {
                    "choose a track, then press Enter".into()
                };
            }
            AuthEvent::Detail {
                kind,
                id,
                title,
                description,
                url,
                related,
            } => {
                self.logs.push(format!("detail: {kind}:{id} {title} {url}"));
                self.library.collection_title = title.clone();
                self.library.collection_description = description;
                if !related.is_empty() {
                    self.library.playlists = related;
                    self.library.selected = 0;
                    self.library.songs.clear();
                }
                self.library.open = true;
                self.status = format!("details: {title}");
            }
            AuthEvent::Message { message } => {
                self.notify("netease", &message);
                self.status = message;
            }
            AuthEvent::Error {
                message,
                code,
                scope,
                retryable,
            } => {
                let scope = if scope.is_empty() { "auth" } else { &scope };
                let mut notification =
                    crate::notifications::NotificationCenter::classify(scope, &message);
                if !code.is_empty() {
                    notification.code = code;
                }
                notification.retryable = retryable;
                self.push_notification(notification);
                self.logs.push(format!("{scope}: {message}"));
                if self.library.open {
                    self.library.notice = format!("加载失败：{message}");
                }
                self.status = message;
            }
            AuthEvent::LoggedOut => {
                self.library.nickname.clear();
                self.library.signature.clear();
                self.library.user_uid = 0;
                self.library.playlists.clear();
                self.library.playlists = crate::library::home_menu();
                self.logs.push("auth: logged out");
                self.status = "logged out".into();
            }
            AuthEvent::Done => {}
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn home_menu_always_exposes_daily_recommendations() {
        let menu = crate::library::home_menu();
        assert_eq!(menu[0].name, "每日推荐");
        assert_eq!(menu[0].kind, "daily");
        assert!(menu.iter().any(|item| item.kind == "radio"));
        assert!(menu.iter().any(|item| item.kind == "recent"));
    }
}
