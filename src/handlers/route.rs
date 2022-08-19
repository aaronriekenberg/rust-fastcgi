use async_trait::async_trait;

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

    fn matches(&self, request_uri: &str) -> bool {
        request_uri == self.expected_uri
    }
}

pub struct Router {
    routes: Vec<Route>,
}

impl Router {
    pub fn new(routes: Vec<Route>) -> Self {
        Self { routes }
    }
}

#[async_trait]
impl crate::handlers::RequestHandler for Router {
    async fn handle(
        &self,
        request: crate::handlers::FastCGIRequest<'_>,
    ) -> crate::handlers::HttpResponse {
        if let Some(request_uri) = request.params().get("request_uri") {
            for route in &self.routes {
                if route.matches(&request_uri) {
                    return route.request_handler.handle(request).await;
                }
            }

            crate::handlers::build_status_code_response(http::StatusCode::NOT_FOUND)
        } else {
            crate::handlers::build_status_code_response(http::StatusCode::BAD_REQUEST)
        }
    }
}
