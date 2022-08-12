use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;

use crate::handlers::RequestHandler;

use log::{debug, error, info, warn};

use tokio::net::{unix::OwnedWriteHalf, UnixListener};

use tokio_fastcgi::{Request, RequestResult, Requests};

/// Encodes the HTTP status code and the response string and sends it back to the webserver.
async fn send_response(
    request: Arc<Request<OwnedWriteHalf>>,
    response: crate::handlers::HttpResponse,
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

fn request_to_fastcgi_request(
    request: Arc<Request<OwnedWriteHalf>>,
) -> crate::handlers::FastCGIRequest {
    let role = match request.role {
        tokio_fastcgi::Role::Authorizer => "Authorizer",
        tokio_fastcgi::Role::Filter => "Filter",
        tokio_fastcgi::Role::Responder => "Responder",
    };

    let params: HashMap<String, String> = match request.str_params_iter() {
        Some(iter) => iter
            .map(|v| (v.0.to_string(), v.1.unwrap_or("[Invalid UTF8]").to_string()))
            .collect(),
        None => HashMap::new(),
    };

    crate::handlers::FastCGIRequest::new(role, request.get_request_id(), params)
}

pub async fn run_server(
    configuration: crate::config::Configuration,
) -> Result<(), Box<dyn std::error::Error>> {
    let commands = configuration.commands();
    info!("commands.len() = {}", commands.len());

    // let addr = "127.0.0.1:8080";
    // let listener = TcpListener::bind(addr).await.unwrap();

    let path = configuration.server_info().socket_path();

    let remove_result = tokio::fs::remove_file(path).await;
    debug!("remove_result = {:?}", remove_result);

    let listener = UnixListener::bind(path)?;

    info!("listening on {:?}", listener.local_addr()?);

    let router = Arc::new(crate::handlers::Router::new());

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
                                let fastcgi_request =
                                    request_to_fastcgi_request(Arc::clone(&request));

                                let response = request_router.handle(fastcgi_request).await;

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