//! The race boards, copied from the board server that hosted them first (its store and its tests, unchanged in
//! behaviour). It speaks the small part of ntfy.sh the lesson page uses, so the page reads either one the same way:
//!   POST /TOPIC                      stores the body (one score or a Reset marker) with the time it came
//!   GET  /TOPIC/json?poll=1&since=12h one JSON line per message: {"id","time","event":"message","topic","message"}
//! Polling only, no Server-Sent Events or long-lived streams: a Cloudflare quick tunnel carries neither.
//! Messages live 12 hours and are kept in a file, so a restart keeps the boards.

use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// 12 hours, one school day: the same as the ntfy.sh fallback keeps, so the boards start empty each morning.
pub const RETENTION_S: u64 = 12 * 3600;
/// ntfy.sh's own message limit (4096 bytes), so anything the fallback takes this takes too. A score is about 60 bytes.
pub const BODY_MAX: usize = 4096;
/// Most messages kept, oldest dropped first. 30 laptops each posting a score every 30 s race for a full 12 hours is
/// 43,200; this is over twice that, and at the body cap still bounds the file at about 400 MB, so a flood cannot fill
/// the disk.
pub const KEEP_MAX: usize = 100_000;

pub struct Msg {
    pub id: String,
    pub time: u64,
    pub topic: String,
    /// The body, already escaped for a JSON string (no quotes around it).
    pub esc: String,
}

impl Msg {
    pub fn json(&self) -> String {
        format!(
            "{{\"id\":\"{}\",\"time\":{},\"event\":\"message\",\"topic\":\"{}\",\"message\":\"{}\"}}",
            self.id, self.time, self.topic, self.esc
        )
    }
    /// One line of the file: time, id, topic and the escaped body, tab-separated (none of them holds a tab).
    fn line(&self) -> String {
        format!("{}\t{}\t{}\t{}\n", self.time, self.id, self.topic, self.esc)
    }
    fn parse(l: &str) -> Option<Msg> {
        let mut p = l.splitn(4, '\t');
        let time = p.next()?.parse().ok()?;
        let id = p.next()?.to_string();
        let topic = p.next()?.to_string();
        let esc = p.next()?.trim_end_matches(['\n', '\r']).to_string();
        valid_topic(&topic).then_some(Msg { id, time, topic, esc })
    }
}

pub struct Store {
    msgs: VecDeque<Msg>,
    path: Option<PathBuf>,
    file_lines: usize,
    seq: u64,
}

pub fn now_s() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// ntfy's topic rule: 1-64 of letters, digits, - and _.
pub fn valid_topic(t: &str) -> bool {
    !t.is_empty() && t.len() <= 64 && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn json_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

impl Store {
    /// Opens the store kept in `path` (None: memory only), dropping what is older than 12 hours.
    pub fn open(path: Option<PathBuf>, now: u64) -> Store {
        let mut msgs = VecDeque::new();
        if let Some(p) = &path {
            if let Ok(f) = File::open(p) {
                for l in BufReader::new(f).lines().map_while(Result::ok) {
                    if let Some(m) = Msg::parse(&l) {
                        msgs.push_back(m);
                    }
                }
            }
        }
        msgs.make_contiguous().sort_by_key(|m| m.time);
        let mut s = Store { msgs, path, file_lines: 0, seq: 0 };
        s.prune(now);
        s.compact();
        s
    }

    fn prune(&mut self, now: u64) {
        let cut = now.saturating_sub(RETENTION_S);
        while self.msgs.front().is_some_and(|m| m.time < cut) || self.msgs.len() > KEEP_MAX {
            self.msgs.pop_front();
        }
    }

    /// Rewrites the file with only the live messages (to a side file, then renamed over: a crash keeps the old one).
    fn compact(&mut self) {
        let Some(p) = &self.path else { return };
        if let Some(d) = p.parent() {
            let _ = fs::create_dir_all(d);
        }
        let tmp = p.with_extension("tmp");
        let body: String = self.msgs.iter().map(Msg::line).collect();
        if fs::write(&tmp, body).and_then(|_| fs::rename(&tmp, p)).is_ok() {
            self.file_lines = self.msgs.len();
        }
    }

    pub fn post(&mut self, topic: &str, body: &str, now: u64) -> &Msg {
        self.prune(now);
        self.seq += 1;
        let m = Msg { id: format!("{:x}{:04x}", now, self.seq & 0xffff), time: now, topic: topic.to_string(), esc: json_escape(body) };
        if let Some(p) = &self.path {
            let ok = OpenOptions::new().create(true).append(true).open(p).and_then(|mut f| f.write_all(m.line().as_bytes()));
            if ok.is_ok() {
                self.file_lines += 1;
            }
        }
        self.msgs.push_back(m);
        // The file only grows between rewrites; rewrite once dead lines outnumber the live ones.
        if self.file_lines > 2 * self.msgs.len() + 100 {
            self.compact();
        }
        self.msgs.back().unwrap()
    }

    /// The topic's messages at or after `since` (unix seconds), oldest first, as ntfy JSON lines.
    pub fn read(&mut self, topic: &str, since: u64, now: u64) -> String {
        self.prune(now);
        let since = since.max(now.saturating_sub(RETENTION_S));
        self.msgs.iter().filter(|m| m.topic == topic && m.time >= since).map(|m| m.json() + "\n").collect()
    }
}

/// ntfy's `since`: "all", a unix time, or a duration back from now (30s, 10m, 12h, 1d). None if it is none of those.
pub fn parse_since(v: &str, now: u64) -> Option<u64> {
    if v.is_empty() || v == "all" {
        return Some(0);
    }
    if let Ok(t) = v.parse::<u64>() {
        return Some(t);
    }
    let (n, unit) = v.split_at(v.len() - 1);
    let n: u64 = n.parse().ok()?;
    let mul = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86400,
        _ => return None,
    };
    Some(now.saturating_sub(n.saturating_mul(mul)))
}
