use ratatui::{prelude::*, widgets::*};
use crate::app::{App, AppMode, Language};

fn color_for_size(size: u64) -> Color {
    const GB: u64 = 1024 * 1024 * 1024;
    const MB: u64 = 1024 * 1024;
    const TEN_MB: u64 = 10 * MB;
    const HUNDRED_MB: u64 = 100 * MB;
    if size >= GB {
        Color::Red
    } else if size >= HUNDRED_MB {
        Color::Yellow
    } else if size >= TEN_MB {
        Color::LightBlue
    } else if size >= MB {
        Color::Green
    } else {
        Color::Gray
    }
}

fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() { return None; }
    let (num_part, suffix) = s.split_at(s.len() - 1);
    let num: f64 = num_part.parse().ok()?;
    let multiplier = match suffix {
        "B" => 1,
        "K" => 1024,
        "M" => 1024 * 1024,
        "G" => 1024 * 1024 * 1024,
        _ => return None,
    };
    Some((num * multiplier as f64) as u64)
}

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // ---- Основной список ----
    let items: Vec<ListItem> = app
        .nodes
        .iter()
        .map(|line| {
            let size_str = line.split_whitespace().next().unwrap_or("0B");
            let size = parse_size(size_str).unwrap_or(0);
            let color = color_for_size(size);
            ListItem::new(line.clone()).style(Style::default().fg(color))
        })
        .collect();

    let title = match app.lang {
        Language::English => format!(
            "{}  |  Items: {}/{}  |  Total Size: {}{}",
            app.current_path.display(),
            app.nodes.len(),
            app.raw_entries.len(),
            App::format_size(app.total_size),
            if !app.filter_query.is_empty() { format!("  |  Filter: '{}'", app.filter_query) } else { "".to_string() }
        ),
        Language::Russian => format!(
            "{}  |  Элементов: {}/{}  |  Общий размер: {}{}",
            app.current_path.display(),
            app.nodes.len(),
            app.raw_entries.len(),
            App::format_size(app.total_size),
            if !app.filter_query.is_empty() { format!("  |  Фильтр: '{}'", app.filter_query) } else { "".to_string() }
        ),
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().bg(Color::Blue))
        .highlight_symbol("> ");

    let list_area = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(2));
    frame.render_stateful_widget(list, list_area, &mut app.list_state);

    // ---- Компактная нижняя строка ----
    let bottom_area = Rect::new(area.x, area.height - 2, area.width, 2);

    let bottom_text = match app.mode {
        AppMode::Browse => match app.lang {
            Language::English => "Help (? or Shift + /)".to_string(),
            Language::Russian => "Справка (? или Shift + /)".to_string(),
        },
        AppMode::ConfirmDelete => match app.lang {
            Language::English => "Delete selected item? (y - yes, n - no)".to_string(),
            Language::Russian => "Удалить выбранный элемент? (y - да, n - нет)".to_string(),
        },
        AppMode::InputPath => {
            let prompt = match app.lang {
                Language::English => "Enter path",
                Language::Russian => "Введите путь",
            };
            format!("{}: {}", prompt, app.input_buffer)
        }
        AppMode::Filter => {
            let prompt = match app.lang {
                Language::English => "Filter query",
                Language::Russian => "Фильтр",
            };
            format!("{}: {}", prompt, app.filter_query)
        }
        AppMode::Help => match app.lang {
            Language::English => "Press any key or '?' to close help".to_string(),
            Language::Russian => "Нажмите любую клавишу или '?' для закрытия".to_string(),
        },
    };

    // ---- Всплывающее окно прогресса сканирования ----
    if app.loading {
        let loading_title = match app.lang {
            Language::English => " Scanning Directory... ",
            Language::Russian => " Сканирование директории... ",
        };

        let info_text = match app.lang {
            Language::English => format!(
                "Scanned files/items: {}\nPath: {}\n\nPlease wait, analyzing disk structure...",
                app.scanned_files_count, app.scanning_path
            ),
            Language::Russian => format!(
                "Обработано файлов/элементов: {}\nПуть: {}\n\nПожалуйста, подождите, идет анализ...",
                app.scanned_files_count, app.scanning_path
            ),
        };

        let popup_block = Block::default()
            .borders(Borders::ALL)
            .title(loading_title)
            .style(Style::default().bg(Color::Black).fg(Color::Yellow));

        let popup_paragraph = Paragraph::new(info_text)
            .block(popup_block)
            .alignment(Alignment::Left);

        let popup_width = 70;
        let popup_height = 8;
        let popup_x = area.width.saturating_sub(popup_width) / 2;
        let popup_y = area.height.saturating_sub(popup_height) / 2;
        let popup_area = Rect::new(popup_x, popup_y, popup_width.min(area.width), popup_height.min(area.height));

        frame.render_widget(Clear, popup_area);
        frame.render_widget(popup_paragraph, popup_area);
    }

    let bottom_paragraph = Paragraph::new(bottom_text)
        .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(Color::Gray)))
        .style(Style::default().fg(Color::White));
    frame.render_widget(bottom_paragraph, bottom_area);

    // ---- Подробное всплывающее окно справки (Help Popup) ----
    if app.mode == AppMode::Help {
        let help_text = match app.lang {
            Language::English => vec![
                Line::from(Span::styled("RustDU - Help & Functions Description", Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow))),
                Line::from(""),
                Line::from("Navigation & Control:"),
                Line::from("  • ↑ / ↓        : Navigate through files and folders"),
                Line::from("  • Enter        : Open selected directory"),
                Line::from("  • Backspace    : Go back to parent directory"),
                Line::from("  • g            : Enter a custom path manually"),
                Line::from("  • /            : Filter/search items by name"),
                Line::from(""),
                Line::from("Actions & Sorting:"),
                Line::from("  • d            : Delete selected file or folder (with confirmation)"),
                Line::from("  • s            : Sort items by size"),
                Line::from("  • n            : Sort items by name"),
                Line::from("  • r            : Refresh current directory scan"),
                Line::from(""),
                Line::from("Settings & System:"),
                Line::from("  • l            : Switch language (English / Русский)"),
                Line::from("  • ? (Shift+/)  : Open or close this help window"),
                Line::from("  • q / Esc      : Quit application"),
                Line::from(""),
                Line::from(Span::styled("Press any key to close", Style::default().fg(Color::DarkGray))),
            ],
            Language::Russian => vec![
                Line::from(Span::styled("RustDU - Справка и описание функций", Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow))),
                Line::from(""),
                Line::from("Навигация и управление:"),
                Line::from("  • ↑ / ↓        : Перемещение по списку файлов и папок"),
                Line::from("  • Enter        : Войти в выбранную директорию"),
                Line::from("  • Backspace    : Вернуться в родительскую папку"),
                Line::from("  • g            : Ввести путь для перехода вручную"),
                Line::from("  • /            : Отфильтровать элементы по имени"),
                Line::from(""),
                Line::from("Действия и сортировка:"),
                Line::from("  • d            : Удалить файл/папку (с подтверждением)"),
                Line::from("  • s            : Сортировать элементы по размеру"),
                Line::from("  • n            : Сортировать элементы по имени"),
                Line::from("  • r            : Обновить сканирование папки"),
                Line::from(""),
                Line::from("Настройки и система:"),
                Line::from("  • l            : Переключить язык (English / Русский)"),
                Line::from("  • ? (Shift+/)  : Открыть или закрыть это окно справки"),
                Line::from("  • q / Esc      : Выход из программы"),
                Line::from(""),
                Line::from(Span::styled("Нажмите любую клавишу для закрытия", Style::default().fg(Color::DarkGray))),
            ],
        };

        let popup_block = Block::default()
            .borders(Borders::ALL)
            .title(match app.lang { Language::English => " Help & About ", Language::Russian => " Справка и описание " })
            .style(Style::default().bg(Color::Black));

        let popup_paragraph = Paragraph::new(help_text)
            .block(popup_block)
            .alignment(Alignment::Left);

        let popup_width = 72;
        let popup_height = 20;
        let popup_x = area.width.saturating_sub(popup_width) / 2;
        let popup_y = area.height.saturating_sub(popup_height) / 2;
        let popup_area = Rect::new(popup_x, popup_y, popup_width.min(area.width), popup_height.min(area.height));

        frame.render_widget(Clear, popup_area);
        frame.render_widget(popup_paragraph, popup_area);
    }
}