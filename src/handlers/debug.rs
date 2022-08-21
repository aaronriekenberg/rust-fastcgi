use std::collections::BTreeMap;

use async_trait::async_trait;

use serde::Serialize;

use crate::handlers::route::URIAndHandler;
use crate::handlers::utils::build_json_response;

#[derive(Debug, Default, Serialize)]
struct RequestInfoResponse<'a> {
    role: &'a str,
    connection_id: u64,
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

const HTTP_HEADER_PREFIX: &'static str = "http_";
const HTTP_HEADER_LEN: usize = HTTP_HEADER_PREFIX.len();

fn decode_http_header_key(key: &str) -> Option<&str> {
    if (key.len() >= HTTP_HEADER_LEN)
        && (HTTP_HEADER_PREFIX.eq_ignore_ascii_case(&key[..HTTP_HEADER_LEN]))
    {
        Some(&key[HTTP_HEADER_LEN..])
    } else {
        None
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
            connection_id: *request.connection_id(),
            request_id: *request.request_id(),
            request_uri: request.request_uri().unwrap_or("UNKNOWN"),
            ..Default::default()
        };

        for (key, value) in request.params().iter() {
            match decode_http_header_key(key) {
                Some(http_header_key) => {
                    response.http_headers.insert(http_header_key, value);
                }
                None => {
                    response.other_params.insert(key, value);
                }
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
