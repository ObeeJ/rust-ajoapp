use std::{collections::HashMap, net::SocketAddr, pin::Pin, sync::Arc, time::Duration};

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::{server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use tokio::{net::TcpListener, signal};
use tracing::{error, info, warn};

pub mod error;
pub mod extractor;
pub mod middleware;
pub mod openapi;
pub mod response;
pub mod router;

pub use error::{Error, Result};
pub use extractor::{FromRequest, Json, Path, Request, State};
pub use middleware::{FnMiddleware, Middleware, Next};
pub use response::{IntoResponse, Response};
pub use router::Router;
pub use ferox_macros::{delete, get, post, put};

// ── Auto-registration ─────────────────────────────────────────────────────────

pub struct StaticRoute {
    pub method:  &'static str,
    pub path:    &'static str,
    pub handler: fn(Request) -> Pin<Box<dyn std::future::Future<Output = Response> + Send>>,
}

#[linkme::distributed_slice]
pub static ROUTES: [StaticRoute];

// ── Config ────────────────────────────────────────────────────────────────────

pub struct Config {
    /// Max request body in bytes (default 1 MB)
    pub body_limit: u64,
    /// Per-request timeout (default 30 s)
    pub request_timeout: Duration,
    /// Allowed CORS origin — must be explicit, never "*" for credentialed requests
    pub cors_origin: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            body_limit:      1024 * 1024,
            request_timeout: Duration::from_secs(30),
            cors_origin:     None,
        }
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    router:      Router,
    middlewares: Vec<Arc<dyn Middleware>>,
    state:       Option<Arc<dyn std::any::Any + Send + Sync>>,
    openapi:     openapi::OpenApi,
    config:      Config,
}

impl App {
    pub fn new() -> Self {
        Self {
            router:      Router::new(),
            middlewares: vec![],
            state:       None,
            openapi:     openapi::OpenApi::new("GlideAPI", "0.1.0"),
            config:      Config::default(),
        }
    }

    pub fn config(mut self, cfg: Config) -> Self { self.config = cfg; self }

    pub fn state<T: Clone + Send + Sync + 'static>(mut self, val: T) -> Self {
        self.state = Some(Arc::new(val));
        self
    }

    pub fn route<F, Fut>(mut self, method: &str, path: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.openapi.add_route(method, path, &format!("{method} {path}"));
        self.router.add(method, path, handler);
        self
    }

    pub fn mount_routes(mut self) -> Self {
        for r in ROUTES {
            self.openapi.add_route(r.method, r.path, &format!("{} {}", r.method, r.path));
            let handler = r.handler;
            self.router.add(r.method, r.path, move |req| handler(req));
        }
        self
    }

    pub fn with<M: Middleware>(mut self, m: M) -> Self {
        self.middlewares.push(Arc::new(m));
        self
    }

    /// Initialise tracing to stdout — call before `listen`
    pub fn init_tracing() {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "ferox=info".into()),
            )
            .init();
    }

    pub async fn listen(self, addr: &str) {
        let addr: SocketAddr = addr.parse().expect("invalid address");
        let listener = TcpListener::bind(addr).await.expect("bind failed");

        info!("🚀 GlideAPI listening on http://{addr}");
        info!("📖 OpenAPI  http://{addr}/_openapi.json");
        info!("🖥️  Swagger  http://{addr}/_docs");

        let router   = Arc::new(self.router);
        let mws      = Arc::new(self.middlewares);
        let state    = Arc::new(self.state);
        let openapi  = Arc::new(serde_json::to_string_pretty(&self.openapi).unwrap());
        let cfg      = Arc::new(self.config);

        loop {
            tokio::select! {
                Ok((stream, peer)) = listener.accept() => {
                    let router  = router.clone();
                    let mws     = mws.clone();
                    let state   = state.clone();
                    let openapi = openapi.clone();
                    let cfg     = cfg.clone();

                    tokio::spawn(async move {
                        let io = TokioIo::new(stream);
                        let svc = service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
                            let router  = router.clone();
                            let mws     = mws.clone();
                            let state   = state.clone();
                            let openapi = openapi.clone();
                            let cfg     = cfg.clone();
                            let peer    = peer;

                            async move {
                                let method = req.method().to_string();
                                let path   = req.uri().path().to_string();
                                let req_id = uuid::Uuid::new_v4().to_string();
                                let origin = req.headers()
                                    .get("origin")
                                    .and_then(|v| v.to_str().ok())
                                    .unwrap_or("")
                                    .to_string();

                                // ── Built-in endpoints ───────────────────
                                if method == "GET" && path == "/_openapi.json" {
                                    return Ok::<_, hyper::Error>(
                                        apply_cors(json_resp(200, openapi.as_ref().clone(), None, vec![]), &origin, &cfg)
                                    );
                                }
                                if method == "GET" && path == "/_docs" {
                                    return Ok(html_resp(swagger_ui()));
                                }

                                // ── CORS preflight ───────────────────────
                                if method == "OPTIONS" {
                                    return Ok(apply_cors(
                                        hyper::Response::builder()
                                            .status(204)
                                            .header("access-control-allow-methods", "GET, POST, PUT, DELETE, OPTIONS")
                                            .header("access-control-allow-headers", "content-type, authorization, x-idempotency-key")
                                            .header("access-control-max-age", "86400")
                                            .body(Full::new(Bytes::new()))
                                            .unwrap(),
                                        &origin, &cfg,
                                    ));
                                }

                                let mut headers = HashMap::new();
                                for (k, v) in req.headers() {
                                    headers.insert(k.to_string(), v.to_str().unwrap_or("").to_string());
                                }

                                // ── Body size limit ──────────────────────
                                let body = match Limited::new(req.into_body(), cfg.body_limit as usize)
                                    .collect()
                                    .await
                                {
                                    Ok(b) => b.to_bytes(),
                                    Err(_) => {
                                        warn!(req_id, "body too large from {peer}");
                                        return Ok(json_resp(413, r#"{"error":"payload too large"}"#.into(), Some(&req_id), vec![]));
                                    }
                                };

                                let fw_req = Request {
                                    method: method.clone(),
                                    path: path.clone(),
                                    headers,
                                    body,
                                    params: HashMap::new(),
                                    state: state.as_ref().clone(),
                                    req_id: req_id.clone(),
                                };

                                // ── Request timeout ──────────────────────
                                let base: Next = Arc::new(move |r| {
                                    let router = router.clone();
                                    Box::pin(async move { router.handle(r).await })
                                });
                                let chain = mws.iter().rev().fold(base, |next, mw| {
                                    let mw = mw.clone();
                                    Arc::new(move |req| mw.handle(req, next.clone()))
                                });

                                let resp = match tokio::time::timeout(cfg.request_timeout, chain(fw_req)).await {
                                    Ok(r) => r,
                                    Err(_) => {
                                        warn!(req_id, "{method} {path} timed out");
                                        Response { status: 504, body: r#"{"error":"gateway timeout"}"#.into(), headers: vec![] }
                                    }
                                };

                                info!(req_id, peer = %peer, "{method} {path} → {}", resp.status);
                                Ok(apply_cors(json_resp(resp.status, resp.body, Some(&req_id), resp.headers), &origin, &cfg))
                            }
                        });
                        if let Err(e) = http1::Builder::new().serve_connection(io, svc).await {
                            error!("connection error: {e}");
                        }
                    });
                }
                _ = shutdown_signal() => {
                    info!("shutting down gracefully");
                    break;
                }
            }
        }
    }
}

impl Default for App {
    fn default() -> Self { Self::new() }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn json_resp(status: u16, body: String, req_id: Option<&str>, extra_headers: Vec<(String, String)>) -> hyper::Response<Full<Bytes>> {
    let mut b = hyper::Response::builder()
        .status(status)
        .header("content-type", "application/json")
        // Security headers on every response
        .header("x-content-type-options", "nosniff")
        .header("x-frame-options", "DENY")
        .header("referrer-policy", "strict-origin-when-cross-origin");

    // No-store on auth-adjacent responses (4xx/2xx from auth paths)
    if status == 200 || status == 201 || status == 401 || status == 403 {
        b = b.header("cache-control", "no-store");
    }

    if let Some(id) = req_id {
        b = b.header("x-request-id", id);
    }
    for (k, v) in &extra_headers {
        b = b.header(k.as_str(), v.as_str());
    }
    b.body(Full::new(Bytes::from(body))).unwrap()
}

/// Apply CORS headers — only sets Allow-Origin if the request origin matches config
fn apply_cors(mut resp: hyper::Response<Full<Bytes>>, origin: &str, cfg: &Config) -> hyper::Response<Full<Bytes>> {
    let allowed = match &cfg.cors_origin {
        Some(o) if o == origin => origin,
        _ => return resp,
    };
    let headers = resp.headers_mut();
    headers.insert("access-control-allow-origin",      allowed.parse().unwrap());
    headers.insert("access-control-allow-credentials", "true".parse().unwrap());
    headers.insert("vary",                             "Origin".parse().unwrap());
    resp
}

fn html_resp(body: &'static str) -> hyper::Response<Full<Bytes>> {
    hyper::Response::builder()
        .status(200)
        .header("content-type", "text/html")
        .body(Full::new(Bytes::from_static(body.as_bytes())))
        .unwrap()
}

fn swagger_ui() -> &'static str {
    r###"<!DOCTYPE html>
<html>
<head>
  <title>GlideAPI Docs</title>
  <meta charset="utf-8"/>
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist/swagger-ui.css"/>
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://unpkg.com/swagger-ui-dist/swagger-ui-bundle.js"></script>
  <script>
    SwaggerUIBundle({ url: "/_openapi.json", dom_id: "#swagger-ui" })
  </script>
</body>
</html>"###
}

async fn shutdown_signal() {
    let ctrl_c = async { signal::ctrl_c().await.expect("ctrl-c handler failed") };
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("SIGTERM handler failed")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
