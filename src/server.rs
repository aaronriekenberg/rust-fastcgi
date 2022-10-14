mod processor;
mod tcp;
mod unix;

use async_trait::async_trait;

use std::sync::Arc;

use crate::handlers::RequestHandler;

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
        let socket_server: Box<dyn SocketServer> = match server_configuration.server_type() {
            crate::config::ServerType::TCP => {
                Box::new(tcp::TcpServer::new(&server_configuration, handlers))
            }
            crate::config::ServerType::UNIX => {
                Box::new(unix::UnixServer::new(&server_configuration, handlers))
            }
        };

        Self { socket_server }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        self.socket_server.run().await
    }
}
