//! Turn an old/new text pair into screen-ready, left/right-aligned rows.
//!
//! The whole point of the alignment: unchanged lines must land on the same
//! screen row in both panes, and changed blocks get padding so additions on the
//! right line up with deletions on the left. With that, the renderer only needs
//! a single shared vertical scroll offset.

use similar::{ChangeTag, DiffTag, TextDiff};

/// One side of a row. Absent (`None` row side) means a padding gap.
#[derive(Debug, Clone)]
pub struct Cell {
    /// 1-based line number in its own file.
    pub num: usize,
    pub text: String,
    /// Byte ranges within `text` that actually changed (intra-line highlight).
    pub inline: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Equal,
    Removed,
    Added,
    /// Both sides present but different (a paired replace line).
    Changed,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub left: Option<Cell>,
    pub right: Option<Cell>,
    pub kind: Kind,
}

/// Strip a single trailing line terminator for display.
fn trim_nl(s: &str) -> String {
    let s = s.strip_suffix('\n').unwrap_or(s);
    let s = s.strip_suffix('\r').unwrap_or(s);
    s.to_string()
}

/// Byte ranges of intra-line changes on one side.
pub type Ranges = Vec<(usize, usize)>;

/// Changed byte ranges on each side, char-level. Returns (old_ranges, new_ranges).
fn inline_ranges(a: &str, b: &str) -> (Ranges, Ranges) {
    let diff = TextDiff::from_chars(a, b);
    let (mut ai, mut bi) = (0usize, 0usize);
    let (mut ar, mut br) = (Vec::new(), Vec::new());
    for ch in diff.iter_all_changes() {
        let len = ch.value().len(); // bytes
        match ch.tag() {
            ChangeTag::Equal => {
                ai += len;
                bi += len;
            }
            ChangeTag::Delete => {
                push_range(&mut ar, ai, ai + len);
                ai += len;
            }
            ChangeTag::Insert => {
                push_range(&mut br, bi, bi + len);
                bi += len;
            }
        }
    }
    (ar, br)
}

/// Append (start,end), merging with the previous range if adjacent.
fn push_range(v: &mut Ranges, start: usize, end: usize) {
    if let Some(last) = v.last_mut()
        && last.1 == start
    {
        last.1 = end;
        return;
    }
    v.push((start, end));
}

/// Build aligned rows from old/new text. Either side may be empty (add/delete
/// of a whole file) — that just yields all Added or all Removed rows.
pub fn build(old: &str, new: &str) -> Vec<Row> {
    let diff = TextDiff::from_lines(old, new);
    let oline = |i: usize| trim_nl(diff.old_slice(i).unwrap_or(""));
    let nline = |i: usize| trim_nl(diff.new_slice(i).unwrap_or(""));
    let mut rows = Vec::new();

    for op in diff.ops() {
        match op.tag() {
            DiffTag::Equal => {
                for (oi, ni) in op.old_range().zip(op.new_range()) {
                    rows.push(Row {
                        left: Some(Cell {
                            num: oi + 1,
                            text: oline(oi),
                            inline: vec![],
                        }),
                        right: Some(Cell {
                            num: ni + 1,
                            text: nline(ni),
                            inline: vec![],
                        }),
                        kind: Kind::Equal,
                    });
                }
            }
            DiffTag::Delete => {
                for oi in op.old_range() {
                    rows.push(Row {
                        left: Some(Cell {
                            num: oi + 1,
                            text: oline(oi),
                            inline: vec![],
                        }),
                        right: None,
                        kind: Kind::Removed,
                    });
                }
            }
            DiffTag::Insert => {
                for ni in op.new_range() {
                    rows.push(Row {
                        left: None,
                        right: Some(Cell {
                            num: ni + 1,
                            text: nline(ni),
                            inline: vec![],
                        }),
                        kind: Kind::Added,
                    });
                }
            }
            DiffTag::Replace => {
                let os: Vec<usize> = op.old_range().collect();
                let ns: Vec<usize> = op.new_range().collect();
                for k in 0..os.len().max(ns.len()) {
                    let (oi, ni) = (os.get(k).copied(), ns.get(k).copied());
                    match (oi, ni) {
                        (Some(oi), Some(ni)) => {
                            // Paired line: compute intra-line highlight.
                            let lt = oline(oi);
                            let rt = nline(ni);
                            let (lr, rr) = inline_ranges(&lt, &rt);
                            rows.push(Row {
                                left: Some(Cell {
                                    num: oi + 1,
                                    text: lt,
                                    inline: lr,
                                }),
                                right: Some(Cell {
                                    num: ni + 1,
                                    text: rt,
                                    inline: rr,
                                }),
                                kind: Kind::Changed,
                            });
                        }
                        (Some(oi), None) => rows.push(Row {
                            left: Some(Cell {
                                num: oi + 1,
                                text: oline(oi),
                                inline: vec![],
                            }),
                            right: None,
                            kind: Kind::Removed,
                        }),
                        (None, Some(ni)) => rows.push(Row {
                            left: None,
                            right: Some(Cell {
                                num: ni + 1,
                                text: nline(ni),
                                inline: vec![],
                            }),
                            kind: Kind::Added,
                        }),
                        (None, None) => unreachable!(),
                    }
                }
            }
        }
    }
    rows
}
