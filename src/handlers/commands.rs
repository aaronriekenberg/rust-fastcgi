use std::{
    process::Output,
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;

use chrono::prelude::Local;

use log::warn;

use tokio::{
    process::Command,
    sync::{Semaphore, SemaphorePermit, TryAcquireError},
};

use serde::Serialize;

use crate::handlers::{
    route::URIAndHandler,
    utils::{build_json_response, build_status_code_response},
    {FastCGIRequest, HttpResponse, RequestHandler},
};

fn current_time_string() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S%.9f %z").to_string()
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

    async fn run_command(
        &self,
        _permit: SemaphorePermit<'_>,
    ) -> Result<(Output, Duration), std::io::Error> {
        let command_start_time = Instant::now();

        let output = Command::new(self.command_info.command())
            .args(self.command_info.args())
            .output()
            .await?;

        let duration = Instant::now() - command_start_time;

        Ok((output, duration))
    }

    fn handle_command_result(
        &self,
        command_result: Result<(Output, Duration), std::io::Error>,
    ) -> HttpResponse {
        let (output, command_duration) = match command_result {
            Err(err) => {
                let response = RunCommandResponse {
                    now: current_time_string(),
                    command_duration_ms: 0,
                    command_info: &self.command_info,
                    command_output: format!("error running command {}", err),
                };
                return build_json_response(response);
            }
            Ok(result) => result,
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

        let result = self.run_command(permit).await;

        self.handle_command_result(result)
    }
}

pub fn create_routes(
    command_configuration: &crate::config::CommandConfiguration,
) -> Vec<URIAndHandler> {
    let mut routes: Vec<URIAndHandler> = Vec::new();

    routes.push((
        "/cgi-bin/commands".to_string(),
        Box::new(AllCommandsHandler::new(
            command_configuration.commands().clone(),
        )),
    ));

    if command_configuration.commands().len() > 0 {
        let run_command_semaphore = Arc::new(Semaphore::new(
            *command_configuration.max_concurrent_commands(),
        ));

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

    routes
}
