//! Key handling for `App`.
//!
//! `handle_key` is the single entry point called from `main.rs` for each
//! `Event::Key`; it dispatches to the right mode-specific handler and
//! returns `true` if the app should quit.

use crossterm::event::KeyCode;
use std::path::PathBuf;

use super::{App, AppMode, Language, SortMode};
use crate::scanner;

impl App {
    /// Returns `true` if the application should quit.
    pub fn handle_key(&mut self, key: KeyCode) -> bool {
        self.notification_msg = None;

        // Global: language toggle (`l`) unless we're typing text.
        if key == KeyCode::Char('l')
            && self.mode != AppMode::InputPath
            && self.mode != AppMode::Filter
        {
            self.lang = match self.lang {
                Language::English => Language::Russian,
                Language::Russian => Language::English,
            };
            return false;
        }

        match self.mode {
            AppMode::Browse => {
                if key == KeyCode::Esc || key == KeyCode::Char('q') {
                    return true;
                }
                self.handle_browse(key);
            }
            AppMode::ConfirmDelete => self.handle_confirm_delete(key),
            AppMode::InputPath => self.handle_input_path(key),
            AppMode::Filter => self.handle_filter(key),
            AppMode::Help => self.handle_help(key),
            AppMode::Plot => {
                // Plot still closes on any key.
                self.mode = AppMode::Browse;
            }
        }

        false
    }

    // ── Browse ─────────────────────────────────────────────────────

    fn handle_browse(&mut self, key: KeyCode) {
        match key {
            KeyCode::Down => {
                let max = self.visible.len().saturating_sub(1);
                let i = self.list_state.selected().unwrap_or(0);
                self.list_state
                    .select(Some(if i >= max { 0 } else { i + 1 }));
            }
            KeyCode::Up => {
                let max = self.visible.len().saturating_sub(1);
                let i = self.list_state.selected().unwrap_or(0);
                self.list_state
                    .select(Some(if i == 0 { max } else { i - 1 }));
            }
            KeyCode::Enter => {
                if let Some(entry) = self.current_entry()
                    && entry.is_dir
                {
                    self.current_path = entry.path.clone();
                    self.filter_query.clear();
                    self.marked_items.clear();
                    self.load_directory();
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
                if let Some(entry) = self.current_entry() {
                    let name = entry.name.clone();
                    if self.marked_items.contains(&name) {
                        self.marked_items.remove(&name);
                    } else {
                        self.marked_items.insert(name);
                    }
                }
            }
            KeyCode::Char('d') => {
                if !self.marked_items.is_empty() {
                    self.mode = AppMode::ConfirmDelete;
                } else if let Some(entry) = self.current_entry() {
                    let name = entry.name.clone();
                    self.marked_items.insert(name);
                    self.mode = AppMode::ConfirmDelete;
                }
            }
            KeyCode::Char('h') => {
                self.show_hidden = !self.show_hidden;
                self.build_visible();
            }
            KeyCode::Char('e') => self.export_report(),
            KeyCode::Char('p') => self.mode = AppMode::Plot,
            KeyCode::Char('g') => {
                self.input_buffer.clear();
                self.mode = AppMode::InputPath;
            }
            KeyCode::Char('/') => self.mode = AppMode::Filter,
            KeyCode::Char('s') => {
                self.sort_mode = SortMode::Size;
                self.apply_sort();
            }
            KeyCode::Char('n') => {
                self.sort_mode = SortMode::Name;
                self.apply_sort();
            }
            KeyCode::Char('t') => {
                self.sort_mode = SortMode::Modified;
                self.apply_sort();
            }
            KeyCode::Char('r') => {
                scanner::clear_cache();
                self.load_directory();
            }
            KeyCode::Char('?') => {
                self.mode = AppMode::Help;
                self.help_scroll = 0;
            }
            _ => {}
        }
    }

    // ── Confirm delete ─────────────────────────────────────────────

    fn handle_confirm_delete(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('y') => {
                let names: Vec<String> = self.marked_items.drain().collect();
                for name in names {
                    let full_path = self.current_path.join(&name);
                    let _ = scanner::delete_entry(&full_path);
                }
                self.load_directory();
                self.mode = AppMode::Browse;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = AppMode::Browse;
            }
            _ => {}
        }
    }

    // ── Input path ─────────────────────────────────────────────────

    fn handle_input_path(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char(c) => self.input_buffer.push(c),
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

    // ── Filter ─────────────────────────────────────────────────────

    fn handle_filter(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char(c) => {
                self.filter_query.push(c);
                self.build_visible();
            }
            KeyCode::Backspace => {
                self.filter_query.pop();
                self.build_visible();
            }
            KeyCode::Enter | KeyCode::Esc => {
                self.mode = AppMode::Browse;
            }
            _ => {}
        }
    }

    // ── Help (scrollable) ──────────────────────────────────────────

    fn handle_help(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                self.help_scroll = self.help_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.help_scroll = self.help_scroll.saturating_add(1);
            }
            KeyCode::PageUp => {
                self.help_scroll = self.help_scroll.saturating_sub(8);
            }
            KeyCode::PageDown => {
                self.help_scroll = self.help_scroll.saturating_add(8);
            }
            KeyCode::Home => {
                self.help_scroll = 0;
            }
            KeyCode::End => {
                self.help_scroll = u16::MAX; // ui.rs clamps it
            }
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Enter => {
                self.mode = AppMode::Browse;
                self.help_scroll = 0;
            }
            _ => {}
        }
    }

    // ── Helpers ────────────────────────────────────────────────────

    fn export_report(&mut self) {
        let path = self.current_path.clone();
        let entries = self.raw_entries.clone();
        let total = self.total_size;

        match scanner::export_report(&path, &entries, total) {
            Ok(_) => {
                self.notification_msg = Some(match self.lang {
                    Language::English => "Report exported to rustdu_report.json!".to_string(),
                    Language::Russian => {
                        "Отчет успешно экспортирован в rustdu_report.json!".to_string()
                    }
                });
            }
            Err(_) => {
                self.notification_msg = Some(match self.lang {
                    Language::English => "Export failed!".to_string(),
                    Language::Russian => "Ошибка экспорта!".to_string(),
                });
            }
        }
    }
}
