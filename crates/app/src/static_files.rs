//! Static file serving for embedded Next.js output
//!
//! This module provides utilities to serve embedded static files from
//! a Next.js static export (`out/` directory).

use axum::{
    body::Body,
    http::{header, Request, Response, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use include_dir::Dir;
use std::path::Path;

/// Create a router that serves embedded static files
/// 
/// # Example
/// 
/// ```rust,ignore
/// use include_dir::{include_dir, Dir};
/// use yeollin::static_router;
/// 
/// static STATIC_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../.yeollin/app/out");
/// 
/// let router = static_router(&STATIC_DIR);
/// ```
pub fn static_router(dir: &'static Dir<'static>) -> Router {
    Router::new()
        .fallback(get(move |req: Request<Body>| async move {
            serve_static_file(dir, req.uri().path())
        }))
}

/// Serve a static file from an embedded directory
fn serve_static_file(dir: &'static Dir<'static>, path: &str) -> impl IntoResponse {
    // Normalize path
    let path = path.trim_start_matches('/');
    
    // Try exact path first
    if let Some(response) = try_serve_file(dir, path) {
        return response;
    }
    
    // Try with .html extension (for Next.js static routes)
    let html_path = format!("{}.html", path);
    if let Some(response) = try_serve_file(dir, &html_path) {
        return response;
    }
    
    // Try index.html in directory
    let index_path = if path.is_empty() {
        "index.html".to_string()
    } else {
        format!("{}/index.html", path)
    };
    if let Some(response) = try_serve_file(dir, &index_path) {
        return response;
    }
    
    // 404 fallback - try serving 404.html if it exists
    if let Some(response) = try_serve_file(dir, "404.html") {
        return (StatusCode::NOT_FOUND, response.into_response()).into_response();
    }
    
    // Ultimate fallback
    (StatusCode::NOT_FOUND, "Not Found").into_response()
}

/// Try to serve a file at the given path
fn try_serve_file(dir: &'static Dir<'static>, path: &str) -> Option<Response<Body>> {
    let file = dir.get_file(path)?;
    let contents = file.contents();
    
    let content_type = guess_content_type(path);
    
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, contents.len())
        .body(Body::from(contents.to_vec()))
        .ok()
}

/// Guess content type from file extension
fn guess_content_type(path: &str) -> &'static str {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    
    match ext {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_type_guessing() {
        assert_eq!(guess_content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(guess_content_type("style.css"), "text/css; charset=utf-8");
        assert_eq!(guess_content_type("app.js"), "application/javascript; charset=utf-8");
        assert_eq!(guess_content_type("data.json"), "application/json; charset=utf-8");
        assert_eq!(guess_content_type("image.png"), "image/png");
    }
}
