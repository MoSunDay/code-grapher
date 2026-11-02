//! File watching with debounce; emits changed file paths on a tokio channel.

use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;

/// Spawn a watcher over `root`. Every debounced change batch is flattened into
/// individual paths sent to the returned receiver. Stops when the sender is dropped.
pub fn spawn_watcher(root: &Path, debounce: Duration) -> anyhow::Result<mpsc::UnboundedReceiver<PathBuf>> {
    let (tx, rx) = mpsc::unbounded_channel();
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        let (ev_tx, ev_rx) = std::sync::mpsc::channel::<DebounceEventResult>();
        let mut debouncer = match new_debouncer(debounce, None, ev_tx) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(error = %e, "watcher init failed");
                return;
            }
        };
        if let Err(e) = debouncer.watch(&root, RecursiveMode::Recursive) {
            tracing::error!(error = %e, root = %root.display(), "watch failed");
            return;
        }
        tracing::info!(root = %root.display(), "watching");
        for result in ev_rx {
            match result {
                Ok(events) => {
                    for ev in events {
                        for path in ev.event.paths {
                            let _ = tx.send(path);
                        }
                    }
                }
                Err(e) => tracing::warn!(error = ?e, "watch error"),
            }
        }
        // debouncer dropped here: watcher stops
    });
    Ok(rx)
}
