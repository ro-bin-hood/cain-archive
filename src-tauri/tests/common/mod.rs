#![allow(dead_code)]
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Form, Json, Router,
};
use futures_util::StreamExt;
use serde_json::json;
use std::{collections::HashMap, sync::{Arc, Mutex}, time::Duration};

/// Deterministic content of a fake file.
pub fn content(item: &str, name: &str) -> Vec<u8> {
    format!("{item}/{name}|").repeat(200).into_bytes()
}

#[derive(Clone, Default)]
pub struct Srv {
    pub hits: Arc<Mutex<HashMap<String, u32>>>,
    pub ranges: Arc<Mutex<Vec<String>>>,
}

pub async fn start() -> (String, Srv) {
    let srv = Srv::default();
    let app = Router::new()
        .route("/download/{item}/{*name}", get(download))
        .route("/metadata/{item}", get(metadata))
        .route("/services/xauthn/", post(login))
        .with_state(srv.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), srv)
}

async fn download(State(s): State<Srv>, Path((item, name)): Path<(String, String)>, headers: HeaderMap) -> Response {
    let hit = {
        let mut h = s.hits.lock().unwrap();
        let c = h.entry(item.clone()).or_default();
        *c += 1;
        *c
    };
    let range = headers.get(header::RANGE).and_then(|v| v.to_str().ok()).map(str::to_string);
    if let Some(r) = &range {
        s.ranges.lock().unwrap().push(r.clone());
    }
    let key = name.strip_prefix("real/").unwrap_or(&name).to_string();
    let body = content(&item, &key);
    match item.as_str() {
        "denied" => StatusCode::FORBIDDEN.into_response(),
        "notfound" => StatusCode::NOT_FOUND.into_response(),
        "busy" if hit == 1 => (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, "0")]).into_response(),
        "authonly" if !name.starts_with("real/") => {
            (StatusCode::FOUND, [(header::LOCATION, format!("/download/authonly/real/{name}"))]).into_response()
        }
        "authonly" => {
            let ok = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()).is_some_and(|c| c.contains("logged-in-sig=SIG"));
            if ok { respond(body, range.as_deref(), false) } else { StatusCode::FORBIDDEN.into_response() }
        }
        "norange" => respond(body, None, false),
        "drop" if hit == 1 => {
            let len = body.len();
            let half = body[..len / 2].to_vec();
            // The pause makes the first half actually reach the client before the connection drops.
            let stream = futures_util::stream::iter(vec![Ok::<_, std::io::Error>(half), Err(std::io::Error::other("drop"))])
                .then(|r| async move {
                    if r.is_err() {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                    r
                });
            Response::builder().header(header::CONTENT_LENGTH, len).body(Body::from_stream(stream)).unwrap()
        }
        "dropmany" if hit <= 4 => {
            let len = body.len();
            let start = range.as_deref().and_then(|r| r.strip_prefix("bytes=")).and_then(|r| r.strip_suffix('-')).and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
            let piece = body[start..(start + len / 5).min(len)].to_vec();
            let stream = futures_util::stream::iter(vec![Ok::<_, std::io::Error>(piece), Err(std::io::Error::other("drop"))]).then(|r| async move {
                if r.is_err() {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                r
            });
            let status = if start > 0 { StatusCode::PARTIAL_CONTENT } else { StatusCode::OK };
            Response::builder().status(status).header(header::CONTENT_LENGTH, len - start).body(Body::from_stream(stream)).unwrap()
        }
        "slow" => respond(body, range.as_deref(), true),
        _ => respond(body, range.as_deref(), false),
    }
}

fn respond(body: Vec<u8>, range: Option<&str>, slow: bool) -> Response {
    let len = body.len();
    let start = range
        .and_then(|r| r.strip_prefix("bytes="))
        .and_then(|r| r.strip_suffix('-'))
        .and_then(|n| n.parse::<usize>().ok());
    let (status, slice, content_range) = match start {
        Some(st) if st >= len => {
            return (StatusCode::RANGE_NOT_SATISFIABLE, [(header::CONTENT_RANGE, format!("bytes */{len}"))]).into_response()
        }
        Some(st) => (StatusCode::PARTIAL_CONTENT, body[st..].to_vec(), Some(format!("bytes {st}-{}/{len}", len - 1))),
        None => (StatusCode::OK, body, None),
    };
    let mut b = Response::builder().status(status).header(header::CONTENT_LENGTH, slice.len());
    if let Some(cr) = content_range {
        b = b.header(header::CONTENT_RANGE, cr);
    }
    if !slow {
        return b.body(Body::from(slice)).unwrap();
    }
    let chunks: Vec<Vec<u8>> = slice.chunks(slice.len() / 20 + 1).map(|c| c.to_vec()).collect();
    let stream = futures_util::stream::iter(chunks).then(|c| async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok::<_, std::io::Error>(c)
    });
    b.body(Body::from_stream(stream)).unwrap()
}

async fn metadata(State(s): State<Srv>, Path(item): Path<String>) -> Json<serde_json::Value> {
    let hit = {
        let mut h = s.hits.lock().unwrap();
        let c = h.entry(format!("meta:{item}")).or_default();
        *c += 1;
        *c
    };
    if item == "missing" || (item == "flaky" && hit > 1) {
        return Json(json!({}));
    }
    Json(json!({
        "metadata": {"title": format!("Titolo {item}")},
        "files": [
            {"name": "a.flac", "size": "1200", "format": "Flac", "source": "original"},
            {"name": "a.mp3", "size": "300", "format": "VBR MP3", "source": "derivative"},
            {"name": format!("{item}_meta.xml"), "source": "original"},
            {"name": "sub/b.pdf", "size": "50", "format": "Text PDF", "source": "original"}
        ]
    }))
}

#[derive(serde::Deserialize)]
struct LoginForm { email: String, password: String }

async fn login(Form(f): Form<LoginForm>) -> Response {
    if f.email != "me@x.it" {
        return (StatusCode::UNAUTHORIZED, Json(json!({"success": false, "values": {"reason": "account_not_found"}}))).into_response();
    }
    if f.password != "pw" {
        return (StatusCode::UNAUTHORIZED, Json(json!({"success": false, "values": {"reason": "account_bad_password"}}))).into_response();
    }
    Json(json!({"success": true, "values": {
        "screenname": "tester",
        "cookies": {"logged-in-user": "me%40x.it", "logged-in-sig": "SIG"},
        "s3": {"access": "AK", "secret": "SK"}
    }})).into_response()
}
