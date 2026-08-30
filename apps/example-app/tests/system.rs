//! Boots the real binary and drives it over HTTP.
//!
//! Unit tests cover each rule in isolation; this exercises the assembled system:
//! plugin registration, migrations, the auth middleware, and the auth
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

async fn login_as(
    client: &reqwest::Client,
    server: &Server,
    username: &str,
    password: &str,
) -> reqwest::Response {
    client
        .post(server.url("/api/auth/login"))
        .json(&serde_json::json!({ "username": username, "password": password }))
        .send()
        .await
        .expect("login request")
}

async fn login(client: &reqwest::Client, server: &Server, password: &str) -> reqwest::Response {
    login_as(client, server, ADMIN, password).await
}

async fn admin_token(client: &reqwest::Client, server: &Server) -> String {
    let tokens: Value = login(client, server, PASSWORD).await.json().await.unwrap();
    tokens["access_token"]
        .as_str()
        .expect("access token")
        .to_string()
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
    let bypass = client
        .get(server.url("/api/example-memo-plugin/1.ico"))
        .send()
        .await
        .unwrap();
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
async fn assembled_system_enforces_role_on_admin_routes() {
    let server = start().await;
    let client = reqwest::Client::new();

    let anonymous = client.get(server.url("/api/auth/users")).send().await.unwrap();
    assert_eq!(anonymous.status(), 401, "the roster must not be public");

    let tokens: Value = login(&client, &server, PASSWORD).await.json().await.unwrap();
    let access = tokens["access_token"].as_str().expect("access token");

    let admin = client
        .get(server.url("/api/auth/users"))
        .bearer_auth(access)
        .send()
        .await
        .unwrap();
    assert_eq!(admin.status(), 200, "the seeded administrator may read it");

    let roster: Value = admin.json().await.unwrap();
    assert_eq!(roster["total"], 1);
    assert_eq!(roster["users"][0]["username"], ADMIN);
    assert_eq!(roster["users"][0]["role"], "admin");
    assert!(
        roster["users"][0].get("passwordHash").is_none(),
        "a password hash must never leave the server"
    );
}

#[tokio::test]
async fn assembled_system_manages_accounts() {
    let server = start().await;
    let client = reqwest::Client::new();
    let token = admin_token(&client, &server).await;

    let created = client
        .post(server.url("/api/auth/users"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "username": "  Editor ",
            "password": "another-long-password",
            "role": "user",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), 200);

    let account: Value = created.json().await.unwrap();
    assert_eq!(account["username"], "editor", "username must be normalised");
    assert!(
        account.get("passwordHash").is_none(),
        "a password hash must never leave the server"
    );
    let editor_id = account["id"].as_i64().expect("new account id");

    let duplicate = client
        .post(server.url("/api/auth/users"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "username": "editor",
            "password": "another-long-password",
            "role": "user",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate.status(), 409, "usernames must stay unique");

    let unknown_role = client
        .post(server.url("/api/auth/users"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "username": "someone",
            "password": "another-long-password",
            "role": "superuser",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(unknown_role.status(), 400, "an unknown role grants nothing");

    let short_password = client
        .post(server.url("/api/auth/users"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "username": "someone",
            "password": "short",
            "role": "user",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(short_password.status(), 400);

    // The 403 branch of the role guard, which could not be reached before there
    // was a way to create a second account.
    let editor: Value = login_as(&client, &server, "editor", "another-long-password")
        .await
        .json()
        .await
        .unwrap();
    let editor_token = editor["access_token"].as_str().expect("editor token");

    let refused = client
        .get(server.url("/api/auth/users"))
        .bearer_auth(editor_token)
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 403, "an ordinary account may not read it");

    let escalation = client
        .patch(server.url(&format!("/api/auth/users/{editor_id}")))
        .bearer_auth(editor_token)
        .json(&serde_json::json!({ "role": "admin" }))
        .send()
        .await
        .unwrap();
    assert_eq!(escalation.status(), 403, "nobody may promote themselves");

    let deleted = client
        .delete(server.url(&format!("/api/auth/users/{editor_id}")))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(deleted.status(), 200);

    let gone = login_as(&client, &server, "editor", "another-long-password").await;
    assert_eq!(gone.status(), 401, "a deleted account must not sign in");
}

#[tokio::test]
async fn assembled_system_refuses_to_lock_itself_out() {
    let server = start().await;
    let client = reqwest::Client::new();
    let token = admin_token(&client, &server).await;

    let roster: Value = client
        .get(server.url("/api/auth/users"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let admin_id = roster["users"][0]["id"].as_i64().expect("administrator id");

    let demoted = client
        .patch(server.url(&format!("/api/auth/users/{admin_id}")))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "role": "user" }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        demoted.status(),
        409,
        "demoting the only administrator would need hand-editing the database to undo"
    );

    let self_deleted = client
        .delete(server.url(&format!("/api/auth/users/{admin_id}")))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(self_deleted.status(), 409);

    let identity: Value = client
        .get(server.url("/api/auth/me"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(identity["role"], "admin", "the refusals must not half-apply");
}

#[tokio::test]
async fn assembled_system_ends_sessions_when_a_password_changes() {
    let server = start().await;
    let client = reqwest::Client::new();

    let tokens: Value = login(&client, &server, PASSWORD).await.json().await.unwrap();
    let access = tokens["access_token"].as_str().expect("access token");
    let refresh = tokens["refresh_token"].as_str().expect("refresh token");

    let wrong_current = client
        .post(server.url("/api/auth/password"))
        .bearer_auth(access)
        .json(&serde_json::json!({
            "currentPassword": "not-the-password",
            "newPassword": "a-replacement-password",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        wrong_current.status(),
        401,
        "a stolen access token alone must not change the password"
    );

    let changed = client
        .post(server.url("/api/auth/password"))
        .bearer_auth(access)
        .json(&serde_json::json!({
            "currentPassword": PASSWORD,
            "newPassword": "a-replacement-password",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(changed.status(), 200);

    let stale = client
        .post(server.url("/api/auth/refresh"))
        .json(&serde_json::json!({ "refresh_token": refresh }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        stale.status(),
        401,
        "a refresh token minted before the change must stop working"
    );

    assert_eq!(login(&client, &server, PASSWORD).await.status(), 401);
    assert_eq!(
        login(&client, &server, "a-replacement-password")
            .await
            .status(),
        200
    );
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
