mod processor;
mod tcp;
mod unix;

use async_trait::async_trait;

use std::sync::Arc;

use crate::{config::ServerType, handlers::RequestHandler};

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
        Self {
            socket_server: match server_configuration.server_type() {
                ServerType::TCP => Box::new(tcp::TcpServer::new(&server_configuration, handlers)),
                ServerType::UNIX => {
                    Box::new(unix::UnixServer::new(&server_configuration, handlers))
                }
            },
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        self.socket_server.run().await
    }
}
