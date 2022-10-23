use std::{collections::HashMap, path::Path, path::PathBuf};

use anyhow::Context;
use async_trait::async_trait;

use crate::handlers::{
    utils::build_status_code_response,
    {FastCGIRequest, HttpResponse, RequestHandler},
};

pub type PathSuffixAndHandler = (PathBuf, Box<dyn RequestHandler>);

pub struct Router {
    uri_to_request_handler: HashMap<String, Box<dyn RequestHandler>>,
}

impl Router {
    pub fn new(
        context_configuration: &crate::config::ContextConfiguration,
        routes: Vec<PathSuffixAndHandler>,
    ) -> anyhow::Result<Self> {
        let mut router = Self {
            uri_to_request_handler: HashMap::with_capacity(routes.len()),
        };

        for (ref path_suffix, handler) in routes {
            let uri = Path::new(context_configuration.context())
                .join(path_suffix)
                .to_str()
                .with_context(|| {
                    format!(
                        "route path contains invalid UTF-8 path_suffix = '{:?}'",
                        path_suffix
                    )
                })?
                .to_owned();

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
