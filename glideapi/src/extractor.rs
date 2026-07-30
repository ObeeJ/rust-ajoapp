use std::collections::HashMap;
use std::sync::Arc;

/// Parsed incoming request passed to handlers
#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: bytes::Bytes,
    pub params: HashMap<String, String>,
    pub req_id: String,
    pub(crate) state: Option<Arc<dyn std::any::Any + Send + Sync>>,
}

/// Typed path param extractor
pub struct Path<T>(pub T);

/// Typed JSON body extractor
pub struct Json<T>(pub T);

/// App state extractor — clones T out of the shared Arc
pub struct State<T: Clone + Send + Sync + 'static>(pub T);

pub trait FromRequest: Sized {
    fn from_request(req: &Request) -> crate::error::Result<Self>;
}

impl<T: std::str::FromStr> FromRequest for Path<T>
where
    T::Err: std::fmt::Display,
{
    fn from_request(req: &Request) -> crate::error::Result<Self> {
        let val = req.params.values().next()
            .ok_or_else(|| crate::error::Error::BadRequest("missing path param".into()))?;
        val.parse::<T>()
            .map(Path)
            .map_err(|e| crate::error::Error::BadRequest(e.to_string()))
    }
}

impl<T: serde::de::DeserializeOwned> FromRequest for Json<T> {
    fn from_request(req: &Request) -> crate::error::Result<Self> {
        serde_json::from_slice(&req.body)
            .map(Json)
            .map_err(|e| crate::error::Error::BadRequest(e.to_string()))
    }
}

impl<T: Clone + Send + Sync + 'static> FromRequest for State<T> {
    fn from_request(req: &Request) -> crate::error::Result<Self> {
        req.state
            .as_ref()
            .and_then(|s| s.downcast_ref::<T>())
            .cloned()
            .map(State)
            .ok_or_else(|| crate::error::Error::Internal("state not found".into()))
    }
}
