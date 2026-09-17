use std::cmp::Reverse;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use serde::Serialize;

static SIZE_CACHE: OnceLock<Mutex<HashMap<PathBuf, u64>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<PathBuf, u64>> {
    SIZE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn clear_cache() {
    if let Some(m) = SIZE_CACHE.get() {
        m.lock().unwrap().clear();
    }
}

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

fn dir_size_with_progress(
    path: &PathBuf,
    tx: &std::sync::mpsc::Sender<ScanMessage>,
    counter: &mut usize,
    ignored: &[String],
    remaining_depth: Option<usize>,
) -> u64 {
    // Only trust cache for full-depth scans.
    let use_cache = remaining_depth.is_none();
    if use_cache && let Some(cached) = cache().lock().unwrap().get(path) {
        return *cached;
    }

    if remaining_depth == Some(0) {
        return 0;
    }

    let mut total: u64 = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            *counter += 1;
            if (*counter).is_multiple_of(50) {
                let _ = tx.send(ScanMessage::Progress {
                    scanned_files: *counter,
                    current_path: path.to_string_lossy().to_string(),
                });
            }

            let name = entry.file_name().to_string_lossy().to_string();
            if ignored.iter().any(|i| i == &name) {
                continue;
            }

            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_file() {
                total = total.saturating_add(meta.len());
            } else if meta.is_dir() {
                let next = remaining_depth.map(|d| d.saturating_sub(1));
                total = total.saturating_add(dir_size_with_progress(
                    &entry.path(),
                    tx,
                    counter,
                    ignored,
                    next,
                ));
            }
        }
    }

    if use_cache {
        cache().lock().unwrap().insert(path.clone(), total);
    }
    total
}

pub fn scan_directory_with_progress(
    path: &PathBuf,
    tx: std::sync::mpsc::Sender<ScanMessage>,
    ignored_dirs: Vec<String>,
    max_depth: Option<usize>,
) -> anyhow::Result<(Vec<FileEntry>, u64)> {
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(path)?;
    let mut total_size: u64 = 0;
    let mut counter = 0usize;

    for entry in read_dir {
        let entry = entry?;
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();

        if ignored_dirs.iter().any(|i| i == &name) {
            continue;
        }

        let entry_path = entry.path();
        let is_dir = metadata.is_dir();

        counter += 1;
        if counter.is_multiple_of(10) {
            let _ = tx.send(ScanMessage::Progress {
                scanned_files: counter,
                current_path: entry_path.to_string_lossy().to_string(),
            });
        }

        let size = if is_dir {
            // --depth applies to the top-level directory listing, so the
            // first subdirectory level still gets `max_depth - 1`.
            let inner = max_depth.map(|d| d.saturating_sub(1));
            dir_size_with_progress(&entry_path, &tx, &mut counter, &ignored_dirs, inner)
        } else {
            metadata.len()
        };
        total_size = total_size.saturating_add(size);

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs());

        entries.push(FileEntry {
            name,
            path: entry_path,
            size,
            is_dir,
            modified,
        });
    }

    entries.sort_by_key(|a| Reverse(a.size));
    let _ = tx.send(ScanMessage::Finished {
        entries: entries.clone(),
        total_size,
    });
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
