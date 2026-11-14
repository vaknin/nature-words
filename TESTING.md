# Testing

Both tests drive a real Chromium (`/usr/bin/chromium`, or set `CHROME=...`) at the two laptop sizes, 1366x768 and 1280x720. Install once:

```sh
cd test && npm ci
```

## Sign-in test (`test/signin.mjs`)

Checks the kid sign-in page (the server's root) and the teacher page (`/teacher/`).

```sh
cd test
node signin.mjs all --local ../server/target/release/lesson-server   # against a throwaway local server
node signin.mjs all "$(cat ~/.local/state/lesson-server/url)"        # against the live server
```

- Sections: `kid`, `teacher`, `race`, `app`, `access`, or `all` (runs kid, teacher and race). `app` and `access` run only on their own; `access` only with `--local`.
- `--local BIN` starts BIN on a free port with its own temp database, board log, teacher key and pages from `server/www`. Build it first with `cd server && cargo build --release`.
- A URL uses that server and the teacher key in `~/.config/lesson-server/teacher.key`.
- Every class it creates is deleted at the end, pass or fail. It exits non-zero on any miss.

## Smoke test (`test/smoke.mjs`)

Opens every teacher stage and every trail page (`#level-1..3`), plays the start quiz and every trail game to the end, and checks the race board on a fresh test topic (never the class board).

```sh
cd test
timeout 400 node smoke.mjs            # the live site, https://vaknin.github.io/nature-words/
timeout 400 node smoke.mjs URL        # any other copy
```

Screenshots go to `test/shots/` (cleared each run). It exits non-zero on any failure.
