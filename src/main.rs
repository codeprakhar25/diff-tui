//! git-ui: VS Code-style split diff viewer in the terminal.

mod app;
mod diff;
mod git;
mod highlight;
mod ui;

use anyhow::{bail, Result};
use app::App;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use std::time::Duration;

/// How often `--watch` re-polls git for changes.
const WATCH_TICK: Duration = Duration::from_millis(400);

const HELP: &str = "\
git-ui — a VS Code-style split git diff viewer for the terminal

USAGE:
    git-ui <file>        Diff a file's working-tree changes vs HEAD
    git-ui all           Diff every uncommitted change in the working tree
    git-ui <commit>      Diff everything a commit changed (vs its parent)

OPTIONS:
    -w, --watch          Live-reload as files change on disk
    -h, --help           Show this help
    -V, --version        Show version

KEYS:
    j/k ↑/↓   scroll        h/l ←/→   scroll horizontally
    n/p Tab   next/prev file g/G       top/bottom
    [ / ]     resize panes   u         split/unified
    s         syntax on/off  q/Esc     quit
";

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{HELP}");
        return Ok(());
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("git-ui {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let watch = args.iter().any(|a| a == "-w" || a == "--watch");
    let arg = match args.iter().find(|a| !a.starts_with('-')) {
        Some(a) => a.clone(),
        None => {
            print!("{HELP}");
            bail!("missing argument");
        }
    };

    let mode = git::resolve_mode(&arg)?;
    let files = git::collect(&mode)?;
    // Without --watch an empty diff is just a message; with --watch we keep the
    // TUI open so changes can appear.
    if files.is_empty() && !watch {
        println!("no traced changes");
        return Ok(());
    }

    let mut app = App::new(mode, files, watch);

    // Headless render for testing where there's no TTY: GITUI_SNAP=WxH dumps a
    // single frame to stdout and exits.
    if let Ok(spec) = std::env::var("GITUI_SNAP") {
        return snapshot(&mut app, &spec);
    }

    let mut term = ratatui::init();
    let res = run(&mut term, &mut app);
    ratatui::restore();
    res
}

fn snapshot(app: &mut App, spec: &str) -> Result<()> {
    let (w, h) = spec
        .split_once('x')
        .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))
        .unwrap_or((100u16, 24u16));
    if let Ok(n) = std::env::var("GITUI_CUR") {
        for _ in 0..n.parse().unwrap_or(0) {
            app.next_file();
        }
    }
    if std::env::var("GITUI_VIEW").as_deref() == Ok("unified") {
        app.toggle_view();
    }
    let backend = ratatui::backend::TestBackend::new(w, h);
    let mut term = ratatui::Terminal::new(backend)?;
    term.draw(|f| ui::render(f, app))?;
    let buf = term.backend().buffer();
    for y in 0..h {
        let mut line = String::new();
        for x in 0..w {
            line.push_str(buf[(x, y)].symbol());
        }
        println!("{}", line.trim_end());
    }
    Ok(())
}

fn run(term: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    while !app.quit {
        term.draw(|f| ui::render(f, app))?;
        // In watch mode, wake on the timer to re-poll git even with no input.
        if app.watch && !event::poll(WATCH_TICK)? {
            app.reload();
            continue;
        }
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            handle_key(app, key.code, key.modifiers);
        }
    }
    Ok(())
}

fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    let page = app.viewport_h.max(1);
    match code {
        KeyCode::Char('q') | KeyCode::Esc => app.quit = true,
        KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => app.quit = true,
        KeyCode::Char('j') | KeyCode::Down => app.scroll_down(1),
        KeyCode::Char('k') | KeyCode::Up => app.scroll_up(1),
        KeyCode::Char('l') | KeyCode::Right => app.scroll_right(4),
        KeyCode::Char('h') | KeyCode::Left => app.scroll_left(4),
        KeyCode::PageDown => app.scroll_down(page),
        KeyCode::PageUp => app.scroll_up(page),
        KeyCode::Char('g') | KeyCode::Home => app.top(),
        KeyCode::Char('G') | KeyCode::End => app.bottom(),
        KeyCode::Char('n') | KeyCode::Tab => app.next_file(),
        KeyCode::Char('p') | KeyCode::BackTab => app.prev_file(),
        KeyCode::Char(']') => app.widen_left(),
        KeyCode::Char('[') => app.widen_right(),
        KeyCode::Char('u') => app.toggle_view(),
        KeyCode::Char('s') => app.toggle_syntax(),
        _ => {}
    }
}
