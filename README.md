# git-ui

**A VS Code-style split git diff viewer for the terminal.**

When you write code through the terminal — with an AI agent like Claude Code or
Codex, or just `vim` and `git` — you lose the editor's pretty side-by-side diff.
`git-ui` brings it back: point it at a file, a commit, or your whole working
tree and get a graphical split diff right in the terminal, deletions on the
left, additions on the right, with syntax highlighting and intra-line change
markers.

```
 M sample.ts
   1   line one                          │   1   line one
   2 - line TWO changed                  │   2 + line TWO edited
   3   line three                        │   3   line three
   4 - inserted line                     │   4 + NEW inserted
   5   line four                         │   5   line four
   6   fn main() {                       │   6   fn main() {
   7 -     println(42)                   │   7 +     println(99)
   8   }                                 │   8   }
 [1/1]  SPLIT  q quit  j/k scroll  h/l ←→  n/p file  g/G top/bot  u view  All
```

---

## Features

- **Side-by-side split diff** — removed lines left (red), added lines right
  (green), unchanged lines aligned across both panes.
- **Intra-line highlighting** — changed words/characters within a line are
  highlighted, like VS Code's inline diff.
- **Syntax highlighting** — language-aware coloring layered under the diff
  colors (powered by [`syntect`](https://github.com/trishume/syntect)).
- **Three modes** — a single file, an entire working tree, or any commit.
- **Unified view** — toggle to a classic single-column `+`/`-` diff.
- **Scrollable file tabs** — move between changed files with one keystroke.
- **Untracked files included** — new files you haven't `git add`ed still show up.
- **Single static binary** — no runtime, no `node_modules`, just `git-ui`.

## Install

Requires a recent [Rust toolchain](https://rustup.rs/) and `git` on your `PATH`.

### From source

```sh
git clone https://github.com/prakhar/git-ui
cd git-ui
cargo install --path .
```

This drops a `git-ui` binary in `~/.cargo/bin` (make sure it's on your `PATH`).

### Build without installing

```sh
cargo build --release
./target/release/git-ui --help
```

## Usage

Run inside any git repository:

```sh
git-ui <file>        # one file: working-tree changes vs HEAD
git-ui all           # every uncommitted change in the working tree
git-ui <commit>      # everything a commit changed (commit vs its parent)
```

Examples:

```sh
git-ui src/main.rs       # what did I change in this file since the last commit?
git-ui all               # review my whole working tree before committing
git-ui HEAD              # what did my last commit actually change?
git-ui a1b2c3d           # inspect an arbitrary commit by hash
```

Argument resolution is automatic: `all` is a keyword, an existing/tracked path
is treated as a file, and anything else is resolved as a commit-ish. On a
name collision a real file wins over a same-named revision.

If a file has no changes, `git-ui` says so instead of showing an empty diff.

## Keybindings

| Key                | Action                          |
| ------------------ | ------------------------------- |
| `j` / `↓`          | Scroll down                     |
| `k` / `↑`          | Scroll up                       |
| `h` / `←`          | Scroll left (long lines)        |
| `l` / `→`          | Scroll right                    |
| `PgUp` / `PgDn`    | Page up / down                  |
| `g` / `G`          | Jump to top / bottom            |
| `n` / `Tab`        | Next file                       |
| `p` / `Shift+Tab`  | Previous file                   |
| `u`                | Toggle split / unified view     |
| `s`                | Toggle syntax highlighting      |
| `q` / `Esc` / `Ctrl+C` | Quit                        |

## How it works

`git-ui` shells out to `git` to read the old and new contents of each file
(`git show HEAD:path`, the file on disk, `git show <commit>:path`, etc.), then:

1. **Aligns** the two versions with a line-level diff
   ([`similar`](https://github.com/mitsuhiko/similar)), inserting padding rows so
   unchanged lines sit on the same screen row in both panes.
2. **Computes intra-line changes** with a character-level diff for paired
   changed lines.
3. **Pre-highlights** each file's syntax in line order (so multi-line
   constructs like block comments resolve correctly) and caches the colors.
4. **Renders** everything with [`ratatui`](https://github.com/ratatui/ratatui):
   syntax color as the foreground, a subtle background tint for changed lines,
   and a stronger tint on the exact changed characters.

## Limitations

- Binary files are not detected and will render as garbled text.
- Renamed files are shown under their new path only.
- The root-commit empty-tree fallback assumes a SHA-1 repository.

## Development

```sh
cargo build      # debug build
cargo test       # run the unit tests
```

For headless rendering (CI, no TTY) the binary supports snapshot env vars that
draw a single frame to stdout: `GITUI_SNAP=WxH`, `GITUI_CUR=N` (preselect file),
`GITUI_VIEW=unified`.

## License

[MIT](LICENSE) © Prakhar Khatri
