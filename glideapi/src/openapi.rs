use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize, Default)]
pub struct OpenApi {
    pub openapi: String,
    pub info: Info,
    pub paths: HashMap<String, PathItem>,
}

#[derive(Serialize, Default)]
pub struct Info {
    pub title: String,
    pub version: String,
}

#[derive(Serialize, Default)]
pub struct PathItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub get: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub put: Option<Option<Operation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete: Option<Operation>,
}

#[derive(Serialize, Default, Clone)]
pub struct Operation {
    pub summary: String,
    pub responses: HashMap<String, OperationResponse>,
}

#[derive(Serialize, Clone)]
pub struct OperationResponse {
    pub description: String,
}

impl OpenApi {
    pub fn new(title: &str, version: &str) -> Self {
        Self {
            openapi: "3.0.0".into(),
            info: Info { title: title.into(), version: version.into() },
            paths: HashMap::new(),
        }
    }

    pub fn add_route(&mut self, method: &str, path: &str, summary: &str) {
        // Convert :param style to {param} for OpenAPI spec
        let oapi_path = path.split('/').map(|seg| {
            if seg.starts_with(':') { format!("{{{}}}", &seg[1..]) } else { seg.to_string() }
        }).collect::<Vec<_>>().join("/");

        let op = Operation {
            summary: summary.into(),
            responses: [("200".to_string(), OperationResponse { description: "OK".into() })]
                .into_iter().collect(),
        };

        let entry = self.paths.entry(oapi_path).or_default();
        match method.to_uppercase().as_str() {
            "GET"    => entry.get = Some(op),
            "POST"   => entry.post = Some(op),
            "DELETE" => entry.delete = Some(op),
            _ => {}
        }
    }
}
