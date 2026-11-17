use tauri::Url;
use tauri::test::MockRuntime;
use tauri::webview::NewWindowResponse;

use crate::security::{allows_navigation, new_window_response};

#[test]
fn navigation_only_accepts_the_packaged_app_origin() {
    for address in [
        "tauri://localhost",
        "tauri://localhost/index.html",
        "tauri://localhost/app?tab=home#section",
    ] {
        let url = Url::parse(address).expect("허용 주소");
        assert!(allows_navigation(&url), "{address}");
    }
    for address in [
        "https://example.com/",
        "http://localhost/",
        "http://tauri.localhost/",
        "tauri://example.com/",
        "tauri://localhost.example.com/",
        "tauri://localhost:1234/",
        "tauri://user@localhost/",
        "tauri://user:pass@localhost/",
        "file:///tmp/index.html",
        "about:blank",
        "javascript:alert(1)",
        "data:text/html,example",
    ] {
        let url = Url::parse(address).expect("거부 주소");
        assert!(!allows_navigation(&url), "{address}");
    }
}

#[test]
fn new_window_requests_are_denied() {
    let NewWindowResponse::Deny = new_window_response::<MockRuntime>() else {
        panic!("새 창이 거부되어야 한다");
    };
}
