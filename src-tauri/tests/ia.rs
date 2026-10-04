mod common;
use ia_downloader_lib::ia;

#[tokio::test]
async fn fetch_item_lists_files_without_service_files() {
    let (base, _) = common::start().await;
    let item = ia::fetch_item(&ia::client(), &base, None, "nasa").await.unwrap();
    let names: Vec<_> = item.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(item.id, "nasa");
    assert_eq!(names, ["a.flac", "a.mp3", "sub/b.pdf"]);
}

#[tokio::test]
async fn fetch_item_reports_missing_items() {
    let (base, _) = common::start().await;
    let err = ia::fetch_item(&ia::client(), &base, None, "missing").await.unwrap_err();
    assert!(err.contains("prova ad accedere"), "{err}");
}

#[tokio::test]
async fn login_returns_session_or_readable_error() {
    let (base, _) = common::start().await;
    let c = ia::client();
    let a = ia::login(&c, &base, "me@x.it", "pw").await.unwrap();
    assert_eq!((a.user.as_str(), a.cookie_sig.as_str(), a.access.as_str(), a.secret.as_str()), ("tester", "SIG", "AK", "SK"));
    assert_eq!(ia::login(&c, &base, "me@x.it", "no").await.unwrap_err(), "Password errata");
    assert_eq!(ia::login(&c, &base, "x@x.it", "pw").await.unwrap_err(), "Account inesistente");
}

#[tokio::test]
async fn get_follows_redirect_keeping_credentials() {
    let (base, _) = common::start().await;
    let c = ia::client();
    let a = ia::Auth { user: "u".into(), cookie_user: "u".into(), cookie_sig: "SIG".into(), access: "a".into(), secret: "s".into() };
    let url = format!("{base}/download/authonly/f.bin");
    let ok = ia::get(&c, &url, &ia::auth_headers(Some(&a))).await.unwrap();
    assert_eq!(ok.status(), 200);
    assert_eq!(ok.bytes().await.unwrap().to_vec(), common::content("authonly", "f.bin"));
    let anon = ia::get(&c, &url, &ia::auth_headers(None)).await.unwrap();
    assert_eq!(anon.status(), 403);
}
