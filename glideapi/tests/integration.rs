use glideapi::{App, FnMiddleware, FromRequest, IntoResponse, Request, Response, State};
use glideapi::response::Json;
use glideapi::{get};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Spin up the app on a random port, return the base URL
async fn spawn(app: App) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    // Drop the listener so App::listen can bind the same port
    drop(listener);
    let addr = format!("127.0.0.1:{port}");
    let url  = format!("http://{addr}");
    tokio::spawn(async move { app.listen(&addr).await });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    url
}

async fn get(url: &str) -> (u16, String) {
    let resp = reqwest::get(url).await.unwrap();
    let status = resp.status().as_u16();
    (status, resp.text().await.unwrap())
}

async fn post_json(url: &str, body: &str) -> (u16, String) {
    let client = reqwest::Client::new();
    let resp = client.post(url).header("content-type", "application/json").body(body.to_string()).send().await.unwrap();
    let status = resp.status().as_u16();
    (status, resp.text().await.unwrap())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_basic_get() {
    let app = App::new().route("GET", "/ping", |_req: Request| async {
        Response::ok(r#"{"pong":true}"#)
    });
    let url = spawn(app).await;
    let (status, body) = get(&format!("{url}/ping")).await;
    assert_eq!(status, 200);
    assert!(body.contains("pong"));
}

#[tokio::test]
async fn test_path_param() {
    let app = App::new().route("GET", "/items/:id", |req: Request| async move {
        let id = req.params.get("id").cloned().unwrap_or_default();
        Response::ok(format!(r#"{{"id":"{id}"}}"#))
    });
    let url = spawn(app).await;
    let (status, body) = get(&format!("{url}/items/42")).await;
    assert_eq!(status, 200);
    assert!(body.contains("42"));
}

#[tokio::test]
async fn test_404() {
    let app = App::new().route("GET", "/exists", |_: Request| async { Response::ok("ok") });
    let url = spawn(app).await;
    let (status, _) = get(&format!("{url}/missing")).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn test_post_json_body() {
    #[derive(Deserialize)]
    struct Payload { name: String }
    #[derive(Serialize)]
    struct Out { greeting: String }

    let app = App::new().route("POST", "/greet", |req: Request| async move {
        match serde_json::from_slice::<Payload>(&req.body) {
            Ok(p) => Json(Out { greeting: format!("hello {}", p.name) }).into_response(),
            Err(_) => Response { status: 400, body: "bad json".into() },
        }
    });
    let url = spawn(app).await;
    let (status, body) = post_json(&format!("{url}/greet"), r#"{"name":"world"}"#).await;
    assert_eq!(status, 200);
    assert!(body.contains("hello world"));
}

#[tokio::test]
async fn test_bad_json_returns_400() {
    #[derive(Deserialize)]
    struct Payload { _name: String }

    let app = App::new().route("POST", "/data", |req: Request| async move {
        match serde_json::from_slice::<Payload>(&req.body) {
            Ok(_)  => Response::ok("ok"),
            Err(_) => Response { status: 400, body: "bad request".into() },
        }
    });
    let url = spawn(app).await;
    let (status, _) = post_json(&format!("{url}/data"), "not json at all").await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn test_state_injection() {
    #[derive(Clone)]
    struct Config { value: u32 }

    let app = App::new()
        .state(Config { value: 99 })
        .route("GET", "/config", |req: Request| async move {
            let cfg = State::<Config>::from_request(&req).unwrap();
            Response::ok(format!(r#"{{"value":{}}}"#, cfg.0.value))
        });
    let url = spawn(app).await;
    let (status, body) = get(&format!("{url}/config")).await;
    assert_eq!(status, 200);
    assert!(body.contains("99"));
}

#[tokio::test]
async fn test_middleware_runs() {
    let counter = Arc::new(Mutex::new(0u32));
    let counter_mw = counter.clone();

    let app = App::new()
        .with(FnMiddleware(move |req: Request, next: glideapi::Next| {
            let c = counter_mw.clone();
            Box::pin(async move {
                *c.lock().unwrap() += 1;
                next(req).await
            })
        }))
        .route("GET", "/mw", |_: Request| async { Response::ok("ok") });

    let url = spawn(app).await;
    get(&format!("{url}/mw")).await;
    get(&format!("{url}/mw")).await;
    assert_eq!(*counter.lock().unwrap(), 2);
}

#[tokio::test]
async fn test_middleware_can_short_circuit() {
    let app = App::new()
        .with(FnMiddleware(|req: Request, next: glideapi::Next| {
            Box::pin(async move {
                if req.headers.get("x-secret").map(|v| v.as_str()) != Some("letmein") {
                    return Response { status: 401, body: "unauthorized".into() };
                }
                next(req).await
            })
        }))
        .route("GET", "/secret", |_: Request| async { Response::ok("secret data") });

    let url = spawn(app).await;

    // Without header → 401
    let (status, _) = get(&format!("{url}/secret")).await;
    assert_eq!(status, 401);

    // With correct header → 200
    let client = reqwest::Client::new();
    let resp = client.get(&format!("{url}/secret")).header("x-secret", "letmein").send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
}

#[tokio::test]
async fn test_openapi_endpoint() {
    let app = App::new().route("GET", "/hello", |_: Request| async { Response::ok("hi") });
    let url = spawn(app).await;
    let (status, body) = get(&format!("{url}/_openapi.json")).await;
    assert_eq!(status, 200);
    let spec: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(spec["openapi"], "3.0.0");
    assert!(spec["paths"]["/hello"]["get"].is_object());
}

#[tokio::test]
async fn test_swagger_ui_endpoint() {
    let app = App::new();
    let url = spawn(app).await;
    let (status, body) = get(&format!("{url}/_docs")).await;
    assert_eq!(status, 200);
    assert!(body.contains("swagger-ui"));
}

#[tokio::test]
async fn test_macro_routes_registered() {
    #[get("/macro-ping")]
    async fn macro_ping(_req: Request) -> Response {
        Response::ok(r#"{"macro":true}"#)
    }

    // mount_routes() picks up the #[get] above via linkme
    let app = App::new().mount_routes();
    let url = spawn(app).await;
    let (status, body) = get(&format!("{url}/macro-ping")).await;
    assert_eq!(status, 200);
    assert!(body.contains("macro"));
}

#[tokio::test]
async fn test_request_id_header_present() {
    let app = App::new().route("GET", "/id", |_: Request| async { Response::ok("ok") });
    let url = spawn(app).await;
    let resp = reqwest::get(&format!("{url}/id")).await.unwrap();
    assert!(resp.headers().contains_key("x-request-id"));
}

#[tokio::test]
async fn test_body_size_limit() {
    use glideapi::Config;
    use std::time::Duration;

    let app = App::new()
        .config(Config { body_limit: 10, request_timeout: Duration::from_secs(5), cors_origin: None })
        .route("POST", "/upload", |_: Request| async { Response::ok("ok") });
    let url = spawn(app).await;

    let client = reqwest::Client::new();
    let big_body = "x".repeat(100);
    let (status, _) = {
        let resp = client.post(&format!("{url}/upload")).body(big_body).send().await.unwrap();
        (resp.status().as_u16(), resp.text().await.unwrap())
    };
    assert_eq!(status, 413);
}

#[tokio::test]
async fn test_request_timeout() {
    use glideapi::Config;
    use std::time::Duration;

    let app = App::new()
        .config(Config { body_limit: 1024 * 1024, request_timeout: Duration::from_millis(50), cors_origin: None })
        .route("GET", "/slow", |_: Request| async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Response::ok("done")
        });
    let url = spawn(app).await;
    let (status, _) = get(&format!("{url}/slow")).await;
    assert_eq!(status, 504);
}
