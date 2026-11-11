#!/bin/bash
# Installs our lesson server for this user: the lesson-server binary (server, and URL watcher as its publish mode)
# in ~/.local/bin, the lesson page in its pages folder, the three user units, enabled and started. Needs cloudflared
# in ~/.local/bin (one binary, no account: github.com/cloudflare/cloudflared/releases, cloudflared-linux-amd64).
# Run again after a change. Once ours answers through its tunnel, it stops and disables the older race-board units
# (nature-board*), so one board server and one tunnel run; it deletes nothing.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
cargo install -q --locked --path "$here/../server" --root "$HOME/.local"
# The live lesson page is never changed here: it is only copied into the server's pages folder.
www="$HOME/.local/share/lesson-server/www/nature-words"
mkdir -p "$www"
install -m644 "$here/../index.html" "$www/index.html"
# The kid sign-in (the server's root) and the teacher page (/teacher/), from server/www, beside nature-words/.
root="$HOME/.local/share/lesson-server/www"
mkdir -p "$root/teacher"
install -m644 "$here/../server/www/index.html" "$root/index.html"
# The whole teacher app (Kitah): the page, its manifest, service worker and icons.
install -m644 "$here/../server/www/teacher/"* "$root/teacher/"
mkdir -p "$HOME/.config/systemd/user"
units="lesson-server.service lesson-tunnel.service lesson-tunnel-url.service"
for u in $units; do install -m644 "$here/$u" "$HOME/.config/systemd/user/$u"; done
systemctl --user daemon-reload
systemctl --user enable -q $units
systemctl --user restart lesson-server.service lesson-tunnel-url.service
# The tunnel is started only when it is down: a restart gives it a new address.
systemctl --user is-active -q lesson-tunnel.service || systemctl --user start lesson-tunnel.service

# Wait for the URL watcher's address to answer through the tunnel. Measured: a fresh quick tunnel answered
# /v1/health after 9.4, 17.0 and 40.6 s (3 restarts); the watcher looks every 15 s. 40.6 + 15 = 56 s, so 90 s
# leaves half again before giving up.
url_file="$HOME/.local/state/lesson-server/url"
deadline=$(( $(date +%s) + 90 ))
until [ -s "$url_file" ] && curl -fsS -m 10 "$(cat "$url_file")/v1/health" 2>/dev/null | grep -q healthy; do
  if [ "$(date +%s)" -ge "$deadline" ]; then
    echo "install: our tunnel did not answer within 90 s; the older race-board units are left running" >&2
    exit 1
  fi
  sleep 1
done
echo "install: lesson server live at $(cat "$url_file")"

old="nature-board.service nature-board-tunnel.service nature-board-url.service"
for u in $old; do
  systemctl --user stop "$u" 2>/dev/null || true
  systemctl --user disable -q "$u" 2>/dev/null || true
done
