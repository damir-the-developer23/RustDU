use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use once_cell::sync::Lazy;

static SIZE_CACHE: Lazy<Mutex<HashMap<PathBuf, u64>>> = Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    #[allow(dead_code)]
    pub path: PathBuf,
    pub size: u64,
    pub is_dir: bool,
}

#[derive(Debug)]
pub enum ScanMessage {
    Progress { scanned_files: usize, current_path: String },
    Finished { entries: Vec<FileEntry>, total_size: u64 },
}

fn dir_size_with_progress(path: &PathBuf, tx: &std::sync::mpsc::Sender<ScanMessage>, counter: &mut usize) -> u64 {
    if let Some(cached) = SIZE_CACHE.lock().unwrap().get(path) {
        return *cached;
    }

    let mut total = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            *counter += 1;
            if *counter % 50 == 0 {
                let _ = tx.send(ScanMessage::Progress {
                    scanned_files: *counter,
                    current_path: path.to_string_lossy().to_string(),
                });
            }

            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_file() {
                total += meta.len();
            } else if meta.is_dir() {
                total += dir_size_with_progress(&entry.path(), tx, counter);
            }
        }
    }
    SIZE_CACHE.lock().unwrap().insert(path.clone(), total);
    total
}

pub fn scan_directory_with_progress(path: &PathBuf, tx: std::sync::mpsc::Sender<ScanMessage>) -> anyhow::Result<(Vec<FileEntry>, u64)> {
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(path)?;
    let mut total_size = 0;
    let mut counter = 0;

    for entry in read_dir {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();
        let is_dir = metadata.is_dir();
        
        counter += 1;
        if counter % 10 == 0 {
            let _ = tx.send(ScanMessage::Progress {
                scanned_files: counter,
                current_path: path.to_string_lossy().to_string(),
            });
        }

        let size = if is_dir {
            dir_size_with_progress(&path, &tx, &mut counter)
        } else {
            metadata.len()
        };
        total_size += size;
        entries.push(FileEntry { name, path, size, is_dir });
    }

    entries.sort_by(|a, b| b.size.cmp(&a.size));
    let _ = tx.send(ScanMessage::Finished { entries: entries.clone(), total_size });
    Ok((entries, total_size))
}

pub fn delete_entry(path: &PathBuf) -> anyhow::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    if let Some(parent) = path.parent() {
        SIZE_CACHE.lock().unwrap().remove(parent);
    }
    Ok(())
}