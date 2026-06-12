#!/bin/sh
# git-ui installer — macOS, Linux, and WSL.
#
#   curl -fsSL https://raw.githubusercontent.com/codeprakhar25/git-ui/main/install.sh | sh
#
# Downloads the prebuilt binary for your platform from the latest GitHub
# release. If no prebuilt binary matches (or download fails) and `cargo` is
# available, it falls back to building from source.
#
# Override the install directory with:  GITUI_BINDIR=/usr/local/bin sh install.sh

set -eu

REPO="codeprakhar25/git-ui"
BIN="git-ui"

err() { printf 'error: %s\n' "$1" >&2; exit 1; }
info() { printf '%s\n' "$1" >&2; }

# --- Pick an install directory we can write to ---------------------------
choose_bindir() {
	if [ -n "${GITUI_BINDIR:-}" ]; then
		printf '%s' "$GITUI_BINDIR"; return
	fi
	for d in "$HOME/.local/bin" "/usr/local/bin"; do
		if [ -d "$d" ] && [ -w "$d" ]; then printf '%s' "$d"; return; fi
	done
	# Default: create ~/.local/bin
	printf '%s' "$HOME/.local/bin"
}

# --- Detect platform -> Rust target triple -------------------------------
detect_target() {
	os=$(uname -s)
	arch=$(uname -m)
	case "$os" in
		Linux)  os_part="unknown-linux-gnu" ;;
		Darwin) os_part="apple-darwin" ;;
		*)      err "unsupported OS: $os (try building from source with cargo)" ;;
	esac
	case "$arch" in
		x86_64|amd64)  arch_part="x86_64" ;;
		arm64|aarch64) arch_part="aarch64" ;;
		*)             err "unsupported arch: $arch" ;;
	esac
	printf '%s-%s' "$arch_part" "$os_part"
}

# --- Fallback: build with cargo ------------------------------------------
build_from_source() {
	command -v cargo >/dev/null 2>&1 || \
		err "no prebuilt binary for your platform and cargo not found. Install Rust from https://rustup.rs"
	info "Building from source with cargo..."
	tmp=$(mktemp -d)
	trap 'rm -rf "$tmp"' EXIT
	git clone --depth 1 "https://github.com/$REPO" "$tmp/src"
	( cd "$tmp/src" && cargo build --release )
	bindir=$(choose_bindir)
	mkdir -p "$bindir"
	install -m 0755 "$tmp/src/target/release/$BIN" "$bindir/$BIN"
	finish "$bindir"
}

# --- Tell the user we're done, nudge PATH if needed ----------------------
finish() {
	bindir="$1"
	info ""
	info "Installed $BIN to $bindir/$BIN"
	case ":$PATH:" in
		*":$bindir:"*) info "Run: $BIN --help" ;;
		*) info "Add it to your PATH, e.g.:"
		   info "  echo 'export PATH=\"$bindir:\$PATH\"' >> ~/.bashrc && source ~/.bashrc"
		   info "Then run: $BIN --help" ;;
	esac
	exit 0
}

main() {
	command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1 || \
		err "need curl or wget"

	target=$(detect_target)
	asset="$BIN-$target.tar.gz"
	url="https://github.com/$REPO/releases/latest/download/$asset"

	info "Downloading $asset ..."
	tmp=$(mktemp -d)
	trap 'rm -rf "$tmp"' EXIT

	if command -v curl >/dev/null 2>&1; then
		http=$(curl -fsSL -w '%{http_code}' -o "$tmp/$asset" "$url" 2>/dev/null) || http="000"
	else
		wget -qO "$tmp/$asset" "$url" && http="200" || http="000"
	fi

	if [ "$http" != "200" ] || [ ! -s "$tmp/$asset" ]; then
		info "No prebuilt binary available (yet) for $target."
		build_from_source
	fi

	tar -xzf "$tmp/$asset" -C "$tmp" || err "failed to extract $asset"
	[ -f "$tmp/$BIN" ] || err "archive did not contain $BIN"

	bindir=$(choose_bindir)
	mkdir -p "$bindir"
	install -m 0755 "$tmp/$BIN" "$bindir/$BIN"
	finish "$bindir"
}

main
