//! Interactive state for the TUI. Rows are precomputed per file so navigation
//! and scrolling are pure index math.

use crate::diff::{self, Row};
use crate::git::{self, FileDiff, Mode};
use crate::highlight::{Highlighter, LineSegs};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum View {
    Split,
    Unified, // wired in M5
}

/// Minimum / maximum left-pane width in split view, as a percentage.
const SPLIT_MIN: u16 = 20;
const SPLIT_MAX: u16 = 80;
const SPLIT_STEP: u16 = 5;

pub struct App {
    /// What we're diffing — kept so `--watch` can re-collect on file changes.
    pub mode: Mode,
    /// Whether we re-poll git and live-reload on a timer.
    pub watch: bool,
    pub files: Vec<FileDiff>,
    /// Aligned rows, one Vec per file (parallel to `files`).
    pub rows: Vec<Vec<Row>>,
    /// Per-file syntax segments for the old/new sides (parallel to `files`).
    pub hl: Vec<(Vec<LineSegs>, Vec<LineSegs>)>,
    /// Highlighter kept alive so watch reloads don't reload syntax defaults.
    hilite: Highlighter,
    /// Whether syntax highlighting is currently applied.
    pub syntax: bool,
    pub cur: usize,
    pub v_offset: usize,
    pub h_offset: usize,
    pub view: View,
    pub quit: bool,
    /// Left-pane width in split view, as a percentage (`[` / `]` adjust it).
    pub split_pct: u16,
    /// Height of the diff body in the last render — needed to clamp scrolling.
    pub viewport_h: usize,
    /// Horizontal scroll (in columns) of the file tab strip.
    pub tab_scroll: usize,
    /// Scrollable line count of the current view, set each render so scrolling
    /// clamps correctly (unified emits more lines than split).
    pub content_len: usize,
}

/// Build aligned rows + syntax segments for a set of files.
fn build_views(
    hilite: &Highlighter,
    files: &[FileDiff],
) -> (Vec<Vec<Row>>, Vec<(Vec<LineSegs>, Vec<LineSegs>)>) {
    let rows = files
        .iter()
        .map(|f| diff::build(f.old.as_deref().unwrap_or(""), f.new.as_deref().unwrap_or("")))
        .collect();
    let hl = files
        .iter()
        .map(|f| {
            (
                hilite.file(&f.path, f.old.as_deref().unwrap_or("")),
                hilite.file(&f.path, f.new.as_deref().unwrap_or("")),
            )
        })
        .collect();
    (rows, hl)
}

impl App {
    pub fn new(mode: Mode, files: Vec<FileDiff>, watch: bool) -> Self {
        let hilite = Highlighter::new();
        let (rows, hl) = build_views(&hilite, &files);
        App {
            mode,
            watch,
            files,
            rows,
            hl,
            hilite,
            syntax: true,
            cur: 0,
            v_offset: 0,
            h_offset: 0,
            view: View::Split,
            quit: false,
            split_pct: 50,
            viewport_h: 0,
            tab_scroll: 0,
            content_len: 0,
        }
    }

    /// Re-collect from git and rebuild views if anything changed. Preserves the
    /// current file selection and scroll position (clamped). Returns whether the
    /// diff actually changed. Used by `--watch`.
    pub fn reload(&mut self) -> bool {
        let Ok(files) = git::collect(&self.mode) else {
            return false;
        };
        if files == self.files {
            return false;
        }
        let (rows, hl) = build_views(&self.hilite, &files);
        self.files = files;
        self.rows = rows;
        self.hl = hl;
        if self.files.is_empty() {
            self.cur = 0;
        } else if self.cur >= self.files.len() {
            self.cur = self.files.len() - 1;
        }
        self.v_offset = self.v_offset.min(self.max_v());
        true
    }

    /// Move the split divider right (left pane wider).
    pub fn widen_left(&mut self) {
        self.split_pct = (self.split_pct + SPLIT_STEP).min(SPLIT_MAX);
    }

    /// Move the split divider left (left pane narrower / right pane wider).
    pub fn widen_right(&mut self) {
        self.split_pct = self.split_pct.saturating_sub(SPLIT_STEP).max(SPLIT_MIN);
    }

    /// Label shown in the tab strip for a file: ` <status> <path> `.
    pub fn tab_label(&self, i: usize) -> String {
        let f = &self.files[i];
        format!(" {} {} ", f.status, f.path)
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows[self.cur]
    }

    /// True when the current file has no actual changes.
    pub fn current_unchanged(&self) -> bool {
        self.rows().iter().all(|r| r.kind == diff::Kind::Equal)
    }

    /// Syntax segments for line `num` (1-based) on the given side of the current
    /// file, or None when highlighting is off / out of range.
    pub fn segs(&self, left: bool, num: usize) -> Option<&LineSegs> {
        if !self.syntax {
            return None;
        }
        let (old, new) = &self.hl[self.cur];
        let side = if left { old } else { new };
        side.get(num.saturating_sub(1))
    }

    pub fn toggle_syntax(&mut self) {
        self.syntax = !self.syntax;
    }

    fn max_v(&self) -> usize {
        if self.files.is_empty() {
            return 0;
        }
        self.content_len.max(self.rows().len()).saturating_sub(self.viewport_h)
    }

    pub fn scroll_down(&mut self, n: usize) {
        self.v_offset = (self.v_offset + n).min(self.max_v());
    }

    pub fn scroll_up(&mut self, n: usize) {
        self.v_offset = self.v_offset.saturating_sub(n);
    }

    pub fn scroll_right(&mut self, n: usize) {
        self.h_offset += n;
    }

    pub fn scroll_left(&mut self, n: usize) {
        self.h_offset = self.h_offset.saturating_sub(n);
    }

    pub fn top(&mut self) {
        self.v_offset = 0;
    }

    pub fn bottom(&mut self) {
        self.v_offset = self.max_v();
    }

    pub fn next_file(&mut self) {
        if self.files.is_empty() {
            return;
        }
        // Wrap past the last file back to the first.
        self.cur = (self.cur + 1) % self.files.len();
        self.reset_scroll();
    }

    pub fn prev_file(&mut self) {
        if self.cur > 0 {
            self.cur -= 1;
            self.reset_scroll();
        }
    }

    fn reset_scroll(&mut self) {
        self.v_offset = 0;
        self.h_offset = 0;
    }

    pub fn toggle_view(&mut self) {
        self.view = match self.view {
            View::Split => View::Unified,
            View::Unified => View::Split,
        };
    }
}
