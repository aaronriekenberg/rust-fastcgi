use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use anyhow::Context;

use log::{debug, info, warn};

use tokio::net::{
    unix::SocketAddr,
    {UnixListener, UnixStream},
};

use tokio_fastcgi::Requests;

use crate::{
    handlers::RequestHandler,
    request::{FastCGIRequest, FastCGIRequestID},
    response::send_response,
};

pub struct Server {
    server_configuration: crate::config::ServerConfiguration,
    handlers: Arc<dyn RequestHandler>,
    next_connection_id: AtomicU64,
}

impl Server {
    pub fn new(
        handlers: Arc<dyn RequestHandler>,
        server_configuration: &crate::config::ServerConfiguration,
    ) -> Self {
        Self {
            server_configuration: server_configuration.clone(),
            handlers,
            next_connection_id: AtomicU64::new(1),
        }
    }

    async fn create_listener(&self) -> anyhow::Result<UnixListener> {
        let path = self.server_configuration.socket_path();

        // do not fail on remove error, the path may not exist.
        let remove_result = tokio::fs::remove_file(path).await;
        debug!("remove_result = {:?}", remove_result);

        let listener = UnixListener::bind(path)
            .with_context(|| format!("UnixListener::bind error path '{}'", path))?;

        let local_addr = listener.local_addr().context("local_addr error")?;

        info!("listening on {:?}", local_addr);

        Ok(listener)
    }

    fn handle_connection(&self, stream: UnixStream, address: SocketAddr) {
        debug!("connection from {:?}", address);

        let conn_handlers = Arc::clone(&self.handlers);

        let connection_id = self.next_connection_id.fetch_add(1, Ordering::Relaxed);

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

                if let Err(err) = request
                    .process(|request| async move {
                        let request_id =
                            FastCGIRequestID::new(connection_id, request.get_request_id());

                        let fastcgi_request = FastCGIRequest::from((request_id, request.as_ref()));

                        let response = request_handlers.handle(fastcgi_request).await;

                        send_response(request, response).await
                    })
                    .await
                {
                    // This is the error handler that is called if the process call returns an error.
                    warn!("request.process failed: err = {}", err,);
                }
            }
        });
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let listener = self
            .create_listener()
            .await
            .context("Server::create_listener error")?;

        loop {
            let connection = listener.accept().await;
            // Accept new connections
            match connection {
                Err(err) => {
                    anyhow::bail!("establishing connection failed err: {}", err);
                }
                Ok((stream, address)) => {
                    self.handle_connection(stream, address);
                }
            }
        }
    }
}
