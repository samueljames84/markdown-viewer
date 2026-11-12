use crate::{Event, settings};
use notify::{Event as FileEvent, RecursiveMode, Watcher};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::mpsc::{self, Sender},
    time::{Duration, Instant},
};
use tao::event_loop::EventLoopProxy;

enum Message {
    Paths(Vec<PathBuf>),
    Changed(notify::Result<FileEvent>),
}

pub struct Watches {
    tx: Sender<Message>,
}

impl Watches {
    pub fn new(proxy: EventLoopProxy<Event>, log: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel();
        let events = tx.clone();
        std::thread::spawn(move || {
            let mut watcher = match notify::recommended_watcher(move |event| {
                let _ = events.send(Message::Changed(event));
            }) {
                Ok(w) => w,
                Err(e) => {
                    let _ = proxy.send_event(Event::Error(format!("Cannot watch files: {e}")));
                    return;
                }
            };
            let mut paths: Vec<PathBuf> = vec![];
            let mut watched = HashSet::new();
            let mut dirty = HashSet::new();
            let mut deadline: Option<Instant> = None;
            loop {
                let message = if let Some(time) = deadline {
                    match rx.recv_timeout(time.saturating_duration_since(Instant::now())) {
                        Ok(m) => Some(m),
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                } else {
                    match rx.recv() {
                        Ok(m) => Some(m),
                        Err(_) => break,
                    }
                };
                match message {
                    Some(Message::Paths(new)) => paths = new,
                    Some(Message::Changed(Ok(event))) => {
                        for path in &paths {
                            if event.need_rescan()
                                || event.paths.iter().any(|p| p == path || path.starts_with(p))
                            {
                                dirty.insert(path.clone());
                            }
                        }
                        if !dirty.is_empty() {
                            deadline = Some(Instant::now() + Duration::from_millis(200));
                        }
                    }
                    Some(Message::Changed(Err(e))) => {
                        settings::log(&log, &format!("Watcher error: {e}"));
                        let _ =
                            proxy.send_event(Event::Error(format!("File watching failed: {e}")));
                    }
                    None => {
                        let changed = dirty.drain().collect();
                        deadline = None;
                        let _ = proxy.send_event(Event::Changed(changed));
                    }
                }
                let desired: HashSet<_> = paths
                    .iter()
                    .filter_map(|path| {
                        let mut parent = path.parent()?;
                        while !parent.is_dir() {
                            parent = parent.parent()?;
                        }
                        Some(parent.to_path_buf())
                    })
                    .collect();
                for old in watched.difference(&desired) {
                    if let Err(e) = watcher.unwatch(old) {
                        settings::log(&log, &format!("Unwatch: {e}"));
                    }
                }
                let mut next = HashSet::new();
                for dir in desired {
                    if watched.contains(&dir) {
                        next.insert(dir);
                    } else {
                        match watcher.watch(&dir, RecursiveMode::NonRecursive) {
                            Ok(()) => {
                                next.insert(dir);
                            }
                            Err(e) => {
                                let _ = proxy.send_event(Event::Error(format!(
                                    "Cannot watch {}: {e}",
                                    dir.display()
                                )));
                            }
                        }
                    }
                }
                watched = next;
            }
        });
        Self { tx }
    }

    pub fn update(&self, paths: Vec<PathBuf>) {
        if self.tx.send(Message::Paths(paths)).is_err() {
            eprintln!("Watcher thread stopped");
        }
    }
}
