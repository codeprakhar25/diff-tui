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
use crate::highlight::{self, LineSegs};

const GUTTER: usize = 5; // 4-digit line number + 1 space
const SIGN: usize = 2; // sign char + 1 space

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Min(0),    // body
        Constraint::Length(1), // status
    ])
    .split(f.area());

    app.viewport_h = chunks[1].height as usize;

    // Watch mode with nothing to show yet (e.g. all changes reverted).
    if app.files.is_empty() {
        let msg = Paragraph::new("watching — no changes yet")
            .style(Style::default().fg(Color::DarkGray))
            .centered();
        f.render_widget(msg, centered_row(chunks[1]));
        render_status(f, chunks[2], app);
        return;
    }

    render_tabs(f, chunks[0], app);

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
    let cur = if app.files.is_empty() { 0 } else { app.cur + 1 };
    let pos = format!(" [{}/{}] ", cur, app.files.len());
    let mode = match app.view {
        View::Split => " SPLIT ",
        View::Unified => " UNIFIED ",
    };
    let scroll = scroll_pct(app);
    let help = "q quit  j/k scroll  h/l ←→  n/p file  [ ] width  u view";
    let mut spans = vec![Span::styled(
        pos,
        Style::default().fg(Color::Black).bg(Color::Gray),
    )];
    if app.watch {
        spans.push(Span::styled(
            " WATCH ",
            Style::default().fg(Color::Black).bg(Color::Green),
        ));
    }
    spans.extend([
        Span::styled(mode, Style::default().fg(Color::Black).bg(Color::Cyan)),
        Span::styled(format!(" {help}  "), Style::default().fg(Color::Gray)),
        Span::styled(scroll, Style::default().fg(Color::DarkGray)),
    ]);
    let line = Line::from(spans);
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
    let panes = Layout::horizontal([
        Constraint::Percentage(app.split_pct),
        Constraint::Percentage(100 - app.split_pct),
    ])
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
        let lseg = row.left.as_ref().and_then(|c| app.segs(true, c.num));
        let rseg = row.right.as_ref().and_then(|c| app.segs(false, c.num));
        left_lines.push(cell_line(&row.left, row.kind, true, app.h_offset, left_w, lseg));
        right_lines.push(cell_line(&row.right, row.kind, false, app.h_offset, right_w, rseg));
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
        let lseg = row.left.as_ref().and_then(|c| app.segs(true, c.num));
        let rseg = row.right.as_ref().and_then(|c| app.segs(false, c.num));
        match row.kind {
            Kind::Equal => {
                lines.push(cell_line(&row.left, Kind::Equal, true, app.h_offset, width, lseg))
            }
            Kind::Removed => {
                lines.push(cell_line(&row.left, Kind::Removed, true, app.h_offset, width, lseg))
            }
            Kind::Added => {
                lines.push(cell_line(&row.right, Kind::Added, false, app.h_offset, width, rseg))
            }
            Kind::Changed => {
                lines.push(cell_line(&row.left, Kind::Changed, true, app.h_offset, width, lseg));
                lines.push(cell_line(&row.right, Kind::Changed, false, app.h_offset, width, rseg));
            }
        }
    }
    app.content_len = lines.len();

    let h = area.height as usize;
    let visible: Vec<Line> = lines.into_iter().skip(app.v_offset).take(h).collect();
    f.render_widget(Paragraph::new(visible), area);
}

/// Build a styled line for one cell. `width` is the full pane inner width.
/// `segs` carries precomputed syntax foreground runs (None = no highlighting).
fn cell_line(
    cell: &Option<Cell>,
    kind: Kind,
    left: bool,
    h_off: usize,
    width: usize,
    segs: Option<&LineSegs>,
) -> Line<'static> {
    let Some(cell) = cell else {
        // Padding gap — faint tilde marker so the eye reads it as "nothing here".
        return Line::from(Span::styled(
            " ".repeat(width.min(2)) + "~",
            Style::default().fg(Color::Rgb(60, 60, 70)),
        ));
    };

    // fallback fg (when no syntax color), sign, subtle whole-line bg, strong
    // inline-change bg.
    let (fallback, sign, line_bg, hot_bg) = match (kind, left) {
        (Kind::Equal, _) => (Color::Gray, ' ', Color::Reset, Color::Reset),
        (Kind::Removed, _) | (Kind::Changed, true) => {
            (Color::Red, '-', Color::Rgb(48, 26, 26), Color::Rgb(95, 35, 35))
        }
        (Kind::Added, _) | (Kind::Changed, false) => {
            (Color::Green, '+', Color::Rgb(24, 42, 24), Color::Rgb(35, 80, 35))
        }
    };

    let gutter = format!("{:>4} ", cell.num);
    let text_w = width.saturating_sub(GUTTER + SIGN);

    let mut spans = vec![
        Span::styled(gutter, Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{sign} "), Style::default().fg(fallback)),
    ];
    spans.extend(styled_text(
        &cell.text,
        segs,
        &cell.inline,
        h_off,
        text_w,
        fallback,
        line_bg,
        hot_bg,
    ));
    Line::from(spans)
}

/// Visible slice of a line as styled spans. Foreground comes from syntax
/// segments (or `fallback_fg`); background is `line_bg`, upgraded to `hot_bg`
/// (plus bold) inside the intra-line changed ranges. `h_off`/`width` are chars.
#[allow(clippy::too_many_arguments)]
fn styled_text(
    text: &str,
    segs: Option<&LineSegs>,
    ranges: &[(usize, usize)],
    h_off: usize,
    width: usize,
    fallback_fg: Color,
    line_bg: Color,
    hot_bg: Color,
) -> Vec<Span<'static>> {
    let in_range = |b: usize| ranges.iter().any(|&(s, e)| b >= s && b < e);
    let style = |fg: Color, hot: bool| {
        let bg = if hot { hot_bg } else { line_bg };
        let mut s = Style::default().fg(fg);
        if bg != Color::Reset {
            s = s.bg(bg);
        }
        if hot {
            s = s.add_modifier(Modifier::BOLD);
        }
        s
    };

    let mut out: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut cur: Option<(Color, bool)> = None;
    let mut shown = 0;

    for (ci, (byte, ch)) in text.char_indices().enumerate() {
        if ci < h_off {
            continue;
        }
        if shown >= width {
            break;
        }
        let fg = segs.and_then(|s| highlight::color_at(s, byte)).unwrap_or(fallback_fg);
        let hot = in_range(byte);
        if cur.is_some() && cur != Some((fg, hot)) {
            let (f, h) = cur.unwrap();
            out.push(Span::styled(std::mem::take(&mut buf), style(f, h)));
        }
        buf.push(ch);
        cur = Some((fg, hot));
        shown += 1;
    }
    if let Some((f, h)) = cur {
        out.push(Span::styled(buf, style(f, h)));
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
        let spans = styled_text(
            "abXYef", None, &[(2, 4)], 0, 20, Color::Red, Color::Reset, Color::Blue,
        );
        let parts: Vec<&str> = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(parts, vec!["ab", "XY", "ef"]);
        assert_eq!(spans[1].style.bg, Some(Color::Blue)); // hot run gets hot_bg
        assert_eq!(spans[0].style.bg, None); // line_bg Reset -> no bg
    }

    #[test]
    fn respects_horizontal_offset_and_width() {
        // Skip 2 chars, show 3.
        let spans = styled_text(
            "abcdefgh", None, &[], 2, 3, Color::Gray, Color::Reset, Color::Reset,
        );
        let joined: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(joined, "cde");
    }

    #[test]
    fn syntax_segments_drive_foreground() {
        // Two color runs from "syntax" -> two spans with those fgs.
        let segs: LineSegs = vec![(0, 2, Color::Red), (2, 4, Color::Green)];
        let spans = styled_text(
            "abcd", Some(&segs), &[], 0, 20, Color::Gray, Color::Reset, Color::Reset,
        );
        let parts: Vec<&str> = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(parts, vec!["ab", "cd"]);
        assert_eq!(spans[0].style.fg, Some(Color::Red));
        assert_eq!(spans[1].style.fg, Some(Color::Green));
    }
}
