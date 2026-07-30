# GlideAPI

**Ergonomic Rust web framework — FastAPI-style DX, Actix-level performance.**

```rust
use glideapi::{App, Request, Response, get, post};
use glideapi::response::Json;
use serde::Serialize;

#[derive(Serialize)]
struct User { id: u32, name: String }

#[get("/users/:id")]
async fn get_user(req: Request) -> Response {
    let id: u32 = req.params["id"].parse().unwrap_or(0);
    Json(User { id, name: format!("User {id}") }).into_response()
}

#[tokio::main]
async fn main() {
    App::init_tracing();
    App::new()
        .mount_routes()
        .listen("0.0.0.0:3000")
        .await;
}
```

## Features

- `#[get]` / `#[post]` / `#[put]` / `#[delete]` — decorator-style routing
- `.mount_routes()` — zero-boilerplate auto-registration
- `State<T>` — typed dependency injection, no `Arc<Mutex>` noise
- Unified `Result<T>` — no `Box<dyn Error>`
- Middleware — composable `async fn(req, next)` chain
- Auto OpenAPI 3.0 at `/_openapi.json`
- Swagger UI at `/_docs`
- Request ID (`x-request-id`) on every response
- Body size limit (default 1 MB)
- Request timeout (default 30s)
- Graceful shutdown on `Ctrl+C` / `SIGTERM`
- Structured logging via `tracing`

## Install

```toml
[dependencies]
glideapi = "0.1"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
```

## License

MIT
