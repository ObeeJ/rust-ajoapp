use glideapi::{App, FromRequest, Json, Request, Response, State};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HealthLog {
    id: u32,
    note: String,
    mood: u8, // 1–10
}

#[derive(Clone)]
struct AppState {
    logs: Arc<Mutex<Vec<HealthLog>>>,
    next_id: Arc<Mutex<u32>>,
}

fn json_response<T: Serialize>(status: u16, body: T) -> Response {
    Response {
        status,
        body: serde_json::to_string(&body).unwrap(),
    }
}

#[tokio::main]
async fn main() {
    let state = AppState {
        logs: Arc::new(Mutex::new(vec![])),
        next_id: Arc::new(Mutex::new(1)),
    };

    App::new()
        .state(state)
        .route("GET", "/health", |_req: Request| async {
            json_response(200, serde_json::json!({ "status": "ok", "service": "cowri" }))
        })
        .route("GET", "/logs", |req: Request| async move {
            let State(state) = State::<AppState>::from_request(&req).unwrap();
            let logs = state.logs.lock().unwrap().clone();
            json_response(200, logs)
        })
        .route("POST", "/logs", |req: Request| async move {
            #[derive(Deserialize)]
            struct Body {
                note: String,
                mood: u8,
            }

            let Json(body) = match Json::<Body>::from_request(&req) {
                Ok(b) => b,
                Err(_) => return json_response(400, serde_json::json!({ "error": "invalid body" })),
            };

            if !(1..=10).contains(&body.mood) {
                return json_response(400, serde_json::json!({ "error": "mood must be 1–10" }));
            }

            let State(state) = State::<AppState>::from_request(&req).unwrap();
            let mut id_lock = state.next_id.lock().unwrap();
            let id = *id_lock;
            *id_lock += 1;
            drop(id_lock);

            let log = HealthLog { id, note: body.note, mood: body.mood };
            state.logs.lock().unwrap().push(log.clone());

            json_response(201, log)
        })
        .listen("0.0.0.0:3000")
        .await;
}
