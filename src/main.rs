use std::sync::Arc;

use tokio::net::{unix::OwnedWriteHalf, UnixListener};
use tokio_fastcgi::{Request, RequestResult, Requests};

use http::{Response, StatusCode};

/// Encodes the HTTP status code and the response string and sends it back to the webserver.
async fn send_response(
    request: Arc<Request<OwnedWriteHalf>>,
    response: http::Response<Option<String>>,
) -> Result<RequestResult, tokio_fastcgi::Error> {
    println!("send_response response = {:?}", response);

    let mut stdout = request.get_stdout();

    stdout
        .write(
            format!(
                "Status: {} {}\n",
                response.status().as_u16(),
                response.status().canonical_reason().unwrap()
            )
            .as_bytes(),
        )
        .await?;

    if response.headers().len() > 0 {
        for (key, value) in response.headers() {
            stdout
                .write(format!("{}: {}\n", key.as_str(), value.to_str().unwrap()).as_bytes())
                .await?;
        }
    }

    stdout.write("\n".as_bytes()).await?;

    if let Some(data) = response.body() {
        stdout.write(data.as_bytes()).await?;
    }

    Ok(RequestResult::Complete(0))
}

async fn process_request(
    request: Arc<Request<OwnedWriteHalf>>,
) -> Result<RequestResult, tokio_fastcgi::Error> {
    // Check that a `request_uri` parameter was passed by the webserver. If this is not the case,
    // fail with a HTTP 400 (Bad Request) error code.

    println!("request = {:?}", request);
    if let Some(request_uri) = request.get_str_param("request_uri").map(String::from) {
        // Split the request URI into the different path componets.
        // The following match is used to extract and verify the path compontens.

        println!("request_uri = '{}'", request_uri);
        let mut request_parts = request_uri.split_terminator('/').fuse();
        match (
            request_parts.next(),
            request_parts.next(),
            request_parts.next(),
            request_parts.next(),
            // request_parts.next(),
        ) {
            // Process /api/<endpoint>[/<selector>]
            // (Some(""), Some("api"), Some(endpoint), selector, None) => {
            //     // process_endpoint(store, request, endpoint, selector).await
            // }

            // Process /cgi-bin/test
            (Some(""), Some("cgi-bin"), Some("test"), None) => {
                let response = Response::builder()
                    .header(http::header::CONTENT_TYPE, "text/test")
                    .status(StatusCode::OK)
                    .body(Some("hello world!".to_string()))
                    .unwrap();

                send_response(request, response).await
            }

            // Everything else will return HTTP 404 (Not Found)
            _ => {
                send_response(
                    request,
                    Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(None)
                        .unwrap(),
                )
                .await
            }
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
    // let addr = "127.0.0.1:8080";
    // let listener = TcpListener::bind(addr).await.unwrap();

    let path = "/Users/aaron/rust-fastcgi/socket";

    let remove_result = tokio::fs::remove_file(path).await;
    println!("remove_result = {:?}", remove_result);

    let listener = UnixListener::bind(path).unwrap();

    println!("listening on {:?}", listener.local_addr().unwrap());

    loop {
        let connection = listener.accept().await;
        // Accept new connections
        match connection {
            Err(err) => {
                println!("Establishing connection failed: {}", err);
                break;
            }
            Ok((stream, address)) => {
                println!("Connection from {:?}", address);

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
                            println!("Processing request failed: {}", err);
                        }
                    }
                });
            }
        }
    }
}
