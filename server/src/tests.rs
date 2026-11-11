use super::*;
use std::fs;

const NOW: u64 = 1_800_000_000;
const KEY: &str = "teacher-secret";

fn app() -> App {
    App::new(Db::memory(), Store::open(None, NOW), KEY.into(), None)
}
fn http(app: &App, req: &str, now: u64) -> String {
    let mut out = Vec::new();
    serve(req.as_bytes(), &mut out, app, now).unwrap();
    String::from_utf8(out).unwrap()
}
fn body_of(out: &str) -> Value {
    serde_json::from_str(out.split("\r\n\r\n").nth(1).unwrap().trim()).unwrap_or(Value::Null)
}
fn req(method: &str, path: &str, headers: &str, body: &str) -> String {
    format!("{method} {path} HTTP/1.1\r\nHost: x\r\n{headers}Content-Length: {}\r\n\r\n{body}", body.len())
}
fn teacher(method: &str, path: &str, body: &str) -> String {
    req(method, path, &format!("X-Teacher-Key: {KEY}\r\n"), body)
}
fn post(topic: &str, body: &str) -> String {
    req("POST", &format!("/{topic}"), "", body)
}
fn get(topic: &str, since: &str) -> String {
    format!("GET /{topic}/json?poll=1&since={since} HTTP/1.1\r\nHost: x\r\n\r\n")
}
/// A class with Noa and Omer; returns the code and Noa's id.
fn class(a: &App) -> (String, i64) {
    let out = http(a, &teacher("POST", "/api/classes", r#"{"name":"5A","students":["Noa K.","Omer"]}"#), NOW);
    let code = body_of(&out)["code"].as_str().unwrap().to_string();
    let r = body_of(&http(a, &req("GET", &format!("/api/roster/{code}"), "", ""), NOW));
    let noa = r["students"].as_array().unwrap().iter().find(|s| s["name"] == "Noa K.").unwrap()["id"].as_i64().unwrap();
    (code, noa)
}
fn try_pic(a: &App, code: &str, kid: i64, pic: u32, now: u64, path: &str) -> String {
    http(a, &req("POST", path, "", &format!(r#"{{"code":"{code}","student":{kid},"picture":{pic}}}"#)), now)
}

#[test]
fn signin_right_picture() {
    let a = app();
    let (code, noa) = class(&a);
    // Before a pick, sign-in asks for one.
    assert!(try_pic(&a, &code, noa, 5, NOW, "/api/signin").starts_with("HTTP/1.1 409"));
    assert!(try_pic(&a, &code, noa, 5, NOW, "/api/pick").starts_with("HTTP/1.1 200"));
    // A second pick (a friend's) is refused.
    assert!(try_pic(&a, &code, noa, 6, NOW, "/api/pick").starts_with("HTTP/1.1 409"));
    let out = try_pic(&a, &code, noa, 5, NOW, "/api/signin");
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    let token = body_of(&out)["token"].as_str().unwrap().to_string();
    let auth = format!("Authorization: Bearer {token}\r\n");
    let me = http(&a, &req("POST", "/api/me", &auth, r#"{"level":"river","words":[{"word":"leaf","right":true}],"race":{"board":"nw-a","score":9}}"#), NOW);
    assert!(me.starts_with("HTTP/1.1 200"), "{me}");
    http(&a, &req("POST", "/api/me", &auth, r#"{"race":{"board":"nw-a","score":4}}"#), NOW);
    let me = body_of(&http(&a, &req("GET", "/api/me", &auth, ""), NOW));
    assert_eq!(me["name"], "Noa K.");
    assert_eq!(me["words"]["leaf"]["last_right"], true);
    assert_eq!(me["races"]["nw-a"], 9, "best score kept");
    assert!(me.get("picture").is_none());
    // The session ends with the lesson.
    assert!(http(&a, &req("GET", "/api/me", &auth, ""), NOW + SESSION_S + 1).starts_with("HTTP/1.1 401"));
    // A picture outside the grid is refused.
    let (_, omer) = (0, body_of(&http(&a, &req("GET", &format!("/api/roster/{code}"), "", ""), NOW))["students"][1]["id"].as_i64().unwrap());
    assert!(try_pic(&a, &code, omer, GRID_SIZE, NOW, "/api/pick").starts_with("HTTP/1.1 400"));
}

#[test]
fn signin_wrong_picture_pauses() {
    let a = app();
    let (code, noa) = class(&a);
    try_pic(&a, &code, noa, 5, NOW, "/api/pick");
    let (first_at, first_s) = PAUSES_S[0];
    for i in 1..first_at {
        let out = try_pic(&a, &code, noa, 6, NOW, "/api/signin");
        assert!(out.starts_with("HTTP/1.1 401"), "{out}");
        assert_eq!(body_of(&out)["tries_left"], first_at - i);
    }
    let out = try_pic(&a, &code, noa, 6, NOW, "/api/signin");
    assert!(out.starts_with("HTTP/1.1 429"), "{out}");
    assert_eq!(body_of(&out)["wait_s"], first_s);
    // While paused even the right picture is not looked at.
    assert!(try_pic(&a, &code, noa, 5, NOW + 1, "/api/signin").starts_with("HTTP/1.1 429"));
    // After the pause, the right picture works and clears the count.
    assert!(try_pic(&a, &code, noa, 5, NOW + first_s, "/api/signin").starts_with("HTTP/1.1 200"));
    // Wrong tries up to the lock: then only the teacher opens it.
    let mut t = NOW + first_s;
    let mut last = String::new();
    for _ in 0..LOCK_AFTER_WRONG {
        t += 3600;
        last = try_pic(&a, &code, noa, 6, t, "/api/signin");
    }
    assert!(last.starts_with("HTTP/1.1 423"), "{last}");
    assert!(try_pic(&a, &code, noa, 5, t + 86400, "/api/signin").starts_with("HTTP/1.1 423"));
    assert!(http(&a, &teacher("POST", &format!("/api/students/{noa}/unlock"), ""), t).starts_with("HTTP/1.1 200"));
    assert!(try_pic(&a, &code, noa, 5, t, "/api/signin").starts_with("HTTP/1.1 200"));
}

#[test]
fn roster_only_by_class_code() {
    let a = app();
    let (code, _) = class(&a);
    let r = body_of(&http(&a, &req("GET", &format!("/api/roster/{code}"), "", ""), NOW));
    assert_eq!(r["grid"], GRID_SIZE);
    assert_eq!(r["students"].as_array().unwrap().len(), 2);
    assert!(!r.to_string().contains("picture"));
    let wrong = if code == "000000" { "000001" } else { "000000" };
    assert!(http(&a, &req("GET", &format!("/api/roster/{wrong}"), "", ""), NOW).starts_with("HTTP/1.1 404"));
    // The class list needs the teacher key.
    assert!(http(&a, &req("GET", "/api/classes", "", ""), NOW).starts_with("HTTP/1.1 401"));
    // Walking codes from one address is stopped; another address is not.
    let ip = "CF-Connecting-IP: 203.0.113.9\r\n";
    for _ in 0..ROSTER_MISSES_PER_WINDOW {
        http(&a, &req("GET", &format!("/api/roster/{wrong}"), ip, ""), NOW);
    }
    assert!(http(&a, &req("GET", &format!("/api/roster/{code}"), ip, ""), NOW).starts_with("HTTP/1.1 429"));
    assert!(http(&a, &req("GET", &format!("/api/roster/{code}"), "", ""), NOW).starts_with("HTTP/1.1 200"));
    assert!(http(&a, &req("GET", &format!("/api/roster/{code}"), ip, ""), NOW + ROSTER_WINDOW_S).starts_with("HTTP/1.1 200"));
}

#[test]
fn teacher_reset_needs_key() {
    let a = app();
    let (code, noa) = class(&a);
    try_pic(&a, &code, noa, 5, NOW, "/api/pick");
    let path = format!("/api/students/{noa}/reset");
    assert!(http(&a, &req("POST", &path, "", ""), NOW).starts_with("HTTP/1.1 401"));
    assert!(http(&a, &req("POST", &path, "X-Teacher-Key: nope\r\n", ""), NOW).starts_with("HTTP/1.1 401"));
    assert!(http(&a, &req("POST", "/api/classes", "", r#"{"name":"x"}"#), NOW).starts_with("HTTP/1.1 401"));
    assert!(http(&a, &req("DELETE", &format!("/api/classes/{code}"), "", ""), NOW).starts_with("HTTP/1.1 401"));
    assert!(http(&a, &teacher("POST", &path, ""), NOW).starts_with("HTTP/1.1 200"));
    // After the reset the kid picks again.
    assert!(try_pic(&a, &code, noa, 7, NOW, "/api/pick").starts_with("HTTP/1.1 200"));
    assert!(try_pic(&a, &code, noa, 7, NOW, "/api/signin").starts_with("HTTP/1.1 200"));
    assert!(http(&a, &teacher("DELETE", &format!("/api/classes/{code}"), ""), NOW).starts_with("HTTP/1.1 200"));
    assert!(http(&a, &req("GET", &format!("/api/roster/{code}"), "", ""), NOW).starts_with("HTTP/1.1 404"));
    // The key file is made once, mode 600, and read back the same.
    use std::os::unix::fs::PermissionsExt;
    let d = std::env::temp_dir().join(format!("lesson-key-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    let k1 = teacher_key(&d.join("teacher.key")).unwrap();
    assert_eq!(fs::metadata(d.join("teacher.key")).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(teacher_key(&d.join("teacher.key")).unwrap(), k1);
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn race_board_post_and_read() {
    // The real server loop on a real socket; two laptops, each its own connection, as in class.
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let a = Arc::new(App::new(Db::memory(), Store::open(None, now_s()), KEY.into(), None));
    std::thread::spawn(move || run(l, a));
    let score = r#"{"n":"Noa","s":12,"p":1,"t":1}"#;
    // Laptop A posts its score.
    let mut laptop_a = std::net::TcpStream::connect(addr).unwrap();
    laptop_a.write_all(post("nw-a", score).as_bytes()).unwrap();
    let mut out_a = String::new();
    laptop_a.read_to_string(&mut out_a).unwrap();
    assert!(out_a.starts_with("HTTP/1.1 200"), "{out_a}");
    let id = body_of(&out_a)["id"].as_str().unwrap().to_string();
    // Laptop B, a separate connection, reads the board for the last 12 hours and finds A's score.
    let mut laptop_b = std::net::TcpStream::connect(addr).unwrap();
    laptop_b.write_all(get("nw-a", "12h").as_bytes()).unwrap();
    let mut out_b = String::new();
    laptop_b.read_to_string(&mut out_b).unwrap();
    assert!(out_b.starts_with("HTTP/1.1 200"), "{out_b}");
    assert!(out_b.contains("\r\nAccess-Control-Allow-Origin: *\r\n"));
    let line: Value = serde_json::from_str(out_b.split("\r\n\r\n").nth(1).unwrap().trim()).unwrap();
    assert_eq!(line["id"], id.as_str());
    assert_eq!(line["topic"], "nw-a");
    assert_eq!(line["event"], "message");
    assert_eq!(line["message"], score);
    // Another board stays empty.
    let mut other = std::net::TcpStream::connect(addr).unwrap();
    other.write_all(get("nw-b", "12h").as_bytes()).unwrap();
    let mut out = String::new();
    other.read_to_string(&mut out).unwrap();
    assert!(out.starts_with("HTTP/1.1 200") && out.ends_with("\r\n\r\n"), "{out}");
}

#[test]
fn since_filters_old_entries() {
    let a = app();
    http(&a, &post("nw-a", "old"), NOW - 3 * 3600);
    http(&a, &post("nw-a", "new"), NOW - 60);
    let out = http(&a, &get("nw-a", "1h"), NOW);
    assert!(out.contains("\"new\"") && !out.contains("\"old\""), "{out}");
    http(&a, &post("nw-a", "older"), NOW - board::RETENTION_S - 10);
    assert!(!http(&a, &get("nw-a", "all"), NOW).contains("older"));
    assert!(http(&a, &get("nw-a", "nonsense"), NOW).starts_with("HTTP/1.1 400"));
}

#[test]
fn oversize_body_refused() {
    let a = app();
    let big = "x".repeat(BODY_MAX + 1);
    assert!(http(&a, &post("nw-a", &big), NOW).starts_with("HTTP/1.1 413"));
    let chunked = format!("POST /nw-a HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{big}\r\n0\r\n\r\n", big.len());
    assert!(http(&a, &chunked, NOW).starts_with("HTTP/1.1 413"));
    assert!(http(&a, &post("nw-a", &"y".repeat(BODY_MAX)), NOW).starts_with("HTTP/1.1 200"));
}

#[test]
fn pages_served_from_folder_only() {
    let d = std::env::temp_dir().join(format!("lesson-www-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join("nature")).unwrap();
    fs::write(d.join("nature/index.html"), "<h1>hi</h1>").unwrap();
    fs::write(d.join(".secret"), "no").unwrap();
    let a = App::new(Db::memory(), Store::open(None, NOW), KEY.into(), Some(d.clone()));
    let out = http(&a, "GET /nature/ HTTP/1.1\r\n\r\n", NOW);
    assert!(out.starts_with("HTTP/1.1 200") && out.contains("text/html") && out.ends_with("<h1>hi</h1>"));
    for bad in ["/.secret", "/../etc/passwd", "/nature/%2e%2e/x"] {
        assert!(http(&a, &format!("GET {bad} HTTP/1.1\r\n\r\n"), NOW).starts_with("HTTP/1.1 404"), "{bad}");
    }
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn survives_restart() {
    let d = std::env::temp_dir().join(format!("lesson-restart-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    let (code, noa) = {
        let a = App::new(Db::open(&d.join("lesson.db")).unwrap(), Store::open(Some(d.join("board.log")), NOW), KEY.into(), None);
        let (code, noa) = class(&a);
        try_pic(&a, &code, noa, 3, NOW, "/api/pick");
        http(&a, &post("nw-a", "{\"n\":\"Omer\",\"s\":9}"), NOW - 10);
        (code, noa)
    };
    let a = App::new(Db::open(&d.join("lesson.db")).unwrap(), Store::open(Some(d.join("board.log")), NOW), KEY.into(), None);
    assert!(try_pic(&a, &code, noa, 3, NOW, "/api/signin").starts_with("HTTP/1.1 200"));
    assert!(http(&a, &get("nw-a", "12h"), NOW).contains("Omer"));
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn real_socket_round_trip() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let a = Arc::new(App::new(Db::memory(), Store::open(None, now_s()), KEY.into(), None));
    let a2 = a.clone();
    std::thread::spawn(move || {
        for c in l.incoming().take(2) {
            let mut s = c.unwrap();
            let rd = s.try_clone().unwrap();
            serve(rd, &mut s, &a2, now_s()).unwrap();
        }
    });
    let ask = |r: String| {
        let mut c = std::net::TcpStream::connect(addr).unwrap();
        c.write_all(r.as_bytes()).unwrap();
        let mut out = String::new();
        c.read_to_string(&mut out).unwrap();
        out
    };
    assert!(ask(post("nw-s", "hello")).starts_with("HTTP/1.1 200"));
    assert!(ask(get("nw-s", "12h")).contains("\"message\":\"hello\""));
}

#[test]
fn teacher_sees_kids_state() {
    let a = app();
    let (code, noa) = class(&a);
    try_pic(&a, &code, noa, 5, NOW, "/api/pick");
    let path = format!("/api/classes/{code}/students");
    // A pause shows its seconds left.
    let mut t = NOW;
    for _ in 0..PAUSES_S[0].0 {
        try_pic(&a, &code, noa, 6, t, "/api/signin");
    }
    let r = body_of(&http(&a, &teacher("GET", &path, ""), t + 10));
    let kid = |r: &Value, id: i64| r["students"].as_array().unwrap().iter().find(|s| s["id"] == id).unwrap().clone();
    assert_eq!(kid(&r, noa)["wait_s"], PAUSES_S[0].1 - 10);
    assert_eq!(kid(&r, noa)["locked"], false);
    // Wrong tries up to the lock.
    let mut wrong = PAUSES_S[0].0;
    while wrong < LOCK_AFTER_WRONG {
        t += 3600;
        try_pic(&a, &code, noa, 6, t, "/api/signin");
        wrong += 1;
    }
    let out = http(&a, &teacher("GET", &path, ""), t);
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    let r = body_of(&out);
    assert!(!r.to_string().contains("picture"), "{r}");
    assert_eq!(r["class"], "5A");
    let names: Vec<&str> = r["students"].as_array().unwrap().iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Noa K.", "Omer"], "A-Z");
    let n = kid(&r, noa);
    assert_eq!((n["locked"].clone(), n["picked"].clone()), (json!(true), json!(true)));
    let omer = &r["students"][1];
    assert_eq!((omer["locked"].clone(), omer["picked"].clone(), omer["wait_s"].clone()), (json!(false), json!(false), json!(0)));
    // The teacher key is needed, and an unknown code is 404.
    assert!(http(&a, &req("GET", &path, "", ""), t).starts_with("HTTP/1.1 401"));
    let wrong_code = if code == "000000" { "000001" } else { "000000" };
    assert!(http(&a, &teacher("GET", &format!("/api/classes/{wrong_code}/students"), ""), t).starts_with("HTTP/1.1 404"));
    // Unlocked, it reads open again.
    assert!(http(&a, &teacher("POST", &format!("/api/students/{noa}/unlock"), ""), t).starts_with("HTTP/1.1 200"));
    let r = body_of(&http(&a, &teacher("GET", &path, ""), t));
    assert_eq!(kid(&r, noa)["locked"], false);
    assert_eq!(kid(&r, noa)["wait_s"], 0);
}

fn open_screen(a: &App, now: u64) -> (String, String) {
    let out = http(a, &req("POST", "/api/screens", "", ""), now);
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    let b = body_of(&out);
    let code = b["code"].as_str().unwrap().to_string();
    assert_eq!(code.len(), CLASS_CODE_DIGITS);
    (b["token"].as_str().unwrap().to_string(), code)
}

#[test]
fn remote_pair_and_move() {
    let mut a = app();
    // No hold: a poll with nothing new answers at once (the hold itself is remote_long_poll_wakes).
    a.remote.hold = Duration::ZERO;
    let (token, code) = open_screen(&a, NOW);
    assert_eq!(token.len(), 32, "as long as a kid's session token");
    let pair = |code: &str, h: &str| http(&a, &req("POST", "/api/screens/pair", h, &format!(r#"{{"code":"{code}"}}"#)), NOW);
    let key = format!("X-Teacher-Key: {KEY}\r\n");
    // Pairing needs the teacher key; a wrong code is 404.
    assert!(pair(&code, "").starts_with("HTTP/1.1 401"));
    let wrong = if code == "000000" { "000001" } else { "000000" };
    assert!(pair(wrong, &key).starts_with("HTTP/1.1 404"));
    let out = pair(&code, &key);
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    let id = body_of(&out)["id"].as_str().unwrap().to_string();
    // The code works once.
    assert!(pair(&code, &key).starts_with("HTTP/1.1 404"));
    // Commands need the key; a bad one is 400.
    let cmd = |b: &str, h: &str| http(&a, &req("POST", &format!("/api/screens/{id}/cmd"), h, b), NOW);
    assert!(cmd(r#"{"cmd":"next"}"#, "").starts_with("HTTP/1.1 401"));
    assert!(cmd(r#"{"cmd":"next"}"#, &key).starts_with("HTTP/1.1 200"));
    assert!(cmd(r#"{"cmd":"stage","stage":3}"#, &key).starts_with("HTTP/1.1 200"));
    for bad in [r#"{"cmd":"jump"}"#, r#"{"cmd":"stage"}"#, r#"{"cmd":"stage","stage":0}"#, r#"{"cmd":"stage","stage":10}"#, "{}"] {
        assert!(cmd(bad, &key).starts_with("HTTP/1.1 400"), "{bad}");
    }
    // The projector's poll after 0 has both, in order, and it is paired.
    let p = body_of(&http(&a, &req("GET", &format!("/api/screens/{token}/cmds?after=0"), "", ""), NOW));
    assert_eq!(p["paired"], true);
    assert_eq!(p["code"], Value::Null);
    assert_eq!(p["seq"], 2);
    assert_eq!(p["cmds"], json!([{"cmd": "next", "seq": 1}, {"cmd": "stage", "stage": 3, "seq": 2}]));
    let p = body_of(&http(&a, &req("GET", &format!("/api/screens/{token}/cmds?after=1"), "", ""), NOW));
    assert_eq!(p["cmds"], json!([{"cmd": "stage", "stage": 3, "seq": 2}]));
    // State round-trips to the phone.
    let st = r#"{"stage":3,"of":6,"name":"Quiz time","item":null}"#;
    assert!(http(&a, &req("POST", &format!("/api/screens/{token}/state"), "", st), NOW).starts_with("HTTP/1.1 200"));
    assert!(http(&a, &req("POST", &format!("/api/screens/{token}/state"), "", "{}"), NOW).starts_with("HTTP/1.1 400"));
    assert!(http(&a, &req("GET", &format!("/api/screens/{id}"), "", ""), NOW + 4).starts_with("HTTP/1.1 401"));
    let s = body_of(&http(&a, &teacher("GET", &format!("/api/screens/{id}"), ""), NOW + 4));
    assert_eq!((s["state"]["stage"].clone(), s["state"]["of"].clone(), s["state"]["name"].clone()), (json!(3), json!(6), json!("Quiz time")));
    assert_eq!(s["polled_s_ago"], 4);
    // Unknown tokens and ids are 404, so the page opens a new screen and the phone pairs again.
    assert!(http(&a, &req("GET", "/api/screens/nope/cmds?after=0", "", ""), NOW).starts_with("HTTP/1.1 404"));
    assert!(http(&a, &teacher("POST", "/api/screens/nope/cmd", r#"{"cmd":"next"}"#), NOW).starts_with("HTTP/1.1 404"));
    // Only the newest commands are kept.
    for _ in 0..remote::REMOTE_CMDS_KEPT + 5 {
        cmd(r#"{"cmd":"space"}"#, &key);
    }
    let p = body_of(&http(&a, &req("GET", &format!("/api/screens/{token}/cmds?after=0"), "", ""), NOW));
    assert_eq!(p["cmds"].as_array().unwrap().len(), remote::REMOTE_CMDS_KEPT);
    // A code expires: an unpaired screen gets a fresh one on its next poll, and the old one no longer pairs.
    let (t2, c2) = open_screen(&a, NOW);
    let later = NOW + remote::REMOTE_CODE_S;
    assert!(http(&a, &teacher("POST", "/api/screens/pair", &format!(r#"{{"code":"{c2}"}}"#)), later).starts_with("HTTP/1.1 404"));
    let p = body_of(&http(&a, &req("GET", &format!("/api/screens/{t2}/cmds?after=0"), "", ""), later));
    let c3 = p["code"].as_str().unwrap().to_string();
    assert_eq!(p["paired"], false);
    assert!(http(&a, &teacher("POST", "/api/screens/pair", &format!(r#"{{"code":"{c3}"}}"#)), later).starts_with("HTTP/1.1 200"));
    // A screen whose projector stopped polling is dropped.
    assert!(http(&a, &req("GET", &format!("/api/screens/{token}/cmds?after=99"), "", ""), NOW + remote::REMOTE_SCREEN_S).starts_with("HTTP/1.1 404"));
}

#[test]
fn remote_long_poll_wakes() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let mut a = App::new(Db::memory(), Store::open(None, now_s()), KEY.into(), None);
    // A short hold, so the idle poll's end is seen in a second rather than REMOTE_HOLD_S.
    let hold = Duration::from_millis(1500);
    a.remote.hold = hold;
    std::thread::spawn(move || run(l, Arc::new(a)));
    let ask = move |r: String| {
        let mut c = std::net::TcpStream::connect(addr).unwrap();
        c.write_all(r.as_bytes()).unwrap();
        let mut out = String::new();
        c.read_to_string(&mut out).unwrap();
        out
    };
    let b = body_of(&ask(req("POST", "/api/screens", "", "")));
    let (token, code) = (b["token"].as_str().unwrap().to_string(), b["code"].as_str().unwrap().to_string());
    let id = body_of(&ask(teacher("POST", "/api/screens/pair", &format!(r#"{{"code":"{code}"}}"#))))["id"].as_str().unwrap().to_string();
    // An idle poll answers empty once the hold ends.
    let t = std::time::Instant::now();
    let p = body_of(&ask(req("GET", &format!("/api/screens/{token}/cmds?after=0"), "", "")));
    let took = t.elapsed();
    assert_eq!(p["cmds"], json!([]));
    assert!(took >= hold && took < hold + Duration::from_secs(1), "idle poll took {took:?}");
    // A held poll answers within 1 s of the phone's command.
    let tk = token.clone();
    let poller = std::thread::spawn(move || {
        let out = ask(req("GET", &format!("/api/screens/{tk}/cmds?after=0"), "", ""));
        (std::time::Instant::now(), out)
    });
    std::thread::sleep(Duration::from_millis(300));
    // While it is held the phone sees the projector as polling now.
    assert_eq!(body_of(&ask(teacher("GET", &format!("/api/screens/{id}"), "")))["polled_s_ago"], 0);
    let sent = std::time::Instant::now();
    assert!(ask(teacher("POST", &format!("/api/screens/{id}/cmd"), r#"{"cmd":"next"}"#)).starts_with("HTTP/1.1 200"));
    let (got, out) = poller.join().unwrap();
    assert!(got.duration_since(sent) < Duration::from_secs(1), "woke after {:?}", got.duration_since(sent));
    assert_eq!(body_of(&out)["cmds"], json!([{"cmd": "next", "seq": 1}]));
}

// ---------- The installable teacher app (Kitah) ----------

#[test]
fn pwa_content_types() {
    let d = std::env::temp_dir().join(format!("lesson-pwa-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join("teacher")).unwrap();
    fs::write(d.join("teacher/manifest.webmanifest"), "{}").unwrap();
    fs::write(d.join("teacher/sw.js"), "//").unwrap();
    fs::write(d.join("teacher/icon-192.png"), "png").unwrap();
    let a = App::new(Db::memory(), Store::open(None, NOW), KEY.into(), Some(d.clone()));
    for (p, t) in [("manifest.webmanifest", "application/manifest+json"), ("sw.js", "text/javascript"), ("icon-192.png", "image/png")] {
        let out = http(&a, &format!("GET /teacher/{p} HTTP/1.1\r\n\r\n"), NOW);
        assert!(out.starts_with("HTTP/1.1 200") && out.contains(&format!("Content-Type: {t}")), "{p}: {out}");
    }
    let _ = fs::remove_dir_all(&d);
}

/// A PNG's width and height from its IHDR chunk: the 8-byte signature, the chunk's length and "IHDR", then two
/// big-endian u32s (PNG spec, section 11.2.2).
fn png_size(b: &[u8]) -> (u32, u32) {
    assert!(b.len() >= 24 && b[..8] == *b"\x89PNG\r\n\x1a\n" && &b[12..16] == b"IHDR", "not a PNG");
    let be = |i: usize| u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    (be(16), be(20))
}

#[test]
fn pwa_manifest_installable() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("www/teacher");
    let m: Value = serde_json::from_str(&fs::read_to_string(dir.join("manifest.webmanifest")).unwrap()).unwrap();
    assert_eq!(m["name"], "Kitah");
    assert_eq!(m["display"], "standalone");
    let (start, scope) = (m["start_url"].as_str().unwrap(), m["scope"].as_str().unwrap());
    assert!(start.starts_with(scope), "start_url {start} outside scope {scope}");
    let icons = m["icons"].as_array().unwrap();
    // Chrome's install criteria (web.dev/articles/install-criteria): a 192 px and a 512 px icon.
    for need in ["192x192", "512x512"] {
        assert!(icons.iter().any(|i| i["sizes"] == need && i["purpose"].as_str().unwrap_or("any").split(' ').any(|p| p == "any")), "no {need} icon for any purpose");
    }
    for i in icons {
        let src = i["src"].as_str().unwrap();
        let file = dir.join(src.strip_prefix(scope).unwrap_or_else(|| panic!("{src} is not under {scope}")));
        let (w, h) = png_size(&fs::read(&file).unwrap_or_else(|e| panic!("{src}: {e}")));
        assert_eq!(format!("{w}x{h}"), i["sizes"].as_str().unwrap(), "{src}");
    }
}

#[test]
fn pwa_sw_skips_api() {
    let sw = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("www/teacher/sw.js")).unwrap();
    assert!(sw.contains("url.pathname.startsWith('/api/')") && sw.contains("url.pathname === '/api'"), "sw.js must leave /api to the network");
    assert!(sw.contains("!url.pathname.startsWith(SCOPE)") && sw.contains("const SCOPE = '/teacher/'"), "sw.js must leave pages outside /teacher/ alone");
}
