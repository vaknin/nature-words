// Included into main.rs at the crate root (see there), so main's own names and imports are already in scope.
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
fn race(token: &str, topic: &str, place: u32, score: i64) -> String {
    req("POST", "/api/race", &format!("Authorization: Bearer {token}\r\n"), &format!(r#"{{"topic":"{topic}","place":{place},"score":{score}}}"#))
}
/// An empty class with joining open; returns its code.
fn empty_class(a: &App) -> String {
    let out = http(a, &teacher("POST", "/api/classes", r#"{"name":"5B","students":[]}"#), NOW);
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    let code = body_of(&out)["code"].as_str().unwrap().to_string();
    let out = http(a, &teacher("POST", &format!("/api/classes/{code}/join"), r#"{"open":true}"#), NOW);
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    code
}
fn join(a: &App, code: &str, name: &str, now: u64) -> String {
    http(a, &req("POST", "/api/join", "", &json!({"code": code, "name": name}).to_string()), now)
}
fn ticket(a: &App, t: &str, now: u64) -> String {
    http(a, &req("GET", &format!("/api/join/{t}"), "", ""), now)
}
fn roster_names(a: &App, code: &str, now: u64) -> Vec<String> {
    let r = body_of(&http(a, &req("GET", &format!("/api/roster/{code}"), "", ""), now));
    r["students"].as_array().unwrap().iter().map(|s| s["name"].as_str().unwrap().to_string()).collect()
}
/// Joins NAME, the teacher approves it; returns the kid's id.
fn join_approved(a: &App, code: &str, name: &str) -> i64 {
    let out = join(a, code, name, NOW);
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    let t = body_of(&out)["ticket"].as_str().unwrap().to_string();
    let r = body_of(&http(a, &teacher("GET", &format!("/api/classes/{code}/students"), ""), NOW));
    let id = r["students"].as_array().unwrap().iter().find(|s| s["status"] == "pending").unwrap()["id"].as_i64().unwrap();
    assert!(http(a, &teacher("POST", &format!("/api/students/{id}/approve"), ""), NOW).starts_with("HTTP/1.1 200"));
    assert_eq!(body_of(&ticket(a, &t, NOW))["id"], id);
    id
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
    // No name: names come only from sign-in (see kahoot_race_uses_approved_name).
    let score = r#"{"s":12,"p":1,"t":1}"#;
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
        let token = body_of(&try_pic(&a, &code, noa, 3, NOW - 10, "/api/pick"))["token"].as_str().unwrap().to_string();
        let out = http(&a, &race(&token, "nw-a", 1, 9), NOW - 10);
        assert!(out.starts_with("HTTP/1.1 200"), "{out}");
        (code, noa)
    };
    let a = App::new(Db::open(&d.join("lesson.db")).unwrap(), Store::open(Some(d.join("board.log")), NOW), KEY.into(), None);
    assert!(try_pic(&a, &code, noa, 3, NOW, "/api/signin").starts_with("HTTP/1.1 200"));
    assert!(http(&a, &get("nw-a", "12h"), NOW).contains("Noa K."));
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

// ---------- Cloudflare Access: the teacher by signed token only ----------

const TEAM: &str = "https://kitah-test.cloudflareaccess.com";
const AUD: &str = "aud-tag-of-the-kitah-app";
const EMAIL: &str = "teacher@example.com";

/// Two throwaway RSA key pairs for these tests, made once per run with openssl (the team's key, and another to forge
/// with), as PKCS#8 PEM.
fn test_keys() -> &'static (String, String) {
    static KEYS: std::sync::OnceLock<(String, String)> = std::sync::OnceLock::new();
    KEYS.get_or_init(|| {
        let make = || {
            let o = std::process::Command::new("openssl").args(["genpkey", "-algorithm", "RSA", "-pkeyopt", "rsa_keygen_bits:2048"]).output().expect("openssl");
            assert!(o.status.success());
            String::from_utf8(o.stdout).unwrap()
        };
        (make(), make())
    })
}
fn rsa_key(pem: &str) -> rsa::RsaPrivateKey {
    use rsa::pkcs8::DecodePrivateKey;
    rsa::RsaPrivateKey::from_pkcs8_pem(pem.trim()).unwrap()
}
fn b64url(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}
/// A JWT signed with `pem` as key id "k1", holding `claims`.
fn jwt(pem: &str, claims: Value) -> String {
    jwt_kid(pem, "k1", claims)
}
/// The same, naming key id `kid`.
fn jwt_kid(pem: &str, kid: &str, claims: Value) -> String {
    use rsa::signature::{SignatureEncoding, Signer};
    let head = b64url(json!({"alg": "RS256", "kid": kid, "typ": "JWT"}).to_string().as_bytes());
    let body = b64url(claims.to_string().as_bytes());
    let sig = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(rsa_key(pem)).sign(format!("{head}.{body}").as_bytes());
    format!("{head}.{body}.{}", b64url(&sig.to_bytes()))
}
fn good_jwt(claims: Value) -> String {
    jwt(&test_keys().0, claims)
}
fn claims() -> Value {
    json!({"iss": TEAM, "aud": [AUD], "email": "Teacher@Example.com", "exp": NOW + 3600, "nbf": NOW - 60, "iat": NOW - 60, "sub": "x"})
}
/// An app with Access on: access.json and the team's certs (JWKS, as Cloudflare serves them) in a temp folder.
fn access_app(tag: &str) -> (App, PathBuf) {
    use rsa::traits::PublicKeyParts;
    let d = std::env::temp_dir().join(format!("lesson-access-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    let pk = rsa_key(&test_keys().0).to_public_key();
    let certs = json!({"keys": [{"kid": "k1", "kty": "RSA", "alg": "RS256", "use": "sig", "e": b64url(&pk.e().to_bytes_be()), "n": b64url(&pk.n().to_bytes_be())}], "public_cert": {}, "public_certs": []});
    fs::write(d.join("certs.json"), certs.to_string()).unwrap();
    fs::write(d.join("access.json"), json!({"team": "kitah-test", "aud": AUD, "email": EMAIL, "certs": d.join("certs.json")}).to_string()).unwrap();
    let access = access::Access::load(&d.join("access.json")).unwrap();
    assert!(access.is_some());
    (App::new(Db::memory(), Store::open(None, NOW), KEY.into(), None).with_access(access), d)
}
/// HMAC-SHA256 (RFC 2104, 64-byte blocks), for the key-confusion forgery below.
fn hmac_sha256(key: &[u8], msg: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut k = if key.len() > 64 { Sha256::digest(key).to_vec() } else { key.to_vec() };
    k.resize(64, 0);
    let pad = |b: u8| k.iter().map(|x| x ^ b).collect::<Vec<u8>>();
    let inner = Sha256::new().chain_update(pad(0x36)).chain_update(msg).finalize();
    Sha256::new().chain_update(pad(0x5c)).chain_update(inner).finalize().to_vec()
}
fn with_token(method: &str, path: &str, token: &str, body: &str) -> String {
    req(method, path, &format!("Cf-Access-Jwt-Assertion: {token}\r\n"), body)
}

#[test]
fn access_token_allows_teacher() {
    let (a, d) = access_app("allow");
    let t = good_jwt(claims());
    let out = http(&a, &with_token("POST", "/api/classes", &t, r#"{"name":"5A","students":["Noa K."]}"#), NOW);
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    let code = body_of(&out)["code"].as_str().unwrap().to_string();
    assert!(http(&a, &with_token("GET", &format!("/api/classes/{code}/students"), &t, ""), NOW).starts_with("HTTP/1.1 200"));
    // The Kitah app's cookie works only with the app's own header (a page on another site can't send that).
    let cookie = format!("Cookie: theme=x; CF_Authorization={t}\r\n");
    assert!(http(&a, &req("GET", "/api/classes", &format!("{cookie}X-Kitah-App: 1\r\n"), ""), NOW).starts_with("HTTP/1.1 200"));
    assert!(http(&a, &req("GET", "/api/classes", &cookie, ""), NOW).starts_with("HTTP/1.1 401"));
    // An nbf a few seconds ahead is not yet good; at nbf it is.
    let mut c = claims();
    c["nbf"] = json!(NOW + 5);
    assert!(http(&a, &with_token("GET", "/api/classes", &good_jwt(c.clone()), ""), NOW).starts_with("HTTP/1.1 401"));
    assert!(http(&a, &with_token("GET", "/api/classes", &good_jwt(c), ""), NOW + 5).starts_with("HTTP/1.1 200"));
    // A plain aud string is fine too.
    let mut c = claims();
    c["aud"] = json!(AUD);
    assert!(http(&a, &with_token("GET", "/api/classes", &good_jwt(c), ""), NOW).starts_with("HTTP/1.1 200"));
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn access_token_refuses_bad() {
    let (a, d) = access_app("refuse");
    let set = |k: &str, v: Value| {
        let mut c = claims();
        c[k] = v;
        good_jwt(c)
    };
    let good = good_jwt(claims());
    let mut tampered = good.clone();
    tampered.insert_str(good.find('.').unwrap() + 1, "e30");
    let mut no_email = claims();
    no_email.as_object_mut().unwrap().remove("email");
    // alg none, with no signature.
    let none = format!("{}.{}.", b64url(json!({"alg": "none", "kid": "k1"}).to_string().as_bytes()), good.split('.').nth(1).unwrap());
    // Key confusion: HS256 with the team's public key (its JWKS modulus) as the HMAC secret, which a verifier that
    // trusts the token's alg would accept.
    let hs256_forgery = {
        use rsa::traits::PublicKeyParts;
        let n = rsa_key(&test_keys().0).to_public_key().n().to_bytes_be();
        let head = b64url(json!({"alg": "HS256", "kid": "k1", "typ": "JWT"}).to_string().as_bytes());
        let body = b64url(claims().to_string().as_bytes());
        let mut forged = Vec::new();
        for secret in [n.clone(), b64url(&n).into_bytes()] {
            forged.push(format!("{head}.{body}.{}", b64url(&hmac_sha256(&secret, format!("{head}.{body}").as_bytes()))));
        }
        forged
    };
    let bad = [
        ("wrong signature", jwt(&test_keys().1, claims())),
        ("tampered claims", tampered),
        ("alg none", none),
        ("expired", set("exp", json!(NOW))),
        ("wrong aud", set("aud", json!(["another-app"]))),
        ("wrong iss", set("iss", json!("https://evil.cloudflareaccess.com"))),
        ("other email", set("email", json!("someone@example.com"))),
        ("no email", good_jwt(no_email)),
        ("not a token", "abc.def".to_string()),
        ("HS256 keyed with the modulus bytes", hs256_forgery[0].clone()),
        ("HS256 keyed with the modulus as base64url", hs256_forgery[1].clone()),
        ("unknown key id", jwt_kid(&test_keys().0, "k9", claims())),
    ];
    for (why, t) in &bad {
        let out = http(&a, &with_token("GET", "/api/classes", t, ""), NOW);
        assert!(out.starts_with("HTTP/1.1 401"), "{why}: {out}");
    }
    // Missing header, and the teacher key alone: 401.
    assert!(http(&a, &req("GET", "/api/classes", "", ""), NOW).starts_with("HTTP/1.1 401"));
    assert!(http(&a, &teacher("GET", "/api/classes", ""), NOW).starts_with("HTTP/1.1 401"));
    assert!(http(&a, &teacher("POST", "/api/classes", r#"{"name":"x"}"#), NOW).starts_with("HTTP/1.1 401"));
    // The good one still works after all that.
    assert!(http(&a, &with_token("GET", "/api/classes", &good, ""), NOW).starts_with("HTTP/1.1 200"));
    // A broken access.json stops the server from starting open.
    fs::write(d.join("broken.json"), r#"{"team":"x"}"#).unwrap();
    assert!(access::Access::load(&d.join("broken.json")).is_err());
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn kahoot_join_approve() {
    let a = app();
    let code = empty_class(&a);
    let r = body_of(&http(&a, &req("GET", &format!("/api/roster/{code}"), "", ""), NOW));
    assert_eq!(r["join_open"], true);
    assert!(r["students"].as_array().unwrap().is_empty());
    let out = join(&a, &code, "noa b", NOW);
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    let b = body_of(&out);
    assert_eq!(b["name"], "Noa B.", "tidied: capital and the initial's dot");
    let t = b["ticket"].as_str().unwrap().to_string();
    assert_eq!(t.len(), 32);
    assert_eq!(body_of(&ticket(&a, &t, NOW))["status"], "pending");
    // Waiting: the kids' roster does not show her, the teacher's list does, first.
    assert!(roster_names(&a, &code, NOW).is_empty());
    http(&a, &teacher("POST", &format!("/api/classes/{code}/students"), r#"{"name":"Avi"}"#), NOW);
    let r = body_of(&http(&a, &teacher("GET", &format!("/api/classes/{code}/students"), ""), NOW));
    assert_eq!(r["join_s"], JOIN_OPEN_S);
    let first = &r["students"][0];
    assert_eq!((first["name"].clone(), first["status"].clone()), (json!("Noa B."), json!("pending")));
    assert_eq!(r["students"][1]["status"], "approved");
    let id = first["id"].as_i64().unwrap();
    // Approving needs the teacher key.
    assert!(http(&a, &req("POST", &format!("/api/students/{id}/approve"), "", ""), NOW).starts_with("HTTP/1.1 401"));
    assert!(http(&a, &teacher("POST", &format!("/api/students/{id}/approve"), ""), NOW).starts_with("HTTP/1.1 200"));
    let s = body_of(&ticket(&a, &t, NOW));
    assert_eq!((s["status"].clone(), s["id"].clone(), s["name"].clone(), s["picked"].clone()), (json!("approved"), json!(id), json!("Noa B."), json!(false)));
    assert_eq!(roster_names(&a, &code, NOW), ["Avi", "Noa B."]);
    // Rejecting an approved kid does nothing: her record stays.
    assert!(http(&a, &teacher("POST", &format!("/api/students/{id}/reject"), ""), NOW).starts_with("HTTP/1.1 404"));
    assert!(try_pic(&a, &code, id, 4, NOW, "/api/pick").starts_with("HTTP/1.1 200"));
    let out = try_pic(&a, &code, id, 4, NOW, "/api/signin");
    let token = body_of(&out)["token"].as_str().unwrap().to_string();
    let me = body_of(&http(&a, &req("GET", "/api/me", &format!("Authorization: Bearer {token}\r\n"), ""), NOW));
    assert_eq!(me["name"], "Noa B.");
}

#[test]
fn kahoot_join_reject() {
    let a = app();
    let code = empty_class(&a);
    let t = body_of(&join(&a, &code, "Dana", NOW))["ticket"].as_str().unwrap().to_string();
    let r = body_of(&http(&a, &teacher("GET", &format!("/api/classes/{code}/students"), ""), NOW));
    let id = r["students"][0]["id"].as_i64().unwrap();
    assert!(http(&a, &req("POST", &format!("/api/students/{id}/reject"), "", ""), NOW).starts_with("HTTP/1.1 401"));
    assert!(http(&a, &teacher("POST", &format!("/api/students/{id}/reject"), ""), NOW).starts_with("HTTP/1.1 200"));
    assert!(ticket(&a, &t, NOW).starts_with("HTTP/1.1 404"));
    assert!(ticket(&a, "00000000000000000000000000000000", NOW).starts_with("HTTP/1.1 404"));
    let r = body_of(&http(&a, &teacher("GET", &format!("/api/classes/{code}/students"), ""), NOW));
    assert!(r["students"].as_array().unwrap().is_empty());
    // The name is free again: the kid types it again.
    assert!(join(&a, &code, "Dana", NOW).starts_with("HTTP/1.1 200"));
}

#[test]
fn kahoot_join_closed_and_full() {
    let a = app();
    let code = empty_class(&a);
    // Taken, any case, waiting names too; the hint says to add the family initial.
    assert!(join(&a, &code, "Omer", NOW).starts_with("HTTP/1.1 200"));
    let out = join(&a, &code, "OMER", NOW);
    assert!(out.starts_with("HTTP/1.1 409") && out.contains("family initial"), "{out}");
    assert!(join(&a, &code, "Omer K.", NOW).starts_with("HTTP/1.1 200"));
    let he = join(&a, &code, "נועה", NOW);
    assert!(he.starts_with("HTTP/1.1 200"), "{he}");
    assert!(join(&a, &code, "נועה", NOW).starts_with("HTTP/1.1 409"));
    // Full, counting the waiting names.
    let letters: Vec<char> = ('A'..='Z').collect();
    let mut n = 3;
    'fill: for x in &letters {
        for y in &letters {
            if n >= CLASS_MAX_KIDS {
                break 'fill;
            }
            let out = join(&a, &code, &format!("Kid {x}{y}").replace(' ', ""), NOW);
            assert!(out.starts_with("HTTP/1.1 200"), "{out}");
            n += 1;
        }
    }
    let out = join(&a, &code, "Last", NOW);
    assert!(out.starts_with("HTTP/1.1 409") && out.contains("full"), "{out}");
    // Closed by the teacher, or by the clock after one lesson.
    let code2 = empty_class(&a);
    assert!(join(&a, &code2, "Tal", NOW + JOIN_OPEN_S - 1).starts_with("HTTP/1.1 200"));
    assert!(join(&a, &code2, "Gil", NOW + JOIN_OPEN_S).starts_with("HTTP/1.1 403"));
    let r = body_of(&http(&a, &req("GET", &format!("/api/roster/{code2}"), "", ""), NOW + JOIN_OPEN_S));
    assert_eq!(r["join_open"], false);
    http(&a, &teacher("POST", &format!("/api/classes/{code2}/join"), r#"{"open":true}"#), NOW);
    assert!(http(&a, &teacher("POST", &format!("/api/classes/{code2}/join"), r#"{"open":false}"#), NOW).starts_with("HTTP/1.1 200"));
    assert!(join(&a, &code2, "Gil", NOW).starts_with("HTTP/1.1 403"));
    let r = body_of(&http(&a, &teacher("GET", &format!("/api/classes/{code2}/students"), ""), NOW));
    assert_eq!(r["join_s"], 0);
    // Opening joining needs the key; a class made with no names is fine.
    assert!(http(&a, &req("POST", &format!("/api/classes/{code2}/join"), "", r#"{"open":true}"#), NOW).starts_with("HTTP/1.1 401"));
    assert!(http(&a, &teacher("POST", "/api/classes", r#"{"name":"5C"}"#), NOW).starts_with("HTTP/1.1 200"));
    // An unknown code counts against the same miss limit as the roster.
    let wrong = if code == "000000" || code2 == "000000" { "000002" } else { "000000" };
    let ip = "CF-Connecting-IP: 203.0.113.7\r\n";
    for _ in 0..ROSTER_MISSES_PER_WINDOW {
        let out = http(&a, &req("POST", "/api/join", ip, &json!({"code": wrong, "name": "x1"}).to_string()), NOW);
        assert!(out.starts_with("HTTP/1.1 404"), "{out}");
    }
    assert!(http(&a, &req("GET", &format!("/api/roster/{code}"), ip, ""), NOW).starts_with("HTTP/1.1 429"));
    assert!(http(&a, &req("POST", "/api/join", ip, &json!({"code": code, "name": "Zed"}).to_string()), NOW).starts_with("HTTP/1.1 429"));
}

#[test]
fn kahoot_name_rule() {
    for (raw, want) in [
        ("Noa", Some("Noa")),
        ("noa b.", Some("Noa B.")),
        ("  Noa   B  ", Some("Noa B.")),
        ("נועה", Some("נועה")),
        ("נועה ב.", Some("נועה ב.")),
        ("Abcdefghijkl", Some("Abcdefghijkl")),
        ("Abcdefghijklm", None),
        ("", None),
        ("   ", None),
        ("Noa Ben", None),
        ("Noa B. C", None),
        ("Noa2", None),
        ("Noa 2", None),
        ("No-a", None),
        ("Noa!", None),
        ("Noa ..", None),
        ("<b>", None),
    ] {
        assert_eq!(kid_name(raw).as_deref(), want, "{raw:?}");
    }
    assert_eq!(KID_WORD_MAX + " K.".len(), NAME_MAX_CHARS);
    let a = app();
    let code = empty_class(&a);
    let out = join(&a, &code, "Noa Ben", NOW);
    assert!(out.starts_with("HTTP/1.1 400"), "{out}");
    assert!(join(&a, &code, "Noa7", NOW).starts_with("HTTP/1.1 400"));
}

#[test]
fn kahoot_pending_cannot_sign_in() {
    let a = app();
    let code = empty_class(&a);
    join(&a, &code, "Noa", NOW);
    let r = body_of(&http(&a, &teacher("GET", &format!("/api/classes/{code}/students"), ""), NOW));
    let id = r["students"][0]["id"].as_i64().unwrap();
    assert!(try_pic(&a, &code, id, 2, NOW, "/api/pick").starts_with("HTTP/1.1 404"));
    assert!(try_pic(&a, &code, id, 2, NOW, "/api/signin").starts_with("HTTP/1.1 404"));
    http(&a, &teacher("POST", &format!("/api/students/{id}/approve"), ""), NOW);
    assert!(try_pic(&a, &code, id, 2, NOW, "/api/pick").starts_with("HTTP/1.1 200"));
}

#[test]
fn kahoot_old_db_migrates() {
    // The schema as it was before kids could join themselves, with one kid who already picked.
    let d = std::env::temp_dir().join(format!("lesson-migrate-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    {
        let c = rusqlite::Connection::open(d.join("lesson.db")).unwrap();
        c.execute_batch(
            "CREATE TABLE classes (id INTEGER PRIMARY KEY, code TEXT NOT NULL UNIQUE, name TEXT NOT NULL, created INTEGER NOT NULL);
             CREATE TABLE students (id INTEGER PRIMARY KEY, class_id INTEGER NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
               name TEXT NOT NULL, picture INTEGER, level TEXT, wrong INTEGER NOT NULL DEFAULT 0, paused_until INTEGER NOT NULL DEFAULT 0,
               locked INTEGER NOT NULL DEFAULT 0, UNIQUE(class_id, name));
             INSERT INTO classes(id, code, name, created) VALUES(1, '123456', '5A', 1);
             INSERT INTO students(id, class_id, name, picture) VALUES(7, 1, 'Omer', 9);",
        )
        .unwrap();
    }
    for _ in 0..2 {
        // Twice: a second open finds the columns there and adds nothing.
        let a = App::new(Db::open(&d.join("lesson.db")).unwrap(), Store::open(None, NOW), KEY.into(), None);
        assert_eq!(roster_names(&a, "123456", NOW), ["Omer"]);
        assert!(try_pic(&a, "123456", 7, 9, NOW, "/api/signin").starts_with("HTTP/1.1 200"));
        assert!(join(&a, "123456", "Dana", NOW).starts_with("HTTP/1.1 403"), "joining starts closed");
    }
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn access_off_uses_key() {
    // No access.json: Access is off and the teacher key works as before.
    let d = std::env::temp_dir().join(format!("lesson-access-off-{}", std::process::id()));
    assert!(access::Access::load(&d.join("access.json")).unwrap().is_none());
    let a = app();
    assert_eq!(body_of(&http(&a, &req("GET", "/api/auth", "", ""), NOW))["access"], false);
    assert!(http(&a, &teacher("GET", "/api/classes", ""), NOW).starts_with("HTTP/1.1 200"));
    assert!(http(&a, &req("GET", "/api/classes", "", ""), NOW).starts_with("HTTP/1.1 401"));
    // A token alone means nothing while Access is off.
    assert!(http(&a, &with_token("GET", "/api/classes", &good_jwt(claims()), ""), NOW).starts_with("HTTP/1.1 401"));
}

// ---------- The teacher gate: the page itself, foreign origins, /api/auth ----------

/// A temp pages folder holding the teacher page and its manifest.
fn teacher_www(tag: &str) -> PathBuf {
    let w = std::env::temp_dir().join(format!("lesson-gate-www-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(w.join("teacher")).unwrap();
    fs::write(w.join("teacher/index.html"), "<h1>teacher</h1>").unwrap();
    fs::write(w.join("teacher/manifest.webmanifest"), r#"{"name":"Kitah"}"#).unwrap();
    w
}
fn auth(a: &App, headers: &str) -> Value {
    body_of(&http(a, &req("GET", "/api/auth", headers, ""), NOW))
}

#[test]
fn teacher_gate_page_needs_token() {
    let (mut a, d) = access_app("gate-page");
    let w = teacher_www("on");
    a.www = Some(w.clone());
    let t = good_jwt(claims());
    for p in ["/teacher/", "/teacher", "/teacher/index.html", "/teacher//index.html"] {
        for m in ["GET", "HEAD"] {
            let out = http(&a, &req(m, p, "", ""), NOW);
            assert!(out.starts_with("HTTP/1.1 401"), "{m} {p}: {out}");
            assert!(http(&a, &with_token(m, p, &t, ""), NOW).starts_with("HTTP/1.1 200"), "{m} {p} with token");
        }
    }
    let out = http(&a, &with_token("GET", "/teacher/", &t, ""), NOW);
    assert!(out.contains("<h1>teacher</h1>"));
    // Another email's token is no better than none.
    let mut c = claims();
    c["email"] = json!("someone@example.com");
    assert!(http(&a, &with_token("GET", "/teacher/", &good_jwt(c), ""), NOW).starts_with("HTTP/1.1 401"));
    // The app's manifest stays open, so Chrome (which fetches it without cookies) can still install the app.
    let out = http(&a, &req("GET", "/teacher/manifest.webmanifest", "", ""), NOW);
    assert!(out.starts_with("HTTP/1.1 200") && out.contains("application/manifest+json"), "{out}");
    // Access off: the page opens without a token, as today.
    let mut off = app();
    off.www = Some(w.clone());
    assert!(http(&off, &req("GET", "/teacher/", "", ""), NOW).starts_with("HTTP/1.1 200"));
    let _ = fs::remove_dir_all(&d);
    let _ = fs::remove_dir_all(&w);
}

#[test]
fn teacher_gate_foreign_origin_refused() {
    let (a, d) = access_app("gate-origin");
    let t = good_jwt(claims());
    let hdr = |origin: &str| format!("Cf-Access-Jwt-Assertion: {t}\r\nOrigin: {origin}\r\n");
    let body = r#"{"name":"5A","students":["Noa K."]}"#;
    for evil in ["https://evil.example", "null", "http://x.evil.example", "http://evil.example/http://x"] {
        let out = http(&a, &req("POST", "/api/classes", &hdr(evil), body), NOW);
        assert!(out.starts_with("HTTP/1.1 403 Forbidden"), "{evil}: {out}");
    }
    let n = |a: &App| a.db().classes().as_array().map_or(0, Vec::len);
    assert_eq!(n(&a), 0, "a refused request made no class");
    // From the request's own host (the tests' Host is x): accepted.
    let out = http(&a, &req("POST", "/api/classes", &hdr("http://x"), body), NOW);
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    assert_eq!(n(&a), 1);
    // The key path is held to the same rule while Access is off.
    let off = app();
    let key = |origin: &str| format!("X-Teacher-Key: {KEY}\r\nOrigin: {origin}\r\n");
    assert!(http(&off, &req("POST", "/api/classes", &key("https://evil.example"), body), NOW).starts_with("HTTP/1.1 403"));
    assert_eq!(n(&off), 0);
    assert!(http(&off, &req("POST", "/api/classes", &key("http://x"), body), NOW).starts_with("HTTP/1.1 200"));
    // Kid routes are not the teacher's and keep working from anywhere (the lesson page posts from its own host).
    assert!(!http(&off, &req("GET", "/api/roster/123456", "Origin: https://evil.example\r\n", ""), NOW).starts_with("HTTP/1.1 403"));
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn teacher_gate_auth_reports() {
    let (a, d) = access_app("gate-auth");
    let t = good_jwt(claims());
    assert_eq!(auth(&a, &format!("Cf-Access-Jwt-Assertion: {t}\r\n")), json!({"access": true, "teacher": true}));
    assert_eq!(auth(&a, &format!("Cookie: CF_Authorization={t}\r\nX-Kitah-App: 1\r\n")), json!({"access": true, "teacher": true}));
    assert_eq!(auth(&a, &format!("Cookie: CF_Authorization={t}\r\n")), json!({"access": true, "teacher": false}));
    assert_eq!(auth(&a, ""), json!({"access": true, "teacher": false}));
    assert_eq!(auth(&a, &format!("X-Teacher-Key: {KEY}\r\n")), json!({"access": true, "teacher": false}));
    let off = app();
    assert_eq!(auth(&off, &format!("X-Teacher-Key: {KEY}\r\n")), json!({"access": false, "teacher": true}));
    assert_eq!(auth(&off, ""), json!({"access": false, "teacher": false}));
    assert_eq!(auth(&off, &format!("Cf-Access-Jwt-Assertion: {t}\r\n")), json!({"access": false, "teacher": false}));
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn kahoot_home_signin_any_time() {
    let a = app();
    let code = empty_class(&a);
    let id = join_approved(&a, &code, "Noa B.");
    assert!(try_pic(&a, &code, id, 11, NOW, "/api/pick").starts_with("HTTP/1.1 200"));
    http(&a, &teacher("POST", &format!("/api/classes/{code}/join"), r#"{"open":false}"#), NOW);
    // Days later, from home, joining closed: the roster still lists her and her animal signs her in.
    let later = NOW + 3 * 86400 + 7 * 3600;
    assert_eq!(roster_names(&a, &code, later), ["Noa B."]);
    assert!(join(&a, &code, "Stranger", later).starts_with("HTTP/1.1 403"));
    assert!(try_pic(&a, &code, id, 11, later, "/api/signin").starts_with("HTTP/1.1 200"));
    assert!(try_pic(&a, &code, id, 12, later, "/api/signin").starts_with("HTTP/1.1 401"));
}

#[test]
fn kahoot_race_uses_approved_name() {
    let a = app();
    let code = empty_class(&a);
    let id = join_approved(&a, &code, "noa b");
    let token = body_of(&try_pic(&a, &code, id, 3, NOW, "/api/pick"))["token"].as_str().unwrap().to_string();
    let out = http(&a, &race(&token, "nw-k", 2, 17), NOW);
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    assert_eq!(body_of(&out)["n"], "Noa B.");
    let line = http(&a, &get("nw-k", "12h"), NOW);
    let msg: Value = serde_json::from_str(body_of(&line)["message"].as_str().unwrap()).unwrap();
    assert_eq!(msg, json!({"n": "Noa B.", "s": 17, "p": 2, "t": NOW * 1000}));
    // The best stays in her record.
    http(&a, &race(&token, "nw-k", 2, 5), NOW);
    let me = body_of(&http(&a, &req("GET", "/api/me", &format!("Authorization: Bearer {token}\r\n"), ""), NOW));
    assert_eq!(me["races"]["nw-k"], 17);
    // No sign-in, no score; a free name through the open board is refused; a reset marker and a plain message pass.
    assert!(http(&a, &race("nope", "nw-k", 2, 99), NOW).starts_with("HTTP/1.1 401"));
    let out = http(&a, &post("nw-k", r#"{"n":"Hacker","s":99,"p":2,"t":1}"#), NOW);
    assert!(out.starts_with("HTTP/1.1 403"), "{out}");
    assert!(!http(&a, &get("nw-k", "12h"), NOW).contains("Hacker"));
    assert!(http(&a, &post("nw-k", r#"{"reset":1,"t":1}"#), NOW).starts_with("HTTP/1.1 200"));
    assert!(http(&a, &race(&token, "bad topic!", 2, 5), NOW).starts_with("HTTP/1.1 400"));
}
