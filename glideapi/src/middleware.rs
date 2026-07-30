use std::{future::Future, pin::Pin, sync::Arc};
use crate::{extractor::Request, response::Response};

pub type Next = Arc<dyn Fn(Request) -> Pin<Box<dyn Future<Output = Response> + Send>> + Send + Sync>;

/// Implement this to write middleware
pub trait Middleware: Send + Sync + 'static {
    fn handle(&self, req: Request, next: Next) -> Pin<Box<dyn Future<Output = Response> + Send>>;
}

/// Wrap a simple async closure as middleware
pub struct FnMiddleware<F>(pub F);

impl<F, Fut> Middleware for FnMiddleware<F>
where
    F: Fn(Request, Next) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Response> + Send + 'static,
{
    fn handle(&self, req: Request, next: Next) -> Pin<Box<dyn Future<Output = Response> + Send>> {
        Box::pin((self.0)(req, next))
    }
}
