use async_trait::async_trait;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;

use log::{debug, info};

use tokio::net::{TcpListener, TcpStream};

use tokio_fastcgi::Requests;

use crate::{
    connection::{FastCGIConnectionID, FastCGIConnectionIDFactory},
    handlers::RequestHandler,
};

pub struct TcpServer {
    server_configuration: crate::config::ServerConfiguration,
    handlers: Arc<dyn RequestHandler>,
    connection_id_factory: FastCGIConnectionIDFactory,
}

impl TcpServer {
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

    async fn create_listener(&self) -> anyhow::Result<TcpListener> {
        let bind_address = self.server_configuration.bind_address();

        let listener = TcpListener::bind(bind_address)
            .await
            .with_context(|| format!("TcpListener::bind error bind_address '{}'", bind_address))?;

        let local_addr = listener.local_addr().context("local_addr error")?;

        info!("TcpServer listening on {:?}", local_addr);

        Ok(listener)
    }

    fn handle_connection(&self, stream: TcpStream, address: SocketAddr) {
        debug!("connection from {:?}", address);

        let connection_id = self.connection_id_factory.new_connection_id();

        // If the socket connection was established successfully spawn a new task to handle
        // the requests that the webserver will send us.
        tokio::spawn(
            TcpServerConnectionProcessor::new(
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
impl super::SocketServer for TcpServer {
    async fn run(&self) -> anyhow::Result<()> {
        let listener = self
            .create_listener()
            .await
            .context("TcpServer::create_listener error")?;

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

struct TcpServerConnectionProcessor {
    connection_id: FastCGIConnectionID,
    stream: TcpStream,
    handlers: Arc<dyn RequestHandler>,
    max_concurrent_connections: u8,
    max_requests_per_connection: u8,
}

impl TcpServerConnectionProcessor {
    fn new(
        stream: TcpStream,
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
