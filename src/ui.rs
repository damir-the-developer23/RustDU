use ratatui::{prelude::*, widgets::*};

use crate::app::{App, AppMode, Language};
use crate::scanner::FileEntry;

// ── Colors ─────────────────────────────────────────────────────────

fn color_for_size(size: u64) -> Color {
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * MB;
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

// ── Row formatting ─────────────────────────────────────────────────

fn format_entry_line(app: &App, entry: &FileEntry) -> String {
    let mark = if app.marked_items.contains(&entry.name) {
        "[X]"
    } else {
        "[ ]"
    };
    let size_str = App::format_size(entry.size);
    let percent = if app.total_size > 0 {
        (entry.size as f64 / app.total_size as f64) * 100.0
    } else {
        0.0
    };
    let bar_width = 16usize;
    let filled = (((percent / 100.0) * bar_width as f64).round() as usize).min(bar_width);
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(bar_width - filled));
    let icon = if entry.is_dir { "📁" } else { "💾" };
    format!(
        "{} {:>8}  {}  {:>5.1}%  {}  {}",
        mark, size_str, bar, percent, icon, entry.name
    )
}

// ── Render ─────────────────────────────────────────────────────────

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // ── Main list ──────────────────────────────────────────────────
    let items: Vec<ListItem> = app
        .visible
        .iter()
        .map(|&i| {
            let entry = &app.raw_entries[i];
            let line = format_entry_line(app, entry);
            let color = color_for_size(entry.size);
            ListItem::new(line).style(Style::default().fg(color))
        })
        .collect();

    let title = match app.lang {
        Language::English => format!(
            "{}  |  Items: {}/{}  |  Total: {}{}{}",
            app.current_path.display(),
            app.visible.len(),
            app.raw_entries.len(),
            App::format_size(app.total_size),
            if app.show_hidden {
                " [Hidden: ON]"
            } else {
                " [Hidden: OFF]"
            },
            if !app.filter_query.is_empty() {
                format!("  |  Filter: '{}'", app.filter_query)
            } else {
                String::new()
            }
        ),
        Language::Russian => format!(
            "{}  |  Элементов: {}/{}  |  Всего: {}{}{}",
            app.current_path.display(),
            app.visible.len(),
            app.raw_entries.len(),
            App::format_size(app.total_size),
            if app.show_hidden {
                " [Скрытые: ВКЛ]"
            } else {
                " [Скрытые: ВЫКЛ]"
            },
            if !app.filter_query.is_empty() {
                format!("  |  Фильтр: '{}'", app.filter_query)
            } else {
                String::new()
            }
        ),
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().bg(Color::Blue))
        .highlight_symbol("> ");

    let list_area = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(2));
    frame.render_stateful_widget(list, list_area, &mut app.list_state);

    // ── Bottom bar ─────────────────────────────────────────────────
    let bottom_area = Rect::new(area.x, area.height.saturating_sub(2), area.width, 2);

    let bottom_text = if let Some(msg) = &app.notification_msg {
        msg.clone()
    } else {
        match app.mode {
            AppMode::Browse => match app.lang {
                Language::English => "?  — all keybindings and legend   |   q  — quit".to_string(),
                Language::Russian => "?  — все клавиши и легенда   |   q  — выход".to_string(),
            },
            AppMode::ConfirmDelete => match app.lang {
                Language::English => "Delete selected/marked items? (y - yes, n - no)".to_string(),
                Language::Russian => "Удалить выбранные элементы? (y - да, n - нет)".to_string(),
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
                Language::English => "↑/↓ scroll · Esc / q / ? close".to_string(),
                Language::Russian => "↑/↓ прокрутка · Esc / q / ? закрыть".to_string(),
            },
            AppMode::Plot => match app.lang {
                Language::English => "Press any key to close".to_string(),
                Language::Russian => "Нажмите любую клавишу для закрытия".to_string(),
            },
        }
    };

    let bottom_paragraph = Paragraph::new(bottom_text)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::Gray)),
        )
        .style(Style::default().fg(Color::White));
    frame.render_widget(bottom_paragraph, bottom_area);

    // ── Loading popup ──────────────────────────────────────────────
    if app.loading {
        let loading_title = match app.lang {
            Language::English => " Scanning Directory... ",
            Language::Russian => " Сканирование директории... ",
        };
        let info_text = match app.lang {
            Language::English => format!(
                "Scanned files: {}\nPath: {}\n\nPlease wait...",
                app.scanned_files_count, app.scanning_path
            ),
            Language::Russian => format!(
                "Обработано файлов: {}\nПуть: {}\n\nПожалуйста, подождите...",
                app.scanned_files_count, app.scanning_path
            ),
        };
        let popup_block = Block::default()
            .borders(Borders::ALL)
            .title(loading_title)
            .style(Style::default().bg(Color::Black).fg(Color::Yellow));
        let popup_paragraph = Paragraph::new(info_text).block(popup_block);
        let popup_width = 70u16;
        let popup_height = 8u16;
        let popup_area = Rect::new(
            area.width.saturating_sub(popup_width) / 2,
            area.height.saturating_sub(popup_height) / 2,
            popup_width.min(area.width),
            popup_height.min(area.height),
        );
        frame.render_widget(Clear, popup_area);
        frame.render_widget(popup_paragraph, popup_area);
    }

    // ── Plot popup ─────────────────────────────────────────────────
    if app.mode == AppMode::Plot {
        let mut chart_lines = vec![
            Line::from(Span::styled(
                "Top Space Usage Chart",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Yellow),
            )),
            Line::from(""),
        ];

        for entry in app.raw_entries.iter().take(8) {
            let pct = if app.total_size > 0 {
                (entry.size as f64 / app.total_size as f64) * 100.0
            } else {
                0.0
            };
            let bars = "█".repeat((pct / 2.5) as usize);
            chart_lines.push(Line::from(format!(
                "{:>12} | {:5.1}% | {}",
                App::format_size(entry.size),
                pct,
                bars
            )));
        }

        let popup_block = Block::default()
            .borders(Borders::ALL)
            .title(" Disk Usage Chart ")
            .style(Style::default().bg(Color::Black));
        let popup_paragraph = Paragraph::new(chart_lines).block(popup_block);
        let popup_area = Rect::new(
            area.width.saturating_sub(74) / 2,
            area.height.saturating_sub(14) / 2,
            74.min(area.width),
            14.min(area.height),
        );
        frame.render_widget(Clear, popup_area);
        frame.render_widget(popup_paragraph, popup_area);
    }

    // ── Help popup (scrollable) ────────────────────────────────────
    if app.mode == AppMode::Help {
        let help_lines = build_help_lines(app.lang);
        let total = help_lines.len() as u16;

        let popup_w = 78u16.min(area.width.saturating_sub(2));
        let popup_h = 28u16.min(area.height.saturating_sub(2));
        let inner_h = popup_h.saturating_sub(2);
        let max_scroll = total.saturating_sub(inner_h);
        let scroll = app.help_scroll.min(max_scroll);
        app.help_scroll = scroll;

        let popup_area = Rect::new(
            area.width.saturating_sub(popup_w) / 2,
            area.height.saturating_sub(popup_h) / 2,
            popup_w,
            popup_h,
        );

        let title = match app.lang {
            Language::English => {
                if max_scroll == 0 {
                    " Help ".to_string()
                } else {
                    format!(" Help · ↑/↓ [{}/{}] ", scroll + 1, max_scroll + 1)
                }
            }
            Language::Russian => {
                if max_scroll == 0 {
                    " Справка ".to_string()
                } else {
                    format!(" Справка · ↑/↓ [{}/{}] ", scroll + 1, max_scroll + 1)
                }
            }
        };

        let popup = Paragraph::new(help_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .style(Style::default().bg(Color::Black)),
            )
            .scroll((scroll, 0));

        frame.render_widget(Clear, popup_area);
        frame.render_widget(popup, popup_area);
    }
}

// ── Help content ───────────────────────────────────────────────────

fn build_help_lines(lang: Language) -> Vec<Line<'static>> {
    macro_rules! h {
        ($t:expr) => {
            Line::from(Span::styled(
                $t,
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Yellow),
            ))
        };
    }
    macro_rules! k {
        ($k:expr, $d:expr) => {
            Line::from(vec![
                Span::styled(format!("  {:16}", $k), Style::default().fg(Color::Cyan)),
                Span::raw($d),
            ])
        };
    }
    macro_rules! blank {
        () => {
            Line::from("")
        };
    }

    match lang {
        Language::English => vec![
            h!("RustDU — Keybindings & Legend"),
            blank!(),
            h!("Navigation"),
            k!("↑ / ↓", "Move cursor up / down"),
            k!("Home / End", "Jump to first / last entry"),
            k!("Enter", "Enter the selected directory"),
            k!("Backspace", "Go to the parent directory"),
            k!("g", "Jump to path (type + Enter, Esc cancels)"),
            k!("/", "Filter entries by name (Esc clears)"),
            blank!(),
            h!("Selection & Actions"),
            k!("Space", "Mark / unmark item for batch action"),
            k!("d", "Delete current item, or all marked items"),
            k!("y / n", "Confirm / cancel deletion prompt"),
            blank!(),
            h!("Sorting"),
            k!("s", "Sort by size (largest first, default)"),
            k!("n", "Sort by name (A → Z)"),
            k!("t", "Sort by modified time (newest first)"),
            blank!(),
            h!("View & Tools"),
            k!("h", "Toggle hidden files (starting with '.')"),
            k!("r", "Refresh directory and clear size cache"),
            k!("p", "Chart of the top-8 largest items"),
            k!("e", "Export report to rustdu_report.json"),
            blank!(),
            h!("System"),
            k!("l", "Switch language (EN / RU)"),
            k!("?  or  Shift+/", "Toggle this help screen"),
            k!("q / Esc", "Quit RustDU"),
            blank!(),
            h!("Colors"),
            Line::from(vec![
                Span::styled("  ≥ 1 GB     ", Style::default().fg(Color::Red)),
                Span::styled("≥ 100 MB     ", Style::default().fg(Color::Yellow)),
                Span::styled("≥ 10 MB     ", Style::default().fg(Color::LightBlue)),
                Span::styled("≥ 1 MB     ", Style::default().fg(Color::Green)),
                Span::styled("< 1 MB", Style::default().fg(Color::Gray)),
            ]),
            blank!(),
            h!("Icons"),
            Line::from("  📁  directory           💾  file"),
            blank!(),
            h!("Percentage bar"),
            Line::from("  Each row shows its share of the total as a 16-char bar:"),
            Line::from(vec![
                Span::styled("  ████████████░░░░", Style::default().fg(Color::Cyan)),
                Span::raw("  ≈ 75 % of the total size"),
            ]),
            blank!(),
            h!("Command-line flags"),
            k!("--json", "Dump JSON report to stdout and exit (no TUI)"),
            k!("--top N", "Show only the top N entries"),
            k!("--depth N", "Limit directory recursion depth"),
            k!("--version", "Print version and exit"),
            k!("--help", "Print CLI help and exit"),
            blank!(),
            Line::from(Span::styled(
                "  ↑/↓ or j/k scroll · PgUp/PgDn · Home/End · Esc / q / ? to close",
                Style::default().fg(Color::DarkGray),
            )),
        ],

        Language::Russian => vec![
            h!("RustDU — Горячие клавиши и легенда"),
            blank!(),
            h!("Навигация"),
            k!("↑ / ↓", "Перемещение курсора"),
            k!("Home / End", "К первому / последнему элементу"),
            k!("Enter", "Войти в выбранную папку"),
            k!("Backspace", "Перейти в родительскую папку"),
            k!("g", "Перейти к пути (ввод + Enter, Esc отмена)"),
            k!("/", "Фильтр по имени (Esc сбрасывает)"),
            blank!(),
            h!("Выбор и действия"),
            k!("Пробел", "Отметить / снять отметку для пакетной операции"),
            k!("d", "Удалить текущий или все отмеченные"),
            k!("y / n", "Подтвердить / отменить удаление"),
            blank!(),
            h!("Сортировка"),
            k!("s", "По размеру (от большего, по умолчанию)"),
            k!("n", "По имени (А → Я)"),
            k!("t", "По времени изменения (сначала новые)"),
            blank!(),
            h!("Вид и инструменты"),
            k!("h", "Показать / скрыть скрытые файлы (с '.')"),
            k!("r", "Обновить папку и сбросить кэш размеров"),
            k!("p", "График топ-8 самых больших элементов"),
            k!("e", "Экспорт отчёта в rustdu_report.json"),
            blank!(),
            h!("Система"),
            k!("l", "Сменить язык (EN / RU)"),
            k!("?  или  Shift+/", "Открыть / закрыть эту справку"),
            k!("q / Esc", "Выйти из RustDU"),
            blank!(),
            h!("Цвета"),
            Line::from(vec![
                Span::styled("  ≥ 1 ГБ     ", Style::default().fg(Color::Red)),
                Span::styled("≥ 100 МБ     ", Style::default().fg(Color::Yellow)),
                Span::styled("≥ 10 МБ     ", Style::default().fg(Color::LightBlue)),
                Span::styled("≥ 1 МБ     ", Style::default().fg(Color::Green)),
                Span::styled("< 1 МБ", Style::default().fg(Color::Gray)),
            ]),
            blank!(),
            h!("Иконки"),
            Line::from("  📁  папка               💾  файл"),
            blank!(),
            h!("Полоса процентов"),
            Line::from("  Каждая строка показывает долю от общего размера (16 симв.):"),
            Line::from(vec![
                Span::styled("  ████████████░░░░", Style::default().fg(Color::Cyan)),
                Span::raw("  ≈ 75 % от общего размера"),
            ]),
            blank!(),
            h!("Флаги командной строки"),
            k!("--json", "Вывести JSON-отчёт в stdout и выйти (без TUI)"),
            k!("--top N", "Показать только топ N элементов"),
            k!("--depth N", "Ограничить глубину рекурсии"),
            k!("--version", "Показать версию и выйти"),
            k!("--help", "Показать справку CLI и выйти"),
            blank!(),
            Line::from(Span::styled(
                "  ↑/↓ или j/k — прокрутка · PgUp/PgDn · Home/End · Esc / q / ? — закрыть",
                Style::default().fg(Color::DarkGray),
            )),
        ],
    }
}
