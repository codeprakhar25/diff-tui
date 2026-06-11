//! Rendering. M2: split panes, red/green line coloring, vertical + horizontal
//! scroll, header + status bar. Intra-line highlight comes in M3.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{App, View};
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

    render_tabs(f, chunks[0], app);
    app.viewport_h = chunks[1].height as usize;

    if app.current_unchanged() {
        app.content_len = 0;
        let msg = Paragraph::new("no traced changes for this file")
            .style(Style::default().fg(Color::DarkGray))
            .centered();
        f.render_widget(msg, centered_row(chunks[1]));
    } else {
        match app.view {
            View::Split => render_split(f, chunks[1], app),
            View::Unified => render_unified(f, chunks[1], app),
        }
    }

    render_status(f, chunks[2], app);
}

/// Horizontal file tab strip. Tabs are status-colored; the active one is
/// highlighted. When tabs overflow the width, the strip scrolls to keep the
/// active tab in view.
fn render_tabs(f: &mut Frame, area: Rect, app: &mut App) {
    let area_w = area.width as usize;

    // Lay out tab start columns + widths, then nudge tab_scroll so the active
    // tab is fully visible.
    let widths: Vec<usize> = (0..app.files.len())
        .map(|i| app.tab_label(i).chars().count())
        .collect();
    let starts: Vec<usize> = widths
        .iter()
        .scan(0, |acc, &w| {
            let s = *acc;
            *acc += w;
            Some(s)
        })
        .collect();
    let a_start = starts[app.cur];
    let a_end = a_start + widths[app.cur];
    if a_start < app.tab_scroll {
        app.tab_scroll = a_start;
    } else if a_end > app.tab_scroll + area_w {
        app.tab_scroll = a_end.saturating_sub(area_w);
    }

    let mut spans = Vec::with_capacity(app.files.len());
    for i in 0..app.files.len() {
        let label = app.tab_label(i);
        let style = if i == app.cur {
            Style::default()
                .fg(Color::White)
                .bg(Color::Rgb(55, 55, 70))
                .add_modifier(Modifier::BOLD)
        } else {
            status_style(app.files[i].status).bg(Color::Rgb(28, 28, 34))
        };
        spans.push(Span::styled(label, style));
    }

    f.render_widget(
        Paragraph::new(Line::from(spans))
            .scroll((0, app.tab_scroll as u16))
            .style(Style::default().bg(Color::Rgb(28, 28, 34))),
        area,
    );
}

fn render_status(f: &mut Frame, area: Rect, app: &App) {
    let pos = format!(" [{}/{}] ", app.cur + 1, app.files.len());
    let mode = match app.view {
        View::Split => " SPLIT ",
        View::Unified => " UNIFIED ",
    };
    let scroll = scroll_pct(app);
    let help = "q quit  j/k scroll  h/l ←→  n/p file  g/G top/bot  u view";
    let line = Line::from(vec![
        Span::styled(pos, Style::default().fg(Color::Black).bg(Color::Gray)),
        Span::styled(mode, Style::default().fg(Color::Black).bg(Color::Cyan)),
        Span::styled(format!(" {help}  "), Style::default().fg(Color::Gray)),
        Span::styled(scroll, Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(Color::Rgb(30, 30, 35))),
        area,
    );
}

/// "Top" / "Bot" / "NN%" describing vertical scroll position.
fn scroll_pct(app: &App) -> String {
    let max = app.content_len.saturating_sub(app.viewport_h);
    if max == 0 {
        return "All".to_string();
    }
    if app.v_offset == 0 {
        "Top".to_string()
    } else if app.v_offset >= max {
        "Bot".to_string()
    } else {
        format!("{}%", app.v_offset * 100 / max)
    }
}

fn render_split(f: &mut Frame, area: Rect, app: &mut App) {
    app.content_len = app.rows().len();
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

/// Classic single-column unified diff. A changed row becomes two lines (the
/// removed line then the added line); equal rows stay one context line.
fn render_unified(f: &mut Frame, area: Rect, app: &mut App) {
    let width = area.width as usize;
    let mut lines: Vec<Line> = Vec::new();
    for row in app.rows() {
        match row.kind {
            Kind::Equal => lines.push(cell_line(&row.left, Kind::Equal, true, app.h_offset, width)),
            Kind::Removed => {
                lines.push(cell_line(&row.left, Kind::Removed, true, app.h_offset, width))
            }
            Kind::Added => {
                lines.push(cell_line(&row.right, Kind::Added, false, app.h_offset, width))
            }
            Kind::Changed => {
                lines.push(cell_line(&row.left, Kind::Changed, true, app.h_offset, width));
                lines.push(cell_line(&row.right, Kind::Changed, false, app.h_offset, width));
            }
        }
    }
    app.content_len = lines.len();

    let h = area.height as usize;
    let visible: Vec<Line> = lines.into_iter().skip(app.v_offset).take(h).collect();
    f.render_widget(Paragraph::new(visible), area);
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

    let (fg, sign, hl_bg) = match (kind, left) {
        (Kind::Equal, _) => (Color::Gray, ' ', Color::Reset),
        (Kind::Removed, _) => (Color::Red, '-', Color::Rgb(90, 30, 30)),
        (Kind::Added, _) => (Color::Green, '+', Color::Rgb(30, 70, 30)),
        (Kind::Changed, true) => (Color::Red, '-', Color::Rgb(90, 30, 30)),
        (Kind::Changed, false) => (Color::Green, '+', Color::Rgb(30, 70, 30)),
    };

    let gutter = format!("{:>4} ", cell.num);
    let text_w = width.saturating_sub(GUTTER + SIGN);

    let mut spans = vec![
        Span::styled(gutter, Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{sign} "), Style::default().fg(fg)),
    ];
    spans.extend(text_spans(&cell.text, &cell.inline, h_off, text_w, fg, hl_bg));
    Line::from(spans)
}

/// Visible slice of a line as styled spans, with intra-line changed ranges
/// (byte offsets into `text`) drawn brighter. `h_off`/`width` are in characters.
fn text_spans(
    text: &str,
    ranges: &[(usize, usize)],
    h_off: usize,
    width: usize,
    fg: Color,
    hl_bg: Color,
) -> Vec<Span<'static>> {
    let base = Style::default().fg(fg);
    let hl = Style::default()
        .fg(Color::White)
        .bg(hl_bg)
        .add_modifier(Modifier::BOLD);
    let in_range = |b: usize| ranges.iter().any(|&(s, e)| b >= s && b < e);

    let mut out: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut buf_hot = false;
    let mut shown = 0;

    for (ci, (byte, ch)) in text.char_indices().enumerate() {
        if ci < h_off {
            continue;
        }
        if shown >= width {
            break;
        }
        let hot = in_range(byte);
        if !buf.is_empty() && hot != buf_hot {
            out.push(Span::styled(std::mem::take(&mut buf), if buf_hot { hl } else { base }));
        }
        buf.push(ch);
        buf_hot = hot;
        shown += 1;
    }
    if !buf.is_empty() {
        out.push(Span::styled(buf, if buf_hot { hl } else { base }));
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_changed_range_into_hot_span() {
        // "abXYef" with bytes 2..4 changed -> ["ab", "XY"(hot), "ef"].
        let spans = text_spans("abXYef", &[(2, 4)], 0, 20, Color::Red, Color::Blue);
        let parts: Vec<&str> = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(parts, vec!["ab", "XY", "ef"]);
        assert_eq!(spans[1].style.bg, Some(Color::Blue)); // the highlighted run
        assert_eq!(spans[0].style.bg, None);
    }

    #[test]
    fn respects_horizontal_offset_and_width() {
        // Skip 2 chars, show 3.
        let spans = text_spans("abcdefgh", &[], 2, 3, Color::Gray, Color::Reset);
        let joined: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(joined, "cde");
    }
}
