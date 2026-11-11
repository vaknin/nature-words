#!/bin/bash
# Installs the race boards' own server for this user: the board binary (server, and URL watcher as its publish mode) in ~/.local/bin, the three
# user units, enabled and started. Needs cloudflared in ~/.local/bin (one binary, no account:
# github.com/cloudflare/cloudflared/releases, cloudflared-linux-amd64). Run again after a change.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
cargo install -q --locked --path "$here/../board" --root "$HOME/.local"
mkdir -p "$HOME/.config/systemd/user"
units="nature-board.service nature-board-tunnel.service nature-board-url.service"
for u in $units; do install -m644 "$here/$u" "$HOME/.config/systemd/user/$u"; done
systemctl --user daemon-reload
systemctl --user enable -q $units
systemctl --user restart nature-board.service nature-board-url.service
# The tunnel is started only when it is down: a restart gives it a new address, and the class falls back to
# ntfy.sh until the new one is published.
systemctl --user is-active -q nature-board-tunnel.service || systemctl --user start nature-board-tunnel.service
