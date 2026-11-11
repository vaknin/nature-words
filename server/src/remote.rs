//! The phone remote for the projected lesson: the teacher walks around the class and moves the projector from his
//! phone, through our server (the projector laptop and the phone never talk to each other directly).
//!
//! The projector laptop is a school laptop that must never hold the teacher key, so it pairs kahoot style:
//!   POST /api/screens                      (no key) opens a screen: {token, code, seq}; the token stays on that page
//!   GET  /api/screens/TOKEN/cmds?after=SEQ (no key) a long poll: answers at once with every command newer than SEQ,
//!                                          else holds until one comes, the screen is paired or the hold ends; {cmds, seq, paired, code}
//!   POST /api/screens/TOKEN/state          (no key) what the projector shows: {stage, of, name, item, qr}
//! Behind the teacher key (the same gate as every teacher route):
//!   POST /api/screens/pair {code}          the six digits shown on the projector, once; returns the screen's id
//!   POST /api/screens/ID/cmd {cmd, stage?} cmd: next, prev, space, stage (1..=9) or qr
//!   GET  /api/screens/ID                   the last state the projector reported, and seconds since it last polled
//! Long poll, not Server-Sent Events: quick tunnels do not carry SSE (deploy/README.md). Memory only: no kid data,
//! nothing in SQLite; after a restart every token and id is unknown (404), so the projector opens a new screen and
//! the phone pairs again.

use crate::{db, CLASS_MAX_KIDS, CONN_MAX};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How long a projector's poll is held open when nothing happens: 50 s. Cloudflare closes a request whose origin
/// has not answered in 100 s (error 524, developers.cloudflare.com/support/troubleshooting/http-status-codes/
/// cloudflare-5xx-errors/error-524/); half of it leaves room for a slow tunnel. Measured through a fresh quick tunnel
/// (one run, five holds at once): 25, 50, 90, 99 and 110 s all came back 200, so a quick tunnel does not cut at 100 s,
/// but a named tunnel on our domain is held to it; 50 s stays under it either way.
pub const REMOTE_HOLD_S: u64 = 50;
/// A pairing code works for 10 minutes, then the projector shows a fresh one: long enough to read it off the wall
/// and type six digits on a phone (seconds, test/remote.mjs), short enough that a code photographed off the
/// projector is dead by the next lesson. It works once either way, and only with the teacher key.
pub const REMOTE_CODE_S: u64 = 10 * 60;
/// A screen whose projector has not polled for one lesson (45 min, SESSION_S) is dropped: a laptop that sleeps
/// for a while inside a lesson comes back to the same screen and the same paired phone.
pub const REMOTE_SCREEN_S: u64 = crate::SESSION_S;
/// The most screens open at once: each holds one connection while it waits, and a full class needs up to 80 of
/// the CONN_MAX connections (40 laptops, each a read and a post), so held polls never crowd out a class.
pub const REMOTE_SCREENS_MAX: usize = CONN_MAX - 2 * CLASS_MAX_KIDS as usize;
/// Commands kept per screen for a projector that is between polls: a poll round trip through the tunnel is under
/// 1 s (slowest seen in test/remote.mjs, printed at its end) and a finger taps at most about 7 times a second
/// (finger tapping test norms, 5 to 7 Hz), so 16 is twice what a fast teacher can tap between two polls.
/// A projector further behind than that gets the newest 16 only.
pub const REMOTE_CMDS_KEPT: usize = 16;
/// A screen id: 8 random bytes, so an id a phone kept from before a restart never names a new screen.
const REMOTE_ID_BYTES: usize = 8;
/// The screen token: 16 random bytes, the same as a kid's session token (db.rs).
const REMOTE_TOKEN_BYTES: usize = 16;
/// A stage name or item the projector reports: the lesson's longest stage name is 17 characters and items are
/// word ids, so 64 holds any lesson's with room; anything longer is cut, as nobody reads it on a phone.
const REMOTE_TEXT_MAX: usize = 64;
/// Stages 1 to 9: the projector's number keys, the same as the page's own 1-9 keys.
const REMOTE_STAGE_MAX: u64 = 9;

struct Screen {
    id: String,
    /// The pairing code and the time it stops working; None once paired.
    code: Option<(String, u64)>,
    paired: bool,
    seq: u64,
    cmds: VecDeque<Value>,
    state: Value,
    /// When the projector last polled, and how many of its polls are open now.
    polled: u64,
    held: u32,
}

pub struct Remote {
    screens: Mutex<HashMap<String, Screen>>,
    woke: Condvar,
    /// REMOTE_HOLD_S; the tests shorten it.
    pub hold: Duration,
}

pub enum Out {
    Ok(Value),
    NotFound,
    Bad(&'static str),
    Full,
}

fn fresh_code(screens: &HashMap<String, Screen>) -> String {
    loop {
        let c = db::random_code();
        if !screens.values().any(|s| s.code.as_ref().is_some_and(|(x, _)| *x == c)) {
            return c;
        }
    }
}

fn text(v: Option<&Value>) -> Value {
    match v.and_then(Value::as_str) {
        Some(s) => Value::String(s.chars().filter(|c| !c.is_control()).take(REMOTE_TEXT_MAX).collect()),
        None => match v.and_then(Value::as_u64) {
            Some(n) => Value::String(n.to_string()),
            None => Value::Null,
        },
    }
}

impl Remote {
    pub fn new() -> Remote {
        Remote { screens: Mutex::new(HashMap::new()), woke: Condvar::new(), hold: Duration::from_secs(REMOTE_HOLD_S) }
    }

    fn lock(&self, now: u64) -> MutexGuard<'_, HashMap<String, Screen>> {
        let mut g = self.screens.lock().unwrap_or_else(|e| e.into_inner());
        g.retain(|_, s| s.held > 0 || now < s.polled + REMOTE_SCREEN_S);
        g
    }

    /// A new screen for a projector page.
    pub fn open(&self, now: u64) -> Out {
        let mut g = self.lock(now);
        if g.len() >= REMOTE_SCREENS_MAX {
            // Room is made from screens no phone uses, the one that polled longest ago first; a paired one stays.
            let Some(old) = g.iter().filter(|(_, s)| !s.paired).min_by_key(|(_, s)| (s.held, s.polled)).map(|(t, _)| t.clone()) else {
                return Out::Full;
            };
            g.remove(&old);
            self.woke.notify_all();
        }
        let token = db::random_hex(REMOTE_TOKEN_BYTES);
        let code = fresh_code(&g);
        let s = Screen { id: db::random_hex(REMOTE_ID_BYTES), code: Some((code.clone(), now + REMOTE_CODE_S)), paired: false, seq: 0, cmds: VecDeque::new(), state: Value::Null, polled: now, held: 0 };
        g.insert(token.clone(), s);
        Out::Ok(json!({"token": token, "code": code, "seq": 0, "hold_s": self.hold.as_secs_f64()}))
    }

    /// The projector's long poll: commands after `after`, at once if any, else when one comes or the hold ends.
    pub fn poll(&self, token: &str, after: u64, now: u64) -> Out {
        let start = Instant::now();
        let deadline = start + self.hold;
        let mut g = self.lock(now);
        let mut counted = false;
        // A poll also answers when the screen gets paired, so the projector shows "Phone ✓" at once.
        let was_paired = g.get(token).map(|s| s.paired);
        loop {
            let t = now + start.elapsed().as_secs();
            let Some(s) = g.get(token) else { return Out::NotFound };
            if !s.paired && s.code.as_ref().is_none_or(|(_, until)| t >= *until) {
                let c = fresh_code(&g);
                g.get_mut(token).unwrap().code = Some((c, t + REMOTE_CODE_S));
            }
            let s = g.get_mut(token).unwrap();
            s.polled = t;
            let cmds: Vec<Value> = s.cmds.iter().filter(|c| c["seq"].as_u64().unwrap_or(0) > after).cloned().collect();
            let left = deadline.saturating_duration_since(Instant::now());
            if !cmds.is_empty() || left.is_zero() || was_paired != Some(s.paired) {
                if counted {
                    s.held -= 1;
                }
                let code = if s.paired { Value::Null } else { json!(s.code.as_ref().map(|(c, _)| c.clone())) };
                return Out::Ok(json!({"cmds": cmds, "seq": s.seq, "paired": s.paired, "code": code}));
            }
            if !counted {
                s.held += 1;
                counted = true;
            }
            g = self.woke.wait_timeout(g, left).unwrap_or_else(|e| e.into_inner()).0;
        }
    }

    /// What the projector shows now: {stage, of, name, item, qr}.
    pub fn report(&self, token: &str, body: &Value, now: u64) -> Out {
        let (Some(stage), Some(of)) = (body.get("stage").and_then(Value::as_u64), body.get("of").and_then(Value::as_u64)) else {
            return Out::Bad("needs stage and of");
        };
        let mut g = self.lock(now);
        let Some(s) = g.get_mut(token) else { return Out::NotFound };
        s.state = json!({"stage": stage, "of": of, "name": text(body.get("name")), "item": text(body.get("item")), "qr": body.get("qr").and_then(Value::as_bool).unwrap_or(false), "at": now});
        Out::Ok(json!({"ok": true}))
    }

    /// Teacher: links the phone to the screen showing `code`. A code works once and only until it expires.
    pub fn pair(&self, code: &str, now: u64) -> Out {
        let mut g = self.lock(now);
        let Some(s) = g.values_mut().find(|s| s.code.as_ref().is_some_and(|(c, until)| c == code && now < *until)) else {
            return Out::NotFound;
        };
        s.code = None;
        s.paired = true;
        let id = s.id.clone();
        self.woke.notify_all();
        Out::Ok(json!({"id": id}))
    }

    /// Teacher: one command to the paired screen `id`.
    pub fn command(&self, id: &str, body: &Value, now: u64) -> Out {
        let cmd = body.get("cmd").and_then(Value::as_str).unwrap_or("");
        let mut c = json!({"cmd": cmd});
        match cmd {
            "next" | "prev" | "space" | "qr" => {}
            "stage" => match body.get("stage").and_then(Value::as_u64) {
                Some(n) if (1..=REMOTE_STAGE_MAX).contains(&n) => c["stage"] = json!(n),
                _ => return Out::Bad("stage needs a number from 1 to 9"),
            },
            _ => return Out::Bad("cmd is next, prev, space, stage or qr"),
        }
        let mut g = self.lock(now);
        let Some(s) = g.values_mut().find(|s| s.paired && s.id == id) else { return Out::NotFound };
        s.seq += 1;
        c["seq"] = json!(s.seq);
        s.cmds.push_back(c);
        while s.cmds.len() > REMOTE_CMDS_KEPT {
            s.cmds.pop_front();
        }
        let seq = s.seq;
        self.woke.notify_all();
        Out::Ok(json!({"seq": seq}))
    }

    /// Teacher: what the paired screen `id` shows, and how long since its projector last polled (0 while one is open).
    pub fn status(&self, id: &str, now: u64) -> Out {
        let g = self.lock(now);
        let Some(s) = g.values().find(|s| s.paired && s.id == id) else { return Out::NotFound };
        let ago = if s.held > 0 { 0 } else { now.saturating_sub(s.polled) };
        Out::Ok(json!({"id": s.id, "state": s.state, "polled_s_ago": ago, "hold_s": self.hold.as_secs_f64()}))
    }
}
