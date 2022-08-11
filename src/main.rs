mod config;

use std::collections::BTreeMap;
use std::fmt::{Debug, Write};
use std::sync::Arc;

use async_trait::async_trait;

use log::{debug, error, info, warn};

use tokio::net::{unix::OwnedWriteHalf, UnixListener};

use tokio::process::Command;

use tokio_fastcgi::{Request, RequestResult, Requests};

use serde::Serialize;

type HttpResponse = http::Response<Option<String>>;

#[async_trait]
trait RequestHandler: Send + Sync {
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
    http_headers: BTreeMap<String, String>,
    other_params: BTreeMap<String, String>,
}

struct DebugHandler {}

#[async_trait]
impl RequestHandler for DebugHandler {
    async fn handle(&self, request: Arc<Request<OwnedWriteHalf>>) -> HttpResponse {
        let mut debug_response = DebugResponse::default();

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

struct Router {
    routes: Vec<Route>,
}

impl Router {
    fn new() -> Self {
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

/// Encodes the HTTP status code and the response string and sends it back to the webserver.
async fn send_response(
    request: Arc<Request<OwnedWriteHalf>>,
    response: HttpResponse,
) -> Result<RequestResult, tokio_fastcgi::Error> {
    debug!("send_response response = {:?}", response);

    let mut response_string = String::new();

    write!(
        response_string,
        "Status: {} {}\n",
        response.status().as_u16(),
        response.status().canonical_reason().unwrap_or("UNKNOWN")
    )
    .unwrap();

    for (key, value) in response.headers() {
        write!(
            response_string,
            "{}: {}\n",
            key.as_str(),
            value.to_str().unwrap_or("UNKNOWN")
        )
        .unwrap();
    }

    response_string.push('\n');

    if let Some(data) = response.body() {
        response_string.push_str(data);
    }

    request
        .get_stdout()
        .write(response_string.as_bytes())
        .await?;

    Ok(RequestResult::Complete(0))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder().format_timestamp_nanos().init();

    let configuration = config::read_configuration("config.json").await?;

    let commands = configuration.commands();
    info!("commands.len() = {}", commands.len());

    // let addr = "127.0.0.1:8080";
    // let listener = TcpListener::bind(addr).await.unwrap();

    let path = configuration.server_info().socket_path();

    let remove_result = tokio::fs::remove_file(path).await;
    debug!("remove_result = {:?}", remove_result);

    let listener = UnixListener::bind(path).unwrap();

    info!("listening on {:?}", listener.local_addr().unwrap());

    let router = Arc::new(Router::new());

    loop {
        let connection = listener.accept().await;
        // Accept new connections
        match connection {
            Err(err) => {
                error!("Establishing connection failed: {}", err);
                break;
            }
            Ok((stream, address)) => {
                debug!("Connection from {:?}", address);

                let conn_router = Arc::clone(&router);

                // If the socket connection was established successfully spawn a new task to handle
                // the requests that the webserver will send us.
                tokio::spawn(async move {
                    // Create a new requests handler it will collect the requests from the server and
                    // supply a streaming interface.
                    let mut requests = Requests::from_split_socket(stream.into_split(), 10, 10);

                    // Loop over the requests via the next method and process them.
                    while let Ok(Some(request)) = requests.next().await {
                        let request_router = Arc::clone(&conn_router);

                        if let Err(err) = request
                            .process(|request| async move {
                                let response = request_router.handle(Arc::clone(&request)).await;

                                send_response(request, response).await.unwrap()
                            })
                            .await
                        {
                            // This is the error handler that is called if the process call returns an error.
                            warn!("Processing request failed: {}", err);
                        }
                    }
                });
            }
        }
    }

    Ok(())
}
