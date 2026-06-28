use axum::{
    Router,
    extract::Request,
    http::{Uri, header},
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
};
use tower_http::services::{ServeDir, ServeFile};

/// Rewrite extensionless paths (e.g. `/about`) to their `.html` file
/// (`/about.html`) so a clean URL serves the same page as the file.
async fn rewrite_path(mut req: Request, next: Next) -> Response {
    let path = req.uri().path();

    // Skip the root, any directory request (trailing `/`), and anything
    // that already has an extension in its last segment (e.g. `.html`,
    // `.css`, `.png`).
    let last_segment = path.rsplit('/').next().unwrap_or("");
    let needs_rewrite = path != "/" && !path.ends_with('/') && !last_segment.contains('.');

    if needs_rewrite {
        let query = req
            .uri()
            .query()
            .map(|q| format!("?{q}"))
            .unwrap_or_default();
        if let Ok(uri) = format!("{path}.html{query}").parse::<Uri>() {
            *req.uri_mut() = uri;
        }
    }

    next.run(req).await
}

/// Redirect any request whose Host starts with `www.` to the bare domain.
async fn redirect_www(req: Request, next: Next) -> Response {
    if let Some(host) = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
    {
        if let Some(bare) = host.strip_prefix("www.") {
            let path = req
                .uri()
                .path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or("/");
            // Strip an optional :port so the redirect target is clean.
            let bare = bare.split(':').next().unwrap_or(bare);
            let target = format!("https://{bare}{path}");
            return Redirect::permanent(&target).into_response();
        }
    }
    next.run(req).await
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = Router::new()
        .fallback_service(
            ServeDir::new("public").not_found_service(ServeFile::new("public/404.html")),
        )
        .layer(middleware::from_fn(rewrite_path))
        .layer(middleware::from_fn(redirect_www));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:9000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
