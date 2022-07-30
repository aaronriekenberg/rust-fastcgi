use std::{collections::HashMap, sync::Arc};

use tokio::net::{unix::OwnedWriteHalf, UnixListener};
use tokio_fastcgi::{Request, RequestResult, Requests};

/// Define some response codes to use.
#[derive(Debug)]
struct HttpResponse {
    code: u16,
    message: &'static str,
    data: Option<String>,
    headers: HashMap<String, String>,
}

impl HttpResponse {
    fn ok(data: Option<String>, headers: HashMap<String, String>) -> Self {
        Self {
            code: 200,
            message: "Ok",
            data,
            headers,
        }
    }

    fn error(code: u16, message: &'static str) -> Self {
        Self {
            code,
            message,
            data: None,
            headers: HashMap::new(),
        }
    }

    fn e400() -> Self {
        Self::error(400, "Bad Request")
    }

    fn e404() -> Self {
        Self::error(404, "Not Found")
    }

    fn e405() -> Self {
        Self::error(405, "Method Not Allowed")
    }

    fn e500() -> Self {
        Self::error(500, "Internal Server Error")
    }
}

/// Encodes the HTTP status code and the response string and sends it back to the webserver.
async fn send_response(
    request: Arc<Request<OwnedWriteHalf>>,
    response: HttpResponse,
) -> Result<RequestResult, tokio_fastcgi::Error> {
    println!("send_response response = {:?}", response);

    let mut stdout = request.get_stdout();

    stdout
        .write(format!("Status: {} {}\n", response.code, response.message).as_bytes())
        .await?;

    if response.headers.len() > 0 {
        for (key, value) in &response.headers {
            stdout
                .write(format!("{}: {}\n", key, value).as_bytes())
                .await?;
        }
    }

    stdout.write("\n".as_bytes()).await?;

    if let Some(data) = response.data {
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
                let data = Some("hello world!".to_string());

                let mut headers: HashMap<String, String> = HashMap::new();
                headers.insert("Content-Type".to_string(), "text/things".to_string());

                send_response(request, HttpResponse::ok(data, headers)).await
            }

            // Verything else will return HTTP 404 (Not Found)
            _ => send_response(request, HttpResponse::e404()).await,
        }
    } else {
        send_response(request, HttpResponse::e400()).await
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
