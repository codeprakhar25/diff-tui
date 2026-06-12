#!/usr/bin/env bash
# Regenerate the README demo GIF.
#
# Builds a throwaway demo repo with a realistic diff, runs the real git-ui TUI
# inside a headless tmux session, records it with asciinema, and renders a GIF
# with agg. Drives the UI with scripted keystrokes — no manual interaction.
#
# Requires: cargo, tmux, asciinema, agg.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ASSET_DIR="${ROOT_DIR}/assets"
CAST_FILE="${ASSET_DIR}/demo.cast"
GIF_FILE="${ASSET_DIR}/demo.gif"
COLS=100
ROWS=28
SESSION="gituidemo"

for tool in tmux asciinema agg; do
  command -v "$tool" >/dev/null 2>&1 || { echo "ERROR: $tool required" >&2; exit 1; }
done

echo "Building release binary..."
( cd "$ROOT_DIR" && cargo build --release --quiet )
BIN="${ROOT_DIR}/target/release/git-ui"

# ── Build the demo repo ──────────────────────────────────────────────────────
DEMO_REPO="$(mktemp -d)"
build_repo() {
  cd "$DEMO_REPO"
  git init -q
  git config user.email demo@example.com
  git config user.name demo

  cat >server.py <<'PY'
import json
from http.server import BaseHTTPRequestHandler, HTTPServer


def load_config(path):
    with open(path) as f:
        return json.load(f)


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        body = {"status": "ok", "version": 1}
        self.wfile.write(json.dumps(body).encode())


def main():
    server = HTTPServer(("0.0.0.0", 8000), Handler)
    print("listening on :8000")
    server.serve_forever()


if __name__ == "__main__":
    main()
PY
  git add server.py
  git commit -qm "initial server"

  # Now edit it (the working-tree changes git-ui will show).
  cat >server.py <<'PY'
import json
from http.server import BaseHTTPRequestHandler, HTTPServer


def load_config(path, default=None):
    try:
        with open(path) as f:
            return json.load(f)
    except FileNotFoundError:
        return default or {}


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        body = {"status": "ok", "version": 2}
        self.wfile.write(json.dumps(body).encode())


def main():
    server = HTTPServer(("0.0.0.0", 8080), Handler)
    print("serving on http://localhost:8080")
    server.serve_forever()


if __name__ == "__main__":
    main()
PY

  # A brand-new untracked file (shows up as an addition).
  cat >cache.py <<'PY'
import time


class TTLCache:
    def __init__(self, ttl=60):
        self.ttl = ttl
        self.store = {}

    def get(self, key):
        item = self.store.get(key)
        if item and time.time() - item[1] < self.ttl:
            return item[0]
        return None

    def set(self, key, value):
        self.store[key] = (value, time.time())
PY
  cd "$ROOT_DIR"
}
build_repo

# ── Drive the TUI inside tmux, recorded by asciinema ─────────────────────────
# Recording and driving are separate: asciinema records a *foreground* `tmux
# attach`, while a background process feeds keystrokes to the same session.
export TERM=xterm-256color

# Intro: a clean prompt that "types" `git-ui all` before launching the TUI, so
# the GIF opens with the command rather than a raw binary path.
INTRO="$(mktemp)"
cat >"$INTRO" <<INTROEOF
#!/usr/bin/env bash
cd "$DEMO_REPO"
clear
sleep 0.5
printf '\033[36m~/demo\033[0m \033[1;32m❯\033[0m '
cmd='git-ui all'
for ((i = 0; i < \${#cmd}; i++)); do printf '%s' "\${cmd:i:1}"; sleep 0.07; done
sleep 0.6
printf '\n'
exec "$BIN" all
INTROEOF
chmod +x "$INTRO"

tmux kill-session -t "$SESSION" 2>/dev/null || true
tmux -f /dev/null new-session -d -s "$SESSION" -x "$COLS" -y "$ROWS" "bash $INTRO"
tmux set -g status off
tmux set -ga terminal-overrides ",xterm-256color:RGB"

# Background keystroke driver. Ends by killing the session so the foreground
# attach (and thus asciinema) returns.
(
  key() { tmux send-keys -t "$SESSION" "$1"; sleep "${2:-0.9}"; }
  sleep 3.4   # wait out the intro typing + TUI launch
  key j 0.5; key j 0.5; key j 0.5; key j 1.0   # scroll the diff
  key ] 0.6; key ] 1.2                          # widen the left pane
  key n 1.4                                      # next file (cache.py)
  key u 1.6                                      # unified view
  key u 1.2                                      # back to split
  key q 0.6                                      # quit git-ui
  sleep 0.5
  tmux kill-session -t "$SESSION" 2>/dev/null || true
) &
DRIVER_PID=$!

mkdir -p "$ASSET_DIR"
echo "Recording..."
asciinema rec "$CAST_FILE" \
  --overwrite --quiet \
  --cols "$COLS" --rows "$ROWS" \
  --idle-time-limit 2 \
  --command "tmux attach -t ${SESSION}"

wait "$DRIVER_PID" 2>/dev/null || true
tmux kill-session -t "$SESSION" 2>/dev/null || true

echo "Rendering GIF..."
agg "$CAST_FILE" "$GIF_FILE" \
  --theme github-dark \
  --font-size 15 \
  --speed 1.0 \
  --idle-time-limit 2 \
  --last-frame-duration 3

rm -rf "$DEMO_REPO" "$INTRO"
echo "Wrote ${GIF_FILE}"
