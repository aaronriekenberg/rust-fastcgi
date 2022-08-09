use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::Arc;

use log::{debug, error, info, warn};

use tokio::net::{unix::OwnedWriteHalf, UnixListener};
use tokio_fastcgi::{Request, RequestResult, Requests};

use http::{Response, StatusCode};

use serde::Serialize;

/// Encodes the HTTP status code and the response string and sends it back to the webserver.
async fn send_response(
    request: Arc<Request<OwnedWriteHalf>>,
    response: http::Response<Option<String>>,
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

#[derive(Debug, Default, Serialize)]
struct DebugResponse {
    http_headers: BTreeMap<String, String>,
    other_params: BTreeMap<String, String>,
}

async fn process_debug_request(
    request: Arc<Request<OwnedWriteHalf>>,
) -> Result<RequestResult, tokio_fastcgi::Error> {
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

    let json_result = serde_json::to_string(&debug_response);

    match json_result {
        Err(e) => {
            warn!("json serialization error {}", e);

            send_response(
                request,
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(None)
                    .unwrap(),
            )
            .await
        }
        Ok(json_string) => {
            send_response(
                request,
                Response::builder()
                    .status(StatusCode::OK)
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .body(Some(json_string))
                    .unwrap(),
            )
            .await
        }
    }
}

async fn process_request(
    request: Arc<Request<OwnedWriteHalf>>,
) -> Result<RequestResult, tokio_fastcgi::Error> {
    // Check that a `request_uri` parameter was passed by the webserver. If this is not the case,
    // fail with a HTTP 400 (Bad Request) error code.

    if let Some(request_uri) = request.get_str_param("request_uri").map(String::from) {
        // Split the request URI into the different path componets.
        // The following match is used to extract and verify the path compontens.

        if request_uri.starts_with("/cgi-bin/debug") {
            process_debug_request(request).await
        } else {
            send_response(
                request,
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(None)
                    .unwrap(),
            )
            .await
        }
    } else {
        send_response(
            request,
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(None)
                .unwrap(),
        )
        .await
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    // let addr = "127.0.0.1:8080";
    // let listener = TcpListener::bind(addr).await.unwrap();

    let path = "./socket";

    let remove_result = tokio::fs::remove_file(path).await;
    debug!("remove_result = {:?}", remove_result);

    let listener = UnixListener::bind(path).unwrap();

    info!("listening on {:?}", listener.local_addr().unwrap());

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

                // If the socket connection was established successfully spawn a new task to handle
                // the requests that the webserver will send us.
                tokio::spawn(async move {
                    // Create a new requests handler it will collect the requests from the server and
                    // supply a streaming interface.
                    let mut requests = Requests::from_split_socket(stream.into_split(), 10, 10);

                    // Loop over the requests via the next method and process them.
                    while let Ok(Some(request)) = requests.next().await {
                        if let Err(err) = request
                            .process(
                                |request| async move { process_request(request).await.unwrap() },
                            )
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
}
