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
    let s = s.trim_start_matches("[X]").trim_start_matches("[ ]").trim();
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
            let size_str = line.split_whitespace().nth(1).unwrap_or("0B");
            let size = parse_size(size_str).unwrap_or(0);
            let color = color_for_size(size);
            ListItem::new(line.clone()).style(Style::default().fg(color))
        })
        .collect();

    let title = match app.lang {
        Language::English => format!(
            "{}  |  Items: {}/{}  |  Total: {}{}{}",
            app.current_path.display(),
            app.nodes.len(),
            app.raw_entries.len(),
            App::format_size(app.total_size),
            if app.show_hidden { " [Hidden: ON]" } else { " [Hidden: OFF]" },
            if !app.filter_query.is_empty() { format!("  |  Filter: '{}'", app.filter_query) } else { "".to_string() }
        ),
        Language::Russian => format!(
            "{}  |  Элементов: {}/{}  |  Всего: {}{}{}",
            app.current_path.display(),
            app.nodes.len(),
            app.raw_entries.len(),
            App::format_size(app.total_size),
            if app.show_hidden { " [Скрытые: ВКЛ]" } else { " [Скрытые: ВЫКЛ]" },
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

    let bottom_text = if let Some(msg) = &app.notification_msg {
        msg.clone()
    } else {
        match app.mode {
            AppMode::Browse => match app.lang {
                Language::English => "Help (? or Shift + /) | Space: Select | p: Chart | e: Export | h: Hidden".to_string(),
                Language::Russian => "Справка (? или Shift + /) | Пробел: Выбрать | p: График | e: Экспорт | h: Скрытые".to_string(),
            },
            AppMode::ConfirmDelete => match app.lang {
                Language::English => "Delete selected/marked items? (y - yes, n - no)".to_string(),
                Language::Russian => "Удалить выбранные элементы? (y - да, n - нет)".to_string(),
            },
            AppMode::InputPath => {
                let prompt = match app.lang { Language::English => "Enter path", Language::Russian => "Введите путь" };
                format!("{}: {}", prompt, app.input_buffer)
            }
            AppMode::Filter => {
                let prompt = match app.lang { Language::English => "Filter query", Language::Russian => "Фильтр" };
                format!("{}: {}", prompt, app.filter_query)
            }
            AppMode::Help | AppMode::Plot => match app.lang {
                Language::English => "Press any key or '?' / 'p' to close".to_string(),
                Language::Russian => "Нажмите любую клавишу для закрытия".to_string(),
            },
        }
    };

    // ---- Попап сканирования ----
    if app.loading {
        let loading_title = match app.lang { Language::English => " Scanning Directory... ", Language::Russian => " Сканирование директории... " };
        let info_text = match app.lang {
            Language::English => format!("Scanned files: {}\nPath: {}\n\nPlease wait...", app.scanned_files_count, app.scanning_path),
            Language::Russian => format!("Обработано файлов: {}\nПуть: {}\n\nПожалуйста, подождите...", app.scanned_files_count, app.scanning_path),
        };
        let popup_block = Block::default().borders(Borders::ALL).title(loading_title).style(Style::default().bg(Color::Black).fg(Color::Yellow));
        let popup_paragraph = Paragraph::new(info_text).block(popup_block);
        let popup_width = 70;
        let popup_height = 8;
        let popup_area = Rect::new(area.width.saturating_sub(popup_width) / 2, area.height.saturating_sub(popup_height) / 2, popup_width.min(area.width), popup_height.min(area.height));
        frame.render_widget(Clear, popup_area);
        frame.render_widget(popup_paragraph, popup_area);
    }

    let bottom_paragraph = Paragraph::new(bottom_text)
        .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(Color::Gray)))
        .style(Style::default().fg(Color::White));
    frame.render_widget(bottom_paragraph, bottom_area);

    // ---- Попап Графика (Plot / Chart) ----
    if app.mode == AppMode::Plot {
        let mut chart_lines = vec![
            Line::from(Span::styled("Top Space Usage Chart", Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow))),
            Line::from(""),
        ];
        
        for entry in app.raw_entries.iter().take(8) {
            let pct = if app.total_size > 0 { (entry.size as f64 / app.total_size as f64) * 100.0 } else { 0.0 };
            let bars = "█".repeat((pct / 2.5) as usize);
            chart_lines.push(Line::from(format!("{:>12} | {:5.1}% | {}", App::format_size(entry.size), pct, bars)));
        }

        let popup_block = Block::default().borders(Borders::ALL).title(" Disk Usage Chart ").style(Style::default().bg(Color::Black));
        let popup_paragraph = Paragraph::new(chart_lines).block(popup_block);
        let popup_area = Rect::new(area.width.saturating_sub(74) / 2, area.height.saturating_sub(14) / 2, 74.min(area.width), 14.min(area.height));
        frame.render_widget(Clear, popup_area);
        frame.render_widget(popup_paragraph, popup_area);
    }

    // ---- Попап Справки (Help) ----
    if app.mode == AppMode::Help {
        let help_text = match app.lang {
            Language::English => vec![
                Line::from(Span::styled("RustDU - Advanced Help", Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow))),
                Line::from(""),
                Line::from("Navigation & Selection:"),
                Line::from("  • ↑ / ↓        : Navigate files/folders"),
                Line::from("  • Enter        : Open directory / Backspace: Parent"),
                Line::from("  • Space        : Mark/unmark item for batch action"),
                Line::from("  • g / /        : Custom path input / Filter by query"),
                Line::from(""),
                Line::from("Actions & Tools:"),
                Line::from("  • d            : Delete single or batch-marked items"),
                Line::from("  • h            : Toggle hidden files (starting with '.')"),
                Line::from("  • e            : Export report to rustdu_report.json"),
                Line::from("  • p            : Open top-items ASCII chart diagram"),
                Line::from("  • s / n / r    : Sort by size / name / Refresh scan"),
                Line::from(""),
                Line::from("System:"),
                Line::from("  • l            : Switch language (EN / RU)"),
                Line::from("  • ?            : Toggle help  |  q / Esc : Quit"),
            ],
            Language::Russian => vec![
                Line::from(Span::styled("RustDU - Расширенная справка", Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow))),
                Line::from(""),
                Line::from("Навигация и выбор:"),
                Line::from("  • ↑ / ↓        : Перемещение по списку"),
                Line::from("  • Enter        : Войти в папку / Backspace: Назад"),
                Line::from("  • Пробел       : Выбрать элемент для пакетного удаления"),
                Line::from("  • g / /        : Ввести путь / Фильтр по названию"),
                Line::from(""),
                Line::from("Действия и инструменты:"),
                Line::from("  • d            : Удалить элемент(ы)"),
                Line::from("  • h            : Показать/скрыть скрытые файлы (начинающиеся с '.')"),
                Line::from("  • e            : Экспортировать отчет в JSON"),
                Line::from("  • p            : Открыть график распределения места"),
                Line::from("  • s / n / r    : Сортировка по размеру/имени / Обновить"),
                Line::from(""),
                Line::from("Система:"),
                Line::from("  • l            : Сменить язык (EN / RU)"),
                Line::from("  • ?            : Справка  |  q / Esc : Выход"),
            ],
        };

        let popup_block = Block::default().borders(Borders::ALL).title(" Help ").style(Style::default().bg(Color::Black));
        let popup_paragraph = Paragraph::new(help_text).block(popup_block);
        let popup_area = Rect::new(area.width.saturating_sub(76) / 2, area.height.saturating_sub(21) / 2, 76.min(area.width), 21.min(area.height));
        frame.render_widget(Clear, popup_area);
        frame.render_widget(popup_paragraph, popup_area);
    }
}