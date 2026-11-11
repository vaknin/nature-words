# Our lesson server, live

`sh deploy/install.sh` builds and installs it; run it again after any change. `sh deploy/signin-proof.sh URL`
signs a kid in through the public address end to end (on a throwaway class it deletes after).

## Pages

- `/` the kid sign-in (design A): class code, tap your name, tap your secret animal (from `server/www`).
- `/teacher/` the teacher page: classes, new class, the code on the projector, each kid's Locked or Waiting badge
  with Unlock and New animal. Only the teacher gets in (see "Teacher sign-in" below): through Cloudflare Access
  once it is on, by the teacher key until then.
- `/nature-words/` the lesson.

### The Kitah app (the teacher page, installed)

`/teacher/` is also an installable web app named Kitah. On Android Chrome open `/teacher/`, open the menu, tap
**Install app**; it then opens standalone (no address bar) from its own green K icon. Its service worker serves the
teacher page network-first (a cached copy when offline) and never caches or answers `/api`, so classes and kids are
always live. Prove it installable with Chrome's own check (headless, a phone-size window):
`cd test && npm ci && node signin.mjs app "$(cat ~/.local/state/lesson-server/url)"`
(or `node signin.mjs app --local ../server/target/debug/lesson-server`).

Test both pages in a real browser against the live address (throwaway classes, deleted after):
`cd test && npm ci && node signin.mjs all "$(cat ~/.local/state/lesson-server/url)"`
(or `node signin.mjs all --local ../server/target/debug/lesson-server` for a local throwaway server).

## Phone remote

The teacher moves the projected lesson from his phone while he walks around the class, through this server (the
projector laptop and the phone never talk to each other directly):

1. On the projector laptop open the lesson's teacher view, `/nature-words/#teacher-1` here or the GitHub Pages copy
   (it finds this server through its `board.json`). The top bar shows a small `Phone: 123456`.
2. On the phone open `/teacher/`, tap **Projector remote** and type those six digits once. The label turns to
   `Phone ✓`; the phone remembers the pairing.
3. Back and Next, Show / Play (the Space key), a chip per stage and QR move the projector exactly as its keys do.
   The phone shows the stage the projector is on.

Full screen stays one tap on the projector laptop: browsers allow it only from a gesture on that machine. A code
works once and for 10 minutes; the projector laptop never holds the teacher key. Screens live in memory, so after
a server restart the projector shows a new code and the phone says **Projector not connected** with **Pair again**.
The projector long-polls (held up to 50 s, under Cloudflare's 100 s limit) because quick tunnels carry no SSE.

Test it in a real browser against the live address (headless; projector and phone in two browsers):
`cd test && npm ci && node remote.mjs all "$(cat ~/.local/state/lesson-server/url)"`
(or `node remote.mjs all --local ../server/target/debug/lesson-server` locally). It prints the slowest
command-to-move time.

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

## Teacher sign-in

Every teacher request (the classes, the kids, pairing and driving the projector, and the `/teacher/` page itself) is
checked by the server, so the address alone gives nothing:

- **Access on** (an `access.json`, below): only Cloudflare's signed token for his one email is the teacher. Cloudflare
  adds it as the `Cf-Access-Jwt-Assertion` header on every path its Access application covers; the server checks
  its RS256 signature against the team's keys (`https://TEAM.cloudflareaccess.com/cdn-cgi/access/certs`, kept an
  hour), the issuer, the application's AUD tag, the expiry and the email. The teacher app also sends Cloudflare's
  `CF_Authorization` cookie, which counts only together with the app's own `X-Kitah-App` header. The teacher key is
  refused. Without a good token `/teacher/` itself answers 401; the app's manifest, icons and `sw.js` stay open,
  because Chrome fetches the manifest without cookies and Install would break. The page then shows Sign out
  (Cloudflare's `/cdn-cgi/access/logout`), and when the sign-in has ended it says so with one button that reloads
  (Cloudflare asks Google again); it never shows a key box.
- **Access off** (no `access.json`): the key in `~/.config/lesson-server/teacher.key` (made on first run, mode 600),
  sent as the `X-Teacher-Key` header; the page asks for it once. This is how the live server runs until the
  kitah.co.il zone is active.
- Either way, a teacher request whose `Origin` header names another host than the request's own `Host` is refused
  403 before it runs, so a page on another site can't ride his cookie (Cloudflare adds its token header to any
  request carrying the cookie, a cross-site form post included). This relies on cloudflared passing the public Host
  through unchanged, its default (no `httpHostHeader` set).

`GET /api/auth` answers `{"access": true|false, "teacher": true|false}`; the page asks it first.

**access.json** is `~/.config/lesson-server/access.json` (the `LESSON_ACCESS` variable overrides the path), mode 600:

```json
{"team": "TEAM", "aud": "the application's AUD tag", "email": "his one allowed address"}
```

`team` is the Zero Trust team name (or the whole `TEAM.cloudflareaccess.com`); an optional `certs` names a local key
set file instead of fetching the team's, for tests only. No file means Access is off; a file that is there but
missing a field stops the server from starting (it must not start open when he meant it closed). The start line
in `journalctl --user -u lesson-server` says `Cloudflare Access on` or `Cloudflare Access off`.

### Switching Access on, once the kitah.co.il zone is active

The nameservers wait for ISOC IL's approval; until then this stays off. Then, in the Cloudflare dashboard (Zero
Trust's free plan):

1. Zero Trust, Settings, Authentication: add **Google** as a login method.
2. Access, Applications, **Add an application**, Self-hosted, named Kitah teacher, with these destinations on
   `kitah.co.il`: `teacher`, `teacher/*`, `api/classes`, `api/classes/*`, `api/students/*`, `api/screens/pair`,
   `api/screens/*/cmd`, `api/auth`, and any teacher paths later parts add. Leave out `api/screens` itself and the
   projector's `api/screens/TOKEN/cmds` and `api/screens/TOKEN/state`: the projector laptop is not signed in. A
   teacher path left uncovered (like `GET api/screens/ID`) still works, through the cookie and the app's header.
   A path ending in `/*` does not cover its parent, hence both `teacher` and `teacher/*`.
3. Its policy: **Allow**, Include, Emails, his one address. Nothing else.
4. A second self-hosted application for `teacher/manifest.webmanifest`, `teacher/icon-*`, `teacher/icon.svg` and
   `teacher/sw.js` with one **Bypass** policy for Everyone (the more specific path wins), so Install keeps working.
5. Copy the first application's **Application Audience (AUD) Tag** (its Overview) and the team name (Settings,
   Custom Pages, Team domain), write `access.json` with them and his email, `chmod 600` it, and run
   `systemctl --user restart lesson-server.service`. Its start line then says `Cloudflare Access on`.

Prove the server's token check with the installed binary (it starts its own throwaway server with a key set of its
own, mints tokens, and plays Cloudflare's header in a headless browser; then restarts it without the config and
checks the key works): `cd test && npm ci && node signin.mjs access --local "$HOME/.local/bin/lesson-server"`
(or `--local ../server/target/debug/lesson-server`).

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
