use std::collections::BTreeMap;

use async_trait::async_trait;

use serde::Serialize;

#[derive(Debug, Default, Serialize)]
struct RequestInfoResponse<'a> {
    role: &'static str,
    connection_id: u64,
    request_id: u16,
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
            connection_id: *request.connection_id(),
            request_id: *request.request_id(),
            ..Default::default()
        };

        for (key, value) in request.params().iter() {
            if key.to_ascii_lowercase().starts_with("http_") {
                let http_header_key = &key[5..];
                response.http_headers.insert(http_header_key, value);
            } else {
                response.other_params.insert(key, value);
            }
        }

        crate::handlers::build_json_response(response)
    }
}

pub fn create_routes() -> Vec<crate::handlers::route::Route> {
    let mut routes = Vec::new();

    routes.push(crate::handlers::route::Route::new(
        "/cgi-bin/debug/request_info".to_string(),
        Box::new(crate::handlers::debug::RequestInfoHandler::new()),
    ));

    routes
}
