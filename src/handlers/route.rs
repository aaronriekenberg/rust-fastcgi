use std::collections::HashMap;

use async_trait::async_trait;

use crate::handlers::{
    utils::build_status_code_response,
    {FastCGIRequest, HttpResponse, RequestHandler},
};

pub type URIAndHandler = (String, Box<dyn RequestHandler>);

pub struct Router {
    uri_to_request_handler: HashMap<String, Box<dyn RequestHandler>>,
}

impl Router {
    pub fn new(routes: Vec<URIAndHandler>) -> anyhow::Result<Self> {
        let mut router = Self {
            uri_to_request_handler: HashMap::new(),
        };
        for (ref uri, handler) in routes {
            if router
                .uri_to_request_handler
                .insert(uri.clone(), handler)
                .is_some()
            {
                anyhow::bail!("Router::new error: collision in router uri '{}'", uri);
            }
        }
        Ok(router)
    }
}

#[async_trait]
impl RequestHandler for Router {
    async fn handle(&self, request: FastCGIRequest<'_>) -> HttpResponse {
        match request.request_uri() {
            None => build_status_code_response(http::StatusCode::BAD_REQUEST),
            Some(request_uri) => match self.uri_to_request_handler.get(*request_uri) {
                Some(handler) => handler.handle(request).await,
                None => build_status_code_response(http::StatusCode::NOT_FOUND),
            },
        }
    }
}
