//! Our lesson server, general for every lesson (not one lesson's page): it serves the lesson pages from a folder,
//! keeps each kid's own record, signs kids in without a password, and runs the race boards.
//!
//! Kid sign-in (research/student-signin.md): the kid types the class code (or opens the class's home link), taps
//! their own name, then their own secret picture from a grid; this works from home too, at any time, because the
//! secret picture guards it. A new name joins Kahoot style: while the teacher has joining open (one lesson), the
//! kid types a first name or first name and family initial, and the teacher approves or rejects it. The approved
//! name is the kid's name from then on and the only one the race boards show; there are no free-text names.
//!   GET  /api/roster/CODE           the class's approved names (A-Z), whether each has picked, join_open
//!   POST /api/join   {code,name}    a new name asks to join; returns a ticket (403 closed, 409 taken or full)
//!   GET  /api/join/TICKET           pending, or approved {id,name,picked}; 404 once rejected
//!   POST /api/pick   {code,student,picture}  first sign-in: sets the picture once, returns a session token
//!   POST /api/signin {code,student,picture}  returns a session token; wrong tries pause, then lock
//!   GET  /api/me, POST /api/me      the kid's own record (Authorization: Bearer TOKEN); POST /api/signout
//!   POST /api/race {topic,place,score}  (Bearer TOKEN) posts the score to the board under the approved name
//! Teacher: with ~/.config/lesson-server/access.json (LESSON_ACCESS overrides the path), only a Cloudflare Access
//! token for his one email (access.rs), from the Cf-Access-Jwt-Assertion header, or the CF_Authorization cookie
//! together with X-Kitah-App; the teacher key is then refused. Without that file, header X-Teacher-Key, the key in
//! ~/.config/lesson-server/teacher.key (made on first run, mode 600). GET /api/auth {access, teacher} says which.
//! Teacher routes:
//!   GET/POST /api/classes (the student list may be empty), DELETE /api/classes/CODE,
//!   POST /api/classes/CODE/students, POST /api/classes/CODE/join {open},
//!   GET /api/classes/CODE/students (join_s, and each kid's id, name, status, picked, locked, wait_s; never the
//!   picture), POST /api/students/ID/approve, POST /api/students/ID/reject (a waiting name only),
//!   POST /api/students/ID/reset (new picture), POST /api/students/ID/unlock, DELETE /api/students/ID
//! Phone remote for the projector (remote.rs): POST /api/screens, GET /api/screens/TOKEN/cmds?after=SEQ (a long
//!   poll), POST /api/screens/TOKEN/state, all without a key; with the key POST /api/screens/pair {code},
//!   POST /api/screens/ID/cmd {cmd, stage?}, GET /api/screens/ID.
//! Race boards: POST /TOPIC and GET /TOPIC/json?poll=1&since=12h, ntfy-shaped (see board.rs). Polling only.
//! POST /TOPIC refuses a JSON body carrying a name (n): names come from sign-in, through POST /api/race.
//! GET anything else: a file from the pages folder. Listens on 127.0.0.1 only; the world reaches it through the
//! Cloudflare tunnel (the server is behind CGNAT, no open ports).

mod access;
mod board;
mod db;
mod publish;
mod remote;

use board::{json_escape, now_s, parse_since, valid_topic, Store, BODY_MAX};
use db::{Db, JoinErr, JoinStatus, SignIn};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ---------- Every limit, with its source ----------

/// 8799: this server's own port. The first board server holds 8797 and its tunnel's metrics 8798 (seen with
/// `ss -ltn` on this server); both run side by side until that one is dropped, so this takes the next free one.
pub const PORT: u16 = 8799;
/// 16 pictures (4x4): the lowest blind-guess rate (1 in 16 a try) that still fits 120 px tiles on the school
/// laptops' 1366x768 screen. research/student-signin.md, "Recommendation" and section 2.
pub const GRID_SIZE: u32 = 16;
/// Pauses, as (wrong tries in a row, seconds): 3 wrong tries give a fair go to a kid who mis-taps, then 60 s; the
/// 5th wrong try 5 min. A pause alone cannot protect a 45-minute lesson, so the lock below caps it.
/// research/student-signin.md, "Recommendation" (the lengths are the research's judgment, not a study's).
pub const PAUSES_S: [(u32, u64); 2] = [(3, 60), (5, 5 * 60)];
/// After 6 wrong tries in a row the record waits for the teacher: a blind guesser then has at most 6 in 16 (38%),
/// against 100% with pauses alone. research/student-signin.md, section 2 table; NIST SP 800-63B-4 allows up to 100.
pub const LOCK_AFTER_WRONG: u32 = 6;
/// 45 minutes, one lesson: shared laptops pass between kids, so a sign-in must not outlive it.
/// research/student-signin.md, "Recommendation" (lesson length from a teacher forum; confirm with Aviv).
pub const SESSION_S: u64 = 45 * 60;
/// 6 digits: a million codes. Digits, because these kids barely know the English letters and every laptop has a
/// digit row. With the miss limit below an outsider needs about 1e6 / (live classes) tries per hit, days at most
/// 120 a minute, and a hit shows first names only.
pub const CLASS_CODE_DIGITS: usize = 6;
/// The most kids in one Israeli class: Ministry guidelines define a standard class as up to 40 pupils
/// (https://news.walla.co.il/item/2829029, date not shown; the reform target is 34, so 40 is the safe side).
pub const CLASS_MAX_KIDS: u32 = 40;
/// Unknown class codes one address may try per pause window: a whole class behind the school's one address,
/// each kid mistyping the code as often as a picture before its first pause, still gets through.
pub const ROSTER_MISSES_PER_WINDOW: u32 = CLASS_MAX_KIDS * PAUSES_S[0].0;
/// The window is the first pause's length, the same wait a kid gets for wrong pictures.
pub const ROSTER_WINDOW_S: u64 = PAUSES_S[0].1;
/// A name: one first word of at most 12 letters (the lesson skill's race-name rule), plus " K." for a family
/// initial: 15 characters. Class names and levels take the same cap.
pub const NAME_MAX_CHARS: usize = 12 + 3;
/// A kid's own name: one word of at most 12 letters, the lesson skill's race-name rule (the 12 in NAME_MAX_CHARS).
pub const KID_WORD_MAX: usize = NAME_MAX_CHARS - 3;
/// Joining with a new name stays open one lesson, like a Kahoot PIN: the same 45 minutes as a session.
pub const JOIN_OPEN_S: u64 = SESSION_S;
/// The teacher key: 32 random bytes (256 bits), far above NIST SP 800-63B's 112-bit floor for a secret.
const TEACHER_KEY_BYTES: usize = 32;
/// A request's line and headers: the page's and curl's are under 1 KB, cloudflared adds about 1 KB of its own.
const HEAD_MAX: usize = 16 * 1024;
/// Connections served at once: a class is up to 40 laptops, each with at most a read and a post open (80);
/// the rest is room, and it stays under the quick tunnel's cap of 200 requests in flight.
pub const CONN_MAX: usize = 192;
/// The page waits 4 s for an answer; a request still sending after 10 s has nobody waiting for it.
const IO_TIMEOUT: Duration = Duration::from_secs(10);

// ---------- State ----------

pub struct App {
    pub db: Mutex<Db>,
    pub board: Mutex<Store>,
    pub teacher_key: String,
    /// Cloudflare Access (access.json): when set, only its signed token is the teacher, never the key.
    pub access: Option<access::Access>,
    /// The lesson pages folder; None serves no pages.
    pub www: Option<PathBuf>,
    /// Projector screens and their phone remotes, memory only.
    pub remote: remote::Remote,
    /// Unknown-code lookups per address: (misses, window start). Memory only, never tied to a kid.
    misses: Mutex<HashMap<String, (u32, u64)>>,
}

impl App {
    pub fn new(db: Db, board: Store, teacher_key: String, www: Option<PathBuf>) -> App {
        App { db: Mutex::new(db), board: Mutex::new(board), teacher_key, access: None, www, misses: Mutex::new(HashMap::new()), remote: remote::Remote::new() }
    }
    pub fn with_access(mut self, a: Option<access::Access>) -> App {
        self.access = a;
        self
    }
    /// Is this request the teacher's? With Access on: a good Access token (the header Cloudflare adds, or the cookie
    /// with the Kitah app's own header); the teacher key is refused. With Access off: the teacher key.
    fn is_teacher(&self, rq: &Req, now: u64) -> bool {
        match &self.access {
            Some(a) => {
                let token = rq.header("cf-access-jwt-assertion").or_else(|| rq.header("x-kitah-app").and(rq.header("cookie")).and_then(access::cookie_token));
                token.is_some_and(|t| a.verify(t, now).is_ok())
            }
            None => rq.header("x-teacher-key").is_some_and(|k| same(k.trim(), &self.teacher_key)),
        }
    }
    fn db(&self) -> std::sync::MutexGuard<'_, Db> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Reads the teacher key from `path`, making it on first run (its folder mode 700, the file mode 600).
pub fn teacher_key(path: &Path) -> io::Result<String> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    if let Ok(k) = std::fs::read_to_string(path) {
        let k = k.trim().to_string();
        if !k.is_empty() {
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
            return Ok(k);
        }
    }
    if let Some(d) = path.parent() {
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(d)?;
    }
    let k = db::random_hex(TEACHER_KEY_BYTES);
    let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?;
    f.write_all(format!("{k}\n").as_bytes())?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(k)
}

// ---------- HTTP ----------

pub struct Req<'a> {
    pub method: &'a str,
    pub target: &'a str,
    pub headers: &'a [(String, String)],
    pub body: &'a [u8],
}

impl Req<'_> {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

pub struct Resp {
    pub status: u16,
    pub ctype: &'static str,
    pub body: Vec<u8>,
}

fn err(status: u16, msg: &str) -> Resp {
    Resp { status, ctype: "application/json", body: format!("{{\"code\":{status},\"error\":\"{}\"}}\n", json_escape(msg)).into_bytes() }
}

fn ok(v: Value) -> Resp {
    jresp(200, v)
}

fn jresp(status: u16, v: Value) -> Resp {
    Resp { status, ctype: "application/json", body: (v.to_string() + "\n").into_bytes() }
}

/// A display name, a class name or a level: trimmed, 1 to NAME_MAX_CHARS characters, no control characters.
fn clean_name(v: Option<&Value>) -> Option<String> {
    let s = v?.as_str()?.trim();
    let n = s.chars().count();
    (n >= 1 && n <= NAME_MAX_CHARS && !s.chars().any(char::is_control)).then(|| s.to_string())
}

/// A name a kid types to join: one word of 1 to KID_WORD_MAX letters (Hebrew or English), optionally a space and
/// one letter for the family initial, with or without a dot. No digits, symbols or second word. Returned tidy:
/// the first letter of each part upper case where the script has one, and the initial always with its dot.
pub fn kid_name(raw: &str) -> Option<String> {
    let parts: Vec<&str> = raw.split_whitespace().collect();
    let (word, initial) = match parts.as_slice() {
        [w] => (*w, None),
        [w, i] => (*w, Some(*i)),
        _ => return None,
    };
    let n = word.chars().count();
    if n == 0 || n > KID_WORD_MAX || !word.chars().all(char::is_alphabetic) {
        return None;
    }
    let cap = |s: &str| {
        let mut c = s.chars();
        c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
    };
    let mut out = cap(word);
    if let Some(i) = initial {
        let letter = i.strip_suffix('.').unwrap_or(i);
        let mut cs = letter.chars();
        match (cs.next(), cs.next()) {
            (Some(c), None) if c.is_alphabetic() => out = format!("{out} {}.", cap(letter)),
            _ => return None,
        }
    }
    Some(out)
}

fn same(a: &str, b: &str) -> bool {
    // Every byte compared, so the time taken says nothing about how much matched.
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// What one request gets: no sockets here, so the tests drive it straight.
pub fn route(app: &App, rq: &Req, now: u64) -> Resp {
    let (path, query) = rq.target.split_once('?').unwrap_or((rq.target, ""));
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    match (rq.method, parts.as_slice()) {
        ("OPTIONS", _) => Resp { status: 204, ctype: "text/plain", body: Vec::new() },
        ("GET" | "HEAD", ["v1", "health"]) => Resp { status: 200, ctype: "application/json", body: b"{\"healthy\":true}\n".to_vec() },
        (_, ["api", rest @ ..]) => api(app, rq, rest, now),
        ("POST" | "PUT", [topic]) if valid_topic(topic) => {
            if rq.body.is_empty() {
                return err(400, "empty message");
            }
            // Names on a board come only from sign-in (POST /api/race); a reset marker still comes this way.
            if serde_json::from_slice::<Value>(rq.body).is_ok_and(|v| v.get("n").is_some()) {
                return err(403, "names come from sign-in: post with /api/race");
            }
            let text = String::from_utf8_lossy(rq.body);
            let mut s = app.board.lock().unwrap_or_else(|e| e.into_inner());
            Resp { status: 200, ctype: "application/json", body: (s.post(topic, &text, now).json() + "\n").into_bytes() }
        }
        ("GET" | "HEAD", [topic, "json"]) if valid_topic(topic) => {
            let since = query.split('&').find_map(|kv| kv.strip_prefix("since=")).unwrap_or("all");
            let Some(since) = parse_since(since, now) else { return err(400, "bad since") };
            let mut s = app.board.lock().unwrap_or_else(|e| e.into_inner());
            Resp { status: 200, ctype: "application/x-ndjson", body: s.read(topic, since, now).into_bytes() }
        }
        ("GET" | "HEAD", _) => page(app, rq, path, now),
        _ => err(404, "not found"),
    }
}

fn api(app: &App, rq: &Req, parts: &[&str], now: u64) -> Resp {
    let body: Value = if rq.body.is_empty() { Value::Null } else {
        match serde_json::from_slice(rq.body) {
            Ok(v) => v,
            Err(_) => return err(400, "body is not JSON"),
        }
    };
    let teacher = app.is_teacher(rq, now);
    let kid_session = || rq.header("authorization").and_then(|v| v.strip_prefix("Bearer ")).and_then(|t| app.db().session(t.trim(), now));
    let pick = |b: &Value| -> Option<(String, i64, u32)> {
        Some((b.get("code")?.as_str()?.to_string(), b.get("student")?.as_i64()?, u32::try_from(b.get("picture")?.as_u64()?).ok()?))
    };
    match (rq.method, parts) {
        ("GET", ["roster", code]) => match by_code(app, rq, code, now, |db| db.roster(code, now)) {
            Ok(r) => ok(r),
            Err(e) => e,
        },
        // The teacher page asks for the key only while Access is off.
        ("GET", ["auth"]) => ok(json!({"access": app.access.is_some(), "teacher": teacher})),
        ("POST", ["join"]) => {
            let code = body.get("code").and_then(Value::as_str).unwrap_or("");
            let name = body.get("name").and_then(Value::as_str).unwrap_or("");
            let tidy = kid_name(name);
            // An unknown code is a miss even when the name is bad too, so codes can't be walked with bad names.
            let res = by_code(app, rq, code, now, |db| match tidy.as_deref().map(|n| db.join(code, n, now)) {
                Some(Err(JoinErr::NoClass)) => None,
                None => db.class_id(code).map(|_| None),
                other => Some(other),
            });
            match res {
                Err(e) => e,
                Ok(None) => err(400, "one first name, or a first name and family initial: letters only"),
                Ok(Some(Ok((_, ticket)))) => ok(json!({"ticket": ticket, "name": tidy})),
                Ok(Some(Err(JoinErr::Closed))) => err(403, "joining is closed: ask your teacher"),
                Ok(Some(Err(JoinErr::Taken))) => err(409, "this name is taken: add your family initial"),
                Ok(Some(Err(_))) => err(409, "the class is full: ask your teacher"),
            }
        }
        ("GET", ["join", ticket]) => match app.db().join_status(ticket) {
            Some(JoinStatus::Pending) => ok(json!({"status": "pending"})),
            Some(JoinStatus::Approved { id, name, picked }) => ok(json!({"status": "approved", "id": id, "name": name, "picked": picked})),
            None => err(404, "no such name waiting: type it again"),
        },
        ("POST", ["pick"]) => {
            let Some((code, kid, pic)) = pick(&body) else { return err(400, "needs code, student and picture") };
            match app.db().first_pick(&code, kid, pic, now) {
                Ok(token) => ok(json!({"token": token, "expires_s": SESSION_S})),
                Err("already picked") => err(409, "already picked"),
                Err("no such kid") => err(404, "no such kid in this class"),
                Err(e) => err(400, e),
            }
        }
        ("POST", ["signin"]) => {
            let Some((code, kid, pic)) = pick(&body) else { return err(400, "needs code, student and picture") };
            match app.db().sign_in(&code, kid, pic, now) {
                SignIn::Ok(token) => ok(json!({"token": token, "expires_s": SESSION_S})),
                SignIn::Wrong { tries_left } => jresp(401, json!({"code": 401, "error": "wrong picture", "tries_left": tries_left})),
                SignIn::Paused { wait_s } => jresp(429, json!({"code": 429, "error": "wait", "wait_s": wait_s})),
                SignIn::Locked => err(423, "locked: ask your teacher"),
                SignIn::NoPicture => err(409, "pick your picture first"),
                SignIn::NoSuchKid => err(404, "no such kid in this class"),
            }
        }
        ("GET", ["me"]) => match kid_session().and_then(|k| app.db().record(k)) {
            Some(r) => ok(r),
            None => err(401, "sign in first"),
        },
        ("POST", ["me"]) => {
            let Some(kid) = kid_session() else { return err(401, "sign in first") };
            let db = app.db();
            if body.get("level").is_some() {
                let Some(level) = clean_name(body.get("level")) else { return err(400, "bad level") };
                db.set_level(kid, &level);
            }
            for w in body.get("words").and_then(Value::as_array).into_iter().flatten() {
                match (w.get("word").and_then(Value::as_str), w.get("right").and_then(Value::as_bool)) {
                    (Some(word), Some(right)) if !word.trim().is_empty() => db.word_result(kid, word.trim(), right, now),
                    _ => return err(400, "each word needs word and right"),
                }
            }
            if let Some(r) = body.get("race") {
                match (r.get("board").and_then(Value::as_str), r.get("score").and_then(Value::as_i64)) {
                    (Some(b), Some(s)) if valid_topic(b) => db.race_score(kid, b, s),
                    _ => return err(400, "race needs board and score"),
                }
            }
            ok(db.record(kid).unwrap_or(Value::Null))
        }
        ("POST", ["screens"]) => screen(app.remote.open(now)),
        ("GET", ["screens", token, "cmds"]) => {
            let after = rq.target.split_once('?').map_or("", |(_, q)| q).split('&').find_map(|kv| kv.strip_prefix("after="));
            let Some(after) = after.map_or(Some(0), |a| a.parse::<u64>().ok()) else { return err(400, "bad after") };
            screen(app.remote.poll(token, after, now))
        }
        ("POST", ["screens", token, "state"]) => screen(app.remote.report(token, &body, now)),
        ("POST", ["race"]) => {
            let Some(kid) = kid_session() else { return err(401, "sign in first") };
            let (Some(topic), Some(place), Some(score)) =
                (body.get("topic").and_then(Value::as_str), body.get("place").and_then(Value::as_i64), body.get("score").and_then(Value::as_i64))
            else {
                return err(400, "needs topic, place and score");
            };
            if !valid_topic(topic) || score < 0 {
                return err(400, "bad topic or score");
            }
            let Some(name) = ({
                let db = app.db();
                db.race_score(kid, topic, score);
                db.name(kid)
            }) else {
                return err(401, "sign in first");
            };
            // The page's own board line: name, score, place, and the time in ms, which breaks ties.
            let line = json!({"n": name, "s": score, "p": place, "t": now * 1000});
            let mut s = app.board.lock().unwrap_or_else(|e| e.into_inner());
            s.post(topic, &line.to_string(), now);
            ok(line)
        }
        ("POST", ["signout"]) => {
            if let Some(t) = rq.header("authorization").and_then(|v| v.strip_prefix("Bearer ")) {
                let _ = app.db().conn.execute("DELETE FROM sessions WHERE token=?1", [t.trim()]);
            }
            ok(json!({"ok": true}))
        }
        // Every route below is the teacher's. A browser sends Origin on cross-site requests, and Cloudflare adds its
        // token header to any request carrying the Access cookie (a form post from another site included), so a
        // request from a page on another host is refused before any of them runs. cloudflared passes the public
        // Host through unchanged, so the teacher's own page always matches.
        _ if foreign_origin(rq) => err(403, "not from this site"),
        _ if !teacher => err(401, "teacher sign-in needed"),
        ("POST", ["screens", "pair"]) => match body.get("code").and_then(Value::as_str) {
            Some(code) => screen(app.remote.pair(code.trim(), now)),
            None => err(400, "needs the code"),
        },
        ("POST", ["screens", id, "cmd"]) => screen(app.remote.command(id, &body, now)),
        ("GET", ["screens", id]) => screen(app.remote.status(id, now)),
        ("GET", ["classes"]) => ok(app.db().classes()),
        ("POST", ["classes"]) => {
            let Some(name) = clean_name(body.get("name")) else { return err(400, "needs a class name") };
            let mut kids = Vec::new();
            for k in body.get("students").and_then(Value::as_array).into_iter().flatten() {
                let Some(k) = clean_name(Some(k)) else { return err(400, "a kid's name is empty or too long") };
                kids.push(k);
            }
            if kids.len() > CLASS_MAX_KIDS as usize {
                return err(400, "too many kids for one class");
            }
            match app.db().create_class(&name, &kids, now) {
                Ok((_, code)) => ok(json!({"code": code, "name": name})),
                Err(_) => err(500, "could not save"),
            }
        }
        ("POST", ["classes", code, "join"]) => {
            let Some(open) = body.get("open").and_then(Value::as_bool) else { return err(400, "needs open: true or false") };
            let db = app.db();
            if !db.open_join(code, open, now) {
                return err(404, "no such class");
            }
            ok(json!({"join_s": if open { JOIN_OPEN_S } else { 0 }}))
        }
        ("POST", ["students", id, action @ ("approve" | "reject")]) => {
            let Ok(id) = id.parse::<i64>() else { return err(400, "bad id") };
            let db = app.db();
            let done = if *action == "approve" { db.approve(id) } else { db.reject(id) };
            if done { ok(json!({"ok": true})) } else { err(404, "no such name waiting") }
        }
        ("GET", ["classes", code, "students"]) => match app.db().students(code, now) {
            Some(v) => ok(v),
            None => err(404, "no such class"),
        },
        ("DELETE", ["classes", code]) => {
            if app.db().delete_class(code) { ok(json!({"ok": true})) } else { err(404, "no such class") }
        }
        ("POST", ["classes", code, "students"]) => {
            let Some(name) = clean_name(body.get("name")) else { return err(400, "needs a name") };
            let db = app.db();
            let Some(cid) = db.class_id(code) else { return err(404, "no such class") };
            let count: u32 = db.conn.query_row("SELECT COUNT(*) FROM students WHERE class_id=?1", [cid], |r| r.get(0)).unwrap_or(0);
            if count >= CLASS_MAX_KIDS {
                return err(400, "too many kids for one class");
            }
            match db.add_student(cid, &name) {
                Ok(id) => ok(json!({"id": id, "name": name})),
                Err(_) => err(500, "could not save"),
            }
        }
        ("POST", ["students", id, action @ ("reset" | "unlock")]) => {
            let Ok(id) = id.parse::<i64>() else { return err(400, "bad id") };
            let db = app.db();
            let done = if *action == "reset" { db.reset(id) } else { db.unlock(id) };
            if done { ok(json!({"ok": true})) } else { err(404, "no such kid") }
        }
        ("DELETE", ["students", id]) => {
            let Ok(id) = id.parse::<i64>() else { return err(400, "bad id") };
            let n = app.db().conn.execute("DELETE FROM students WHERE id=?1", [id]).unwrap_or(0);
            if n > 0 { ok(json!({"ok": true})) } else { err(404, "no such kid") }
        }
        _ => err(404, "not found"),
    }
}

/// Does the request carry an Origin whose host (with port) is not its Host header? No Origin is not foreign: curl
/// and same-origin GETs send none, and a browser always sends one on a cross-site POST, PUT or DELETE.
fn foreign_origin(rq: &Req) -> bool {
    let Some(origin) = rq.header("origin") else { return false };
    let host = origin.split_once("://").map_or("", |(_, h)| h).split('/').next().unwrap_or("");
    !rq.header("host").is_some_and(|h| !host.is_empty() && h.trim().eq_ignore_ascii_case(host))
}

fn screen(o: remote::Out) -> Resp {
    match o {
        remote::Out::Ok(v) => ok(v),
        remote::Out::NotFound => err(404, "no such screen: open a new one"),
        remote::Out::Bad(m) => err(400, m),
        remote::Out::Full => err(503, "too many screens open"),
    }
}

/// Something found only by a class code (the roster, a join): `f` gives None when there is no such class. An
/// address that misses too many codes waits, so codes can't be walked.
fn by_code<T>(app: &App, rq: &Req, code: &str, now: u64, f: impl FnOnce(&Db) -> Option<T>) -> Result<T, Resp> {
    // Behind the tunnel every request comes from cloudflared; Cloudflare puts the real address in this header.
    let who = rq.header("cf-connecting-ip").unwrap_or("-").to_string();
    let mut misses = app.misses.lock().unwrap_or_else(|e| e.into_inner());
    misses.retain(|_, (_, start)| now < *start + ROSTER_WINDOW_S);
    if let Some((n, start)) = misses.get(&who) {
        if *n >= ROSTER_MISSES_PER_WINDOW {
            return Err(jresp(429, json!({"code": 429, "error": "wait", "wait_s": start + ROSTER_WINDOW_S - now})));
        }
    }
    let found = code.len() == CLASS_CODE_DIGITS && code.bytes().all(|b| b.is_ascii_digit());
    match found.then(|| f(&app.db())).flatten() {
        Some(r) => Ok(r),
        None => {
            misses.entry(who).or_insert((0, now)).0 += 1;
            Err(err(404, "no class with this code"))
        }
    }
}

fn ctype(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "json" => "application/json",
        // The installable teacher app's manifest, in the type the web app manifest spec registers.
        "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "mp3" => "audio/mpeg",
        "woff2" => "font/woff2",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// A lesson page from the folder: plain names only, no hidden files, nothing outside it. With Access on, the
/// teacher page itself needs the token, so its address alone gives nothing; the teacher app's other files (manifest,
/// icons, service worker) stay open, since Chrome fetches the manifest without cookies and Install would break.
fn page(app: &App, rq: &Req, path: &str, now: u64) -> Resp {
    let Some(www) = &app.www else { return err(404, "not found") };
    let rel = Path::new(path.trim_start_matches('/'));
    let plain = rel.components().all(|c| matches!(c, Component::Normal(s) if !s.to_string_lossy().starts_with('.')));
    if !plain || path.contains('\\') || path.contains('%') {
        return err(404, "not found");
    }
    let mut f = www.join(rel);
    if f.is_dir() {
        f = f.join("index.html");
    }
    // Compared as a path (components), so /teacher, /teacher/ and /teacher//index.html are all this one file.
    if app.access.is_some() && f == www.join("teacher").join("index.html") && !app.is_teacher(rq, now) {
        return err(401, "sign in through Cloudflare Access");
    }
    match std::fs::read(&f) {
        Ok(b) => Resp { status: 200, ctype: ctype(&f), body: b },
        Err(_) => err(404, "not found"),
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        413 => "Payload Too Large",
        423 => "Locked",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

pub fn write_resp<W: Write>(w: &mut W, r: &Resp, head_only: bool) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n\
         Access-Control-Allow-Headers: *\r\nAccess-Control-Max-Age: 86400\r\nX-Content-Type-Options: nosniff\r\n\
         Connection: close\r\n\r\n",
        r.status,
        reason(r.status),
        r.ctype,
        r.body.len()
    );
    w.write_all(head.as_bytes())?;
    if !head_only {
        w.write_all(&r.body)?;
    }
    w.flush()
}

/// Reads one request from `rd` and answers it on `wr`. A body over the cap is refused before it is read.
pub fn serve<R: Read, W: Write>(rd: R, wr: &mut W, app: &App, now: u64) -> io::Result<()> {
    let mut rd = BufReader::new(rd);
    let mut lines = Vec::new();
    let mut seen = 0;
    loop {
        let mut l = String::new();
        let n = (&mut rd).take((HEAD_MAX - seen + 1) as u64).read_line(&mut l)?;
        seen += n;
        if n == 0 {
            return Ok(());
        }
        if seen > HEAD_MAX {
            return write_resp(wr, &err(431, "headers too large"), false);
        }
        let l = l.trim_end().to_string();
        if l.is_empty() {
            break;
        }
        lines.push(l);
    }
    let Some(first) = lines.first() else { return Ok(()) };
    let mut it = first.split_whitespace();
    let (method, target) = (it.next().unwrap_or("").to_string(), it.next().unwrap_or("/").to_string());
    let headers: Vec<(String, String)> = lines[1..].iter().filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))).collect();
    let header = |name: &str| headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone());
    let mut body = Vec::new();
    if header("transfer-encoding").is_some_and(|v| v.to_ascii_lowercase().contains("chunked")) {
        loop {
            let mut l = String::new();
            (&mut rd).take(64).read_line(&mut l)?;
            let size = usize::from_str_radix(l.trim().split(';').next().unwrap_or(""), 16).map_err(|_| io::Error::other("bad chunk"))?;
            if size == 0 {
                break;
            }
            if body.len() + size > BODY_MAX {
                return write_resp(wr, &err(413, "message too large"), false);
            }
            let start = body.len();
            body.resize(start + size, 0);
            rd.read_exact(&mut body[start..])?;
            let mut crlf = [0u8; 2];
            rd.read_exact(&mut crlf)?;
        }
    } else if let Some(len) = header("content-length") {
        let len: usize = len.parse().map_err(|_| io::Error::other("bad length"))?;
        if len > BODY_MAX {
            return write_resp(wr, &err(413, "message too large"), false);
        }
        body.resize(len, 0);
        rd.read_exact(&mut body)?;
    }
    let r = route(app, &Req { method: &method, target: &target, headers: &headers, body: &body }, now);
    write_resp(wr, &r, method == "HEAD")
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

fn env_path(var: &str, default: PathBuf) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or(default)
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("publish") {
        return publish::main();
    }
    let state = env_path("XDG_STATE_HOME", home().join(".local/state")).join("lesson-server");
    let db_path = env_path("LESSON_DB", state.join("lesson.db"));
    let board_path = env_path("LESSON_BOARD", state.join("board.log"));
    let key_path = env_path("LESSON_TEACHER_KEY", home().join(".config/lesson-server/teacher.key"));
    let www = env_path("LESSON_WWW", home().join(".local/share/lesson-server/www"));
    let key = teacher_key(&key_path).unwrap_or_else(|e| panic!("teacher key {}: {e}", key_path.display()));
    // A missing file is Access off; a file that is there but broken refuses to start (load says why).
    let access_path = env_path("LESSON_ACCESS", home().join(".config/lesson-server/access.json"));
    let access = access::Access::load(&access_path).unwrap_or_else(|e| panic!("access config {e}"));
    let teacher_line = match &access {
        Some(a) => format!("teacher: Cloudflare Access on ({}, {}), the teacher key refused", a.team_url, a.email),
        None => format!("teacher: Cloudflare Access off (no {}), the teacher key", access_path.display()),
    };
    let db = Db::open(&db_path).unwrap_or_else(|e| panic!("database {}: {e}", db_path.display()));
    let app = Arc::new(App::new(db, Store::open(Some(board_path), now_s()), key, Some(www.clone())).with_access(access));
    // LESSON_PORT lets a test server run beside the live one on PORT.
    let port: u16 = std::env::var("LESSON_PORT").ok().map(|p| p.parse().unwrap_or_else(|_| panic!("LESSON_PORT {p:?} is not a port"))).unwrap_or(PORT);
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    eprintln!("lesson-server on {addr}: records in {}, pages from {}; {teacher_line}", db_path.display(), www.display());
    run(listener, app);
}

/// Answers every connection on `listener`, each on its own thread, at most CONN_MAX at once.
pub fn run(listener: TcpListener, app: Arc<App>) {
    let open = Arc::new(AtomicUsize::new(0));
    for conn in listener.incoming() {
        let Ok(mut s) = conn else { continue };
        if open.load(Ordering::Relaxed) >= CONN_MAX {
            let _ = write_resp(&mut s, &err(503, "busy"), false);
            continue;
        }
        let (app, open) = (app.clone(), open.clone());
        open.fetch_add(1, Ordering::Relaxed);
        std::thread::spawn(move || {
            let _ = s.set_read_timeout(Some(IO_TIMEOUT));
            let _ = s.set_write_timeout(Some(IO_TIMEOUT));
            if let Ok(rd) = s.try_clone() {
                let _ = serve(rd, &mut s, &app, now_s());
            }
            open.fetch_sub(1, Ordering::Relaxed);
        });
    }
}

#[cfg(test)]
mod tests;
