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

pub(super) struct RequestInfoHandler {}

impl RequestInfoHandler {
    pub(super) fn new() -> Self {
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
