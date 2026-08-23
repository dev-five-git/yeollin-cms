//! Development proxy for forwarding requests to Next.js dev server
//!
//! In development mode, this module proxies non-API requests to the Next.js dev server
//! running on a separate port, allowing a single entry point for the CMS.
//!
//! Supports both HTTP and WebSocket (for HMR) proxying.

use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{header, Request, Response, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message as TungMessage};

/// Shared state for dev proxy
#[derive(Clone)]
pub struct DevProxyState {
    client: Client,
    target_url: String,
    target_ws_url: String,
}

impl DevProxyState {
    pub fn new(target_port: u16) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(5))
                .build()
                .expect("Failed to create HTTP client"),
            target_url: format!("http://127.0.0.1:{}", target_port),
            target_ws_url: format!("ws://127.0.0.1:{}", target_port),
        }
    }
}

/// Create a router that proxies all requests to the Next.js dev server
pub fn dev_proxy_router(target_port: u16) -> Router {
    let state = Arc::new(DevProxyState::new(target_port));

    Router::new()
        // WebSocket routes for HMR
        // Turbopack (Next.js 16 default) uses /_next/hmr; webpack uses /_next/webpack-hmr
        .route("/_next/hmr", get(websocket_handler))
        .route("/_next/webpack-hmr", get(websocket_handler))
        .route("/__nextjs_original-stack-frames", get(websocket_handler))
        // HTTP fallback for everything else
        .fallback(http_proxy_handler)
        .with_state(state)
}

/// WebSocket proxy handler for HMR
async fn websocket_handler(
    State(state): State<Arc<DevProxyState>>,
    ws: WebSocketUpgrade,
    req: Request<Body>,
) -> impl IntoResponse {
    let path = req.uri().path().to_string();
    let query = req
        .uri()
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();

    let ws_url = format!("{}{}{}", state.target_ws_url, path, query);
    tracing::debug!("WebSocket proxy: {}", ws_url);

    ws.on_upgrade(move |socket| handle_websocket(socket, ws_url))
}

/// HTTP proxy handler
async fn http_proxy_handler(
    State(state): State<Arc<DevProxyState>>,
    req: Request<Body>,
) -> Response<Body> {
    let path = req.uri().path().to_string();
    let query = req
        .uri()
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();

    let method = req.method().clone();
    let headers = req.headers().clone();
    let target_url = format!("{}{}{}", state.target_url, path, query);

    // Read body upfront for potential retries
    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!("Failed to read request body: {}", e);
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("Failed to read request body"))
                .unwrap();
        }
    };

    // Send request with retry for connection errors (Next.js might be restarting)
    let mut last_error = None;
    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        // Rebuild request for retry
        let mut retry_req = state.client.request(method.clone(), &target_url);
        for (name, value) in headers.iter() {
            let name_str = name.as_str().to_lowercase();
            if name_str != "host"
                && name_str != "connection"
                && name_str != "keep-alive"
                && name_str != "upgrade"
            {
                if let Ok(value_str) = value.to_str() {
                    retry_req = retry_req.header(name.as_str(), value_str);
                }
            }
        }
        if !body_bytes.is_empty() {
            retry_req = retry_req.body(body_bytes.to_vec());
        }

        match retry_req.send().await {
            Ok(resp) => {
                let status = StatusCode::from_u16(resp.status().as_u16())
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

                // Get response body first (consumes resp)
                let resp_headers = resp.headers().clone();
                let body_bytes = match resp.bytes().await {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        tracing::error!("Failed to read response body: {}", e);
                        return Response::builder()
                            .status(StatusCode::BAD_GATEWAY)
                            .body(Body::from("Failed to read response from upstream"))
                            .unwrap();
                    }
                };

                // Build response with explicit content-length
                let mut response = Response::builder()
                    .status(status)
                    .header(header::CONTENT_LENGTH, body_bytes.len());

                // Copy response headers (except transfer-encoding and content-length which we set ourselves)
                for (name, value) in resp_headers.iter() {
                    let name_str = name.as_str().to_lowercase();
                    if name_str != "transfer-encoding"
                        && name_str != "content-length"
                        && name_str != "connection"
                    {
                        if let Ok(value_str) = value.to_str() {
                            response = response.header(name.as_str(), value_str);
                        }
                    }
                }

                return response
                    .body(Body::from(body_bytes.to_vec()))
                    .unwrap_or_else(|_| {
                        Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Body::from("Failed to build response"))
                            .unwrap()
                    });
            }
            Err(e) => {
                tracing::debug!("Proxy attempt {} failed: {}", attempt + 1, e);
                last_error = Some(e);
            }
        }
    }

    let error_msg = last_error
        .map(|e| e.to_string())
        .unwrap_or_else(|| "Unknown error".to_string());
    tracing::error!("Proxy request failed after retries: {}", error_msg);
    Response::builder()
        .status(StatusCode::BAD_GATEWAY)
        .body(Body::from(format!("Proxy error: {}", error_msg)))
        .unwrap()
}

/// Handle WebSocket proxy connection
async fn handle_websocket(client_socket: WebSocket, target_url: String) {
    // Connect to Next.js WebSocket
    let ws_stream = match connect_async(&target_url).await {
        Ok((stream, _)) => stream,
        Err(e) => {
            tracing::error!(
                "Failed to connect to WebSocket target {}: {}",
                target_url,
                e
            );
            return;
        }
    };

    tracing::debug!("WebSocket connected to {}", target_url);

    let (mut server_sink, mut server_stream) = ws_stream.split();
    let (mut client_sink, mut client_stream) = client_socket.split();

    // Forward messages bidirectionally
    let client_to_server = async {
        while let Some(msg) = client_stream.next().await {
            match msg {
                Ok(msg) => {
                    let tung_msg = axum_to_tungstenite(msg);
                    if let Some(tung_msg) = tung_msg {
                        if server_sink.send(tung_msg).await.is_err() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("WebSocket client error: {}", e);
                    break;
                }
            }
        }
    };

    let server_to_client = async {
        while let Some(msg) = server_stream.next().await {
            match msg {
                Ok(msg) => {
                    let axum_msg = tungstenite_to_axum(msg);
                    if let Some(axum_msg) = axum_msg {
                        if client_sink.send(axum_msg).await.is_err() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("WebSocket server error: {}", e);
                    break;
                }
            }
        }
    };

    // Run both directions concurrently, stop when either ends
    tokio::select! {
        _ = client_to_server => {},
        _ = server_to_client => {},
    }

    tracing::debug!("WebSocket proxy closed for {}", target_url);
}

/// Convert axum WebSocket message to tungstenite message
fn axum_to_tungstenite(msg: Message) -> Option<TungMessage> {
    match msg {
        Message::Text(text) => Some(TungMessage::Text(text.to_string().into())),
        Message::Binary(data) => Some(TungMessage::Binary(data.to_vec().into())),
        Message::Ping(data) => Some(TungMessage::Ping(data.to_vec().into())),
        Message::Pong(data) => Some(TungMessage::Pong(data.to_vec().into())),
        Message::Close(_) => Some(TungMessage::Close(None)),
    }
}

/// Convert tungstenite message to axum WebSocket message
fn tungstenite_to_axum(msg: TungMessage) -> Option<Message> {
    match msg {
        TungMessage::Text(text) => Some(Message::Text(text.to_string().into())),
        TungMessage::Binary(data) => Some(Message::Binary(data.to_vec().into())),
        TungMessage::Ping(data) => Some(Message::Ping(data.to_vec().into())),
        TungMessage::Pong(data) => Some(Message::Pong(data.to_vec().into())),
        TungMessage::Close(_) => Some(Message::Close(None)),
        TungMessage::Frame(_) => None,
    }
}
