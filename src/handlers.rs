mod commands;
mod debug;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use chrono::prelude::Local;

use getset::Getters;

use log::warn;

use serde::Serialize;

use tokio::sync::Semaphore;

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

fn current_time_string() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S%.9f %z").to_string()
}

struct Route {
    expected_uri: String,
    request_handler: Box<dyn RequestHandler>,
}

impl Route {
    fn matches(&self, request_uri: &str) -> bool {
        request_uri == self.expected_uri
    }
}

struct Router {
    routes: Vec<Route>,
}

impl Router {
    fn new(routes: Vec<Route>) -> Self {
        Self { routes }
    }
}

#[async_trait]
impl RequestHandler for Router {
    async fn handle(&self, request: FastCGIRequest<'_>) -> HttpResponse {
        if let Some(request_uri) = request.params().get("request_uri") {
            for route in &self.routes {
                if route.matches(&request_uri) {
                    return route.request_handler.handle(request).await;
                }
            }

            build_status_code_response(http::StatusCode::NOT_FOUND)
        } else {
            build_status_code_response(http::StatusCode::BAD_REQUEST)
        }
    }
}

pub fn create_handlers(configuration: &crate::config::Configuration) -> Arc<dyn RequestHandler> {
    let mut routes = Vec::new();

    routes.push(Route {
        expected_uri: "/cgi-bin/debug/request_info".to_string(),
        request_handler: Box::new(crate::handlers::debug::RequestInfoHandler::new()),
    });

    routes.push(Route {
        expected_uri: "/cgi-bin/commands".to_string(),
        request_handler: Box::new(crate::handlers::commands::AllCommandsHandler::new(
            configuration.command_configuration().commands().clone(),
        )),
    });

    if configuration.command_configuration().commands().len() > 0 {
        let run_command_semaphore = Arc::new(Semaphore::new(
            *configuration
                .command_configuration()
                .max_concurrent_commands(),
        ));

        for command_info in configuration.command_configuration().commands() {
            let expected_uri = format!("/cgi-bin/commands/{}", command_info.id());

            routes.push(Route {
                expected_uri,
                request_handler: Box::new(crate::handlers::commands::RunCommandHandler::new(
                    Arc::clone(&run_command_semaphore),
                    command_info.clone(),
                )),
            });
        }
    }

    Arc::new(Router::new(routes))
}
