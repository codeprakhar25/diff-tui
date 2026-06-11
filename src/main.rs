//! git-ui: VS Code-style split diff viewer in the terminal.

mod app;
mod diff;
mod git;
mod ui;

use anyhow::{bail, Result};
use app::App;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;

fn main() -> Result<()> {
    let arg = match std::env::args().nth(1) {
        Some(a) => a,
        None => bail!("usage: git-ui <file|all|commit>"),
    };

    let mode = git::resolve_mode(&arg)?;
    let files = git::collect(&mode)?;
    if files.is_empty() {
        println!("no traced changes");
        return Ok(());
    }

    let mut app = App::new(files);

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
        KeyCode::Char('u') => app.toggle_view(),
        _ => {}
    }
}
