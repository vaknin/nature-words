//! The race boards' own server. It speaks the small part of ntfy.sh the lesson page uses, so the page reads either
//! one the same way:
//!   POST /TOPIC                      stores the body (one score or a Reset marker) with the time it came
//!   GET  /TOPIC/json?poll=1&since=12h one JSON line per message: {"id","time","event":"message","topic","message"}
//! CORS is open (the page lives on GitHub Pages, another site). Messages live 12 hours and are kept in a file, so a
//! restart keeps the boards. Plain std, no crates: one small job, nothing to update.

use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 12 hours, one school day: the same as the ntfy.sh fallback keeps, so the boards start empty each morning.
pub const RETENTION_S: u64 = 12 * 3600;
/// ntfy.sh's own message limit (4096 bytes), so anything the fallback takes this takes too. A score is about 60 bytes.
pub const BODY_MAX: usize = 4096;
/// A request's line and headers: the page's and curl's are under 1 KB, cloudflared adds about 1 KB of its own.
const HEAD_MAX: usize = 16 * 1024;
/// Most messages kept, oldest dropped first. 30 laptops each posting a score every 30 s race for a full 12 hours is
/// 43,200; this is over twice that, and at the body cap still bounds the file at about 400 MB, so a flood cannot fill
/// the disk.
pub const KEEP_MAX: usize = 100_000;
/// Connections served at once: a class is 30 laptops, each with at most a read and a post open (60); the rest is room.
const CONN_MAX: usize = 256;
/// The page waits 4 s for an answer; a request still sending after 10 s has nobody waiting for it.
const IO_TIMEOUT: Duration = Duration::from_secs(10);

pub struct Msg {
    pub id: String,
    pub time: u64,
    pub topic: String,
    /// The body, already escaped for a JSON string (no quotes around it).
    pub esc: String,
}

impl Msg {
    fn json(&self) -> String {
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

pub struct Resp {
    pub status: u16,
    pub ctype: &'static str,
    pub body: String,
}

fn resp(status: u16, body: &str) -> Resp {
    Resp { status, ctype: "application/json", body: format!("{{\"code\":{status},\"error\":\"{}\"}}\n", json_escape(body)) }
}

/// What one request gets: no sockets here, so the tests drive it straight.
pub fn route(store: &Mutex<Store>, method: &str, target: &str, body: &[u8], now: u64) -> Resp {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    match (method, parts.as_slice()) {
        ("OPTIONS", _) => Resp { status: 204, ctype: "text/plain", body: String::new() },
        ("GET" | "HEAD", [""] | ["v1", "health"]) => Resp { status: 200, ctype: "application/json", body: "{\"healthy\":true}\n".into() },
        ("POST" | "PUT", [topic]) if valid_topic(topic) => {
            if body.is_empty() {
                return resp(400, "empty message");
            }
            if body.len() > BODY_MAX {
                return resp(413, "message too large");
            }
            let text = String::from_utf8_lossy(body);
            let mut s = store.lock().unwrap_or_else(|e| e.into_inner());
            Resp { status: 200, ctype: "application/json", body: s.post(topic, &text, now).json() + "\n" }
        }
        ("GET" | "HEAD", [topic, "json"]) if valid_topic(topic) => {
            let since = query.split('&').find_map(|kv| kv.strip_prefix("since=")).unwrap_or("all");
            let Some(since) = parse_since(since, now) else { return resp(400, "bad since") };
            let mut s = store.lock().unwrap_or_else(|e| e.into_inner());
            Resp { status: 200, ctype: "application/x-ndjson", body: s.read(topic, since, now) }
        }
        _ => resp(404, "not found"),
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

pub fn write_resp<W: Write>(w: &mut W, r: &Resp, head_only: bool) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, PUT, OPTIONS\r\n\
         Access-Control-Allow-Headers: *\r\nAccess-Control-Max-Age: 86400\r\nConnection: close\r\n\r\n",
        r.status,
        reason(r.status),
        r.ctype,
        r.body.len()
    );
    w.write_all(head.as_bytes())?;
    if !head_only {
        w.write_all(r.body.as_bytes())?;
    }
    w.flush()
}

/// Reads one request from `rd` and answers it on `wr`. A body over the cap is refused before it is read.
pub fn serve<R: Read, W: Write>(rd: R, wr: &mut W, store: &Mutex<Store>, now: u64) -> io::Result<()> {
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
            return write_resp(wr, &resp(431, "headers too large"), false);
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
    let header = |name: &str| {
        lines[1..].iter().find_map(|l| l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case(name)).map(|(_, v)| v.trim().to_string()))
    };
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
                return write_resp(wr, &resp(413, "message too large"), false);
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
            return write_resp(wr, &resp(413, "message too large"), false);
        }
        body.resize(len, 0);
        rd.read_exact(&mut body)?;
    }
    let r = route(store, &method, &target, &body, now);
    write_resp(wr, &r, method == "HEAD")
}

fn main() {
    let addr = std::env::var("BOARD_ADDR").unwrap_or_else(|_| "127.0.0.1:8797".into());
    let file = std::env::var_os("BOARD_FILE").map(PathBuf::from).unwrap_or_else(|| {
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"));
        state.join("nature-board/board.log")
    });
    let store = Arc::new(Mutex::new(Store::open(Some(file.clone()), now_s())));
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    eprintln!("nature-board on {addr}, kept in {}", file.display());
    let open = Arc::new(AtomicUsize::new(0));
    for conn in listener.incoming() {
        let Ok(mut s) = conn else { continue };
        if open.load(Ordering::Relaxed) >= CONN_MAX {
            let _ = write_resp(&mut s, &resp(503, "busy"), false);
            continue;
        }
        let (store, open) = (store.clone(), open.clone());
        open.fetch_add(1, Ordering::Relaxed);
        std::thread::spawn(move || {
            let _ = s.set_read_timeout(Some(IO_TIMEOUT));
            let _ = s.set_write_timeout(Some(IO_TIMEOUT));
            if let Ok(rd) = s.try_clone() {
                let _ = serve(rd, &mut s, &store, now_s());
            }
            open.fetch_sub(1, Ordering::Relaxed);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http(store: &Mutex<Store>, req: &str, now: u64) -> String {
        let mut out = Vec::new();
        serve(req.as_bytes(), &mut out, store, now).unwrap();
        String::from_utf8(out).unwrap()
    }
    fn post(topic: &str, body: &str) -> String {
        format!("POST /{topic} HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{body}", body.len())
    }
    fn get(topic: &str, since: &str) -> String {
        format!("GET /{topic}/json?poll=1&since={since} HTTP/1.1\r\nHost: x\r\n\r\n")
    }
    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nature-board-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d.join("board.log")
    }
    const NOW: u64 = 1_800_000_000;

    #[test]
    fn post_then_read_back() {
        let st = Mutex::new(Store::open(None, NOW));
        let score = r#"{"n":"Noa","s":12,"p":1,"t":1}"#;
        assert!(http(&st, &post("nw-a", score), NOW).starts_with("HTTP/1.1 200"));
        let out = http(&st, &get("nw-a", "12h"), NOW + 5);
        let line = out.split("\r\n\r\n").nth(1).unwrap().trim();
        // The exact shape the page reads from ntfy: event "message", the time, and the body as a JSON string.
        assert_eq!(line, format!(r#"{{"id":"{:x}0001","time":{NOW},"event":"message","topic":"nw-a","message":"{}"}}"#, NOW, json_escape(score)));
        // Another topic sees nothing of it.
        assert!(http(&st, &get("nw-b", "12h"), NOW).ends_with("\r\n\r\n"));
    }

    #[test]
    fn since_filters_old_entries() {
        let st = Mutex::new(Store::open(None, NOW));
        http(&st, &post("nw-a", "old"), NOW - 3 * 3600);
        http(&st, &post("nw-a", "new"), NOW - 60);
        let out = http(&st, &get("nw-a", "1h"), NOW);
        assert!(out.contains("\"new\"") && !out.contains("\"old\""), "{out}");
        assert!(http(&st, &get("nw-a", "12h"), NOW).contains("\"old\""));
        // Past 12 hours a message is gone even when asked for all.
        http(&st, &post("nw-a", "older"), NOW - RETENTION_S - 10);
        assert!(!http(&st, &get("nw-a", "all"), NOW).contains("older"));
        assert!(http(&st, &get("nw-a", "nonsense"), NOW).starts_with("HTTP/1.1 400"));
    }

    #[test]
    fn oversize_body_refused() {
        let st = Mutex::new(Store::open(None, NOW));
        let big = "x".repeat(BODY_MAX + 1);
        assert!(http(&st, &post("nw-a", &big), NOW).starts_with("HTTP/1.1 413"));
        let chunked = format!("POST /nw-a HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{big}\r\n0\r\n\r\n", big.len());
        assert!(http(&st, &chunked, NOW).starts_with("HTTP/1.1 413"));
        assert!(!http(&st, &get("nw-a", "all"), NOW).contains("xxx"));
        // Exactly at the cap still goes in.
        assert!(http(&st, &post("nw-a", &"y".repeat(BODY_MAX)), NOW).starts_with("HTTP/1.1 200"));
    }

    #[test]
    fn cors_header_present() {
        let st = Mutex::new(Store::open(None, NOW));
        for req in [post("nw-a", "{}"), get("nw-a", "12h"), "OPTIONS /nw-a HTTP/1.1\r\n\r\n".into(), "GET /nope/x/y HTTP/1.1\r\n\r\n".into()] {
            let out = http(&st, &req, NOW);
            assert!(out.contains("\r\nAccess-Control-Allow-Origin: *\r\n"), "no CORS on {req}: {out}");
        }
    }

    #[test]
    fn survives_restart_from_file() {
        let f = tmp("restart");
        {
            let st = Mutex::new(Store::open(Some(f.clone()), NOW));
            http(&st, &post("nw-a", "{\"n\":\"Omer\",\"s\":9}\ttab"), NOW - 10);
            http(&st, &post("nw-a", "stale"), NOW - RETENTION_S - 10);
        }
        let st = Mutex::new(Store::open(Some(f.clone()), NOW));
        let out = http(&st, &get("nw-a", "12h"), NOW);
        assert!(out.contains(r#"\"n\":\"Omer\""#) && out.contains("\\ttab"), "{out}");
        assert!(!out.contains("stale"));
        // The reopened file holds only the live message.
        assert_eq!(fs::read_to_string(&f).unwrap().lines().count(), 1);
        let _ = fs::remove_dir_all(f.parent().unwrap());
    }

    #[test]
    fn real_socket_round_trip() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let st = Arc::new(Mutex::new(Store::open(None, now_s())));
        let st2 = st.clone();
        std::thread::spawn(move || {
            for c in l.incoming().take(2) {
                let mut s = c.unwrap();
                let rd = s.try_clone().unwrap();
                serve(rd, &mut s, &st2, now_s()).unwrap();
            }
        });
        let ask = |req: String| {
            let mut c = std::net::TcpStream::connect(addr).unwrap();
            c.write_all(req.as_bytes()).unwrap();
            let mut out = String::new();
            c.read_to_string(&mut out).unwrap();
            out
        };
        assert!(ask(post("nw-s", "hello")).starts_with("HTTP/1.1 200"));
        assert!(ask(get("nw-s", "12h")).contains("\"message\":\"hello\""));
    }
}
