//! `lesson-server publish`: the URL watcher. A quick tunnel gets a new https://X.trycloudflare.com each time
//! cloudflared starts. This asks cloudflared's metrics port for the hostname; once our server answers through it,
//! it writes the address to the url file (for install.sh and the proof), then writes board.json {"url": ...} in its
//! own clone of the site and pushes only that file, so GitHub Pages serves it beside the page. Its own clone: it
//! never touches a checkout someone works in. curl and git do the network and the repo (both already here, and git
//! carries the GitHub login).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// 15 s (kept from the race-board watcher): a tunnel restart is on GitHub within a quarter minute plus the push;
/// a local poll this rare costs nothing.
const EVERY: Duration = Duration::from_secs(15);
/// curl's limit for the metrics port: it is on this machine and answered in under 1 ms (measured); 5 s only covers a
/// busy box.
const METRICS_WAIT: &str = "5";
/// curl's limit for /v1/health through the tunnel: through a live quick tunnel it answered in 0.03 to 0.78 s
/// (5 tries, measured); 10 s leaves room for a slow edge without holding the loop past one poll.
const HEALTH_WAIT: &str = "10";
/// Push tries before waiting a poll: a push loses only to a commit landed meanwhile, and one rebase fixes that;
/// three covers two such races in a row.
const PUSH_TRIES: usize = 3;

fn run(dir: &Path, prog: &str, args: &[&str]) -> Option<String> {
    let o = Command::new(prog).args(args).current_dir(dir).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// The value of a string field in a small flat JSON object (cloudflared's answer, our own board.json).
pub fn json_field(s: &str, key: &str) -> Option<String> {
    let rest = &s[s.find(&format!("\"{key}\""))? + key.len() + 2..];
    let rest = &rest[rest.find(':')? + 1..];
    let rest = &rest[rest.find('"')? + 1..];
    Some(rest[..rest.find('"')?].to_string())
}

pub fn board_json(url: &str) -> String {
    format!("{{\"url\": \"{url}\"}}\n")
}

/// Writes `url` as the whole url file: into a temp file beside it, then renamed over it, so a reader never sees
/// half an address and an old one is replaced, never appended to.
pub fn write_url_file(path: &Path, url: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, format!("{url}\n"))?;
    std::fs::rename(&tmp, path)
}

pub fn main() {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let state = home.join(".local/state/lesson-server");
    let repo = std::env::var_os("REPO_DIR").map(PathBuf::from).unwrap_or_else(|| state.join("repo"));
    let url_file = std::env::var_os("LESSON_URL_FILE").map(PathBuf::from).unwrap_or_else(|| state.join("url"));
    let remote = std::env::var("REMOTE").unwrap_or_else(|_| "https://github.com/vaknin/nature-words.git".into());
    // 8796: our cloudflared's metrics port (see lesson-tunnel.service).
    let metrics = std::env::var("METRICS").unwrap_or_else(|_| "http://127.0.0.1:8796".into());
    if !repo.join(".git").exists() {
        let parent = repo.parent().unwrap_or(Path::new("/"));
        let _ = std::fs::create_dir_all(parent);
        run(parent, "git", &["clone", "-q", &remote, &repo.to_string_lossy()]).expect("git clone");
    }
    let mut written = String::new();
    let mut published = String::new();
    loop {
        if let Some(url) = live_url(&repo, &metrics) {
            if url != written && write_url_file(&url_file, &url).is_ok() {
                eprintln!("publish: {url} written to {}", url_file.display());
                written = url.clone();
            }
            if url != published && push(&repo, &url) {
                published = url;
            }
        }
        std::thread::sleep(EVERY);
    }
}

/// The tunnel's address, once our server answers /v1/health through it.
fn live_url(repo: &Path, metrics: &str) -> Option<String> {
    let host = json_field(&run(repo, "curl", &["-fsS", "-m", METRICS_WAIT, &format!("{metrics}/quicktunnel")])?, "hostname")?;
    if host.is_empty() {
        return None;
    }
    let url = format!("https://{host}");
    // A new quick tunnel takes a few seconds to resolve: use it only once the server answers through it.
    if !run(repo, "curl", &["-fsS", "-m", HEALTH_WAIT, &format!("{url}/v1/health")]).is_some_and(|b| b.contains("healthy")) {
        eprintln!("publish: {url} not answering yet");
        return None;
    }
    Some(url)
}

/// Pushes board.json with `url`; true once that is what GitHub has.
fn push(repo: &Path, url: &str) -> bool {
    let msg = "board.json: the lesson server's live address";
    let Some(_) = run(repo, "git", &["fetch", "-q", "origin", "main"]) else { return false };
    let Some(_) = run(repo, "git", &["checkout", "-q", "-B", "main", "origin/main"]) else { return false };
    let file = repo.join("board.json");
    if std::fs::read_to_string(&file).ok().and_then(|s| json_field(&s, "url")).as_deref() == Some(url) {
        eprintln!("publish: {url} already on GitHub");
        return true;
    }
    if std::fs::write(&file, board_json(url)).is_err()
        || run(repo, "git", &["add", "board.json"]).and_then(|_| run(repo, "git", &["commit", "-qm", msg, "--", "board.json"])).is_none()
    {
        return false;
    }
    for _ in 0..PUSH_TRIES {
        if run(repo, "git", &["push", "-q", "origin", "main"]).is_some() {
            eprintln!("publish: {url} pushed");
            return true;
        }
        run(repo, "git", &["pull", "-q", "--rebase", "origin", "main"]);
    }
    eprintln!("publish: push of {url} failed, trying again");
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_cloudflared_and_board_json() {
        assert_eq!(json_field(r#"{"hostname":"a-b.trycloudflare.com"}"#, "hostname").as_deref(), Some("a-b.trycloudflare.com"));
        assert_eq!(json_field(&board_json("https://x.trycloudflare.com"), "url").as_deref(), Some("https://x.trycloudflare.com"));
        assert_eq!(json_field("{}", "url"), None);
    }
    #[test]
    fn url_file_written() {
        let dir = std::env::temp_dir().join(format!("lesson-url-{}", std::process::id()));
        let f = dir.join("state/url");
        write_url_file(&f, "https://old-one.trycloudflare.com").unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "https://old-one.trycloudflare.com\n");
        write_url_file(&f, "https://new-two.trycloudflare.com").unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "https://new-two.trycloudflare.com\n");
        assert!(!f.with_extension("tmp").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
