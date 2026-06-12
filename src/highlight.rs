//! Syntax highlighting via syntect. We pre-highlight whole files (in line
//! order, so multi-line constructs like block comments resolve correctly) and
//! cache per-line foreground segments. The renderer overlays diff coloring on
//! top of these.

use ratatui::style::Color;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Foreground runs for one line: (start_byte, end_byte, color).
pub type LineSegs = Vec<(usize, usize, Color)>;

/// Friendly `--theme` names the user can pass, in help-listing order.
pub const THEMES: &[&str] = &[
    "ocean",
    "eighties",
    "mocha",
    "ocean-light",
    "github",
    "solarized-dark",
    "solarized-light",
];

const DEFAULT_THEME: &str = "base16-ocean.dark";

/// Map a friendly name (or a raw syntect key) to a loaded theme.
fn resolve_theme(ts: &ThemeSet, name: &str) -> Option<Theme> {
    let key = match name {
        "ocean" | "dark" => "base16-ocean.dark",
        "eighties" => "base16-eighties.dark",
        "mocha" => "base16-mocha.dark",
        "ocean-light" | "light" => "base16-ocean.light",
        "github" => "InspiredGitHub",
        "solarized-dark" => "Solarized (dark)",
        "solarized-light" => "Solarized (light)",
        other => other, // allow a raw syntect theme key too
    };
    ts.themes.get(key).cloned()
}

pub struct Highlighter {
    ps: SyntaxSet,
    theme: Theme,
}

impl Highlighter {
    /// Build a highlighter using the named theme, falling back to the default
    /// (ocean dark) if the name isn't recognized.
    pub fn new(theme_name: &str) -> Self {
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let theme =
            resolve_theme(&ts, theme_name).unwrap_or_else(|| ts.themes[DEFAULT_THEME].clone());
        Highlighter { ps, theme }
    }

    /// Per-line foreground segments for `text`, syntax chosen from `path`'s
    /// extension. Unknown extensions fall back to plain text (single segment).
    pub fn file(&self, path: &str, text: &str) -> Vec<LineSegs> {
        let syntax = path
            .rsplit('.')
            .next()
            .filter(|_| path.contains('.'))
            .and_then(|ext| self.ps.find_syntax_by_extension(ext))
            .unwrap_or_else(|| self.ps.find_syntax_plain_text());
        let mut h = HighlightLines::new(syntax, &self.theme);
        let mut out = Vec::new();
        for line in LinesWithEndings::from(text) {
            let mut segs = LineSegs::new();
            let mut b = 0usize;
            if let Ok(ranges) = h.highlight_line(line, &self.ps) {
                for (style, piece) in ranges {
                    let len = piece.len();
                    let c = style.foreground;
                    segs.push((b, b + len, Color::Rgb(c.r, c.g, c.b)));
                    b += len;
                }
            }
            out.push(segs);
        }
        out
    }
}

/// Foreground color at `byte` from precomputed segments, if any.
pub fn color_at(segs: &LineSegs, byte: usize) -> Option<Color> {
    segs.iter()
        .find(|&&(s, e, _)| byte >= s && byte < e)
        .map(|&(_, _, c)| c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn rust_line_gets_multiple_colors() {
        let h = Highlighter::new("ocean");
        let segs = h.file("main.rs", "fn main() { let x = 1; }\n");
        let colors: HashSet<_> = segs[0].iter().map(|&(_, _, c)| c).collect();
        // Keyword/ident/number should not all be one color.
        assert!(colors.len() > 1, "expected syntax colors, got {colors:?}");
    }
}
