use std::collections::HashMap;
use std::error::Error;
use std::fmt::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use log::{debug, error, info, warn};

use tokio::net::unix::SocketAddr;
use tokio::net::UnixStream;
use tokio::{io::AsyncWrite, net::UnixListener};

use tokio_fastcgi::{Request, RequestResult, Requests};

/// Encodes the HTTP status code and the response string and sends it back to the webserver.
async fn send_response<W: AsyncWrite + Unpin>(
    request: Arc<Request<W>>,
    response: crate::handlers::HttpResponse,
) -> RequestResult {
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

    match request.get_stdout().write(response_string.as_bytes()).await {
        Ok(size) => {
            debug!("request.get_stdout().write() success bytes = {}", size);
            RequestResult::Complete(0)
        }
        Err(e) => {
            warn!("request.get_stdout().write() error e = {}", e);
            RequestResult::Complete(1)
        }
    }
}

fn request_to_fastcgi_request<W: AsyncWrite + Unpin>(
    connection_id: u64,
    request: &Request<W>,
) -> Arc<crate::handlers::FastCGIRequest> {
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

    Arc::new(crate::handlers::FastCGIRequest::new(
        role,
        connection_id,
        request.get_request_id(),
        params,
    ))
}

pub struct Server {
    connection_counter: AtomicU64,
    server_configuration: crate::config::ServerConfiguration,
    handlers: Arc<dyn crate::handlers::RequestHandler>,
}

impl Server {
    pub fn new(configuration: crate::config::Configuration) -> Self {
        let handlers = crate::handlers::create_handlers(&configuration);

        Self {
            connection_counter: AtomicU64::new(0),
            server_configuration: configuration.server_configuration().clone(),
            handlers,
        }
    }

    async fn create_listener(&self) -> Result<UnixListener, Box<dyn Error>> {
        let path = self.server_configuration.socket_path();

        // do not fail on remove error, the path may not exist.
        let remove_result = tokio::fs::remove_file(path).await;
        debug!("remove_result = {:?}", remove_result);

        let listener = UnixListener::bind(path)?;

        info!("listening on {:?}", listener.local_addr()?);

        Ok(listener)
    }

    fn next_connection_id(&self) -> u64 {
        self.connection_counter.fetch_add(1, Ordering::Relaxed)
    }

    fn handle_connection(&self, stream: UnixStream, address: SocketAddr) {
        let connection_id = self.next_connection_id();

        debug!(
            "Connection from {:?} connection_id = {}",
            address, connection_id
        );

        let conn_handlers = Arc::clone(&self.handlers);

        let max_concurrent_connections = *self.server_configuration.max_concurrent_connections();
        let max_requests_per_connection = *self.server_configuration.max_requests_per_connection();

        // If the socket connection was established successfully spawn a new task to handle
        // the requests that the webserver will send us.
        tokio::spawn(async move {
            // Create a new requests handler it will collect the requests from the server and
            // supply a streaming interface.
            let mut requests = Requests::from_split_socket(
                stream.into_split(),
                max_concurrent_connections,
                max_requests_per_connection,
            );

            // Loop over the requests via the next method and process them.
            while let Ok(Some(request)) = requests.next().await {
                let request_handlers = Arc::clone(&conn_handlers);

                let fastcgi_request = request_to_fastcgi_request(connection_id, &request);

                let log_err_request = Arc::clone(&fastcgi_request);

                if let Err(err) = request
                    .process(|request| async move {
                        let response = request_handlers.handle(fastcgi_request).await;

                        send_response(request, response).await
                    })
                    .await
                {
                    // This is the error handler that is called if the process call returns an error.
                    warn!(
                        "Processing request failed: request = {:?} err= {}",
                        log_err_request, err,
                    );
                }
            }
        });
    }

    pub async fn run(self) -> Result<(), Box<dyn Error>> {
        let listener = self.create_listener().await?;

        loop {
            let connection = listener.accept().await;
            // Accept new connections
            match connection {
                Err(err) => {
                    error!("Establishing connection failed: {}", err);
                    break;
                }
                Ok((stream, address)) => {
                    self.handle_connection(stream, address);
                }
            }
        }

        Ok(())
    }
}
