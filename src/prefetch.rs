use std::collections::hash_map::DefaultHasher;
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct Prefetcher {
    pending: Arc<Mutex<Option<(u64, String, PathBuf)>>>,
    wake: Arc<Condvar>,
    version: Arc<AtomicU64>,
    pid: Arc<AtomicI32>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl Prefetcher {
    pub fn new() -> Self {
        let pending: Arc<Mutex<Option<(u64, String, PathBuf)>>> = Arc::new(Mutex::new(None));
        let wake = Arc::new(Condvar::new());
        let version = Arc::new(AtomicU64::new(0));
        let pid = Arc::new(AtomicI32::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_pending = Arc::clone(&pending);
        let thread_wake = Arc::clone(&wake);
        let thread_version = Arc::clone(&version);
        let thread_pid = Arc::clone(&pid);
        let thread_stop = Arc::clone(&stop);
        let join = thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                let request = {
                    let mut slot = thread_pending.lock().unwrap();
                    while slot.is_none() && !thread_stop.load(Ordering::Relaxed) {
                        slot = thread_wake.wait(slot).unwrap();
                    }
                    slot.take()
                };
                let Some((request_version, url, cache)) = request else {
                    continue;
                };
                fs::create_dir_all("target/netease-cache").ok();
                let log = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("target/prefetch.log")
                    .ok();
                let mut command = Command::new("uv");
                command
                    .args([
                        "run",
                        "python",
                        "-m",
                        "backend.prefetch",
                        &url,
                        &cache.to_string_lossy(),
                    ])
                    .current_dir("py")
                    .stdout(Stdio::null())
                    .stderr(log.map(Stdio::from).unwrap_or_else(Stdio::null));
                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;
                    command.process_group(0);
                }
                let Ok(mut child) = command.spawn() else {
                    continue;
                };
                thread_pid.store(child.id() as i32, Ordering::Relaxed);
                loop {
                    let superseded = thread_version.load(Ordering::Relaxed) != request_version;
                    if thread_stop.load(Ordering::Relaxed) || superseded {
                        kill_group(child.id() as i32);
                    }
                    if matches!(child.try_wait(), Ok(Some(_))) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(25));
                }
                thread_pid.store(0, Ordering::Relaxed);
            }
        });
        Self {
            pending,
            wake,
            version,
            pid,
            stop,
            join: Some(join),
        }
    }

    pub fn prefetch(&self, url: &str) {
        let version = self.version.fetch_add(1, Ordering::Relaxed) + 1;
        if let Ok(mut slot) = self.pending.lock() {
            *slot = Some((version, url.to_string(), cache_path(url)));
        }
        self.wake.notify_one();
    }
}

impl Drop for Prefetcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let pid = self.pid.load(Ordering::Relaxed);
        if pid > 0 {
            kill_group(pid);
        }
        if let Ok(mut slot) = self.pending.lock() {
            *slot = None;
        }
        self.wake.notify_all();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn kill_group(pid: i32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-pid, libc::SIGTERM);
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
    }
}

pub fn cache_path(url: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    root.join(format!(
        "target/netease-cache/{:016x}.json",
        hasher.finish()
    ))
}

pub fn parse_links(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    if !trimmed.is_empty() && trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return vec![trimmed.to_string()];
    }
    let mut links = Vec::new();
    for token in text.split_whitespace() {
        let token = token.trim_matches(|ch: char| {
            matches!(ch, '"' | '\'' | '<' | '>' | '(' | ')' | ',' | '，' | '。')
        });
        let lower = token.to_ascii_lowercase();
        if lower.contains("music.163.com")
            && lower.contains("id=")
            && !links.iter().any(|x| x == token)
        {
            links.push(token.to_string());
        }
    }
    links
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_multiple_links() {
        let text = "https://music.163.com/song?id=1\n分享 https://music.163.com/song?id=2&x=y";
        assert_eq!(parse_links(text).len(), 2);
    }

    #[test]
    fn ignores_non_netease() {
        assert!(parse_links("https://example.com/song?id=1").is_empty());
    }

    #[test]
    fn cache_path_is_absolute() {
        assert!(cache_path("https://music.163.com/song?id=1").is_absolute());
    }
}
