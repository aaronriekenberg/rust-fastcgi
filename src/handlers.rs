mod commands;
mod debug;
mod route;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use getset::Getters;

use log::warn;

use serde::Serialize;

#[derive(Debug, Getters)]
#[getset(get = "pub")]
pub struct FastCGIRequest<'a> {
    role: &'static str,
    connection_id: u64,
    request_id: u16,
    params: HashMap<&'a str, &'a str>,
}

impl<'a> FastCGIRequest<'a> {
    pub fn new(
        role: &'static str,
        connection_id: u64,
        request_id: u16,
        params: HashMap<&'a str, &'a str>,
    ) -> Self {
        Self {
            role,
            connection_id,
            request_id,
            params,
        }
    }
}

pub type HttpResponse = http::Response<Option<String>>;

#[async_trait]
pub trait RequestHandler: Send + Sync {
    async fn handle(&self, request: FastCGIRequest<'_>) -> HttpResponse;
}

fn build_json_response(response_dto: impl Serialize) -> HttpResponse {
    let json_result = serde_json::to_string(&response_dto);

    match json_result {
        Err(e) => {
            warn!("json serialization error {}", e);

            http::Response::builder()
                .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                .body(None)
                .unwrap()
        }
        Ok(json_string) => http::Response::builder()
            .status(http::StatusCode::OK)
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(Some(json_string))
            .unwrap(),
    }
}

fn build_status_code_response(status_code: http::StatusCode) -> HttpResponse {
    http::Response::builder()
        .status(status_code)
        .body(None)
        .unwrap()
}

pub fn create_handlers(configuration: &crate::config::Configuration) -> Arc<dyn RequestHandler> {
    let mut routes = Vec::new();

    routes.append(&mut debug::create_routes());

    routes.append(&mut commands::create_routes(configuration));

    Arc::new(route::Router::new(routes))
}
