use std::collections::HashMap;

use async_trait::async_trait;

use crate::handlers::utils::build_status_code_response;
use crate::handlers::RequestHandler;

pub type URIAndHandler = (String, Box<dyn RequestHandler>);

pub struct Router {
    uri_to_request_handler: HashMap<String, Box<dyn RequestHandler>>,
}

impl Router {
    pub fn new(routes: Vec<URIAndHandler>) -> Self {
        let mut router = Self {
            uri_to_request_handler: HashMap::new(),
        };
        for (uri, handler) in routes {
            router.uri_to_request_handler.insert(uri, handler);
        }
        router
    }
}

#[async_trait]
impl crate::handlers::RequestHandler for Router {
    async fn handle(
        &self,
        request: crate::handlers::FastCGIRequest<'_>,
    ) -> crate::handlers::HttpResponse {
        if let Some(request_uri) = request.request_uri() {
            match self.uri_to_request_handler.get(*request_uri) {
                Some(handler) => handler.handle(request).await,
                None => build_status_code_response(http::StatusCode::NOT_FOUND),
            }
        } else {
            build_status_code_response(http::StatusCode::BAD_REQUEST)
        }
    }
}
