# Changelog

All notable changes to **RustDU** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] — 2026-09-17

### Added
- **Percentage bars** in the main list (`████████░░░░░░░░`) — each entry now shows its share of the total size as a 16-char bar
- **Sort by modified time** — press `t`
- **`--json` flag** — dump a JSON report to stdout and exit (headless mode for scripts)
- **`--top N` flag** — show only the top N entries
- **`--depth N` flag** — limit directory recursion depth
- **Scrollable help popup** — navigate with `↑/↓`, `j/k`, `PgUp/PgDn`, `Home/End`; the title shows the current position as `[N/M]`
- **Complete in-app help** — every keybinding, color legend, icon legend, bar explanation, and CLI flag listed in a single popup
- **`--version` and `--help` flags** wired through clap
- **`FileEntry::modified`** — Unix timestamp of the last modification, shown as a new sort key

### Changed
- **Split `app.rs` into two modules** — `app/mod.rs` for state and lifecycle, `app/handlers.rs` for key handling per mode
- **Rewrote `config.rs`** with `Config::path()`, `Config::load()`, and `Config::save()` — free-function legacy wrappers removed
- **Config file format is now JSON** at `~/.config/rustdu/config.json` (was TOML)
- **`export_report()` takes `&Path`** instead of `&PathBuf`
- **`tui.rs` exposes `pub type Tui`** for `Terminal<CrosstermBackend<Stdout>>`

### Fixed
- **Filenames with spaces** no longer break `Enter`, `Space`, or `d` — the app now works with `FileEntry` values instead of parsing display strings
- **`ignored_dirs` from config now actually works** — the scanner used a hardcoded list and silently ignored the config
- **Size cache is cleared on refresh** (`r`) — previously showed stale sizes after deletions or edits
- **`Esc` in the Help / Plot popup no longer quits the app** — it only closes the popup; `Esc` in Browse still quits
- **`Space`, `h`, `r` inside the Help popup are ignored** — they no longer close the popup accidentally
- **Removed the fake 5-second loading screen** at startup
- **Fixed README mojibake** — emoji were double-encoded
- **Fixed a translation artifact in LICENSE** — `пустой work` restored to `loss of data`
- **Repository URLs in README now match `Cargo.toml`**

### Removed
- **`walkdir`** dependency — declared but never used
- **`toml`** dependency — replaced by `serde_json`, which was already in the tree
- **`directories`** dependency — replaced by ~5 lines of manual XDG resolution
- **`once_cell`** dependency — replaced by `std::sync::OnceLock`
- **`clap` `derive` feature** — switched to the builder API to drop the `clap_derive` proc-macro crate
- **`delete_target` field** in `App` — dead code, never assigned
- **`Config::is_ignored()`, `get_config_path()`, `load_config()`** — legacy wrappers with no callers

### Performance
- **~12 fewer transitive crates** (79 → ~67) after dependency cleanup
- **Clean build is significantly faster** — the biggest win is dropping `clap_derive` (proc-macro) and the entire `toml` parser subtree
- **Clippy passes with `-D warnings`** — the codebase now uses modern Rust idioms: let-chains, `.is_multiple_of(n)`, `sort_by_key(|a| Reverse(a.size))`, and `.unwrap_or_default()`

[0.5.0]: https://github.com/damir-the-developer23/rustdu/releases/tag/v0.5.0