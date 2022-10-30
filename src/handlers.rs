mod commands;
mod request_info;
mod route;
mod utils;

use async_trait::async_trait;

use crate::{request::FastCGIRequest, response::body::HttpResponseBody, response::HttpResponse};

#[async_trait]
pub trait RequestHandler: Send + Sync {
    async fn handle(&self, request: FastCGIRequest<'_>) -> HttpResponse;
}

pub fn create_handlers() -> anyhow::Result<Box<dyn RequestHandler>> {
    let mut routes = Vec::new();

    routes.append(&mut commands::create_routes()?);

    routes.append(&mut request_info::create_routes());

    Ok(Box::new(route::Router::new(routes)?))
}
