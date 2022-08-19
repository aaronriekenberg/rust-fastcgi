use std::process::Output;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;

use log::warn;

use tokio::process::Command;
use tokio::sync::{Semaphore, SemaphorePermit, TryAcquireError};

use serde::Serialize;
pub(super) struct AllCommandsHandler {
    commands: Vec<crate::config::CommandInfo>,
}

impl AllCommandsHandler {
    pub(super) fn new(commands: Vec<crate::config::CommandInfo>) -> Self {
        Self { commands }
    }
}

#[async_trait]
impl crate::handlers::RequestHandler for AllCommandsHandler {
    async fn handle(
        &self,
        _request: crate::handlers::FastCGIRequest<'_>,
    ) -> crate::handlers::HttpResponse {
        crate::handlers::build_json_response(&self.commands)
    }
}

#[derive(Debug, Serialize)]
struct RunCommandResponse<'a> {
    now: String,
    command_duration_ms: u128,
    command_info: &'a crate::config::CommandInfo,
    command_output: String,
}

pub(super) struct RunCommandHandler {
    run_command_semaphore: Arc<Semaphore>,
    command_info: crate::config::CommandInfo,
}

impl RunCommandHandler {
    pub(super) fn new(
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
    ) -> crate::handlers::HttpResponse {
        let output = match command_result {
            Err(err) => {
                let response = RunCommandResponse {
                    now: crate::handlers::current_time_string(),
                    command_duration_ms: 0,
                    command_info: &self.command_info,
                    command_output: format!("error running command {}", err),
                };
                return crate::handlers::build_json_response(response);
            }
            Ok(output) => output,
        };

        let mut combined_output = String::with_capacity(output.stderr.len() + output.stdout.len());
        combined_output.push_str(&String::from_utf8_lossy(&output.stderr));
        combined_output.push_str(&String::from_utf8_lossy(&output.stdout));

        let response = RunCommandResponse {
            now: crate::handlers::current_time_string(),
            command_duration_ms: command_duration.as_millis(),
            command_info: &self.command_info,
            command_output: combined_output,
        };

        crate::handlers::build_json_response(response)
    }
}

#[async_trait]
impl crate::handlers::RequestHandler for RunCommandHandler {
    async fn handle(
        &self,
        _request: crate::handlers::FastCGIRequest<'_>,
    ) -> crate::handlers::HttpResponse {
        let permit = match self.acquire_run_command_semaphore() {
            Err(err) => {
                warn!("acquire_run_command_semaphore error {}", err);
                return crate::handlers::build_status_code_response(
                    http::StatusCode::TOO_MANY_REQUESTS,
                );
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
