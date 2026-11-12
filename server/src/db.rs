//! The kids' records in SQLite: classes, students, the words each kid knows, best race scores and sign-in sessions.
//! Only what a lesson needs: a display name (first name plus family initial, or a nickname), the secret picture's
//! number, a level, word results and race scores. No email, birth date, full name or address of any kind
//! (research/student-signin.md, section 5).

use crate::{CLASS_CODE_DIGITS, CLASS_MAX_KIDS, GRID_SIZE, JOIN_OPEN_S, LOCK_AFTER_WRONG, PAUSES_S, SESSION_S};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::path::Path;

pub struct Db {
    pub conn: Connection,
}

/// What a sign-in try gets.
#[derive(Debug, PartialEq)]
pub enum SignIn {
    /// Right picture: a session token.
    Ok(String),
    /// Wrong picture; this many tries are left before the next pause.
    Wrong { tries_left: u32 },
    /// Paused: wait this many seconds (the picture was not even looked at).
    Paused { wait_s: u64 },
    /// Too many wrong tries: only the teacher can open it again.
    Locked,
    /// No picture chosen yet: the kid picks one first.
    NoPicture,
    NoSuchKid,
}

/// Why a new name could not join a class.
#[derive(Debug, PartialEq)]
pub enum JoinErr {
    NoClass,
    /// The teacher has not opened joining, or its time ran out.
    Closed,
    /// The name is in the class already (any case).
    Taken,
    /// The class holds CLASS_MAX_KIDS, waiting names counted.
    Full,
}

/// Where a joining laptop's ticket stands.
#[derive(Debug, PartialEq)]
pub enum JoinStatus {
    Pending,
    Approved { id: i64, name: String, picked: bool },
}

/// A fresh random hex string of `bytes` bytes, from the kernel's random source.
pub fn random_hex(bytes: usize) -> String {
    use std::io::Read;
    let mut b = vec![0u8; bytes];
    std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b)).expect("read /dev/urandom");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn random_code() -> String {
    // Each byte maps to a digit with a bias under 3% (256 = 25*10 + 6); fine for a code that only finds a roster.
    let hex = random_hex(CLASS_CODE_DIGITS);
    (0..CLASS_CODE_DIGITS).map(|i| char::from(b'0' + (u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap() % 10))).collect()
}

/// The pause that starts at the `wrong`-th wrong try in a row, if one does.
pub fn pause_after(wrong: u32) -> Option<u64> {
    PAUSES_S.iter().find(|(at, _)| *at == wrong).map(|(_, s)| *s)
}

/// Wrong tries left before the next pause or the lock.
fn tries_left(wrong: u32) -> u32 {
    PAUSES_S.iter().map(|(at, _)| *at).chain([LOCK_AFTER_WRONG]).filter(|at| *at > wrong).min().unwrap_or(0) - wrong
}

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Db> {
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        Db::init(Connection::open(path)?)
    }

    pub fn memory() -> Db {
        Db::init(Connection::open_in_memory().unwrap()).unwrap()
    }

    fn init(conn: Connection) -> rusqlite::Result<Db> {
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS classes (
               id INTEGER PRIMARY KEY, code TEXT NOT NULL UNIQUE, name TEXT NOT NULL, created INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS students (
               id INTEGER PRIMARY KEY, class_id INTEGER NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
               name TEXT NOT NULL, picture INTEGER, level TEXT,
               wrong INTEGER NOT NULL DEFAULT 0, paused_until INTEGER NOT NULL DEFAULT 0,
               locked INTEGER NOT NULL DEFAULT 0, UNIQUE(class_id, name));
             CREATE TABLE IF NOT EXISTS words (
               student_id INTEGER NOT NULL REFERENCES students(id) ON DELETE CASCADE, word TEXT NOT NULL,
               right_count INTEGER NOT NULL DEFAULT 0, tries INTEGER NOT NULL DEFAULT 0,
               last_right INTEGER NOT NULL, last_time INTEGER NOT NULL, PRIMARY KEY(student_id, word));
             CREATE TABLE IF NOT EXISTS races (
               student_id INTEGER NOT NULL REFERENCES students(id) ON DELETE CASCADE, board TEXT NOT NULL,
               best INTEGER NOT NULL, PRIMARY KEY(student_id, board));
             CREATE TABLE IF NOT EXISTS sessions (
               token TEXT PRIMARY KEY, student_id INTEGER NOT NULL REFERENCES students(id) ON DELETE CASCADE,
               expires INTEGER NOT NULL);",
        )?;
        // A database made before kids could join themselves gains the new columns; its kids stay approved.
        let has = |table: &str, col: &str| -> rusqlite::Result<bool> {
            let mut st = conn.prepare(&format!("PRAGMA table_info({table})"))?;
            let cols: Vec<String> = st.query_map([], |r| r.get::<_, String>(1))?.collect::<Result<_, _>>()?;
            Ok(cols.iter().any(|c| c == col))
        };
        for (table, col, def) in [
            ("students", "approved", "INTEGER NOT NULL DEFAULT 1"),
            ("students", "ticket", "TEXT"),
            ("classes", "join_until", "INTEGER NOT NULL DEFAULT 0"),
        ] {
            if !has(table, col)? {
                conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {col} {def}"))?;
            }
        }
        Ok(Db { conn })
    }

    /// A new class with a fresh code nobody else holds; the names become its roster.
    pub fn create_class(&self, name: &str, kids: &[String], now: u64) -> rusqlite::Result<(i64, String)> {
        let code = loop {
            let c = random_code();
            let taken: bool = self.conn.query_row("SELECT EXISTS(SELECT 1 FROM classes WHERE code=?1)", [&c], |r| r.get(0))?;
            if !taken {
                break c;
            }
        };
        self.conn.execute("INSERT INTO classes(code, name, created) VALUES(?1, ?2, ?3)", params![code, name, now as i64])?;
        let id = self.conn.last_insert_rowid();
        for k in kids {
            self.add_student(id, k)?;
        }
        Ok((id, code))
    }

    pub fn add_student(&self, class_id: i64, name: &str) -> rusqlite::Result<i64> {
        self.conn.execute("INSERT OR IGNORE INTO students(class_id, name) VALUES(?1, ?2)", params![class_id, name])?;
        self.conn.query_row("SELECT id FROM students WHERE class_id=?1 AND name=?2", params![class_id, name], |r| r.get(0))
    }

    pub fn class_id(&self, code: &str) -> Option<i64> {
        self.conn.query_row("SELECT id FROM classes WHERE code=?1", [code], |r| r.get(0)).optional().ok().flatten()
    }

    /// Deletes the class and, with it, every record of its kids.
    pub fn delete_class(&self, code: &str) -> bool {
        self.conn.execute("DELETE FROM classes WHERE code=?1", [code]).is_ok_and(|n| n > 0)
    }

    pub fn classes(&self) -> Value {
        let mut st = self.conn.prepare("SELECT c.code, c.name, (SELECT COUNT(*) FROM students s WHERE s.class_id=c.id AND s.approved=1) FROM classes c ORDER BY c.id").unwrap();
        let rows = st.query_map([], |r| Ok(json!({"code": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "kids": r.get::<_, i64>(2)?})));
        Value::Array(rows.map(|it| it.filter_map(Result::ok).collect()).unwrap_or_default())
    }

    /// The class's approved names only, A-Z, whether each kid has picked a picture yet, and whether joining is
    /// open. Never the picture; never a name still waiting for the teacher.
    pub fn roster(&self, code: &str, now: u64) -> Option<Value> {
        let (id, name, until): (i64, String, i64) =
            self.conn.query_row("SELECT id, name, join_until FROM classes WHERE code=?1", [code], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional().ok()??;
        let mut st = self.conn.prepare("SELECT id, name, picture IS NOT NULL FROM students WHERE class_id=?1 AND approved=1 ORDER BY name COLLATE NOCASE").ok()?;
        let kids: Vec<Value> = st
            .query_map([id], |r| Ok(json!({"id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?, "picked": r.get::<_, bool>(2)?})))
            .ok()?
            .filter_map(Result::ok)
            .collect();
        Some(json!({"class": name, "grid": GRID_SIZE, "students": kids, "join_open": until.max(0) as u64 > now}))
    }

    /// Teacher: the class's name, join_s (seconds of joining left, 0 when closed) and, per kid (names waiting for
    /// approval first, then A-Z), id, name, status (pending or approved), picked, locked and wait_s (seconds left of
    /// a pause, 0 when none). Never the picture.
    pub fn students(&self, code: &str, now: u64) -> Option<Value> {
        let (id, name, join_until): (i64, String, i64) =
            self.conn.query_row("SELECT id, name, join_until FROM classes WHERE code=?1", [code], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional().ok()??;
        let mut st = self
            .conn
            .prepare("SELECT id, name, picture IS NOT NULL, locked, paused_until, approved FROM students WHERE class_id=?1 ORDER BY approved, name COLLATE NOCASE")
            .ok()?;
        let kids: Vec<Value> = st
            .query_map([id], |r| {
                let until = r.get::<_, i64>(4)?.max(0) as u64;
                let status = if r.get::<_, bool>(5)? { "approved" } else { "pending" };
                Ok(json!({"id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?, "status": status, "picked": r.get::<_, bool>(2)?,
                          "locked": r.get::<_, bool>(3)?, "wait_s": until.saturating_sub(now)}))
            })
            .ok()?
            .filter_map(Result::ok)
            .collect();
        Some(json!({"code": code, "class": name, "join_s": (join_until.max(0) as u64).saturating_sub(now), "students": kids}))
    }

    /// Teacher: opens joining for JOIN_OPEN_S from now, or closes it. False when there is no such class.
    pub fn open_join(&self, code: &str, open: bool, now: u64) -> bool {
        let until = if open { now + JOIN_OPEN_S } else { 0 };
        self.conn.execute("UPDATE classes SET join_until=?1 WHERE code=?2", params![until as i64, code]).unwrap_or(0) > 0
    }

    /// A new name asks to join: it waits for the teacher, and the laptop polls with the ticket it gets back.
    pub fn join(&self, code: &str, name: &str, now: u64) -> Result<(i64, String), JoinErr> {
        let (cid, until): (i64, i64) = self
            .conn
            .query_row("SELECT id, join_until FROM classes WHERE code=?1", [code], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()
            .ok()
            .flatten()
            .ok_or(JoinErr::NoClass)?;
        if until.max(0) as u64 <= now {
            return Err(JoinErr::Closed);
        }
        let names: Vec<String> = {
            let mut st = self.conn.prepare("SELECT name FROM students WHERE class_id=?1").map_err(|_| JoinErr::NoClass)?;
            let rows = st.query_map([cid], |r| r.get::<_, String>(0)).map_err(|_| JoinErr::NoClass)?;
            rows.filter_map(Result::ok).collect()
        };
        // Case-insensitive for Hebrew and English alike (SQLite's NOCASE folds ASCII only).
        if names.iter().any(|n| n.to_lowercase() == name.to_lowercase()) {
            return Err(JoinErr::Taken);
        }
        if names.len() >= CLASS_MAX_KIDS as usize {
            return Err(JoinErr::Full);
        }
        // 16 bytes, like a session token: the ticket is what a laptop shows to learn it was approved.
        let ticket = random_hex(16);
        self.conn
            .execute("INSERT INTO students(class_id, name, approved, ticket) VALUES(?1, ?2, 0, ?3)", params![cid, name, ticket])
            .map_err(|_| JoinErr::Taken)?;
        Ok((self.conn.last_insert_rowid(), ticket))
    }

    /// Where a ticket stands; None once the teacher rejected it (the row is gone) or for an unknown ticket.
    pub fn join_status(&self, ticket: &str) -> Option<JoinStatus> {
        let (id, name, picked, approved): (i64, String, bool, bool) = self
            .conn
            .query_row("SELECT id, name, picture IS NOT NULL, approved FROM students WHERE ticket=?1", [ticket], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .optional()
            .ok()??;
        Some(if approved { JoinStatus::Approved { id, name, picked } } else { JoinStatus::Pending })
    }

    /// Teacher: the waiting name becomes the kid's name from now on.
    pub fn approve(&self, kid: i64) -> bool {
        self.conn.execute("UPDATE students SET approved=1 WHERE id=?1", [kid]).unwrap_or(0) > 0
    }

    /// Teacher: a waiting name is dropped. Only a waiting one: an approved kid's record is never deleted here.
    pub fn reject(&self, kid: i64) -> bool {
        self.conn.execute("DELETE FROM students WHERE id=?1 AND approved=0", [kid]).unwrap_or(0) > 0
    }

    /// The kid's id, only when they are in the class with this code and the teacher has approved their name.
    fn kid_in(&self, code: &str, kid: i64) -> Option<i64> {
        self.conn
            .query_row("SELECT s.id FROM students s JOIN classes c ON c.id=s.class_id WHERE c.code=?1 AND s.id=?2 AND s.approved=1", params![code, kid], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    }

    fn new_session(&self, kid: i64, now: u64) -> String {
        // 16 bytes is 128 bits, above the 112 bits NIST SP 800-63B asks of a session secret.
        let token = random_hex(16);
        let _ = self.conn.execute("DELETE FROM sessions WHERE expires < ?1", [now as i64]);
        let _ = self.conn.execute("INSERT INTO sessions(token, student_id, expires) VALUES(?1, ?2, ?3)", params![token, kid, (now + SESSION_S) as i64]);
        token
    }

    /// First sign-in: the kid picks their secret picture and is signed in. Refused once a picture is set.
    pub fn first_pick(&self, code: &str, kid: i64, picture: u32, now: u64) -> Result<String, &'static str> {
        if picture >= GRID_SIZE {
            return Err("no such picture");
        }
        let kid = self.kid_in(code, kid).ok_or("no such kid")?;
        let n = self.conn.execute("UPDATE students SET picture=?1, wrong=0, paused_until=0, locked=0 WHERE id=?2 AND picture IS NULL", params![picture, kid]).map_err(|_| "db")?;
        if n == 0 {
            return Err("already picked");
        }
        Ok(self.new_session(kid, now))
    }

    pub fn sign_in(&self, code: &str, kid: i64, picture: u32, now: u64) -> SignIn {
        let Some(kid) = self.kid_in(code, kid) else { return SignIn::NoSuchKid };
        let (pic, wrong, paused_until, locked): (Option<u32>, u32, u64, bool) = match self
            .conn
            .query_row("SELECT picture, wrong, paused_until, locked FROM students WHERE id=?1", [kid], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? as u64, r.get(3)?)))
        {
            Ok(v) => v,
            Err(_) => return SignIn::NoSuchKid,
        };
        if locked {
            return SignIn::Locked;
        }
        if paused_until > now {
            return SignIn::Paused { wait_s: paused_until - now };
        }
        let Some(pic) = pic else { return SignIn::NoPicture };
        if pic == picture {
            let _ = self.conn.execute("UPDATE students SET wrong=0, paused_until=0 WHERE id=?1", [kid]);
            return SignIn::Ok(self.new_session(kid, now));
        }
        let wrong = wrong + 1;
        let locked = wrong >= LOCK_AFTER_WRONG;
        let until = pause_after(wrong).map(|p| now + p).unwrap_or(0);
        let _ = self.conn.execute("UPDATE students SET wrong=?1, paused_until=?2, locked=?3 WHERE id=?4", params![wrong, until as i64, locked, kid]);
        if locked {
            return SignIn::Locked;
        }
        if until > 0 {
            return SignIn::Paused { wait_s: until - now };
        }
        SignIn::Wrong { tries_left: tries_left(wrong) }
    }

    /// Teacher: clears the kid's picture (they pick a new one at their next sign-in) and any pause or lock, and
    /// signs them out everywhere. Their words and scores stay.
    pub fn reset(&self, kid: i64) -> bool {
        let n = self.conn.execute("UPDATE students SET picture=NULL, wrong=0, paused_until=0, locked=0 WHERE id=?1", [kid]).unwrap_or(0);
        let _ = self.conn.execute("DELETE FROM sessions WHERE student_id=?1", [kid]);
        n > 0
    }

    /// Teacher: opens a paused or locked record again; the picture stays.
    pub fn unlock(&self, kid: i64) -> bool {
        self.conn.execute("UPDATE students SET wrong=0, paused_until=0, locked=0 WHERE id=?1", [kid]).unwrap_or(0) > 0
    }

    pub fn session(&self, token: &str, now: u64) -> Option<i64> {
        self.conn.query_row("SELECT student_id FROM sessions WHERE token=?1 AND expires>?2", params![token, now as i64], |r| r.get(0)).optional().ok().flatten()
    }

    /// The kid's own record: name, class, level, words and best race scores. Never the picture.
    pub fn record(&self, kid: i64) -> Option<Value> {
        let (name, class, level): (String, String, Option<String>) = self
            .conn
            .query_row("SELECT s.name, c.name, s.level FROM students s JOIN classes c ON c.id=s.class_id WHERE s.id=?1", [kid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()
            .ok()??;
        let mut words = serde_json::Map::new();
        let mut st = self.conn.prepare("SELECT word, right_count, tries, last_right, last_time FROM words WHERE student_id=?1 ORDER BY word").ok()?;
        for w in st
            .query_map([kid], |r| Ok((r.get::<_, String>(0)?, json!({"right": r.get::<_, i64>(1)?, "tries": r.get::<_, i64>(2)?, "last_right": r.get::<_, bool>(3)?, "last_time": r.get::<_, i64>(4)?}))))
            .ok()?
            .flatten()
        {
            words.insert(w.0, w.1);
        }
        let mut races = serde_json::Map::new();
        let mut st = self.conn.prepare("SELECT board, best FROM races WHERE student_id=?1").ok()?;
        for r in st.query_map([kid], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))).ok()?.flatten() {
            races.insert(r.0, json!(r.1));
        }
        Some(json!({"id": kid, "name": name, "class": class, "level": level, "words": words, "races": races}))
    }

    /// The kid's approved name, the only name the race boards show.
    pub fn name(&self, kid: i64) -> Option<String> {
        self.conn.query_row("SELECT name FROM students WHERE id=?1 AND approved=1", [kid], |r| r.get(0)).optional().ok().flatten()
    }

    pub fn set_level(&self, kid: i64, level: &str) {
        let _ = self.conn.execute("UPDATE students SET level=?1 WHERE id=?2", params![level, kid]);
    }

    pub fn word_result(&self, kid: i64, word: &str, right: bool, now: u64) {
        let _ = self.conn.execute(
            "INSERT INTO words(student_id, word, right_count, tries, last_right, last_time) VALUES(?1, ?2, ?3, 1, ?3, ?4)
             ON CONFLICT(student_id, word) DO UPDATE SET right_count=right_count+?3, tries=tries+1, last_right=?3, last_time=?4",
            params![kid, word, right as i64, now as i64],
        );
    }

    /// Keeps the higher of the kid's best and this score.
    pub fn race_score(&self, kid: i64, board: &str, score: i64) {
        let _ = self.conn.execute(
            "INSERT INTO races(student_id, board, best) VALUES(?1, ?2, ?3)
             ON CONFLICT(student_id, board) DO UPDATE SET best=MAX(best, ?3)",
            params![kid, board, score],
        );
    }
}
