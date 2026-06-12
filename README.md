<div align="center">

# git-ui

[![CI](https://github.com/codeprakhar25/git-ui/actions/workflows/ci.yml/badge.svg)](https://github.com/codeprakhar25/git-ui/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/codeprakhar25/git-ui?color=2ea44f)](https://github.com/codeprakhar25/git-ui/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/built%20with-rust-orange.svg?logo=rust)](https://www.rust-lang.org)
[![Install](https://img.shields.io/badge/install-curl%20%7C%20sh-2ea44f.svg)](#install)
[![Platforms](https://img.shields.io/badge/platforms-macOS%20%C2%B7%20Linux%20%C2%B7%20WSL-lightgrey.svg)](#install)

**A VS Code-style split git diff viewer for the terminal.**

When you code through the terminal — driving an AI agent like Claude Code or Codex,
or just `vim` and `git` — you lose the editor's pretty side-by-side diff. `git-ui`
brings it back: deletions left, additions right, syntax highlighting, intra-line
change markers, and a `--watch` mode that updates live as your files change.

</div>

## Demo

<div align="center">

![git-ui split diff demo — scrolling, resizing panes, switching files, toggling unified view](assets/demo.gif)

</div>

A single working-tree diff in split view: red deletions on the left, green
additions on the right, intra-line changes highlighted, panes resized with
`[` / `]`, files switched with `n`, and a toggle to unified view with `u`.

## Why this exists

The editor diff is one of the nicest things you give up when an AI agent writes
code in your terminal. You can `git diff`, but a flat `+`/`-` wall is hard to read
fast — no alignment, no side-by-side, no clear sense of *what within a line* moved.

`git-ui` is a tiny single-binary TUI that puts the editor-grade diff back in the
terminal, so you can review changes — yours or an agent's — without leaving it.

## Features

- **Side-by-side split diff** — removed lines left (red), added right (green),
  unchanged lines aligned across both panes on the same row.
- **Live watch mode** — `--watch` re-reads on a timer so the diff updates as your
  editor or an AI agent writes files, keeping your scroll position.
- **Intra-line highlighting** — the exact changed words/characters within a line
  are tinted, like VS Code's inline diff.
- **Syntax highlighting** — language-aware coloring under the diff colors, with
  several selectable themes (`--theme`).
- **Resizable panes** — widen either side with `[` / `]` for long lines.
- **Three modes** — a single file, the whole working tree, or any commit.
- **Unified view** — toggle to a classic single-column `+`/`-` diff with `u`.
- **Untracked files included** — new files you haven't `git add`ed still show up.
- **Single static binary** — no runtime, no `node_modules`, just `git-ui`.

## Install

### One line (macOS, Linux, WSL)

```sh
curl -fsSL https://raw.githubusercontent.com/codeprakhar25/git-ui/main/install.sh | sh
```

Downloads the prebuilt binary for your platform from the latest release and
installs it to `~/.local/bin` (override with `GITUI_BINDIR=/usr/local/bin`). If no
prebuilt binary matches and `cargo` is present, it builds from source. WSL uses the
Linux binary. `wget` works too:

```sh
wget -qO- https://raw.githubusercontent.com/codeprakhar25/git-ui/main/install.sh | sh
```

### From source

Requires a recent [Rust toolchain](https://rustup.rs/) and `git` on your `PATH`.

```sh
git clone https://github.com/codeprakhar25/git-ui
cd git-ui
cargo install --path .
```

## Usage

Run inside any git repository:

```sh
git-ui <file>          # one file: working-tree changes vs HEAD
git-ui all             # every uncommitted change in the working tree
git-ui <commit>        # everything a commit changed (commit vs its parent)
git-ui all --watch     # live-reload as files change (great while coding)
git-ui all --theme github   # pick a syntax theme
```

Examples:

```sh
git-ui src/main.rs     # what did I change in this file since the last commit?
git-ui all             # review my whole working tree before committing
git-ui HEAD            # what did my last commit actually change?
git-ui a1b2c3d         # inspect an arbitrary commit by hash
```

Argument resolution is automatic: `all` is a keyword, an existing/tracked path is
treated as a file, and anything else is resolved as a commit-ish. On a name
collision a real file wins over a same-named revision. If a file has no changes,
`git-ui` says so instead of showing an empty diff.

## Keybindings

| Key                    | Action                          |
| ---------------------- | ------------------------------- |
| `j` / `↓`              | Scroll down                     |
| `k` / `↑`              | Scroll up                       |
| `h` / `←`              | Scroll left (long lines)        |
| `l` / `→`              | Scroll right                    |
| `PgUp` / `PgDn`        | Page up / down                  |
| `g` / `G`              | Jump to top / bottom            |
| `n` / `Tab`            | Next file (wraps to first)      |
| `p` / `Shift+Tab`      | Previous file                   |
| `[` / `]`              | Narrow / widen left pane        |
| `u`                    | Toggle split / unified view     |
| `s`                    | Toggle syntax highlighting      |
| `q` / `Esc` / `Ctrl+C` | Quit                            |

## Themes

Pass `--theme <name>` (default `ocean`):

| Name              | Style                       |
| ----------------- | --------------------------- |
| `ocean`           | Base16 Ocean dark (default) |
| `eighties`        | Base16 Eighties dark        |
| `mocha`           | Base16 Mocha dark           |
| `ocean-light`     | Base16 Ocean light          |
| `github`          | GitHub light                |
| `solarized-dark`  | Solarized dark              |
| `solarized-light` | Solarized light             |

## How it works

`git-ui` shells out to `git` to read the old and new contents of each file
(`git show HEAD:path`, the file on disk, `git show <commit>:path`, …), then:

1. **Aligns** the two versions with a line-level diff
   ([`similar`](https://github.com/mitsuhiko/similar)), inserting padding rows so
   unchanged lines sit on the same screen row in both panes.
2. **Computes intra-line changes** with a character-level diff for paired
   changed lines.
3. **Pre-highlights** each file's syntax in line order (so multi-line constructs
   like block comments resolve correctly) and caches the colors.
4. **Renders** everything with [`ratatui`](https://github.com/ratatui/ratatui):
   syntax color as the foreground, a subtle background tint for changed lines, and
   a stronger tint on the exact changed characters.

In `--watch` mode the diff is re-collected on a short timer; if anything changed
the views are rebuilt in place, preserving the selected file and scroll position.

## Development

```sh
cargo build      # debug build
cargo test       # run the unit tests
cargo clippy --all-targets -- -D warnings   # lint (matches CI)
cargo fmt --all  # format
```

Regenerate the demo GIF (needs `tmux`, `asciinema`, and `agg`):

```sh
bash scripts/record-demo.sh
```

For headless rendering (CI, no TTY) the binary supports snapshot env vars that draw
a single frame to stdout: `GITUI_SNAP=WxH`, `GITUI_CUR=N` (preselect file),
`GITUI_VIEW=unified`.

## Limitations

- Binary files are not detected and will render as garbled text.
- Renamed files are shown under their new path only.
- The root-commit empty-tree fallback assumes a SHA-1 repository.

## License

[MIT](LICENSE) © Prakhar Khatri
