//! Rendering. M2: split panes, red/green line coloring, vertical + horizontal
//! scroll, header + status bar. Intra-line highlight comes in M3.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;
use crate::diff::{Cell, Kind};

const GUTTER: usize = 5; // 4-digit line number + 1 space
const SIGN: usize = 2; // sign char + 1 space

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Min(0),    // body
        Constraint::Length(1), // status
    ])
    .split(f.area());

    render_header(f, chunks[0], app);
    app.viewport_h = chunks[1].height as usize;

    if app.current_unchanged() {
        let msg = Paragraph::new("no traced changes for this file")
            .style(Style::default().fg(Color::DarkGray))
            .centered();
        f.render_widget(msg, centered_row(chunks[1]));
    } else {
        render_split(f, chunks[1], app);
    }

    render_status(f, chunks[2], app);
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let cur = app.current();
    let pos = format!("[{}/{}]", app.cur + 1, app.files.len());
    let line = Line::from(vec![
        Span::styled(
            format!(" {} ", cur.path),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("({}) ", cur.status), status_style(cur.status)),
        Span::styled(pos, Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(Color::Rgb(30, 30, 35))),
        area,
    );
}

fn render_status(f: &mut Frame, area: Rect, _app: &App) {
    let help = " q quit   j/k scroll   h/l ←→   n/p file   g/G top/bot   u view ";
    f.render_widget(
        Paragraph::new(help).style(
            Style::default()
                .fg(Color::Gray)
                .bg(Color::Rgb(30, 30, 35)),
        ),
        area,
    );
}

fn render_split(f: &mut Frame, area: Rect, app: &App) {
    let panes = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let rows = app.rows();
    let h = area.height as usize;
    let start = app.v_offset;
    let window = rows.iter().skip(start).take(h);

    let mut left_lines = Vec::with_capacity(h);
    let mut right_lines = Vec::with_capacity(h);
    let left_w = panes[0].width.saturating_sub(1) as usize; // -1 for divider border
    let right_w = panes[1].width as usize;
    for row in window {
        left_lines.push(cell_line(&row.left, row.kind, true, app.h_offset, left_w));
        right_lines.push(cell_line(&row.right, row.kind, false, app.h_offset, right_w));
    }

    f.render_widget(
        Paragraph::new(left_lines).block(Block::new().borders(Borders::RIGHT)),
        panes[0],
    );
    f.render_widget(Paragraph::new(right_lines), panes[1]);
}

/// Build a styled line for one cell. `width` is the full pane inner width.
fn cell_line(cell: &Option<Cell>, kind: Kind, left: bool, h_off: usize, width: usize) -> Line<'static> {
    let Some(cell) = cell else {
        // Padding gap — faint tilde marker so the eye reads it as "nothing here".
        return Line::from(Span::styled(
            " ".repeat(width.min(2)) + "~",
            Style::default().fg(Color::Rgb(60, 60, 70)),
        ));
    };

    let (fg, sign) = match (kind, left) {
        (Kind::Equal, _) => (Color::Gray, ' '),
        (Kind::Removed, _) => (Color::Red, '-'),
        (Kind::Added, _) => (Color::Green, '+'),
        (Kind::Changed, true) => (Color::Red, '-'),
        (Kind::Changed, false) => (Color::Green, '+'),
    };

    let gutter = format!("{:>4} ", cell.num);
    let text_w = width.saturating_sub(GUTTER + SIGN);
    let body: String = cell.text.chars().skip(h_off).take(text_w).collect();

    Line::from(vec![
        Span::styled(gutter, Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{sign} "), Style::default().fg(fg)),
        Span::styled(body, Style::default().fg(fg)),
    ])
}

fn status_style(status: char) -> Style {
    let c = match status {
        'A' => Color::Green,
        'D' => Color::Red,
        'M' => Color::Yellow,
        _ => Color::Cyan,
    };
    Style::default().fg(c)
}

/// A 1-high rect centered vertically in `area` (for the no-changes message).
fn centered_row(area: Rect) -> Rect {
    let y = area.y + area.height / 2;
    Rect::new(area.x, y, area.width, 1)
}
