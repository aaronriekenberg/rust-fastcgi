mod processor;
mod tcp;
mod unix;

use async_trait::async_trait;

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
    pub fn new(handlers: Box<dyn RequestHandler>) -> Self {
        let connection_processor = ConnectionProcessor::new(handlers);

        let server_configuration = crate::config::get_configuration().server_configuration();

        Self {
            socket_server: match server_configuration.server_type() {
                ServerType::TCP => Box::new(TcpServer::new(connection_processor)),
                ServerType::UNIX => Box::new(UnixServer::new(connection_processor)),
            },
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        self.socket_server.run().await
    }
}
