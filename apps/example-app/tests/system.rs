//! Boots the real binary and drives it over HTTP.
//!
//! Unit tests cover each rule in isolation; this exercises the assembled system:
//! plugin registration, migrations, the auth middleware, and the auth-users
//! routes all have to agree for these to pass.

use std::net::TcpListener;
use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tempfile::TempDir;
use tokio::process::{Child, Command};

const ADMIN: &str = "admin";
const PASSWORD: &str = "system-test-password";
const SECRET: &str = "0123456789abcdef0123456789abcdef0123456789abcdef";

/// Kills the server even when an assertion unwinds the test.
struct Server {
    child: Child,
    base: String,
    _workdir: TempDir,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().expect("read bound port").port()
}

async fn start() -> Server {
    let workdir = TempDir::new().expect("temp working directory");
    let port = free_port();

    let child = Command::new(env!("CARGO_BIN_EXE_example-app"))
        .current_dir(workdir.path())
        .env("PORT", port.to_string())
        .env("JWT_SECRET", SECRET)
        .env("YEOLLIN_ADMIN_USERNAME", ADMIN)
        .env("YEOLLIN_ADMIN_PASSWORD", PASSWORD)
        .env_remove("YEOLLIN_DEV_PROXY")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn example-app");

    let server = Server {
        child,
        base: format!("http://127.0.0.1:{port}"),
        _workdir: workdir,
    };

    let client = reqwest::Client::new();
    for _ in 0..120 {
        if let Ok(response) = client.get(server.url("/health")).send().await {
            if response.status().is_success() {
                return server;
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    panic!("server did not become ready");
}

impl Server {
    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }
}

async fn login(client: &reqwest::Client, server: &Server, password: &str) -> reqwest::Response {
    client
        .post(server.url("/api/auth/login"))
        .json(&serde_json::json!({ "username": ADMIN, "password": password }))
        .send()
        .await
        .expect("login request")
}

#[tokio::test]
async fn assembled_system_enforces_authentication() {
    let server = start().await;
    let client = reqwest::Client::new();

    let health = client.get(server.url("/health")).send().await.unwrap();
    assert!(health.status().is_success(), "health must be public");

    let protected = client.get(server.url("/api/menus")).send().await.unwrap();
    assert_eq!(protected.status(), 401, "API must reject anonymous callers");

    // The suffix that previously reached a handler without authentication.
    let bypass = client.get(server.url("/memo/1.ico")).send().await.unwrap();
    assert_ne!(
        bypass.status(),
        400,
        "`.ico` suffix must not reach a route handler"
    );

    // Dev-only Vite paths must not be exempt outside dev mode.
    let dev_asset = client
        .get(server.url("/src/app/page.tsx"))
        .send()
        .await
        .unwrap();
    assert_ne!(dev_asset.status(), 200, "dev asset paths must not bypass auth");

    // Prefix widening: /health is public, /healthz is not.
    let widened = client.get(server.url("/healthz")).send().await.unwrap();
    assert_ne!(widened.status(), 200, "prefix must not widen public access");
}

#[tokio::test]
async fn assembled_system_rotates_and_revokes_refresh_tokens() {
    let server = start().await;
    let client = reqwest::Client::new();

    let rejected = login(&client, &server, "not-the-password").await;
    assert_eq!(rejected.status(), 401);

    let accepted = login(&client, &server, PASSWORD).await;
    assert_eq!(accepted.status(), 200);
    let tokens: Value = accepted.json().await.unwrap();

    let access = tokens["access_token"].as_str().expect("access token");
    let refresh = tokens["refresh_token"].as_str().expect("refresh token");
    assert!(
        !refresh.contains('.'),
        "refresh token must be opaque, not a JWT"
    );

    let me = client
        .get(server.url("/api/auth/me"))
        .bearer_auth(access)
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), 200);
    let identity: Value = me.json().await.unwrap();
    assert_eq!(identity["username"], ADMIN);

    let rotated: Value = client
        .post(server.url("/api/auth/refresh"))
        .json(&serde_json::json!({ "refresh_token": refresh }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let next = rotated["refresh_token"].as_str().expect("rotated token");
    assert_ne!(next, refresh, "refresh must rotate");

    let replayed = client
        .post(server.url("/api/auth/refresh"))
        .json(&serde_json::json!({ "refresh_token": refresh }))
        .send()
        .await
        .unwrap();
    assert_eq!(replayed.status(), 401, "a spent refresh token must not work");

    let logout = client
        .post(server.url("/api/auth/logout"))
        .json(&serde_json::json!({ "refresh_token": next }))
        .send()
        .await
        .unwrap();
    assert_eq!(logout.status(), 200);

    let after_logout = client
        .post(server.url("/api/auth/refresh"))
        .json(&serde_json::json!({ "refresh_token": next }))
        .send()
        .await
        .unwrap();
    assert_eq!(after_logout.status(), 401, "logout must revoke the session");
}

#[tokio::test]
async fn assembled_system_throttles_repeated_failures() {
    let server = start().await;
    let client = reqwest::Client::new();

    for attempt in 1..=5 {
        let response = login(&client, &server, "wrong").await;
        assert_eq!(response.status(), 401, "attempt {attempt} must be rejected");
    }

    let throttled = login(&client, &server, "wrong").await;
    assert_eq!(throttled.status(), 429, "the 6th attempt must be throttled");

    // Throttling must hold even for the correct password, or it would be
    // trivially bypassed by guessing until the right one is tried.
    let correct = login(&client, &server, PASSWORD).await;
    assert_eq!(correct.status(), 429);
}
