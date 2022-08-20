use std::collections::HashMap;

use async_trait::async_trait;

use crate::handlers::utils::build_status_code_response;

pub struct Route {
    expected_uri: String,
    request_handler: Box<dyn crate::handlers::RequestHandler>,
}

impl Route {
    pub fn new(
        expected_uri: String,
        request_handler: Box<dyn crate::handlers::RequestHandler>,
    ) -> Self {
        Self {
            expected_uri,
            request_handler,
        }
    }
}

pub struct Router {
    uri_to_request_handler: HashMap<String, Box<dyn crate::handlers::RequestHandler>>,
}

impl Router {
    pub fn new(routes: Vec<Route>) -> Self {
        let mut router = Self {
            uri_to_request_handler: HashMap::new(),
        };
        for route in routes {
            router
                .uri_to_request_handler
                .insert(route.expected_uri, route.request_handler);
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
