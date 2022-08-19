use std::collections::{BTreeMap, HashMap};
use std::process::Output;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;

use chrono::prelude::Local;

use getset::Getters;

use log::warn;

use tokio::process::Command;
use tokio::sync::{Semaphore, SemaphorePermit, TryAcquireError};

use serde::Serialize;

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

#[derive(Debug, Default, Serialize)]
struct RequestInfoResponse<'a> {
    role: &'static str,
    connection_id: u64,
    request_id: u16,
    http_headers: BTreeMap<&'a str, &'a str>,
    other_params: BTreeMap<&'a str, &'a str>,
}

struct RequestInfoHandler {}

impl RequestInfoHandler {
    fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl RequestHandler for RequestInfoHandler {
    async fn handle(&self, request: FastCGIRequest<'_>) -> HttpResponse {
        let mut response = RequestInfoResponse {
            role: request.role(),
            connection_id: *request.connection_id(),
            request_id: *request.request_id(),
            ..Default::default()
        };

        for (key, value) in request.params().iter() {
            if key.to_ascii_lowercase().starts_with("http_") {
                let http_header_key = &key[5..];
                response.http_headers.insert(http_header_key, value);
            } else {
                response.other_params.insert(key, value);
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
    async fn handle(&self, _request: FastCGIRequest<'_>) -> HttpResponse {
        build_json_response(&self.commands)
    }
}

#[derive(Debug, Serialize)]
struct RunCommandResponse<'a> {
    now: String,
    command_duration_ms: u128,
    command_info: &'a crate::config::CommandInfo,
    command_output: String,
}

struct RunCommandHandler {
    run_command_semaphore: Arc<Semaphore>,
    command_info: crate::config::CommandInfo,
}

impl RunCommandHandler {
    fn new(
        run_command_semaphore: Arc<Semaphore>,
        command_info: crate::config::CommandInfo,
    ) -> Self {
        Self {
            run_command_semaphore,
            command_info,
        }
    }

    fn acquire_run_command_semaphore(&self) -> Result<SemaphorePermit<'_>, TryAcquireError> {
        self.run_command_semaphore.try_acquire()
    }

    fn handle_command_result(
        &self,
        command_result: Result<Output, std::io::Error>,
        command_duration: Duration,
    ) -> HttpResponse {
        let output = match command_result {
            Err(err) => {
                let response = RunCommandResponse {
                    now: current_time_string(),
                    command_duration_ms: 0,
                    command_info: &self.command_info,
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
            now: current_time_string(),
            command_duration_ms: command_duration.as_millis(),
            command_info: &self.command_info,
            command_output: combined_output,
        };

        build_json_response(response)
    }
}

#[async_trait]
impl RequestHandler for RunCommandHandler {
    async fn handle(&self, _request: FastCGIRequest<'_>) -> HttpResponse {
        let permit = match self.acquire_run_command_semaphore() {
            Err(err) => {
                warn!("acquire_run_command_semaphore error {}", err);
                return build_status_code_response(http::StatusCode::TOO_MANY_REQUESTS);
            }
            Ok(permit) => permit,
        };

        let command_start_time = Instant::now();

        let command_result = Command::new(self.command_info.command())
            .args(self.command_info.args())
            .output()
            .await;

        let command_duration = Instant::now() - command_start_time;

        drop(permit);

        self.handle_command_result(command_result, command_duration)
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
        request_handler: Box::new(RequestInfoHandler::new()),
    });

    routes.push(Route {
        expected_uri: "/cgi-bin/commands".to_string(),
        request_handler: Box::new(AllCommandsHandler::new(
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
                request_handler: Box::new(RunCommandHandler::new(
                    Arc::clone(&run_command_semaphore),
                    command_info.clone(),
                )),
            });
        }
    }

    Arc::new(Router::new(routes))
}
