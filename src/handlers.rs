mod commands;
mod debug;
mod route;
mod utils;

use std::sync::Arc;

use async_trait::async_trait;

use crate::request::FastCGIRequest;
use crate::response::HttpResponse;

#[async_trait]
pub trait RequestHandler: Send + Sync {
    async fn handle(&self, request: FastCGIRequest<'_>) -> HttpResponse;
}

pub fn create_handlers(configuration: &crate::config::Configuration) -> Arc<dyn RequestHandler> {
    let mut routes = Vec::new();

    routes.append(&mut commands::create_routes(configuration));

    routes.append(&mut debug::create_routes());

    Arc::new(route::Router::new(routes))
}
