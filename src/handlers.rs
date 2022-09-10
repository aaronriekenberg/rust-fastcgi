mod commands;
mod debug;
mod route;
mod utils;

use std::sync::Arc;

use async_trait::async_trait;

use crate::{request::FastCGIRequest, response::HttpResponse};

#[async_trait]
pub trait RequestHandler: Send + Sync {
    async fn handle(&self, request: FastCGIRequest<'_>) -> HttpResponse;
}

pub fn create_handlers(
    configuration: &crate::config::Configuration,
) -> anyhow::Result<Arc<dyn RequestHandler>> {
    let mut routes = Vec::new();

    routes.append(&mut commands::create_routes(
        configuration.command_configuration(),
    )?);

    routes.append(&mut debug::create_routes());

    Ok(Arc::new(route::Router::new(routes)?))
}
