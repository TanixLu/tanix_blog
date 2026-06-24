use axum::{
    Router,
    extract::Request,
    http::header,
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
};
use tower_http::services::{ServeDir, ServeFile};

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
async fn main() {
    let app = Router::new()
        .fallback_service(
            ServeDir::new("public").not_found_service(ServeFile::new("public/404.html")),
        )
        .layer(middleware::from_fn(redirect_www));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:9000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
