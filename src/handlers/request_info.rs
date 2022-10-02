use std::collections::BTreeMap;

use async_trait::async_trait;

use serde::Serialize;

use crate::handlers::{
    route::URIAndHandler,
    utils::build_json_response,
    {FastCGIRequest, HttpResponse, RequestHandler},
};

#[derive(Debug, Default, Serialize)]
struct RequestInfoResponse<'a> {
    fastcgi_role: &'a str,
    fastcgi_connection_id: u64,
    fastcgi_request_id: u16,
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
impl RequestHandler for RequestInfoHandler {
    async fn handle(&self, request: FastCGIRequest<'_>) -> HttpResponse {
        let mut response = RequestInfoResponse {
            fastcgi_role: request.role(),
            fastcgi_connection_id: *request.request_id().connection_id(),
            fastcgi_request_id: *request.request_id().request_id(),
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
    vec![(
        "/cgi-bin/request_info".to_string(),
        Box::new(RequestInfoHandler::new()),
    )]
}
