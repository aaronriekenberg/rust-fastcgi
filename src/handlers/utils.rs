use log::warn;

use serde::Serialize;

use crate::response::{HttpResponse, HttpResponseBody};

pub fn build_json_body_response(http_response_body: HttpResponseBody) -> HttpResponse {
    http::Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Some(http_response_body))
        .unwrap()
}

pub fn build_json_response(response_dto: impl Serialize) -> HttpResponse {
    let json_result = serde_json::to_string(&response_dto);

    match json_result {
        Err(e) => {
            warn!("build_json_response serialization error {}", e);

            http::Response::builder()
                .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                .body(None)
                .unwrap()
        }
        Ok(json_string) => build_json_body_response(HttpResponseBody::from(json_string)),
    }
}

pub fn build_status_code_response(status_code: http::StatusCode) -> HttpResponse {
    http::Response::builder()
        .status(status_code)
        .body(None)
        .unwrap()
}
