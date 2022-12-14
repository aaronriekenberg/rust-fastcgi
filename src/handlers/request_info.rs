use std::{collections::BTreeMap, path::PathBuf};

use async_trait::async_trait;

use serde::Serialize;

use crate::{
    connection::FastCGIConnectionID,
    handlers::{
        route::PathSuffixAndHandler,
        utils::build_json_response,
        {FastCGIRequest, HttpResponse, RequestHandler},
    },
};

#[derive(Debug, Serialize)]
struct RequestInfoResponse<'a> {
    fastcgi_role: &'a str,
    fastcgi_connection_id: FastCGIConnectionID,
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
            fastcgi_connection_id: *request.connection_id(),
            fastcgi_request_id: request.request_id().0,
            request_uri: request.request_uri().unwrap_or("[Unknown URI]"),
            http_headers: BTreeMap::new(),
            other_params: BTreeMap::new(),
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

pub fn create_routes() -> Vec<PathSuffixAndHandler> {
    vec![(
        PathBuf::from("request_info"),
        Box::new(RequestInfoHandler::new()),
    )]
}
