mod tcp;
mod unix;

use async_trait::async_trait;

use std::sync::Arc;

use log::warn;

use tokio::io::AsyncWrite;

use crate::{
    connection::FastCGIConnectionID, handlers::RequestHandler, request::FastCGIRequest,
    response::Responder,
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

struct ServerRequestProcessor<W>
where
    W: AsyncWrite + Unpin,
{
    connection_id: FastCGIConnectionID,
    request: tokio_fastcgi::Request<W>,
    handlers: Arc<dyn RequestHandler>,
}

impl<W> ServerRequestProcessor<W>
where
    W: AsyncWrite + Unpin,
{
    fn new(
        connection_id: FastCGIConnectionID,
        request: tokio_fastcgi::Request<W>,
        handlers: Arc<dyn RequestHandler>,
    ) -> Self {
        Self {
            connection_id,
            request,
            handlers,
        }
    }

    async fn run(self) {
        if let Err(err) = self
            .request
            .process(|request| async move {
                let fastcgi_request = FastCGIRequest::new(self.connection_id, request.as_ref());

                let http_response = self.handlers.handle(fastcgi_request).await;

                Responder::new(request, http_response).respond().await
            })
            .await
        {
            // This is the error handler that is called if the process call returns an error.
            warn!("request.process failed: err = {}", err,);
        }
    }
}
