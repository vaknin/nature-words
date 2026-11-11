//! `nature-board publish`: the URL watcher. A quick tunnel gets a new https://X.trycloudflare.com each time
//! cloudflared starts. This asks cloudflared's metrics port for the hostname; when it changes and the board answers
//! through it, it writes board.json {"url": ...} in its own clone of the site and pushes only that file, so GitHub
//! Pages serves it beside the page. Its own clone: it never touches a checkout someone works in.
//! curl and git do the network and the repo (both already here, and git carries the GitHub login).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// 15 s: a tunnel restart is on GitHub within a quarter minute plus the push; a local poll this rare costs nothing.
const EVERY: Duration = Duration::from_secs(15);

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

pub fn main() {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let repo = std::env::var_os("REPO_DIR").map(PathBuf::from).unwrap_or_else(|| home.join(".local/state/nature-board/repo"));
    let remote = std::env::var("REMOTE").unwrap_or_else(|_| "https://github.com/vaknin/nature-words.git".into());
    let metrics = std::env::var("METRICS").unwrap_or_else(|_| "http://127.0.0.1:8798".into());
    if !repo.join(".git").exists() {
        let parent = repo.parent().unwrap_or(Path::new("/"));
        let _ = std::fs::create_dir_all(parent);
        run(parent, "git", &["clone", "-q", &remote, &repo.to_string_lossy()]).expect("git clone");
    }
    let mut published = String::new();
    loop {
        if let Some(url) = tick(&repo, &metrics, &published) {
            published = url;
        }
        std::thread::sleep(EVERY);
    }
}

/// One look: Some(url) once `url` is what board.json on GitHub says.
fn tick(repo: &Path, metrics: &str, published: &str) -> Option<String> {
    let host = json_field(&run(repo, "curl", &["-fsS", "-m5", &format!("{metrics}/quicktunnel")])?, "hostname")?;
    let url = format!("https://{host}");
    if host.is_empty() || url == published {
        return None;
    }
    // A new quick tunnel takes a few seconds to resolve: publish only once the board answers through it.
    if !run(repo, "curl", &["-fsS", "-m10", &format!("{url}/v1/health")]).is_some_and(|b| b.contains("healthy")) {
        eprintln!("publish: {url} not answering yet");
        return None;
    }
    run(repo, "git", &["fetch", "-q", "origin", "main"])?;
    run(repo, "git", &["checkout", "-q", "-B", "main", "origin/main"])?;
    let file = repo.join("board.json");
    if std::fs::read_to_string(&file).ok().and_then(|s| json_field(&s, "url")).as_deref() == Some(url.as_str()) {
        eprintln!("publish: {url} already on GitHub");
        return Some(url);
    }
    std::fs::write(&file, board_json(&url)).ok()?;
    run(repo, "git", &["commit", "-qm", "board.json: the race boards' live server address", "--", "board.json"])
        .or_else(|| run(repo, "git", &["add", "board.json"]).and_then(|_| run(repo, "git", &["commit", "-qm", "board.json: the race boards' live server address", "--", "board.json"])))?;
    for _ in 0..3 {
        if run(repo, "git", &["push", "-q", "origin", "main"]).is_some() {
            eprintln!("publish: {url} pushed");
            return Some(url);
        }
        run(repo, "git", &["pull", "-q", "--rebase", "origin", "main"]);
    }
    eprintln!("publish: push of {url} failed, trying again");
    None
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
}
