//! NFS-safe per-repository write lock: `flock(<repo_id>/.lock)`.
//!
//! Every mutating operation (draft sync, approve/merge) must hold this lock
//! for its whole critical section. Reads never take the lock; they go through
//! git refs, not the worktree.

use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::path::Path;

use crate::paths::{canonical_root, repo_dir, LOCK_FILE};

/// Exclusive lock guard over `<root>/<repo_id>/.lock`; released on drop.
pub struct RepoLock {
    _file: File,
}

/// Acquire the exclusive write lock for one repository partition.
pub fn acquire(root: &Path, repo_id: &str) -> Result<RepoLock> {
    let dir = repo_dir(&canonical_root(root)?, repo_id)?;
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(dir.join(LOCK_FILE))
        .with_context(|| format!("open {}", dir.join(LOCK_FILE).display()))?;
    loop {
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if rc == 0 {
            return Ok(RepoLock { _file: file });
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(error).context("flock feature repository");
        }
    }
}
