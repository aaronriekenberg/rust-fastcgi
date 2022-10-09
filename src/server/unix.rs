use async_trait::async_trait;

use std::sync::Arc;

use anyhow::Context;

use log::{debug, info};

use tokio::net::{
    unix::SocketAddr,
    {UnixListener, UnixStream},
};

use tokio_fastcgi::Requests;

use crate::{
    connection::{FastCGIConnectionID, FastCGIConnectionIDFactory},
    handlers::RequestHandler,
};

pub struct UnixServer {
    server_configuration: crate::config::ServerConfiguration,
    handlers: Arc<dyn RequestHandler>,
    connection_id_factory: FastCGIConnectionIDFactory,
}

impl UnixServer {
    pub fn new(
        server_configuration: &crate::config::ServerConfiguration,
        handlers: Arc<dyn RequestHandler>,
    ) -> Self {
        Self {
            server_configuration: server_configuration.clone(),
            handlers,
            connection_id_factory: FastCGIConnectionIDFactory::new(),
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

        let connection_id = self.connection_id_factory.new_connection_id();

        // If the socket connection was established successfully spawn a new task to handle
        // the requests that the webserver will send us.
        tokio::spawn(
            UnixServerConnectionProcessor::new(
                stream,
                connection_id,
                Arc::clone(&self.handlers),
                *self.server_configuration.max_concurrent_connections(),
                *self.server_configuration.max_requests_per_connection(),
            )
            .run(),
        );
    }
}

#[async_trait]
impl super::SocketServer for UnixServer {
    async fn run(&self) -> anyhow::Result<()> {
        let listener = self
            .create_listener()
            .await
            .context("UnixServer::create_listener error")?;

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

struct UnixServerConnectionProcessor {
    connection_id: FastCGIConnectionID,
    stream: UnixStream,
    handlers: Arc<dyn RequestHandler>,
    max_concurrent_connections: u8,
    max_requests_per_connection: u8,
}

impl UnixServerConnectionProcessor {
    fn new(
        stream: UnixStream,
        connection_id: FastCGIConnectionID,
        handlers: Arc<dyn RequestHandler>,
        max_concurrent_connections: u8,
        max_requests_per_connection: u8,
    ) -> Self {
        Self {
            stream,
            connection_id,
            handlers,
            max_concurrent_connections,
            max_requests_per_connection,
        }
    }

    async fn run(self) {
        // Create a new requests handler it will collect the requests from the server and
        // supply a streaming interface.
        let mut requests = Requests::from_split_socket(
            self.stream.into_split(),
            self.max_concurrent_connections,
            self.max_requests_per_connection,
        );

        // Loop over the requests via the next method and process them.
        while let Ok(Some(request)) = requests.next().await {
            tokio::spawn(
                super::ServerRequestProcessor::new(
                    self.connection_id,
                    request,
                    Arc::clone(&self.handlers),
                )
                .run(),
            );
        }
    }
}
