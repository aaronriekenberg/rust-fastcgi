use std::sync::Arc;

use anyhow::Context;

use async_trait::async_trait;

use chrono::prelude::Local;

use log::warn;

use tokio::{
    process::Command,
    sync::{Semaphore, SemaphorePermit},
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::handlers::{
    route::URIAndHandler,
    utils::{build_json_response, build_json_string_response, build_status_code_response},
    {FastCGIRequest, HttpResponse, RequestHandler},
};

fn current_time_string() -> String {
    Local::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

struct AllCommandsHandler {
    json_string: String,
}

impl AllCommandsHandler {
    fn new(commands: &Vec<crate::config::CommandInfo>) -> anyhow::Result<Self> {
        let json_string = serde_json::to_string(commands)
            .context("AllCommandsHandler::new: json marshal error")?;

        Ok(Self { json_string })
    }
}

#[async_trait]
impl RequestHandler for AllCommandsHandler {
    async fn handle(&self, _request: FastCGIRequest<'_>) -> HttpResponse {
        build_json_string_response(self.json_string.clone())
    }
}

#[derive(thiserror::Error, Debug)]
enum RunCommandSemaporeAcquireError {
    #[error("acquire timeout: {0}")]
    Timeout(#[from] tokio::time::error::Elapsed),

    #[error("acquire error: {0}")]
    AcquireError(#[from] tokio::sync::AcquireError),
}

struct RunCommandSemapore {
    semapore: Semaphore,
    acquire_timeout: Duration,
}

impl RunCommandSemapore {
    fn new(command_configuration: &crate::config::CommandConfiguration) -> Arc<Self> {
        Arc::new(Self {
            semapore: Semaphore::new(*command_configuration.max_concurrent_commands()),
            acquire_timeout: Duration::from_millis(
                *command_configuration.semaphore_acquire_timeout_millis(),
            ),
        })
    }

    async fn acquire(&self) -> Result<SemaphorePermit<'_>, RunCommandSemaporeAcquireError> {
        let result = tokio::time::timeout(self.acquire_timeout, self.semapore.acquire()).await?;

        let permit = result?;

        Ok(permit)
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
    run_command_semaphore: Arc<RunCommandSemapore>,
    command_info: crate::config::CommandInfo,
}

impl RunCommandHandler {
    fn new(
        run_command_semaphore: Arc<RunCommandSemapore>,
        command_info: crate::config::CommandInfo,
    ) -> Self {
        Self {
            run_command_semaphore,
            command_info,
        }
    }

    async fn run_command(
        &self,
        _permit: SemaphorePermit<'_>,
    ) -> Result<std::process::Output, std::io::Error> {
        let output = Command::new(self.command_info.command())
            .args(self.command_info.args())
            .output()
            .await?;

        Ok(output)
    }

    fn handle_command_result(
        &self,
        command_result: Result<std::process::Output, std::io::Error>,
        command_duration: Duration,
    ) -> HttpResponse {
        let response = RunCommandResponse {
            now: current_time_string(),
            command_duration_ms: command_duration.as_millis(),
            command_info: &self.command_info,
            command_output: match command_result {
                Err(err) => {
                    format!("error running command {}", err)
                }
                Ok(command_output) => {
                    let mut combined_output = String::with_capacity(
                        command_output.stderr.len() + command_output.stdout.len(),
                    );
                    combined_output.push_str(&String::from_utf8_lossy(&command_output.stderr));
                    combined_output.push_str(&String::from_utf8_lossy(&command_output.stdout));
                    combined_output
                }
            },
        };

        build_json_response(response)
    }
}

#[async_trait]
impl RequestHandler for RunCommandHandler {
    async fn handle(&self, _request: FastCGIRequest<'_>) -> HttpResponse {
        let permit = match self.run_command_semaphore.acquire().await {
            Err(err) => {
                warn!("run_command_semaphore.acquire error: {}", err);
                return build_status_code_response(http::StatusCode::TOO_MANY_REQUESTS);
            }
            Ok(permit) => permit,
        };

        let command_start_time = Instant::now();
        let command_result = self.run_command(permit).await;
        let command_duration = command_start_time.elapsed();

        self.handle_command_result(command_result, command_duration)
    }
}

pub fn create_routes(
    command_configuration: &crate::config::CommandConfiguration,
) -> anyhow::Result<Vec<URIAndHandler>> {
    let mut routes: Vec<URIAndHandler> = Vec::new();

    routes.push((
        "/cgi-bin/commands".to_string(),
        Box::new(AllCommandsHandler::new(command_configuration.commands())?),
    ));

    if command_configuration.commands().len() > 0 {
        let run_command_semaphore = RunCommandSemapore::new(command_configuration);

        for command_info in command_configuration.commands() {
            let expected_uri = format!("/cgi-bin/commands/{}", command_info.id());

            routes.push((
                expected_uri,
                Box::new(RunCommandHandler::new(
                    Arc::clone(&run_command_semaphore),
                    command_info.clone(),
                )),
            ));
        }
    }

    Ok(routes)
}
