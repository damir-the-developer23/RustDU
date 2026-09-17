mod app;
mod config;
mod scanner;
mod tui;
mod ui;

use clap::{Arg, ArgAction, Command};
use crossterm::event::{self, Event};
use std::path::PathBuf;
use std::time::Duration;

use app::App;

struct Args {
    path: PathBuf,
    json: bool,
    top: Option<usize>,
    depth: Option<usize>,
}

fn parse_args() -> Args {
    let m = Command::new("rustdu")
        .version(env!("CARGO_PKG_VERSION"))
        .about("A fast, interactive terminal disk usage analyzer")
        .arg(Arg::new("path").default_value(".").help("Path to analyze"))
        .arg(
            Arg::new("json")
                .long("json")
                .action(ArgAction::SetTrue)
                .help("Output JSON report to stdout and exit (no TUI)"),
        )
        .arg(
            Arg::new("top")
                .long("top")
                .value_parser(clap::value_parser!(usize))
                .help("Show only the top N entries"),
        )
        .arg(
            Arg::new("depth")
                .long("depth")
                .value_parser(clap::value_parser!(usize))
                .help("Limit directory recursion depth (1 = only immediate entries)"),
        )
        .get_matches();

    Args {
        path: PathBuf::from(m.get_one::<String>("path").unwrap()),
        json: m.get_flag("json"),
        top: m.get_one::<usize>("top").copied(),
        depth: m.get_one::<usize>("depth").copied(),
    }
}

fn run_json(args: &Args) -> anyhow::Result<()> {
    let cfg = config::Config::load();
    let (tx, _rx) = std::sync::mpsc::channel();
    let (mut entries, total) =
        scanner::scan_directory_with_progress(&args.path, tx, cfg.ignored_dirs, args.depth)?;
    if let Some(n) = args.top {
        entries.truncate(n);
    }

    #[derive(serde::Serialize)]
    struct Out {
        path: String,
        total_size: u64,
        entries: Vec<scanner::FileEntry>,
    }

    let out = Out {
        path: args.path.to_string_lossy().to_string(),
        total_size: total,
        entries,
    };
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

fn run_tui(args: Args) -> anyhow::Result<()> {
    let mut terminal = tui::init()?;
    let mut app = App::new(args.path, args.depth, args.top);

    let result = (|| -> anyhow::Result<()> {
        loop {
            app.poll_scanner();
            terminal.draw(|frame| ui::render(frame, &mut app))?;

            if event::poll(Duration::from_millis(50))?
                && let Event::Key(key) = event::read()?
                && app.handle_key(key.code)
            {
                break;
            }
        }
        Ok(())
    })();

    tui::restore()?;
    result
}

fn main() -> anyhow::Result<()> {
    let args = parse_args();
    if args.json {
        run_json(&args)
    } else {
        run_tui(args)
    }
}
