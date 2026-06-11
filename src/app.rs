//! Interactive state for the TUI. Rows are precomputed per file so navigation
//! and scrolling are pure index math.

use crate::diff::{self, Row};
use crate::git::FileDiff;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum View {
    Split,
    Unified, // wired in M5
}

pub struct App {
    pub files: Vec<FileDiff>,
    /// Aligned rows, one Vec per file (parallel to `files`).
    pub rows: Vec<Vec<Row>>,
    pub cur: usize,
    pub v_offset: usize,
    pub h_offset: usize,
    pub view: View,
    pub quit: bool,
    /// Height of the diff body in the last render — needed to clamp scrolling.
    pub viewport_h: usize,
}

impl App {
    pub fn new(files: Vec<FileDiff>) -> Self {
        let rows = files
            .iter()
            .map(|f| {
                diff::build(
                    f.old.as_deref().unwrap_or(""),
                    f.new.as_deref().unwrap_or(""),
                )
            })
            .collect();
        App {
            files,
            rows,
            cur: 0,
            v_offset: 0,
            h_offset: 0,
            view: View::Split,
            quit: false,
            viewport_h: 0,
        }
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows[self.cur]
    }

    pub fn current(&self) -> &FileDiff {
        &self.files[self.cur]
    }

    /// True when the current file has no actual changes.
    pub fn current_unchanged(&self) -> bool {
        self.rows().iter().all(|r| r.kind == diff::Kind::Equal)
    }

    fn max_v(&self) -> usize {
        self.rows().len().saturating_sub(self.viewport_h)
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
        if self.cur + 1 < self.files.len() {
            self.cur += 1;
            self.reset_scroll();
        }
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
