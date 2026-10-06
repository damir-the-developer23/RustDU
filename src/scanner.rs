//! Directory scanner.
//!
//! Sizes are computed recursively and cached in a global `HashMap`.
//! The top-level listing and every recursive step are parallelised with
//! `rayon`. Progress messages are funnelled through a `Mutex<Sender>`
//! so the UI thread sees live updates without blocking the workers
//! (`std::sync::mpsc::Sender` is `Send` but not `Sync`).

use std::cmp::Reverse;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::UNIX_EPOCH;

use rayon::prelude::*;
use serde::Serialize;

// ── Size cache ─────────────────────────────────────────────────────────────

static SIZE_CACHE: OnceLock<Mutex<HashMap<PathBuf, u64>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<PathBuf, u64>> {
    SIZE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn clear_cache() {
    if let Some(m) = SIZE_CACHE.get() {
        m.lock().unwrap().clear();
    }
}

// ── Data types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub is_dir: bool,
    /// Unix timestamp (seconds) of last modification, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<u64>,
}

#[derive(Debug)]
pub enum ScanMessage {
    Progress {
        scanned_files: usize,
        current_path: String,
    },
    Finished {
        entries: Vec<FileEntry>,
        total_size: u64,
    },
}

/// Shared progress sink — see the module-level comment.
type ProgressSender = Mutex<std::sync::mpsc::Sender<ScanMessage>>;

// ── Progress helper ────────────────────────────────────────────────────────

/// Bump the shared counter and, every `every`-th file, send a progress
/// message through the (locked) channel.
#[inline]
fn report_progress(tx: &ProgressSender, counter: &AtomicUsize, every: usize, path: &Path) {
    let n = counter.fetch_add(1, Ordering::Relaxed) + 1;
    if n.is_multiple_of(every) {
        if let Ok(guard) = tx.lock() {
            let _ = guard.send(ScanMessage::Progress {
                scanned_files: n,
                current_path: path.to_string_lossy().to_string(),
            });
        }
    }
}

// ── Recursive size computation (parallel) ──────────────────────────────────

fn dir_size_parallel(
    path: &Path,
    tx: &ProgressSender,
    counter: &AtomicUsize,
    ignored: &[String],
    remaining_depth: Option<usize>,
) -> u64 {
    // Only trust the cache for full-depth scans.
    let use_cache = remaining_depth.is_none();
    if use_cache {
        if let Some(cached) = cache().lock().unwrap().get(path) {
            return *cached;
        }
    }

    if remaining_depth == Some(0) {
        return 0;
    }

    let Ok(read_dir) = fs::read_dir(path) else {
        return 0;
    };

    let entries: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();

    let total: u64 = entries
        .par_iter()
        .map(|entry| {
            report_progress(tx, counter, 50, path);

            let name = entry.file_name().to_string_lossy().to_string();
            if ignored.iter().any(|i| i == &name) {
                return 0u64;
            }

            let Ok(meta) = entry.metadata() else {
                return 0u64;
            };

            if meta.is_file() {
                meta.len()
            } else if meta.is_dir() {
                let next = remaining_depth.map(|d| d.saturating_sub(1));
                dir_size_parallel(&entry.path(), tx, counter, ignored, next)
            } else {
                0u64
            }
        })
        .sum();

    if use_cache {
        cache().lock().unwrap().insert(path.to_path_buf(), total);
    }
    total
}

// ── Public API ─────────────────────────────────────────────────────────────

pub fn scan_directory_with_progress(
    path: &PathBuf,
    tx: std::sync::mpsc::Sender<ScanMessage>,
    ignored_dirs: Vec<String>,
    max_depth: Option<usize>,
) -> anyhow::Result<(Vec<FileEntry>, u64)> {
    let read_dir = fs::read_dir(path)?;
    let dir_entries: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();

    let counter = AtomicUsize::new(0);
    let tx = ProgressSender::new(tx);

    let entries: Vec<FileEntry> = dir_entries
        .par_iter()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if ignored_dirs.iter().any(|i| i == &name) {
                return None;
            }

            let metadata = entry.metadata().ok()?;
            let entry_path = entry.path();
            let is_dir = metadata.is_dir();

            report_progress(&tx, &counter, 10, &entry_path);

            let size = if is_dir {
                // `--depth` applies to the top-level listing, so the first
                // subdirectory level still gets `max_depth - 1`.
                let inner = max_depth.map(|d| d.saturating_sub(1));
                dir_size_parallel(&entry_path, &tx, &counter, &ignored_dirs, inner)
            } else {
                metadata.len()
            };

            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs());

            Some(FileEntry {
                name,
                path: entry_path,
                size,
                is_dir,
                modified,
            })
        })
        .collect();

    let mut entries = entries;
    entries.sort_by_key(|a| Reverse(a.size));

    let total_size = entries
        .iter()
        .map(|e| e.size)
        .fold(0u64, u64::saturating_add);

    if let Ok(guard) = tx.lock() {
        let _ = guard.send(ScanMessage::Finished {
            entries: entries.clone(),
            total_size,
        });
    }

    Ok((entries, total_size))
}

pub fn delete_entry(path: &PathBuf) -> anyhow::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    if let Some(parent) = path.parent() {
        cache().lock().unwrap().remove(parent);
    }
    Ok(())
}

pub fn export_report(path: &Path, entries: &[FileEntry], total_size: u64) -> anyhow::Result<()> {
    #[derive(Serialize)]
    struct Report {
        path: String,
        total_size: u64,
        entries: Vec<FileEntry>,
    }
    let report = Report {
        path: path.to_string_lossy().to_string(),
        total_size,
        entries: entries.to_vec(),
    };
    let json = serde_json::to_string_pretty(&report)?;
    fs::write("rustdu_report.json", json)?;
    Ok(())
}
