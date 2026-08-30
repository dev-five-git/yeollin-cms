//! Deferred, signed delivery for committed event envelopes.

use std::{
    fmt::Write,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};

use anyhow::Context;
use hmac::{Hmac, KeyInit, Mac};
use reqwest::{redirect::Policy, Url};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use sha2::Sha256;
use yeollin_plugin::EventEnvelope;

use crate::models::{deliveries, endpoints};

pub(crate) const STATUS_PENDING: &str = "pending";
pub(crate) const STATUS_DELIVERED: &str = "delivered";
pub(crate) const STATUS_DEAD_LETTER: &str = "dead_letter";
pub(crate) const MAX_ATTEMPTS: i32 = 5;
const ID_BYTES: usize = 16;
const MAX_ERROR_LENGTH: usize = 1_024;

pub(crate) async fn deliver_event(
    event: EventEnvelope,
    db: DatabaseConnection,
) -> anyhow::Result<()> {
    let configured = endpoints::Entity::find()
        .filter(endpoints::Column::Enabled.eq(true))
        .all(&db)
        .await?;
    let mut retryable_failures = Vec::new();

    for endpoint in configured {
        if !matches_event(&endpoint.event_names, &event.name)? {
            continue;
        }

        let delivery = find_or_create_delivery(&db, &endpoint, &event).await?;
        if matches!(
            delivery.status.as_str(),
            STATUS_DELIVERED | STATUS_DEAD_LETTER
        ) {
            continue;
        }

        let attempt = delivery.attempts.saturating_add(1);
        let now = chrono::Utc::now();
        let result = send(&endpoint, &delivery.id, &event).await;
        let mut active: deliveries::ActiveModel = delivery.into();
        active.attempts = Set(attempt);
        active.updated_at = Set(now.into());

        match result {
            Ok(status) => {
                active.status = Set(STATUS_DELIVERED.to_string());
                active.response_status = Set(Some(i32::from(status.as_u16())));
                active.last_error = Set(None);
                active.delivered_at = Set(Some(now.into()));
            }
            Err(failure) => {
                let message = truncate_error(failure.message);
                active.response_status = Set(failure.response_status);
                active.last_error = Set(Some(message.clone()));
                active.delivered_at = Set(None);
                if attempt >= MAX_ATTEMPTS {
                    active.status = Set(STATUS_DEAD_LETTER.to_string());
                    tracing::error!(
                        delivery_id = %active_id(&active),
                        webhook_id = %endpoint.id,
                        event_id = event.id,
                        attempts = attempt,
                        error = %message,
                        "Webhook delivery entered the dead-letter state"
                    );
                } else {
                    active.status = Set(STATUS_PENDING.to_string());
                    retryable_failures.push(format!("{}: {message}", endpoint.name));
                }
            }
        }
        active.update(&db).await?;
    }

    if retryable_failures.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("webhook delivery failed: {}", retryable_failures.join("; "))
    }
}

async fn find_or_create_delivery(
    db: &DatabaseConnection,
    endpoint: &endpoints::Model,
    event: &EventEnvelope,
) -> anyhow::Result<deliveries::Model> {
    if let Some(delivery) = deliveries::Entity::find()
        .filter(deliveries::Column::WebhookId.eq(&endpoint.id))
        .filter(deliveries::Column::EventId.eq(event.id))
        .one(db)
        .await?
    {
        return Ok(delivery);
    }

    let now = chrono::Utc::now();
    Ok(deliveries::ActiveModel {
        id: Set(random_id()),
        webhook_id: Set(endpoint.id.clone()),
        event_id: Set(event.id),
        event_name: Set(event.name.clone()),
        status: Set(STATUS_PENDING.to_string()),
        attempts: Set(0),
        response_status: Set(None),
        last_error: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        delivered_at: Set(None),
    }
    .insert(db)
    .await?)
}

async fn send(
    endpoint: &endpoints::Model,
    delivery_id: &str,
    event: &EventEnvelope,
) -> Result<reqwest::StatusCode, DeliveryFailure> {
    let url = validate_url(&endpoint.url).map_err(DeliveryFailure::without_status)?;
    let resolved = resolve_and_validate(&url, endpoint.allow_private_networks)
        .await
        .map_err(DeliveryFailure::without_status)?;
    let host = url
        .host_str()
        .ok_or_else(|| DeliveryFailure::without_status("Webhook URL has no host"))?;
    let mut client =
        reqwest::Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(
                u64::try_from(endpoint.timeout_seconds).unwrap_or(1),
            ));
    if host.parse::<IpAddr>().is_err() {
        client = client.resolve_to_addrs(host, &resolved);
    }
    let client = client
        .build()
        .map_err(|error| DeliveryFailure::without_status(error.to_string()))?;
    let body = serde_json::to_vec(event)
        .map_err(|error| DeliveryFailure::without_status(error.to_string()))?;
    let signature = signature(&endpoint.secret, &body)
        .map_err(|error| DeliveryFailure::without_status(error.to_string()))?;
    let response = client
        .post(url)
        .header("content-type", "application/json")
        .header("user-agent", "Yeollin-CMS-Webhooks/1.0")
        .header("x-yeollin-event", &event.name)
        .header("x-yeollin-delivery", delivery_id)
        .header("x-yeollin-signature", format!("sha256={signature}"))
        .body(body)
        .send()
        .await
        .map_err(|error| DeliveryFailure::without_status(error.to_string()))?;
    let status = response.status();
    if status.is_success() {
        Ok(status)
    } else {
        Err(DeliveryFailure {
            response_status: Some(i32::from(status.as_u16())),
            message: format!("Endpoint returned HTTP {}", status.as_u16()),
        })
    }
}

pub(crate) fn validate_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value.trim()).map_err(|_| "URL must be absolute".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("URL must use http or https".to_string());
    }
    if url.host_str().is_none() {
        return Err("URL must include a host".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("URL must not contain credentials".to_string());
    }
    if url.fragment().is_some() {
        return Err("URL must not contain a fragment".to_string());
    }
    Ok(url)
}

async fn resolve_and_validate(url: &Url, allow_private: bool) -> Result<Vec<SocketAddr>, String> {
    let host = url
        .host_str()
        .ok_or_else(|| "URL has no host".to_string())?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "URL has no usable port".to_string())?;
    let addresses = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, port)]
    } else {
        tokio::net::lookup_host((host, port))
            .await
            .with_context(|| format!("Could not resolve `{host}`"))
            .map_err(|error| error.to_string())?
            .collect()
    };
    if addresses.is_empty() {
        return Err("Webhook host resolved to no addresses".to_string());
    }
    if !allow_private && addresses.iter().any(|address| blocked_ip(address.ip())) {
        return Err(
            "Webhook host resolves to a private, loopback, or link-local address".to_string(),
        );
    }
    Ok(addresses)
}

fn blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => blocked_ipv4(ip),
        IpAddr::V6(ip) => blocked_ipv6(ip),
    }
}

fn blocked_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip.is_broadcast()
}

fn blocked_ipv6(ip: Ipv6Addr) -> bool {
    let octets = ip.octets();
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip.is_unicast_link_local()
        || octets[0] & 0xfe == 0xfc
        || ip.to_ipv4_mapped().is_some_and(blocked_ipv4)
}

fn matches_event(value: &serde_json::Value, event_name: &str) -> anyhow::Result<bool> {
    let names: Vec<String> =
        serde_json::from_value(value.clone()).context("stored webhook event filter is invalid")?;
    Ok(names.is_empty() || names.iter().any(|name| name == event_name))
}

fn signature(secret: &str, body: &[u8]) -> anyhow::Result<String> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .context("webhook secret could not initialize HMAC")?;
    mac.update(body);
    Ok(mac
        .finalize()
        .into_bytes()
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        }))
}

fn random_id() -> String {
    rand::random::<[u8; ID_BYTES]>().iter().fold(
        String::with_capacity(ID_BYTES * 2),
        |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        },
    )
}

fn truncate_error(error: String) -> String {
    error.chars().take(MAX_ERROR_LENGTH).collect()
}

fn active_id(active: &deliveries::ActiveModel) -> &str {
    match &active.id {
        sea_orm::ActiveValue::Set(id) | sea_orm::ActiveValue::Unchanged(id) => id,
        sea_orm::ActiveValue::NotSet => "unknown",
    }
}

struct DeliveryFailure {
    response_status: Option<i32>,
    message: String,
}

impl DeliveryFailure {
    fn without_status(message: impl Into<String>) -> Self {
        Self {
            response_status: None,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicU16, Ordering},
        Arc,
    };

    use axum::{
        body::Bytes,
        extract::State,
        http::{HeaderMap, StatusCode},
        routing::post,
        Router,
    };
    use sea_orm::PaginatorTrait;
    use tokio::{sync::mpsc, task::JoinHandle};

    use super::*;

    #[derive(Debug)]
    struct CapturedRequest {
        headers: HeaderMap,
        body: Vec<u8>,
    }

    #[derive(Clone)]
    struct ReceiverState {
        status: Arc<AtomicU16>,
        delay: Duration,
        captured: mpsc::UnboundedSender<CapturedRequest>,
    }

    async fn capture(
        State(state): State<ReceiverState>,
        headers: HeaderMap,
        body: Bytes,
    ) -> StatusCode {
        if !state.delay.is_zero() {
            tokio::time::sleep(state.delay).await;
        }
        let _ = state.captured.send(CapturedRequest {
            headers,
            body: body.to_vec(),
        });
        StatusCode::from_u16(state.status.load(Ordering::SeqCst)).unwrap()
    }

    async fn receiver(
        status: StatusCode,
        delay: Duration,
    ) -> (
        String,
        Arc<AtomicU16>,
        mpsc::UnboundedReceiver<CapturedRequest>,
        JoinHandle<()>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (captured, received) = mpsc::unbounded_channel();
        let status = Arc::new(AtomicU16::new(status.as_u16()));
        let state = ReceiverState {
            status: Arc::clone(&status),
            delay,
            captured,
        };
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/hook", post(capture))
                    .with_state(state),
            )
            .await
            .unwrap();
        });
        (format!("http://{address}/hook"), status, received, server)
    }

    async fn database() -> DatabaseConnection {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let metadata = crate::metadata();
        (metadata.on_init.as_ref().unwrap())(db.clone())
            .await
            .unwrap();
        db
    }

    async fn endpoint(
        db: &DatabaseConnection,
        id: &str,
        url: String,
        event_names: &[&str],
        allow_private: bool,
        timeout_seconds: i32,
    ) {
        let now = chrono::Utc::now();
        endpoints::ActiveModel {
            id: Set(id.to_string()),
            name: Set(format!("Endpoint {id}")),
            url: Set(url),
            secret: Set("0123456789abcdef0123456789abcdef".to_string()),
            event_names: Set(serde_json::json!(event_names)),
            allow_private_networks: Set(allow_private),
            timeout_seconds: Set(timeout_seconds),
            enabled: Set(true),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(db)
        .await
        .unwrap();
    }

    fn event(id: i64, name: &str) -> EventEnvelope {
        EventEnvelope {
            id,
            name: name.to_string(),
            payload: serde_json::json!({ "value": "signed" }),
            audit: true,
            created_at: chrono::Utc::now().into(),
        }
    }

    #[test]
    fn hmac_signature_is_stable() {
        assert_eq!(
            signature("secret", b"body").unwrap(),
            "dc46983557fea127b43af721467eb9b3fde2338fe3e14f51952aa8478c13d355"
        );
    }

    #[test]
    fn blocks_non_public_address_classes() {
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.1.1",
            "0.0.0.0",
            "::1",
            "fe80::1",
            "fd00::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(
                blocked_ip(address.parse().unwrap()),
                "did not block {address}"
            );
        }
        assert!(!blocked_ip("1.1.1.1".parse().unwrap()));
        assert!(!blocked_ip("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn webhook_urls_exclude_ambiguous_authorities() {
        assert!(validate_url("https://example.com/hooks?id=1").is_ok());
        for invalid in [
            "ftp://example.com/hook",
            "https://user:pass@example.com/hook",
            "https://example.com/hook#fragment",
            "/relative",
        ] {
            assert!(validate_url(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[tokio::test]
    async fn exact_filter_delivers_a_signed_envelope_once() {
        let db = database().await;
        let (url, _status, mut received, server) =
            receiver(StatusCode::NO_CONTENT, Duration::ZERO).await;
        endpoint(&db, "matching", url.clone(), &["memo.created"], true, 2).await;
        endpoint(&db, "filtered", url, &["memo.updated"], true, 2).await;
        let envelope = event(41, "memo.created");

        deliver_event(envelope.clone(), db.clone()).await.unwrap();
        let captured = tokio::time::timeout(Duration::from_secs(1), received.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(captured.body, serde_json::to_vec(&envelope).unwrap());
        assert_eq!(captured.headers["x-yeollin-event"], "memo.created");
        assert!(
            captured.headers["x-yeollin-delivery"]
                .to_str()
                .unwrap()
                .len()
                == ID_BYTES * 2
        );
        let expected = format!(
            "sha256={}",
            signature("0123456789abcdef0123456789abcdef", &captured.body).unwrap()
        );
        assert_eq!(captured.headers["x-yeollin-signature"], expected);
        assert!(received.try_recv().is_err());
        assert_eq!(deliveries::Entity::find().count(&db).await.unwrap(), 1);

        deliver_event(envelope, db).await.unwrap();
        assert!(
            received.try_recv().is_err(),
            "successful delivery was resent"
        );
        server.abort();
    }

    #[tokio::test]
    async fn failures_stop_retrying_in_the_dead_letter_state() {
        let db = database().await;
        let (url, _status, mut received, server) =
            receiver(StatusCode::SERVICE_UNAVAILABLE, Duration::ZERO).await;
        endpoint(&db, "failing", url, &[], true, 2).await;
        let envelope = event(42, "memo.created");

        for attempt in 1..=MAX_ATTEMPTS {
            let result = deliver_event(envelope.clone(), db.clone()).await;
            assert_eq!(result.is_err(), attempt < MAX_ATTEMPTS);
            received.recv().await.unwrap();
        }
        let delivery = deliveries::Entity::find().one(&db).await.unwrap().unwrap();
        assert_eq!(delivery.status, STATUS_DEAD_LETTER);
        assert_eq!(delivery.attempts, MAX_ATTEMPTS);
        assert_eq!(delivery.response_status, Some(503));

        deliver_event(envelope, db).await.unwrap();
        assert!(received.try_recv().is_err(), "dead letter was sent again");
        server.abort();
    }

    #[tokio::test]
    async fn local_addresses_require_the_explicit_opt_out() {
        let db = database().await;
        let (url, _status, mut received, server) =
            receiver(StatusCode::NO_CONTENT, Duration::ZERO).await;
        endpoint(&db, "blocked", url, &[], false, 2).await;

        let error = deliver_event(event(43, "memo.created"), db.clone())
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("private, loopback, or link-local"));
        assert!(received.try_recv().is_err());
        let delivery = deliveries::Entity::find().one(&db).await.unwrap().unwrap();
        assert_eq!(delivery.status, STATUS_PENDING);
        assert_eq!(delivery.attempts, 1);
        server.abort();
    }

    #[tokio::test]
    async fn per_delivery_timeout_is_enforced() {
        let db = database().await;
        let (url, _status, _received, server) =
            receiver(StatusCode::NO_CONTENT, Duration::from_secs(3)).await;
        endpoint(&db, "slow", url, &[], true, 1).await;
        let started = tokio::time::Instant::now();

        assert!(deliver_event(event(44, "memo.created"), db).await.is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
        server.abort();
    }
}
