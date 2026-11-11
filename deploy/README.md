# Our lesson server, live

`sh deploy/install.sh` builds and installs it; run it again after any change. `sh deploy/signin-proof.sh URL`
signs a kid in through the public address end to end (on a throwaway class it deletes after).

## Pages

- `/` the kid sign-in (design A): class code, tap your name, tap your secret animal (from `server/www`).
- `/teacher/` the teacher page: classes, new class, the code on the projector, each kid's Locked or Waiting badge
  with Unlock and New animal. It asks for the teacher key once.
- `/nature-words/` the lesson.

Test both pages in a real browser against the live address (throwaway classes, deleted after):
`cd test && npm ci && node signin.mjs all "$(cat ~/.local/state/lesson-server/url)"`
(or `node signin.mjs all --local ../server/target/debug/lesson-server` for a local throwaway server).

## What runs (user systemd units)

| Unit | What | Port |
|---|---|---|
| `lesson-server.service` | `~/.local/bin/lesson-server`: kid sign-in (class code, name, secret picture), kids' records (SQLite), race boards, lesson pages from `~/.local/share/lesson-server/www` | 127.0.0.1:8799 |
| `lesson-tunnel.service` | `~/.local/bin/cloudflared` quick tunnel to 127.0.0.1:8799 | metrics 127.0.0.1:8796 |
| `lesson-tunnel-url.service` | `lesson-server publish`: reads the tunnel's address from the metrics port, waits until our server answers through it, writes it to `~/.local/state/lesson-server/url`, and pushes it as `board.json` so the GitHub Pages lesson finds the race boards | none |

Ports: 8799 for the server because 8797 and 8798 were held by the older race-board server and its tunnel
(seen with `ss -ltn`); 8796 for our tunnel's metrics, free in `ss -ltn`. Both listen on 127.0.0.1 only: the
server is behind CGNAT and nothing is opened to the internet; the world reaches it only through the tunnel.

Once ours answers through its tunnel, `install.sh` stops and disables the older `nature-board`,
`nature-board-tunnel` and `nature-board-url` units, so one board server and one tunnel run. It deletes nothing.

Teacher actions need the key in `~/.config/lesson-server/teacher.key` (made on first run, mode 600), sent as the
`X-Teacher-Key` header.

## The address is temporary until the domain is bought

The quick tunnel is **temporary**: it gets a new random `https://X.trycloudflare.com` address on each restart
of `lesson-tunnel.service`, and Cloudflare gives it no uptime promise. The URL watcher republishes each new
address within one poll (15 s) once it answers. A quick tunnel also carries at most 200 requests in flight,
above the server's own cap of 192 connections (CONN_MAX).

Race boards poll (`GET /TOPIC/json?poll=1&since=12h`): quick tunnels do not carry Server-Sent Events, so the
server offers no stream.

## The one step to a named tunnel on our own domain

Buying the domain is Aviv's (it costs money). Once it is bought and added to his Cloudflare account, switch to a
named tunnel, which keeps one fixed address:

```sh
cloudflared tunnel login && cloudflared tunnel create lesson && cloudflared tunnel route dns lesson lessons.DOMAIN
```

then change `ExecStart` in `lesson-tunnel.service` to
`%h/.local/bin/cloudflared tunnel --no-autoupdate --metrics 127.0.0.1:8796 run --url http://127.0.0.1:8799 lesson`
and run `sh deploy/install.sh`. The address is then `https://lessons.DOMAIN` for good, and the watcher is no
longer needed (a named tunnel has no `/quicktunnel` hostname to follow).
