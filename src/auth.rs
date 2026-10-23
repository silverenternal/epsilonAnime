use crate::library::{PlaylistItem, SongItem};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthEvent {
    Qr {
        url: String,
        lines: Vec<String>,
    },
    User {
        uid: u64,
        nickname: String,
        signature: String,
    },
    Playlists {
        items: Vec<PlaylistItem>,
        #[serde(default)]
        cached: bool,
    },
    Songs {
        playlist_id: u64,
        #[serde(default = "playlist_kind")]
        kind: String,
        #[serde(default)]
        offset: usize,
        #[serde(default)]
        total: usize,
        #[serde(default)]
        title: String,
        #[serde(default)]
        description: String,
        #[serde(default)]
        cached: bool,
        items: Vec<SongItem>,
    },
    Detail {
        kind: String,
        id: u64,
        title: String,
        #[serde(default)]
        description: String,
        #[serde(default)]
        url: String,
        #[serde(default)]
        related: Vec<PlaylistItem>,
    },
    Message {
        message: String,
    },
    LoggedOut,
    Error {
        message: String,
        #[serde(default)]
        code: String,
        #[serde(default)]
        scope: String,
        #[serde(default)]
        retryable: bool,
    },
    Done,
}

fn playlist_kind() -> String {
    "playlist".into()
}

pub struct AuthClient {
    child: Child,
    rx: Receiver<AuthEvent>,
}

impl AuthClient {
    pub fn start(command: &str, value: Option<&str>) -> Result<Self> {
        std::fs::create_dir_all("target").ok();
        let log = Arc::new(Mutex::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open("target/auth.log")
                .context("open auth log")?,
        ));
        let mut cmd = Command::new("uv");
        cmd.args(["run", "python", "-m", "backend.auth", command])
            .args(value)
            .current_dir("py")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        let mut child = cmd.spawn().context("spawn auth backend")?;
        let stdout = child.stdout.take().context("auth stdout")?;
        let stderr = child.stderr.take().context("auth stderr")?;
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let event = serde_json::from_str(&line).unwrap_or(AuthEvent::Error {
                    message: format!(
                        "bad auth event: {}",
                        line.chars().take(120).collect::<String>()
                    ),
                    code: "auth.parse".into(),
                    scope: "auth".into(),
                    retryable: false,
                });
                if tx.send(event).is_err() {
                    break;
                }
            }
            let _ = tx.send(AuthEvent::Done);
        });
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if let Ok(mut file) = log.lock() {
                    let _ = writeln!(file, "{line}");
                }
            }
        });
        Ok(Self { child, rx })
    }

    pub fn drain(&self) -> Vec<AuthEvent> {
        self.rx.try_iter().collect()
    }
}

impl Drop for AuthClient {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return;
        }
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGTERM);
        }
        #[cfg(not(unix))]
        let _ = self.child.kill();
        let _ = self.child.wait();
        thread::sleep(Duration::from_millis(5));
    }
}
