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
