use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, COOKIE, LOCATION, USER_AGENT};
use reqwest::{redirect::Policy, Client, Response, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

pub const BASE_URL: &str = "https://archive.org";
pub const UA: &str = "ia-downloader/2.0";
const SKIP_SUFFIX: [&str; 5] = ["_meta.xml", "_files.xml", "_meta.sqlite", "_archive.torrent", "_reviews.xml"];

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedLink {
    pub item_id: String,
    pub file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub size: u64,
    pub format: String,
    pub original: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Item {
    pub id: String,
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Auth {
    pub user: String,
    pub cookie_user: String,
    pub cookie_sig: String,
    pub access: String,
    pub secret: String,
}

fn is_ident(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

pub fn parse_link(input: &str) -> Option<ParsedLink> {
    let s = input.trim();
    let Some(rest) = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://")) else {
        return is_ident(s).then(|| ParsedLink { item_id: s.to_string(), file: None });
    };
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    let rest = rest.strip_prefix("archive.org/")?;
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let (kind, rest) = rest.split_once('/')?;
    let (id, path) = match rest.split_once('/') {
        Some((i, p)) => (i, p),
        None => (rest, ""),
    };
    if !is_ident(id) {
        return None;
    }
    let file = match kind {
        "details" => None,
        "download" if path.is_empty() => None,
        "download" => Some(urlencoding::decode(path).ok()?.into_owned()),
        _ => return None,
    };
    Some(ParsedLink { item_id: id.to_string(), file })
}

pub fn files_from_metadata(meta: &Value) -> Vec<FileEntry> {
    let Some(files) = meta.get("files").and_then(Value::as_array) else { return vec![] };
    files
        .iter()
        .filter_map(|f| {
            let name = f.get("name")?.as_str()?.to_string();
            if SKIP_SUFFIX.iter().any(|s| name.ends_with(s)) {
                return None;
            }
            let size = match f.get("size") {
                Some(Value::String(s)) => s.parse().unwrap_or(0),
                Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
                _ => 0,
            };
            let format = f.get("format").and_then(Value::as_str).unwrap_or("").to_string();
            let original = f.get("source").and_then(Value::as_str) == Some("original");
            Some(FileEntry { name, size, format, original })
        })
        .collect()
}

/// Client senza redirect automatici: li segue `get`, che sa quando tenere le credenziali.
pub fn client() -> Client {
    Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(30))
        .read_timeout(Duration::from_secs(60))
        .build()
        .expect("client HTTP")
}

pub fn auth_headers(auth: Option<&Auth>) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(USER_AGENT, HeaderValue::from_static(UA));
    if let Some(a) = auth {
        if let Ok(v) = HeaderValue::from_str(&format!("logged-in-user={}; logged-in-sig={}", a.cookie_user, a.cookie_sig)) {
            h.insert(COOKIE, v);
        }
        if let Ok(v) = HeaderValue::from_str(&format!("LOW {}:{}", a.access, a.secret)) {
            h.insert(AUTHORIZATION, v);
        }
    }
    h
}

fn trusted(orig: &Url, next: &Url) -> bool {
    next.host_str() == orig.host_str()
        || next.host_str().is_some_and(|h| h == "archive.org" || h.ends_with(".archive.org"))
}

/// GET che segue i redirect a mano. reqwest toglierebbe Cookie/Authorization quando il redirect
/// cambia host (archive.org/download → iaNNN.us.archive.org), ma archive.org ne ha bisogno.
pub async fn get(client: &Client, url: &str, headers: &HeaderMap) -> Result<Response, String> {
    let orig = Url::parse(url).map_err(|e| e.to_string())?;
    let mut url = orig.clone();
    for _ in 0..10 {
        let mut h = headers.clone();
        if !trusted(&orig, &url) {
            h.remove(COOKIE);
            h.remove(AUTHORIZATION);
        }
        let resp = client.get(url.clone()).headers(h).send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_redirection() {
            return Ok(resp);
        }
        let loc = resp.headers().get(LOCATION).and_then(|v| v.to_str().ok()).ok_or("redirect senza Location")?;
        url = url.join(loc).map_err(|e| e.to_string())?;
    }
    Err("troppi redirect".into())
}

pub async fn fetch_item(client: &Client, base: &str, auth: Option<&Auth>, id: &str) -> Result<Item, String> {
    let resp = get(client, &format!("{base}/metadata/{id}"), &auth_headers(auth))
        .await
        .map_err(|e| format!("Errore di rete: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Errore HTTP {}", resp.status().as_u16()));
    }
    let meta: Value = resp.json().await.map_err(|e| format!("Risposta non valida: {e}"))?;
    let files = files_from_metadata(&meta);
    if files.is_empty() {
        return Err(if auth.is_some() {
            "Item vuoto, inesistente o riservato".into()
        } else {
            "Item vuoto, inesistente o ad accesso ristretto (prova ad accedere)".into()
        });
    }
    Ok(Item { id: id.to_string(), files })
}

pub fn parse_login_response(v: &Value, email: &str) -> Result<Auth, String> {
    if v.get("success").and_then(Value::as_bool) != Some(true) {
        let reason = v.pointer("/values/reason").and_then(Value::as_str).unwrap_or("sconosciuto");
        return Err(match reason {
            "account_not_found" => "Account inesistente".into(),
            "account_bad_password" => "Password errata".into(),
            r => format!("Login fallito ({r})"),
        });
    }
    let s = |p: &str| v.pointer(p).and_then(Value::as_str).map(str::to_string).ok_or_else(|| "Risposta di login incompleta".to_string());
    Ok(Auth {
        user: s("/values/screenname").unwrap_or_else(|_| email.to_string()),
        cookie_user: s("/values/cookies/logged-in-user")?,
        cookie_sig: s("/values/cookies/logged-in-sig")?,
        access: s("/values/s3/access")?,
        secret: s("/values/s3/secret")?,
    })
}

pub async fn login(client: &Client, base: &str, email: &str, password: &str) -> Result<Auth, String> {
    let resp = client
        .post(format!("{base}/services/xauthn/?op=login"))
        .header(USER_AGENT, UA)
        .form(&[("email", email), ("password", password)])
        .send()
        .await
        .map_err(|e| format!("Errore di rete: {e}"))?;
    let v: Value = resp.json().await.map_err(|e| format!("Risposta non valida: {e}"))?;
    parse_login_response(&v, email)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn p(id: &str, file: Option<&str>) -> Option<ParsedLink> {
        Some(ParsedLink { item_id: id.into(), file: file.map(Into::into) })
    }

    #[test]
    fn parses_all_link_forms() {
        assert_eq!(parse_link("https://archive.org/details/nasa"), p("nasa", None));
        assert_eq!(parse_link("  http://www.archive.org/details/nasa/?tab=1  "), p("nasa", None));
        assert_eq!(parse_link("https://archive.org/download/nasa"), p("nasa", None));
        assert_eq!(parse_link("https://archive.org/download/nasa/dir/a%20b%23.zip"), p("nasa", Some("dir/a b#.zip")));
        assert_eq!(parse_link("nasa_2020-v1.0"), p("nasa_2020-v1.0", None));
    }

    #[test]
    fn rejects_invalid_links() {
        for bad in ["", "   ", "https://example.com/details/x", "https://archive.org/search?q=x", "two words", "https://archive.org/details/"] {
            assert_eq!(parse_link(bad), None, "{bad}");
        }
    }

    #[test]
    fn metadata_skips_service_files_and_parses_sizes() {
        let meta = json!({"files": [
            {"name": "a.flac", "size": "1200", "format": "Flac", "source": "original"},
            {"name": "b.mp3", "size": 300, "source": "derivative"},
            {"name": "x_meta.xml"}, {"name": "x_files.xml"}, {"name": "x_meta.sqlite"},
            {"name": "x_archive.torrent"}, {"name": "x_reviews.xml"},
            {"name": "c.txt"}
        ]});
        let files = files_from_metadata(&meta);
        assert_eq!(files, vec![
            FileEntry { name: "a.flac".into(), size: 1200, format: "Flac".into(), original: true },
            FileEntry { name: "b.mp3".into(), size: 300, format: "".into(), original: false },
            FileEntry { name: "c.txt".into(), size: 0, format: "".into(), original: false },
        ]);
        assert!(files_from_metadata(&json!({})).is_empty());
    }

    #[test]
    fn auth_headers_carry_cookie_and_s3_key() {
        let a = Auth { user: "u".into(), cookie_user: "me%40x.it".into(), cookie_sig: "SIG".into(), access: "AK".into(), secret: "SK".into() };
        let h = auth_headers(Some(&a));
        assert_eq!(h["cookie"], "logged-in-user=me%40x.it; logged-in-sig=SIG");
        assert_eq!(h["authorization"], "LOW AK:SK");
        assert_eq!(h["user-agent"], UA);
        let anon = auth_headers(None);
        assert!(anon.get("cookie").is_none() && anon.get("authorization").is_none());
    }

    #[test]
    fn login_response_errors_are_readable() {
        let e = |r: &str| parse_login_response(&json!({"success": false, "values": {"reason": r}}), "x").unwrap_err();
        assert_eq!(e("account_not_found"), "Account inesistente");
        assert_eq!(e("account_bad_password"), "Password errata");
        assert_eq!(e("other"), "Login fallito (other)");
    }

    #[test]
    fn credentials_follow_only_trusted_redirects() {
        let o = Url::parse("http://127.0.0.1:1/download/x").unwrap();
        assert!(trusted(&o, &Url::parse("http://127.0.0.1:1/other").unwrap()));
        assert!(trusted(&o, &Url::parse("https://ia800.us.archive.org/f").unwrap()));
        assert!(trusted(&o, &Url::parse("https://archive.org/f").unwrap()));
        assert!(!trusted(&o, &Url::parse("https://evil-archive.org/f").unwrap()));
        assert!(!trusted(&o, &Url::parse("https://example.com/f").unwrap()));
    }
}
