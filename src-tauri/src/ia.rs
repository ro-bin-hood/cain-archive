use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, COOKIE, LOCATION, USER_AGENT};
use crate::error::{AppError, AppResult};
use reqwest::{redirect::Policy, Client, Response, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

pub const BASE_URL: &str = "https://archive.org";
pub const UA: &str = "cain-archive/2.0";
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
    pub title: Option<String>,
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

pub fn is_ident(s: &str) -> bool {
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

/// Client without automatic redirects: `get` follows them and knows when to keep credentials.
pub fn client() -> Client {
    Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(30))
        .read_timeout(Duration::from_secs(60))
        .build()
        .expect("HTTP client")
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
    // Never send credentials in clear text after starting on HTTPS.
    if orig.scheme() == "https" && next.scheme() != "https" {
        return false;
    }
    next.host_str() == orig.host_str()
        || next.host_str().is_some_and(|h| h == "archive.org" || h.ends_with(".archive.org"))
}

/// Largest JSON body accepted from archive.org (metadata of items with very many files runs to
/// tens of MB).
const MAX_JSON: usize = 256 << 20;

/// Reads a JSON body, refusing anything larger than `MAX_JSON` instead of filling memory.
async fn json_capped(mut resp: Response) -> AppResult<Value> {
    let too_large = || AppError::InvalidResponse { detail: "response too large".into() };
    if resp.content_length().is_some_and(|n| n > MAX_JSON as u64) {
        return Err(too_large());
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| AppError::Network { detail: e.to_string() })? {
        if body.len() + chunk.len() > MAX_JSON {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|e| AppError::InvalidResponse { detail: e.to_string() })
}

/// GET that follows redirects by hand. reqwest would drop Cookie/Authorization when the redirect
/// changes host (archive.org/download → iaNNN.us.archive.org), but archive.org needs them.
pub async fn get(client: &Client, url: &str, headers: &HeaderMap) -> AppResult<Response> {
    let orig = Url::parse(url).map_err(AppError::other)?;
    let mut url = orig.clone();
    for _ in 0..10 {
        let mut h = headers.clone();
        if !trusted(&orig, &url) {
            h.remove(COOKIE);
            h.remove(AUTHORIZATION);
        }
        let resp = client.get(url.clone()).headers(h).send().await.map_err(|e| AppError::Network { detail: e.to_string() })?;
        if !resp.status().is_redirection() {
            return Ok(resp);
        }
        let loc = resp.headers().get(LOCATION).and_then(|v| v.to_str().ok()).ok_or(AppError::RedirectWithoutLocation)?;
        url = url.join(loc).map_err(AppError::other)?;
    }
    Err(AppError::TooManyRedirects)
}

/// `metadata.title` can be a string or a list of strings.
pub fn title_of(meta: &Value) -> Option<String> {
    match meta.pointer("/metadata/title")? {
        Value::String(s) => Some(s.clone()),
        Value::Array(a) => a.first()?.as_str().map(str::to_string),
        _ => None,
    }
}

pub async fn fetch_item(client: &Client, base: &str, auth: Option<&Auth>, id: &str) -> AppResult<Item> {
    let resp = get(client, &format!("{base}/metadata/{id}"), &auth_headers(auth)).await?;
    if !resp.status().is_success() {
        return Err(AppError::Http { status: resp.status().as_u16() });
    }
    let meta = json_capped(resp).await?;
    let files = files_from_metadata(&meta);
    if files.is_empty() {
        return Err(if auth.is_some() { AppError::ItemUnavailable } else { AppError::ItemUnavailableLogIn });
    }
    Ok(Item { id: id.to_string(), title: title_of(&meta), files })
}

pub fn parse_login_response(v: &Value, email: &str) -> AppResult<Auth> {
    if v.get("success").and_then(Value::as_bool) != Some(true) {
        let reason = v.pointer("/values/reason").and_then(Value::as_str).unwrap_or("unknown");
        return Err(match reason {
            "account_not_found" => AppError::AccountNotFound,
            "account_bad_password" => AppError::WrongPassword,
            r => AppError::LoginFailed { reason: r.to_string() },
        });
    }
    let s = |p: &str| v.pointer(p).and_then(Value::as_str).map(str::to_string).ok_or(AppError::IncompleteLoginResponse);
    Ok(Auth {
        user: s("/values/screenname").unwrap_or_else(|_| email.to_string()),
        cookie_user: s("/values/cookies/logged-in-user")?,
        cookie_sig: s("/values/cookies/logged-in-sig")?,
        access: s("/values/s3/access")?,
        secret: s("/values/s3/secret")?,
    })
}

pub async fn login(client: &Client, base: &str, email: &str, password: &str) -> AppResult<Auth> {
    let resp = client
        .post(format!("{base}/services/xauthn/?op=login"))
        .header(USER_AGENT, UA)
        .form(&[("email", email), ("password", password)])
        .send()
        .await
        .map_err(|e| AppError::Network { detail: e.to_string() })?;
    let v = json_capped(resp).await?;
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
        assert_eq!(e("account_not_found"), AppError::AccountNotFound);
        assert_eq!(e("account_bad_password"), AppError::WrongPassword);
        assert_eq!(e("other"), AppError::LoginFailed { reason: "other".into() });
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
    #[test]
    fn title_is_read_from_string_or_first_of_list() {
        assert_eq!(title_of(&json!({"metadata": {"title": "Xbox (A)"}})), Some("Xbox (A)".into()));
        assert_eq!(title_of(&json!({"metadata": {"title": ["Primo", "Secondo"]}})), Some("Primo".into()));
        assert_eq!(title_of(&json!({"metadata": {}})), None);
        assert_eq!(title_of(&json!({})), None);
    }
}
