use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use crate::data::Msg;

pub struct Watcher {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Watcher {
    pub fn start(root: PathBuf, tx: Sender<Msg>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();

        let handle = thread::spawn(move || {
            let mut last_modified = scan_max_mtime(&root);
            let mut last_trigger = Instant::now();

            while !flag.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(100));

                if flag.load(Ordering::Relaxed) {
                    break;
                }

                let current_max = scan_max_mtime(&root);
                if current_max > last_modified {
                    last_modified = current_max;
                    if last_trigger.elapsed() >= Duration::from_millis(100) {
                        last_trigger = Instant::now();
                        let _ = tx.send(Msg::WatchReload);
                    }
                }
            }
        });

        Self {
            stop,
            handle: Some(handle),
        }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop();
    }
}

fn scan_max_mtime(dir: &Path) -> Option<SystemTime> {
    let mut max_mtime = None;
    scan_dir(dir, &mut max_mtime);
    max_mtime
}

fn scan_dir(dir: &Path, max: &mut Option<SystemTime>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') {
                    continue;
                }
            }
            scan_dir(&path, max);
        } else if path.extension().map_or(false, |ext| ext == "dart") {
            if let Ok(meta) = entry.metadata() {
                if let Ok(mtime) = meta.modified() {
                    *max = match *max {
                        Some(prev) if mtime > prev => Some(mtime),
                        None => Some(mtime),
                        other => other,
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_dir_finds_dart_files() {
        let temp = std::env::temp_dir().join(format!("frun_watch_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp);
        let file = temp.join("main.dart");
        fs::write(&file, "void main() {}").unwrap();

        let mtime = scan_max_mtime(&temp);
        assert!(mtime.is_some());

        let _ = fs::remove_dir_all(&temp);
    }
}
