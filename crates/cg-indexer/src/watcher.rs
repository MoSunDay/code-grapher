//! File watching with debounce; emits changed paths and fatal errors on a tokio channel.
//! The caller receives a receiver plus a stop flag so the watcher thread can be
//! shut down quietly when it is no longer needed.

use notify::{event::ModifyKind, EventKind, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum WatchEvent {
    Changed(PathBuf),
    Error(String),
}

/// True for events implying actual data change (create/remove/rename/write);
/// false for pure reads or metadata-only updates (atime, chmod), which must
/// not trigger re-parsing.
/// Paths worth waking the indexer for: source files, plus directories or
/// vanished paths (a delete/rename may remove many files without per-file events).
pub fn is_watch_relevant(path: &Path) -> bool {
    cg_core::lang::lang_for_path(path).is_some() || path.is_dir() || !path.exists()
}

/// A directory create/delete/rename can change many files with only one event
/// (no per-file events), so the caller should run an incremental repo scan
/// instead of indexing an exact path list.
pub fn needs_repo_scan(paths: &[PathBuf]) -> bool {
    paths
        .iter()
        .any(|p| p.is_dir() || (cg_core::lang::lang_for_path(p).is_none() && !p.exists()))
}

/// True for events implying actual data change (create/remove/rename/write);
/// false for pure reads or metadata-only updates (atime, chmod), which must
/// not trigger re-parsing.
fn is_data_change(kind: &EventKind) -> bool {
    !matches!(
        kind,
        EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(_))
    )
}

fn send_error(tx: &mpsc::UnboundedSender<WatchEvent>, message: impl Into<String>) {
    let _ = tx.send(WatchEvent::Error(message.into()));
}

fn format_errors(errors: &[notify::Error]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// Spawn a watcher over `root`. Every debounced change batch is flattened into
/// individual path events; setup/stream failures are reported as errors.
///
/// Returns `(receiver, stop)`:
/// - `receiver` yields [`WatchEvent`] values. When the consumer drops it, the
///   watcher thread notices the failed send and exits.
/// - `stop` is a shared flag: once set (with any memory ordering), the watcher
///   thread exits quietly at its next poll (about every 250ms) without sending
///   a final "watcher stream closed" error.
pub fn spawn_watcher(
    root: &Path,
    debounce: Duration,
) -> anyhow::Result<(mpsc::UnboundedReceiver<WatchEvent>, Arc<AtomicBool>)> {
    let (tx, rx) = mpsc::unbounded_channel();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        let (ev_tx, ev_rx) = std::sync::mpsc::channel::<DebounceEventResult>();
        let mut debouncer = match new_debouncer(debounce, None, ev_tx) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(error = %e, "watcher init failed");
                send_error(&tx, format!("watcher init failed: {e}"));
                return;
            }
        };
        if let Err(e) = debouncer.watch(&root, RecursiveMode::Recursive) {
            tracing::error!(error = %e, root = %root.display(), "watch failed");
            send_error(&tx, format!("watch failed for {}: {e}", root.display()));
            return;
        }
        tracing::info!(root = %root.display(), "watching");
        loop {
            if stop_flag.load(Ordering::Relaxed) {
                tracing::info!(root = %root.display(), "watcher stopped by request");
                return; // intentional stop: stay silent
            }
            let result = match ev_rx.recv_timeout(Duration::from_millis(250)) {
                Ok(result) => result,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            };
            match result {
                Ok(events) => {
                    for ev in events {
                        if !is_data_change(&ev.event.kind) {
                            continue;
                        }
                        for path in ev.event.paths {
                            if tx.send(WatchEvent::Changed(path)).is_err() {
                                tracing::debug!("watch receiver dropped; stopping watcher");
                                return;
                            }
                        }
                    }
                }
                Err(errors) => {
                    let message = format_errors(&errors);
                    tracing::warn!(error = %message, "watch error");
                    if tx
                        .send(WatchEvent::Error(format!("watch stream error: {message}")))
                        .is_err()
                    {
                        tracing::debug!("watch receiver dropped; stopping watcher");
                        return;
                    }
                }
            }
        }
        // Debouncer channel disconnected; report unless stopped intentionally.
        if !stop_flag.load(Ordering::Relaxed) {
            send_error(&tx, "watcher stream closed");
        }
        // debouncer dropped here: watcher stops
    });
    Ok((rx, stop))
}

#[cfg(test)]
mod tests {
    use super::{is_watch_relevant, needs_repo_scan, spawn_watcher};
    use std::path::Path;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    #[test]
    fn directory_events_trigger_a_repo_scan() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir(&src).unwrap();
        let file = dir.path().join("lib.rs");
        std::fs::write(&file, "pub fn a() {}\n").unwrap();

        assert!(is_watch_relevant(&src));
        assert!(is_watch_relevant(&file));
        assert!(is_watch_relevant(Path::new("/definitely/missing/lib.rs")));
        let txt = dir.path().join("README.md");
        std::fs::write(&txt, "docs\n").unwrap();
        assert!(!is_watch_relevant(&txt));

        assert!(needs_repo_scan(std::slice::from_ref(&src)));
        assert!(!needs_repo_scan(&[file]));
        assert!(!needs_repo_scan(&[dir.path().join("gone.rs")]));
        assert!(needs_repo_scan(&[dir.path().join("gone-dir")]));
    }

    #[tokio::test]
    async fn stop_flag_closes_watcher_channel() {
        let dir = tempfile::tempdir().unwrap();
        let (mut rx, stop) = spawn_watcher(dir.path(), Duration::from_millis(50)).unwrap();

        // Request shutdown immediately; the thread polls the flag every 250ms.
        stop.store(true, Ordering::Relaxed);

        // Drain any in-flight events, then require the channel to close in time.
        let closed = tokio::time::timeout(Duration::from_secs(2), async {
            while rx.recv().await.is_some() {}
        })
        .await;
        assert!(
            closed.is_ok(),
            "watcher channel stayed open >2s after stop request"
        );
    }
}
