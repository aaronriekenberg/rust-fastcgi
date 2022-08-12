use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use async_trait::async_trait;

use getset::Getters;

use log::warn;

use tokio::process::Command;

use serde::Serialize;

#[derive(Debug, Getters)]
#[getset(get = "pub")]
pub struct FastCGIRequest {
    role: &'static str,
    request_id: u16,
    params: HashMap<String, String>,
}

impl FastCGIRequest {
    pub fn new(role: &'static str, request_id: u16, params: HashMap<String, String>) -> Self {
        Self {
            role,
            request_id,
            params,
        }
    }
}

pub type HttpResponse = http::Response<Option<String>>;

#[async_trait]
pub trait RequestHandler: Send + Sync {
    async fn handle(&self, request: FastCGIRequest) -> HttpResponse;
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

#[derive(Debug, Default, Serialize)]
struct RequestInfoResponse {
    role: &'static str,
    request_id: u16,
    http_headers: BTreeMap<String, String>,
    other_params: BTreeMap<String, String>,
}

struct RequestInfoHandler {}

impl RequestInfoHandler {
    fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl RequestHandler for RequestInfoHandler {
    async fn handle(&self, request: FastCGIRequest) -> HttpResponse {
        let mut response = RequestInfoResponse {
            role: request.role(),
            request_id: *request.request_id(),
            ..Default::default()
        };

        for param in request.params().iter() {
            let lower_case_key = param.0.to_ascii_lowercase();
            let value = param.1;

            if lower_case_key.starts_with("http_") {
                let http_header_key = &lower_case_key[5..];
                response
                    .http_headers
                    .insert(http_header_key.to_string(), value.clone());
            } else {
                response
                    .other_params
                    .insert(lower_case_key, value.clone());
            }
        }

        build_json_response(response)
    }
}

struct AllCommandsHandler {
    commands: Vec<crate::config::CommandInfo>,
}

impl AllCommandsHandler {
    fn new(commands: Vec<crate::config::CommandInfo>) -> Self {
        Self { commands }
    }
}

#[async_trait]
impl RequestHandler for AllCommandsHandler {
    async fn handle(&self, _request: FastCGIRequest) -> HttpResponse {
        build_json_response(&self.commands)
    }
}

#[derive(Debug, Serialize)]
struct RunCommandResponse {
    command_info: crate::config::CommandInfo,
    command_output: String,
}

struct RunCommandHandler {
    command_info: crate::config::CommandInfo,
}

impl RunCommandHandler {
    fn new(command_info: crate::config::CommandInfo) -> Self {
        Self { command_info }
    }
}

#[async_trait]
impl RequestHandler for RunCommandHandler {
    async fn handle(&self, _request: FastCGIRequest) -> HttpResponse {
        let command_result = Command::new(self.command_info.command())
            .args(self.command_info.args())
            .output()
            .await;

        let output = match command_result {
            Err(err) => {
                let response = RunCommandResponse {
                    command_info: self.command_info.clone(),
                    command_output: format!("error running command {}", err),
                };
                return build_json_response(response);
            }
            Ok(output) => output,
        };

        let mut combined_output = String::with_capacity(output.stderr.len() + output.stdout.len());
        combined_output.push_str(&String::from_utf8_lossy(&output.stderr));
        combined_output.push_str(&String::from_utf8_lossy(&output.stdout));

        let response = RunCommandResponse {
            command_info: self.command_info.clone(),
            command_output: combined_output,
        };

        build_json_response(response)
    }
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
    async fn handle(&self, request: FastCGIRequest) -> HttpResponse {
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
        request_handler: Box::new(RequestInfoHandler::new()),
    });

    routes.push(Route {
        expected_uri: "/cgi-bin/commands".to_string(),
        request_handler: Box::new(AllCommandsHandler::new(
            configuration.command_configuration().commands().clone(),
        )),
    });

    for command_info in configuration.command_configuration().commands() {
        let expected_uri = format!("/cgi-bin/commands/{}", command_info.id());

        routes.push(Route {
            expected_uri,
            request_handler: Box::new(RunCommandHandler::new(command_info.clone())),
        });
    }

    Arc::new(Router::new(routes))
}
