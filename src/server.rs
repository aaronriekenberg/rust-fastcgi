use std::{error::Error, sync::Arc};

use log::{debug, error, info, warn};

use tokio::net::{
    unix::SocketAddr,
    {UnixListener, UnixStream},
};

use tokio_fastcgi::Requests;

use crate::{request::FastCGIRequest, response::send_response};

pub struct Server {
    server_configuration: crate::config::ServerConfiguration,
    handlers: Arc<dyn crate::handlers::RequestHandler>,
}

impl Server {
    pub fn new(
        handlers: Arc<dyn crate::handlers::RequestHandler>,
        server_configuration: &crate::config::ServerConfiguration,
    ) -> Self {
        Self {
            server_configuration: server_configuration.clone(),
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

    fn handle_connection(&self, stream: UnixStream, address: SocketAddr) {
        debug!("Connection from {:?}", address);

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

                if let Err(err) = request
                    .process(|request| async move {
                        let fastcgi_request = FastCGIRequest::from(request.as_ref());

                        let response = request_handlers.handle(fastcgi_request).await;

                        send_response(request, response).await
                    })
                    .await
                {
                    // This is the error handler that is called if the process call returns an error.
                    warn!("Processing request failed: err = {}", err,);
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
