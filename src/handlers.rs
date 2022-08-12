use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;

use log::warn;

use tokio::net::unix::OwnedWriteHalf;

use tokio::process::Command;

use tokio_fastcgi::Request;

use serde::Serialize;

pub type HttpResponse = http::Response<Option<String>>;

#[async_trait]
pub trait RequestHandler: Send + Sync {
    async fn handle(&self, request: Arc<Request<OwnedWriteHalf>>) -> HttpResponse;
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
struct DebugResponse {
    role: &'static str,
    request_id: u16,
    http_headers: BTreeMap<String, String>,
    other_params: BTreeMap<String, String>,
}

struct DebugHandler {}

#[async_trait]
impl RequestHandler for DebugHandler {
    async fn handle(&self, request: Arc<Request<OwnedWriteHalf>>) -> HttpResponse {
        let mut debug_response = DebugResponse {
            role: match request.role {
                tokio_fastcgi::Role::Authorizer => "Authorizer",
                tokio_fastcgi::Role::Filter => "Filter",
                tokio_fastcgi::Role::Responder => "Responder",
            },
            request_id: request.get_request_id(),
            ..Default::default()
        };

        if let Some(str_params) = request.str_params_iter() {
            for param in str_params {
                let value = param.1.unwrap_or("[Invalid UTF8]").to_string();

                let lower_case_key = param.0.to_ascii_lowercase();
                if lower_case_key.starts_with("http_") {
                    let http_header_key = &lower_case_key[5..];
                    debug_response
                        .http_headers
                        .insert(http_header_key.to_string(), value);
                } else {
                    debug_response.other_params.insert(lower_case_key, value);
                }
            }
        }

        build_json_response(debug_response)
    }
}

#[derive(Debug, Default, Serialize)]
struct CommandResponse {
    command_output: String,
}

struct CommandHandler {}

#[async_trait]
impl RequestHandler for CommandHandler {
    async fn handle(&self, _request: Arc<Request<OwnedWriteHalf>>) -> HttpResponse {
        let command_result = Command::new("ls").arg("-latrh").output().await;

        let output = match command_result {
            Err(err) => {
                let command_response = CommandResponse {
                    command_output: format!("error running command {}", err),
                };
                return build_json_response(command_response);
            }
            Ok(output) => output,
        };

        let mut combined_output = String::with_capacity(output.stderr.len() + output.stdout.len());
        combined_output.push_str(&String::from_utf8_lossy(&output.stderr));
        combined_output.push_str(&String::from_utf8_lossy(&output.stdout));

        let command_response = CommandResponse {
            command_output: combined_output,
        };

        build_json_response(command_response)
    }
}

struct Route {
    url_prefix: String,
    request_handler: Box<dyn RequestHandler>,
}

impl Route {
    fn matches(&self, request_uri: &str) -> bool {
        request_uri.starts_with(&self.url_prefix)
    }
}

pub struct Router {
    routes: Vec<Route>,
}

impl Router {
    pub fn new() -> Self {
        let mut routes = Vec::new();

        routes.push(Route {
            url_prefix: "/cgi-bin/debug".to_string(),
            request_handler: Box::new(DebugHandler {}),
        });

        routes.push(Route {
            url_prefix: "/cgi-bin/commands/ls".to_string(),
            request_handler: Box::new(CommandHandler {}),
        });

        Self { routes }
    }
}

#[async_trait]
impl RequestHandler for Router {
    async fn handle(&self, request: Arc<Request<OwnedWriteHalf>>) -> HttpResponse {
        if let Some(request_uri) = request.get_str_param("request_uri").map(String::from) {
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
