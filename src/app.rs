use crossterm::event::KeyCode;
use std::path::PathBuf;
use ratatui::widgets::ListState;
use std::thread;
use std::sync::mpsc::{self, Receiver};
use std::collections::HashSet;

use crate::scanner;
use crate::config;

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
}

pub struct App {
    pub current_path: PathBuf,
    pub nodes: Vec<String>,
    pub list_state: ListState,
    pub mode: AppMode,
    pub input_buffer: String,
    pub filter_query: String,
    pub loading: bool,
    pub scanned_files_count: usize,
    pub scanning_path: String,
    pub delete_target: Option<String>,
    pub total_size: u64,
    pub sort_mode: SortMode,
    pub raw_entries: Vec<scanner::FileEntry>,
    pub lang: Language,
    pub rx_scanner: Option<Receiver<scanner::ScanMessage>>,
    pub show_hidden: bool,
    pub marked_items: HashSet<String>,
    pub notification_msg: Option<String>,
}

impl App {
    pub fn new(path: PathBuf) -> Self {
        let cfg = config::load_config();
        let lang = if cfg.default_lang == "ru" { Language::Russian } else { Language::English };
        
        let mut state = ListState::default();
        state.select(Some(0));
        let mut app = App {
            current_path: path,
            nodes: Vec::new(),
            list_state: state,
            mode: AppMode::Browse,
            input_buffer: String::new(),
            filter_query: String::new(),
            loading: false,
            scanned_files_count: 0,
            scanning_path: String::new(),
            delete_target: None,
            total_size: 0,
            sort_mode: SortMode::Size,
            raw_entries: Vec::new(),
            lang,
            rx_scanner: None,
            show_hidden: cfg.show_hidden,
            marked_items: HashSet::new(),
            notification_msg: None,
        };
        app.load_directory();
        app
    }

    pub fn poll_scanner(&mut self) {
        if let Some(rx) = &self.rx_scanner {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    scanner::ScanMessage::Progress { scanned_files, current_path } => {
                        self.scanned_files_count = scanned_files;
                        self.scanning_path = current_path;
                    }
                    scanner::ScanMessage::Finished { entries, total_size } => {
                        self.raw_entries = entries;
                        self.total_size = total_size;
                        self.apply_sort();
                        self.list_state.select(Some(0));
                        self.loading = false;
                        self.rx_scanner = None;
                        break;
                    }
                }
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyCode) {
        self.notification_msg = None; // сбрасываем уведомление при нажатии
        if let KeyCode::Char('l') = key {
            if self.mode != AppMode::InputPath && self.mode != AppMode::Filter {
                self.lang = match self.lang {
                    Language::English => Language::Russian,
                    Language::Russian => Language::English,
                };
                return;
            }
        }

        match self.mode {
            AppMode::Browse => self.handle_browse(key),
            AppMode::ConfirmDelete => self.handle_confirm_delete(key),
            AppMode::InputPath => self.handle_input_path(key),
            AppMode::Filter => self.handle_filter(key),
            AppMode::Help | AppMode::Plot => {
                match key {
                    KeyCode::Char('?') | KeyCode::Esc | KeyCode::Enter | KeyCode::Char('p') => {
                        self.mode = AppMode::Browse;
                    }
                    _ => {
                        self.mode = AppMode::Browse;
                    }
                }
            }
        }
    }

    fn handle_browse(&mut self, key: KeyCode) {
        match key {
            KeyCode::Down => {
                let i = self.list_state.selected().unwrap_or(0);
                let max = self.nodes.len().saturating_sub(1);
                self.list_state.select(Some(if i == max { 0 } else { i + 1 }));
            }
            KeyCode::Up => {
                let i = self.list_state.selected().unwrap_or(0);
                let max = self.nodes.len().saturating_sub(1);
                self.list_state.select(Some(if i == 0 { max } else { i - 1 }));
            }
            KeyCode::Enter => {
                if let Some(selected) = self.list_state.selected() {
                    if let Some(entry_str) = self.nodes.get(selected) {
                        let name = entry_str.split_whitespace().last().unwrap_or(entry_str);
                        let name = name.trim_start_matches("[X] ").trim_start_matches("[ ] ");
                        let new_path = self.current_path.join(name);
                        if new_path.is_dir() {
                            self.current_path = new_path;
                            self.filter_query.clear();
                            self.marked_items.clear();
                            self.load_directory();
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(parent) = self.current_path.parent() {
                    self.current_path = parent.to_path_buf();
                    self.filter_query.clear();
                    self.marked_items.clear();
                    self.load_directory();
                }
            }
            KeyCode::Char(' ') => {
                if let Some(selected) = self.list_state.selected() {
                    if let Some(entry_str) = self.nodes.get(selected) {
                        let name = entry_str.split_whitespace().last().unwrap_or(entry_str);
                        let name = name.trim_start_matches("[X] ").trim_start_matches("[ ] ");
                        if self.marked_items.contains(name) {
                            self.marked_items.remove(name);
                        } else {
                            self.marked_items.insert(name.to_string());
                        }
                        self.build_display_strings();
                    }
                }
            }
            KeyCode::Char('d') => {
                if !self.marked_items.is_empty() {
                    self.mode = AppMode::ConfirmDelete;
                } else if let Some(selected) = self.list_state.selected() {
                    if let Some(entry_str) = self.nodes.get(selected) {
                        let name = entry_str.split_whitespace().last().unwrap_or(entry_str);
                        let name = name.trim_start_matches("[X] ").trim_start_matches("[ ] ");
                        self.marked_items.insert(name.to_string());
                        self.mode = AppMode::ConfirmDelete;
                    }
                }
            }
            KeyCode::Char('h') => {
                self.show_hidden = !self.show_hidden;
                self.build_display_strings();
            }
            KeyCode::Char('e') => {
                match scanner::export_report(&self.current_path, &self.raw_entries, self.total_size) {
                    Ok(_) => {
                        self.notification_msg = Some(match self.lang {
                            Language::English => "Report exported to rustdu_report.json!".to_string(),
                            Language::Russian => "Отчет успешно экспортирован в rustdu_report.json!".to_string(),
                        });
                    }
                    Err(_) => {
                        self.notification_msg = Some("Export failed!".to_string());
                    }
                }
            }
            KeyCode::Char('p') => {
                self.mode = AppMode::Plot;
            }
            KeyCode::Char('g') => {
                self.input_buffer.clear();
                self.mode = AppMode::InputPath;
            }
            KeyCode::Char('/') => {
                self.mode = AppMode::Filter;
            }
            KeyCode::Char('s') => {
                self.sort_mode = SortMode::Size;
                self.apply_sort();
            }
            KeyCode::Char('n') => {
                self.sort_mode = SortMode::Name;
                self.apply_sort();
            }
            KeyCode::Char('r') => {
                self.load_directory();
            }
            KeyCode::Char('?') => {
                self.mode = AppMode::Help;
            }
            _ => {}
        }
    }

    fn handle_confirm_delete(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('y') => {
                for name in self.marked_items.drain() {
                    let full_path = self.current_path.join(&name);
                    let _ = scanner::delete_entry(&full_path);
                }
                if let Some(target) = self.delete_target.take() {
                    let full_path = self.current_path.join(&target);
                    let _ = scanner::delete_entry(&full_path);
                }
                self.load_directory();
                self.mode = AppMode::Browse;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.delete_target = None;
                self.mode = AppMode::Browse;
            }
            _ => {}
        }
    }

    fn handle_input_path(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Enter => {
                let path = PathBuf::from(&self.input_buffer);
                if path.exists() && path.is_dir() {
                    self.current_path = path;
                    self.filter_query.clear();
                    self.marked_items.clear();
                    self.load_directory();
                }
                self.mode = AppMode::Browse;
                self.input_buffer.clear();
            }
            KeyCode::Esc => {
                self.mode = AppMode::Browse;
                self.input_buffer.clear();
            }
            _ => {}
        }
    }

    fn handle_filter(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char(c) => {
                self.filter_query.push(c);
                self.apply_sort();
            }
            KeyCode::Backspace => {
                self.filter_query.pop();
                self.apply_sort();
            }
            KeyCode::Enter | KeyCode::Esc => {
                self.mode = AppMode::Browse;
            }
            _ => {}
        }
    }

    fn apply_sort(&mut self) {
        match self.sort_mode {
            SortMode::Size => {
                self.raw_entries.sort_by(|a, b| b.size.cmp(&a.size));
            }
            SortMode::Name => {
                self.raw_entries.sort_by(|a, b| a.name.cmp(&b.name));
            }
        }
        self.build_display_strings();
    }

    pub fn build_display_strings(&mut self) {
        let mut display = Vec::new();
        for entry in &self.raw_entries {
            if !self.show_hidden && entry.name.starts_with('.') {
                continue;
            }
            if !self.filter_query.is_empty() && !entry.name.to_lowercase().contains(&self.filter_query.to_lowercase()) {
                continue;
            }

            let size_str = Self::format_size(entry.size);
            let icon = if entry.is_dir { "📁" } else { "💾" };
            let percent = if self.total_size > 0 {
                (entry.size as f64 / self.total_size as f64) * 100.0
            } else {
                0.0
            };
            let mark = if self.marked_items.contains(&entry.name) { "[X]" } else { "[ ]" };
            display.push(format!("{} {:>8}  {:>5.1}%  {}  {}", mark, size_str, percent, icon, entry.name));
        }
        self.nodes = display;
    }

    pub fn load_directory(&mut self) {
        self.loading = true;
        self.scanned_files_count = 0;
        self.scanning_path = self.current_path.to_string_lossy().to_string();
        let path = self.current_path.clone();
        let (tx, rx) = mpsc::channel();
        self.rx_scanner = Some(rx);

        thread::spawn(move || {
            let _ = scanner::scan_directory_with_progress(&path, tx);
        });
    }

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