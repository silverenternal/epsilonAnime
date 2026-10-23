use crate::cover::Cover;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

enum Command {
    Load {
        id: u64,
        path: String,
        cols: usize,
        rows: usize,
        particle_path: Option<String>,
    },
    Stop,
}

pub struct LoadedCover {
    pub id: u64,
    pub result: Result<Cover, String>,
}

pub struct CoverLoader {
    tx: Option<Sender<Command>>,
    rx: Receiver<LoadedCover>,
    join: Option<JoinHandle<()>>,
}

impl CoverLoader {
    pub fn new() -> Self {
        let (tx, cmd_rx) = mpsc::channel::<Command>();
        let (result_tx, rx) = mpsc::channel::<LoadedCover>();
        let join = thread::spawn(move || {
            while let Ok(command) = cmd_rx.recv() {
                match command {
                    Command::Load {
                        id,
                        path,
                        cols,
                        rows,
                        particle_path,
                    } => {
                        let result = Cover::load(&path, cols, rows, particle_path.as_deref())
                            .map_err(|err| err.to_string());
                        if result_tx.send(LoadedCover { id, result }).is_err() {
                            break;
                        }
                    }
                    Command::Stop => break,
                }
            }
        });
        Self {
            tx: Some(tx),
            rx,
            join: Some(join),
        }
    }

    pub fn request(
        &self,
        id: u64,
        path: String,
        cols: usize,
        rows: usize,
        particle_path: Option<String>,
    ) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::Load {
                id,
                path,
                cols,
                rows,
                particle_path,
            });
        }
    }

    pub fn drain(&self) -> Vec<LoadedCover> {
        self.rx.try_iter().collect()
    }
}

impl Drop for CoverLoader {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Command::Stop);
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
