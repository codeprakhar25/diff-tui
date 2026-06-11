//! M1: validate mode-resolution + diff alignment by dumping to stdout.
//! TUI lands in M2.

mod diff;
mod git;

use anyhow::{bail, Result};
use diff::{Kind, Row};

const W: usize = 50; // column width per side for the stdout dump

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

    for f in &files {
        println!("\n=== {} ({}) ===", f.path, f.status);
        let old = f.old.as_deref().unwrap_or("");
        let new = f.new.as_deref().unwrap_or("");
        let rows = diff::build(old, new);
        if rows.iter().all(|r| r.kind == Kind::Equal) {
            println!("  no traced changes for this file");
            continue;
        }
        for row in &rows {
            println!("{}", fmt_row(row));
        }
    }
    Ok(())
}

fn fmt_row(row: &Row) -> String {
    let (lsign, ltext) = side(&row.left, row.kind, true);
    let (rsign, rtext) = side(&row.right, row.kind, false);
    format!("{lsign} {ltext:<W$} │ {rsign} {rtext}", W = W)
}

/// Render one cell to (sign, text). `[..]` markers wrap inline-changed spans so
/// we can eyeball intra-line highlight before real styling exists.
fn side(cell: &Option<diff::Cell>, kind: Kind, left: bool) -> (char, String) {
    let sign = match (kind, cell.is_some()) {
        (_, false) => ' ',
        (Kind::Equal, _) => ' ',
        (Kind::Removed, _) => '-',
        (Kind::Added, _) => '+',
        (Kind::Changed, _) => {
            if left {
                '-'
            } else {
                '+'
            }
        }
    };
    let text = match cell {
        None => String::new(),
        Some(c) => format!("{:>4} {}", c.num, mark_inline(&c.text, &c.inline)),
    };
    (sign, text)
}

fn mark_inline(text: &str, ranges: &[(usize, usize)]) -> String {
    if ranges.is_empty() {
        return text.to_string();
    }
    let mut out = String::new();
    let mut pos = 0;
    for &(s, e) in ranges {
        if s > pos {
            out.push_str(&text[pos..s]);
        }
        out.push('[');
        out.push_str(&text[s..e]);
        out.push(']');
        pos = e;
    }
    out.push_str(&text[pos..]);
    out
}
