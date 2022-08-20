mod commands;
mod debug;
mod route;
mod utils;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use getset::Getters;

#[derive(Debug, Getters)]
#[getset(get = "pub")]
pub struct FastCGIRequest<'a> {
    role: &'a str,
    connection_id: u64,
    request_id: u16,
    params: HashMap<&'a str, &'a str>,
    request_uri: Option<&'a str>,
}

impl<'a> FastCGIRequest<'a> {
    pub fn new(
        role: &'a str,
        connection_id: u64,
        request_id: u16,
        params: HashMap<&'a str, &'a str>,
    ) -> Self {
        let request_uri = match params.get("request_uri") {
            Some(request_uri) => Some(*request_uri),
            None => None,
        };

        Self {
            role,
            connection_id,
            request_id,
            params,
            request_uri,
        }
    }
}

pub type HttpResponse = http::Response<Option<String>>;

#[async_trait]
pub trait RequestHandler: Send + Sync {
    async fn handle(&self, request: FastCGIRequest<'_>) -> HttpResponse;
}

pub fn create_handlers(configuration: &crate::config::Configuration) -> Arc<dyn RequestHandler> {
    let mut routes = Vec::new();

    routes.append(&mut debug::create_routes());

    routes.append(&mut commands::create_routes(configuration));

    Arc::new(route::Router::new(routes))
}
