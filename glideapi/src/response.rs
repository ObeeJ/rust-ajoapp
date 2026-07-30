/// A minimal HTTP response
#[derive(Debug, Default)]
pub struct Response {
    pub status:  u16,
    pub body:    String,
    pub headers: Vec<(String, String)>,
}

impl Response {
    pub fn ok(body: impl Into<String>) -> Self {
        Self { status: 200, body: body.into(), headers: vec![] }
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((key.into(), value.into()));
        self
    }
}

/// Anything that can become a Response
pub trait IntoResponse {
    fn into_response(self) -> Response;
}

impl IntoResponse for Response {
    fn into_response(self) -> Response { self }
}

impl IntoResponse for &'static str {
    fn into_response(self) -> Response { Response::ok(self) }
}

impl IntoResponse for String {
    fn into_response(self) -> Response { Response::ok(self) }
}

/// Json<T> wrapper — serialises T as JSON with content-type header
pub struct Json<T>(pub T);

impl<T: serde::Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        match serde_json::to_string(&self.0) {
            Ok(body) => Response::ok(body),
            Err(e) => Response { status: 500, body: e.to_string(), headers: vec![] },
        }
    }
}

impl<T: IntoResponse> IntoResponse for crate::error::Result<T> {
    fn into_response(self) -> Response {
        match self {
            Ok(v) => v.into_response(),
            Err(e) => e.into_response(),
        }
    }
}
