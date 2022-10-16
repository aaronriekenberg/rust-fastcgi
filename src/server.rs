mod processor;
mod tcp;
mod unix;

use async_trait::async_trait;

use std::sync::Arc;

use crate::{
    config::ServerType,
    handlers::RequestHandler,
    server::{processor::ConnectionProcessor, tcp::TcpServer, unix::UnixServer},
};

#[async_trait]
trait SocketServer {
    async fn run(&self) -> anyhow::Result<()>;
}

pub struct Server {
    socket_server: Box<dyn SocketServer>,
}

impl Server {
    pub fn new(
        handlers: Arc<dyn RequestHandler>,
        server_configuration: &crate::config::ServerConfiguration,
    ) -> Self {
        let connection_processor = ConnectionProcessor::new(
            handlers,
            server_configuration.fastcgi_connection_configuration(),
        );

        Self {
            socket_server: match server_configuration.server_type() {
                ServerType::TCP => {
                    Box::new(TcpServer::new(server_configuration, connection_processor))
                }
                ServerType::UNIX => {
                    Box::new(UnixServer::new(server_configuration, connection_processor))
                }
            },
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        self.socket_server.run().await
    }
}
