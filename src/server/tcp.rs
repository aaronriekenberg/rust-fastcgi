use async_trait::async_trait;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;

use log::{debug, info};

use tokio::net::{TcpListener, TcpStream};

use crate::{
    connection::FastCGIConnectionIDFactory, handlers::RequestHandler,
    server::processor::ConnectionProcessor,
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
        let connection_id = self.connection_id_factory.new_connection_id();

        debug!("connection_id {:?} from {:?}", connection_id, address);

        ConnectionProcessor::new(
            connection_id,
            Arc::clone(&self.handlers),
            self.server_configuration.fastcgi_connection_configuration(),
        )
        .start(stream.into_split());
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
