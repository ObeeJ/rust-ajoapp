use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};
use crate::{extractor::Request, response::Response};

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;
type Handler = Arc<dyn Fn(Request) -> BoxFuture<Response> + Send + Sync>;

#[derive(Clone)]
struct Route {
    method: String,
    segments: Vec<String>,
    handler: Handler,
}

#[derive(Clone, Default)]
pub struct Router {
    routes: Vec<Route>,
}

impl Router {
    pub fn new() -> Self { Self::default() }

    pub fn add<F, Fut>(&mut self, method: &str, path: &str, handler: F)
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.routes.push(Route {
            method: method.to_uppercase(),
            segments: path.trim_matches('/').split('/').map(String::from).collect(),
            handler: Arc::new(move |req| Box::pin(handler(req))),
        });
    }

    pub async fn handle(&self, req: Request) -> Response {
        let incoming: Vec<&str> = req.path.trim_matches('/').split('/').collect();

        for route in &self.routes {
            if route.method != req.method { continue; }
            if route.segments.len() != incoming.len() { continue; }

            let mut params = HashMap::new();
            let matched = route.segments.iter().zip(incoming.iter()).all(|(seg, inc)| {
                if seg.starts_with(':') {
                    params.insert(seg[1..].to_string(), inc.to_string());
                    true
                } else {
                    seg == inc
                }
            });

            if matched {
                let mut req = req;
                req.params = params;
                return (route.handler)(req).await;
            }
        }

        Response { status: 404, body: format!("no route for {} {}", req.method, req.path), headers: vec![] }
    }
}
