//! Cloudflare Access for the teacher: when ~/.config/lesson-server/access.json exists (LESSON_ACCESS overrides the
//! path), only a request carrying Cloudflare's signed Access token for his one allowed email is the teacher's. The
//! token comes in the Cf-Access-Jwt-Assertion header (Cloudflare adds it on every path its Access application
//! covers) or, from the Kitah app, in the CF_Authorization cookie together with the app's X-Kitah-App header (so a
//! page on another site can't ride the cookie: a custom header needs a CORS preflight this server never grants with
//! credentials). The teacher key is then refused. Without the file the teacher key works as before.
//!
//! access.json: {"team": "NAME" (or NAME.cloudflareaccess.com), "aud": "the application's AUD tag",
//!               "email": "the one allowed address", "certs": "optional local JWKS file, for tests"}
//! The token is an RS256 JWT (https://developers.cloudflare.com/cloudflare-one/access-controls/applications/http-apps/
//! authorization-cookie/validating-json/): its signature must verify against the team's keys from
//! https://TEAM.cloudflareaccess.com/cdn-cgi/access/certs, iss must be the team URL, aud must hold the tag, exp and
//! nbf must hold now, and email must be the allowed one (case ignored).

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rsa::pkcs1v15::{Signature, VerifyingKey};
use rsa::signature::Verifier;
use rsa::{BigUint, RsaPublicKey};
use serde_json::Value;
use sha2::Sha256;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// The team's keys are kept 1 hour. Cloudflare rotates its Access signing key every 6 weeks and keeps the previous
/// one valid 7 days (the page above); a token with a key not in the cache makes a fresh fetch anyway, so this only
/// bounds how long a key Cloudflare withdrew early (a manual rotation after a security event) is still trusted.
pub const CERTS_KEEP_S: u64 = 3600;
/// A token naming a key we don't have refetches at most once a minute, so made-up key ids can't turn every request
/// into a fetch. A real new key is published from its rotation on, with the old one valid 7 days beside it, so a
/// minute's wait never shuts him out.
pub const CERTS_REFETCH_MIN_S: u64 = 60;
/// curl's limit for the certs: through the internet a small JSON answers in well under a second; 10 s is the same
/// limit publish.rs gives its health check through the tunnel (measured 0.03 to 0.78 s there).
const CERTS_WAIT: &str = "10";

pub struct Access {
    pub team_url: String,
    pub aud: String,
    pub email: String,
    certs_file: Option<PathBuf>,
    keys: Mutex<Keys>,
}

#[derive(Default)]
struct Keys {
    by_kid: HashMap<String, RsaPublicKey>,
    fetched: u64,
    tried: u64,
}

fn b64(s: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(s.trim_end_matches('=')).ok()
}

/// The RSA keys of a JWKS document, by key id.
pub fn parse_jwks(text: &str) -> HashMap<String, RsaPublicKey> {
    let mut out = HashMap::new();
    let Ok(v) = serde_json::from_str::<Value>(text) else { return out };
    for k in v.get("keys").and_then(Value::as_array).into_iter().flatten() {
        if k.get("kty").and_then(Value::as_str) != Some("RSA") {
            continue;
        }
        let (Some(kid), Some(n), Some(e)) = (k.get("kid").and_then(Value::as_str), k.get("n").and_then(Value::as_str), k.get("e").and_then(Value::as_str)) else {
            continue;
        };
        if let (Some(n), Some(e)) = (b64(n), b64(e)) {
            if let Ok(pk) = RsaPublicKey::new(BigUint::from_bytes_be(&n), BigUint::from_bytes_be(&e)) {
                out.insert(kid.to_string(), pk);
            }
        }
    }
    out
}

impl Access {
    /// The config at `path`, or None when there is no file (Access off). A file that is there but wrong is an
    /// error: the server must not start open when he meant it closed.
    pub fn load(path: &Path) -> Result<Option<Access>, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let field = |k: &str| v.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(String::from);
        let (Some(team), Some(aud), Some(email)) = (field("team"), field("aud"), field("email")) else {
            return Err(format!("{}: needs team, aud and email", path.display()));
        };
        Ok(Some(Access::new(&team, &aud, &email, field("certs").map(PathBuf::from))))
    }

    pub fn new(team: &str, aud: &str, email: &str, certs_file: Option<PathBuf>) -> Access {
        let host = team.trim_start_matches("https://").trim_end_matches('/');
        let host = if host.contains('.') { host.to_string() } else { format!("{host}.cloudflareaccess.com") };
        Access { team_url: format!("https://{host}"), aud: aud.into(), email: email.into(), certs_file, keys: Mutex::new(Keys::default()) }
    }

    fn fetch(&self) -> Option<HashMap<String, RsaPublicKey>> {
        let text = match &self.certs_file {
            Some(f) => std::fs::read_to_string(f).ok()?,
            None => {
                let url = format!("{}/cdn-cgi/access/certs", self.team_url);
                let o = Command::new("curl").args(["-fsS", "-m", CERTS_WAIT, &url]).output().ok()?;
                if !o.status.success() {
                    return None;
                }
                String::from_utf8(o.stdout).ok()?
            }
        };
        let keys = parse_jwks(&text);
        (!keys.is_empty()).then_some(keys)
    }

    fn key(&self, kid: &str, now: u64) -> Option<RsaPublicKey> {
        let mut k = self.keys.lock().unwrap_or_else(|e| e.into_inner());
        let stale = now >= k.fetched + CERTS_KEEP_S;
        let unknown = !k.by_kid.contains_key(kid) && now >= k.tried + CERTS_REFETCH_MIN_S;
        if stale || unknown {
            k.tried = now;
            if let Some(fresh) = self.fetch() {
                k.by_kid = fresh;
                k.fetched = now;
            }
        }
        k.by_kid.get(kid).cloned()
    }

    /// Ok when `token` is a good Access token for the allowed email at `now`; else why not.
    pub fn verify(&self, token: &str, now: u64) -> Result<(), &'static str> {
        let mut parts = token.trim().split('.');
        let (Some(h), Some(p), Some(s), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else { return Err("not a JWT") };
        let head: Value = b64(h).and_then(|b| serde_json::from_slice(&b).ok()).ok_or("bad header")?;
        if head.get("alg").and_then(Value::as_str) != Some("RS256") {
            return Err("not RS256");
        }
        let kid = head.get("kid").and_then(Value::as_str).ok_or("no kid")?;
        let key = self.key(kid, now).ok_or("unknown key")?;
        let sig = Signature::try_from(b64(s).ok_or("bad signature")?.as_slice()).map_err(|_| "bad signature")?;
        VerifyingKey::<Sha256>::new(key).verify(format!("{h}.{p}").as_bytes(), &sig).map_err(|_| "signature does not verify")?;
        let claims: Value = b64(p).and_then(|b| serde_json::from_slice(&b).ok()).ok_or("bad claims")?;
        if claims.get("iss").and_then(Value::as_str).map(|i| i.trim_end_matches('/')) != Some(self.team_url.as_str()) {
            return Err("wrong issuer");
        }
        let aud_ok = match claims.get("aud") {
            Some(Value::String(a)) => *a == self.aud,
            Some(Value::Array(a)) => a.iter().any(|x| x.as_str() == Some(self.aud.as_str())),
            _ => false,
        };
        if !aud_ok {
            return Err("wrong audience");
        }
        let exp = claims.get("exp").and_then(Value::as_u64).ok_or("no exp")?;
        if now >= exp {
            return Err("expired");
        }
        if claims.get("nbf").and_then(Value::as_u64).is_some_and(|nbf| now < nbf) {
            return Err("not yet valid");
        }
        if !claims.get("email").and_then(Value::as_str).is_some_and(|e| e.trim().eq_ignore_ascii_case(self.email.trim())) {
            return Err("not the allowed email");
        }
        Ok(())
    }
}

/// The CF_Authorization cookie's value in a Cookie header, if any.
pub fn cookie_token(cookie: &str) -> Option<&str> {
    cookie.split(';').find_map(|kv| kv.trim().strip_prefix("CF_Authorization=")).map(str::trim).filter(|t| !t.is_empty())
}
