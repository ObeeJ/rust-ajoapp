use glideapi::{App, FnMiddleware, FromRequest, IntoResponse, Request, Response, State};
use glideapi::response::Json;
use glideapi::{get, post};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
struct Db { name: String }

#[derive(Serialize)]
struct User { id: u32, name: String }

#[derive(Deserialize)]
struct CreateUser { name: String }

#[get("/health")]
async fn health(_req: Request) -> Response {
    Response::ok(r#"{"status":"ok"}"#)
}

#[get("/users/:id")]
async fn get_user(req: Request) -> Response {
    let db = State::<Db>::from_request(&req).unwrap();
    let id: u32 = req.params.get("id").and_then(|v| v.parse().ok()).unwrap_or(0);
    Json(User { id, name: format!("{} #{id}", db.0.name) }).into_response()
}

#[post("/users")]
async fn create_user(req: Request) -> Response {
    match serde_json::from_slice::<CreateUser>(&req.body) {
        Ok(p) => Json(User { id: 1, name: p.name }).into_response(),
        Err(e) => Response { status: 400, body: e.to_string() },
    }
}

#[tokio::main]
async fn main() {
    App::new()
        .state(Db { name: "User".into() })
        .with(FnMiddleware(|req: Request, next: glideapi::Next| {
            Box::pin(async move {
                println!("→ {} {}", req.method, req.path);
                let resp = next(req).await;
                println!("← {}", resp.status);
                resp
            })
        }))
        .mount_routes()   // ← picks up all #[get] / #[post] handlers automatically
        .listen("0.0.0.0:3000")
        .await;
}
