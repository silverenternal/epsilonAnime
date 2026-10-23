use crate::audio::Backend;
use crate::events::BackendEvent;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct SessionUpdate {
    pub generation: u64,
    pub event: BackendEvent,
}

enum Command {
    Load {
        generation: u64,
        url: String,
        cache_path: Option<PathBuf>,
    },
    Preload {
        url: String,
        cache_path: Option<PathBuf>,
    },
    Control {
        command: serde_json::Value,
    },
    Shutdown,
}

pub struct SessionManager {
    tx: Option<Sender<Command>>,
    rx: Receiver<SessionUpdate>,
    join: Option<JoinHandle<()>>,
}

impl SessionManager {
    pub fn new() -> Self {
        let (tx, cmd_rx) = mpsc::channel::<Command>();
        let (event_tx, rx) = mpsc::channel::<SessionUpdate>();
        let join = thread::spawn(move || {
            let mut backend: Option<(u64, Backend)> = None;
            let mut prewarm = Backend::spawn_idle().ok();
            let mut prepared: Option<(String, Backend)> = None;
            loop {
                match cmd_rx.recv_timeout(Duration::from_millis(12)) {
                    Ok(Command::Load {
                        generation,
                        url,
                        cache_path,
                    }) => {
                        let prepared_matches = prepared
                            .as_ref()
                            .is_some_and(|(prepared_url, _)| prepared_url == &url);
                        let prepared_instance = if prepared_matches {
                            prepared.take().map(|(_, instance)| instance)
                        } else {
                            prepared = None;
                            None
                        };
                        backend = None;
                        event_tx
                            .send(SessionUpdate {
                                generation,
                                event: BackendEvent::State {
                                    state: "loading".into(),
                                },
                            })
                            .ok();
                        let instance = prepared_instance
                            .or_else(|| prewarm.take())
                            .or_else(|| Backend::spawn_idle().ok());
                        match instance {
                            Some(mut instance) => {
                                let activated = if prepared_matches {
                                    instance.start_preloaded()
                                } else {
                                    instance.activate(&url, cache_path.as_deref(), false)
                                };
                                if let Err(err) = activated {
                                    event_tx
                                        .send(SessionUpdate {
                                            generation,
                                            event: BackendEvent::Error {
                                                message: format!("activate backend: {err}"),
                                                id: "activate".into(),
                                                scope: "session".into(),
                                                level: "error".into(),
                                                retryable: true,
                                            },
                                        })
                                        .ok();
                                } else {
                                    backend = Some((generation, instance));
                                }
                            }
                            None => {
                                event_tx
                                    .send(SessionUpdate {
                                        generation,
                                        event: BackendEvent::Error {
                                            message: "start prewarmed backend failed".into(),
                                            id: "prewarm".into(),
                                            scope: "session".into(),
                                            level: "error".into(),
                                            retryable: true,
                                        },
                                    })
                                    .ok();
                            }
                        }
                        prewarm = Backend::spawn_idle().ok();
                    }
                    Ok(Command::Preload { url, cache_path }) => {
                        if prepared
                            .as_ref()
                            .is_some_and(|(prepared_url, _)| prepared_url == &url)
                        {
                            continue;
                        }
                        prepared = None;
                        if let Some(mut instance) = Backend::spawn_idle().ok()
                            && instance.activate(&url, cache_path.as_deref(), true).is_ok()
                        {
                            prepared = Some((url, instance));
                        }
                    }
                    Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Ok(Command::Control { command }) => {
                        if let Some((_, instance)) = &mut backend {
                            let _ = instance.send_command(command);
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                if let Some((generation, instance)) = &backend {
                    for event in instance.drain() {
                        if event_tx
                            .send(SessionUpdate {
                                generation: *generation,
                                event,
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                }
            }
        });
        Self {
            tx: Some(tx),
            rx,
            join: Some(join),
        }
    }

    pub fn load(&self, generation: u64, url: String, cache_path: Option<PathBuf>) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::Load {
                generation,
                url,
                cache_path,
            });
        }
    }

    pub fn control_current(&self, command: serde_json::Value) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::Control { command });
        }
    }

    pub fn preload(&self, url: String, cache_path: Option<PathBuf>) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::Preload { url, cache_path });
        }
    }

    pub fn drain(&self) -> Vec<SessionUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = self.rx.try_recv() {
            updates.push(update);
        }
        updates
    }
}

impl Drop for SessionManager {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Command::Shutdown);
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
