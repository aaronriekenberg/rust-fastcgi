use std::collections::BTreeMap;

use async_trait::async_trait;

use serde::Serialize;

use crate::handlers::route::URIAndHandler;
use crate::handlers::utils::build_json_response;

#[derive(Debug, Default, Serialize)]
struct RequestInfoResponse<'a> {
    role: &'a str,
    request_id: u16,
    request_uri: &'a str,
    http_headers: BTreeMap<&'a str, &'a str>,
    other_params: BTreeMap<&'a str, &'a str>,
}

struct RequestInfoHandler {}

impl RequestInfoHandler {
    fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl crate::handlers::RequestHandler for RequestInfoHandler {
    async fn handle(
        &self,
        request: crate::handlers::FastCGIRequest<'_>,
    ) -> crate::handlers::HttpResponse {
        let mut response = RequestInfoResponse {
            role: request.role(),
            request_id: *request.request_id(),
            request_uri: request.request_uri().unwrap_or("[Unknown URI]"),
            ..Default::default()
        };

        for (key, value) in request.params().iter() {
            if key.starts_with("http_") {
                let http_header_key = &key[5..];
                response.http_headers.insert(http_header_key, value);
            } else {
                response.other_params.insert(key, value);
            }
        }

        build_json_response(response)
    }
}

pub fn create_routes() -> Vec<URIAndHandler> {
    let mut routes: Vec<URIAndHandler> = Vec::new();

    routes.push((
        "/cgi-bin/debug/request_info".to_string(),
        Box::new(RequestInfoHandler::new()),
    ));

    routes
}
