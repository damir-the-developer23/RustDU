//! Application state and lifecycle.
//!
//! Key handling lives in `handlers.rs` (child module).

use std::cmp::Reverse;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use ratatui::widgets::ListState;

use crate::config::Config;
use crate::scanner::{self, FileEntry, ScanMessage};

mod handlers;

// ── Enums ──────────────────────────────────────────────────────────

#[derive(PartialEq, Clone, Copy)]
pub enum Language {
    English,
    Russian,
}

#[derive(PartialEq)]
pub enum AppMode {
    Browse,
    ConfirmDelete,
    InputPath,
    Filter,
    Help,
    Plot,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SortMode {
    Size,
    Name,
    Modified,
}

// ── State ──────────────────────────────────────────────────────────

pub struct App {
    pub current_path: PathBuf,

    /// All entries in the current directory.
    pub raw_entries: Vec<FileEntry>,
    /// Indices into `raw_entries` that pass the current filter, in display order.
    pub visible: Vec<usize>,

    pub list_state: ListState,
    pub mode: AppMode,

    pub input_buffer: String,
    pub filter_query: String,

    pub loading: bool,
    pub scanned_files_count: usize,
    pub scanning_path: String,

    pub total_size: u64,
    pub sort_mode: SortMode,
    pub lang: Language,

    pub rx_scanner: Option<Receiver<ScanMessage>>,

    pub show_hidden: bool,
    pub marked_items: HashSet<String>,
    pub notification_msg: Option<String>,

    /// Scroll offset for the help popup (in lines).
    pub help_scroll: u16,

    pub config: Config,
    pub max_depth: Option<usize>,
    pub top_n: Option<usize>,
}

// ── Lifecycle ──────────────────────────────────────────────────────

impl App {
    pub fn new(path: PathBuf, max_depth: Option<usize>, top_n: Option<usize>) -> Self {
        let cfg = Config::load();
        let lang = if cfg.default_lang == "ru" {
            Language::Russian
        } else {
            Language::English
        };

        let mut state = ListState::default();
        state.select(Some(0));

        let mut app = App {
            current_path: path,
            raw_entries: Vec::new(),
            visible: Vec::new(),
            list_state: state,
            mode: AppMode::Browse,
            input_buffer: String::new(),
            filter_query: String::new(),
            loading: false,
            scanned_files_count: 0,
            scanning_path: String::new(),
            total_size: 0,
            sort_mode: SortMode::Size,
            lang,
            rx_scanner: None,
            show_hidden: cfg.show_hidden,
            marked_items: HashSet::new(),
            notification_msg: None,
            help_scroll: 0,
            config: cfg,
            max_depth,
            top_n,
        };
        app.load_directory();
        app
    }

    /// Drain any pending scanner messages. Must be called once per frame.
    pub fn poll_scanner(&mut self) {
        let Some(rx) = self.rx_scanner.take() else {
            return;
        };

        let mut finished = false;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                ScanMessage::Progress {
                    scanned_files,
                    current_path,
                } => {
                    self.scanned_files_count = scanned_files;
                    self.scanning_path = current_path;
                }
                ScanMessage::Finished {
                    entries,
                    total_size,
                } => {
                    self.raw_entries = entries;
                    self.total_size = total_size;
                    self.apply_sort();
                    self.list_state.select(Some(0));
                    self.loading = false;
                    finished = true;
                }
            }
        }

        if !finished {
            self.rx_scanner = Some(rx);
        }
    }

    /// Spawn a background scan of `current_path`.
    pub fn load_directory(&mut self) {
        self.loading = true;
        self.scanned_files_count = 0;
        self.scanning_path = self.current_path.to_string_lossy().to_string();

        let path = self.current_path.clone();
        let ignored = self.config.ignored_dirs.clone();
        let depth = self.max_depth;

        let (tx, rx) = mpsc::channel();
        self.rx_scanner = Some(rx);

        thread::spawn(move || {
            let _ = scanner::scan_directory_with_progress(&path, tx, ignored, depth);
        });
    }

    /// Entry currently under the cursor, if any.
    fn current_entry(&self) -> Option<&FileEntry> {
        let sel = self.list_state.selected()?;
        let idx = *self.visible.get(sel)?;
        self.raw_entries.get(idx)
    }

    // ── Sorting & filtering ────────────────────────────────────────

    fn apply_sort(&mut self) {
        match self.sort_mode {
            SortMode::Size => {
                self.raw_entries.sort_by_key(|a| Reverse(a.size));
            }
            SortMode::Name => {
                self.raw_entries.sort_by(|a, b| a.name.cmp(&b.name));
            }
            SortMode::Modified => {
                self.raw_entries
                    .sort_by_key(|a| Reverse(a.modified.unwrap_or(0)));
            }
        }
        self.build_visible();
    }

    /// Recompute `visible` from `raw_entries` according to filter / hidden / top_n.
    pub fn build_visible(&mut self) {
        self.visible.clear();
        let needle = self.filter_query.to_lowercase();
        let mut count = 0usize;

        for (i, entry) in self.raw_entries.iter().enumerate() {
            if !self.show_hidden && entry.name.starts_with('.') {
                continue;
            }
            if !needle.is_empty() && !entry.name.to_lowercase().contains(&needle) {
                continue;
            }
            if let Some(top) = self.top_n
                && count >= top
            {
                break;
            }
            self.visible.push(i);
            count += 1;
        }

        // Clamp cursor if the filtered list shrank.
        let max = self.visible.len().saturating_sub(1);
        let cur = self.list_state.selected().unwrap_or(0);
        if cur > max {
            self.list_state.select(Some(max));
        }
    }

    // ── Formatting ─────────────────────────────────────────────────

    pub fn format_size(size: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;
        if size >= GB {
            format!("{:.1}G", size as f64 / GB as f64)
        } else if size >= MB {
            format!("{:.1}M", size as f64 / MB as f64)
        } else if size >= KB {
            format!("{:.1}K", size as f64 / KB as f64)
        } else {
            format!("{}B", size)
        }
    }
}
