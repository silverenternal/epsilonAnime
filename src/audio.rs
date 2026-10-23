use crate::events::BackendEvent;
#[cfg(unix)]
use crate::pcm_analyzer::{PcmAnalyzer, socket_path};
use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct Backend {
    child: Child,
    rx: Receiver<BackendEvent>,
    stdin: Option<ChildStdin>,
    #[cfg(unix)]
    _analyzer: Option<PcmAnalyzer>,
}

impl Backend {
    pub fn spawn_idle() -> Result<Self> {
        #[cfg(unix)]
        static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(1);
        fs::create_dir_all("target").ok();
        let log_path = PathBuf::from("target/backend.log");
        let log = Arc::new(Mutex::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .context("open backend log")?,
        ));
        let mut command = Command::new("uv");
        command
            .args(["run", "python", "-m", "backend.main"])
            .current_dir("py")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let (tx, rx) = mpsc::channel();
        #[cfg(unix)]
        let analyzer = {
            let socket = socket_path(SOCKET_COUNTER.fetch_add(1, Ordering::Relaxed));
            let analyzer = PcmAnalyzer::bind(socket.clone(), tx.clone()).ok();
            if analyzer.is_some() {
                command.env("EPSILON_ANIME_PCM_SOCKET", &socket);
            }
            analyzer
        };
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command.spawn().context("spawn uv python backend")?;
        let stdout = child.stdout.take().context("backend stdout")?;
        let stderr = child.stderr.take().context("backend stderr")?;
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if line.trim().is_empty() {
                    continue;
                }
                let event = BackendEvent::parse(&line).unwrap_or_else(|err| BackendEvent::Error {
                    message: format!("bad backend event: {err}; line={}", truncate(&line)),
                    id: "parse".into(),
                    scope: "backend".into(),
                    level: "error".into(),
                    retryable: false,
                });
                if tx.send(event).is_err() {
                    break;
                }
            }
            let _ = tx.send(BackendEvent::State {
                state: "backend_eof".into(),
            });
        });

        let log_writer = Arc::clone(&log);
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if let Ok(mut file) = log_writer.lock() {
                    let _ = writeln!(file, "{line}");
                }
            }
        });

        let stdin = child.stdin.take().context("backend stdin")?;
        Ok(Self {
            child,
            rx,
            stdin: Some(stdin),
            #[cfg(unix)]
            _analyzer: analyzer,
        })
    }

    pub fn activate(&mut self, url: &str, cache_path: Option<&Path>, preload: bool) -> Result<()> {
        let payload = serde_json::json!({
            "url": url,
            "cache_path": cache_path.map(|path| path.to_string_lossy().to_string()),
            "preload": preload,
        });
        self.send_command(payload)
    }

    pub fn start_preloaded(&mut self) -> Result<()> {
        self.send_command(serde_json::json!({"cmd": "start"}))
    }

    pub fn send_command(&mut self, command: serde_json::Value) -> Result<()> {
        let stdin = self.stdin.as_mut().context("backend stdin unavailable")?;
        writeln!(stdin, "{command}")?;
        stdin.flush()?;
        Ok(())
    }

    pub fn drain(&self) -> Vec<BackendEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.rx.try_recv() {
            events.push(event);
        }
        events
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let group = -(self.child.id() as i32);
            unsafe {
                libc::kill(group, libc::SIGTERM);
            }
            for _ in 0..20 {
                if matches!(self.child.try_wait(), Ok(Some(_))) {
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
            unsafe {
                libc::kill(group, libc::SIGKILL);
            }
        }
        #[cfg(not(unix))]
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn truncate(line: &str) -> String {
    line.chars().take(180).collect()
}
