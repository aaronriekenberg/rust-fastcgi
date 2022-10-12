mod tcp;
mod unix;

use async_trait::async_trait;

use std::sync::Arc;

use log::warn;

use tokio::io::{AsyncRead, AsyncWrite};

use tokio_fastcgi::{Request, Requests};

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

struct ServerConnectionProcessor {
    connection_id: FastCGIConnectionID,
    handlers: Arc<dyn RequestHandler>,
    fastcgi_connection_configuration: crate::config::FastCGIConnectionConfiguration,
}

impl ServerConnectionProcessor {
    fn new(
        connection_id: FastCGIConnectionID,
        handlers: Arc<dyn RequestHandler>,
        fastcgi_connection_configuration: &crate::config::FastCGIConnectionConfiguration,
    ) -> Arc<Self> {
        Arc::new(Self {
            connection_id,
            handlers,
            fastcgi_connection_configuration: fastcgi_connection_configuration.clone(),
        })
    }

    async fn process_one_request<W>(self: Arc<Self>, request: Request<W>)
    where
        W: AsyncWrite + Unpin,
    {
        if let Err(err) = request
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

    async fn run<R, W>(self: Arc<Self>, split_socket: (R, W))
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        // Create a new requests handler it will collect the requests from the server and
        // supply a streaming interface.
        let mut requests = Requests::from_split_socket(
            split_socket,
            *self
                .fastcgi_connection_configuration
                .max_concurrent_connections(),
            *self
                .fastcgi_connection_configuration
                .max_requests_per_connection(),
        );

        // Loop over the requests via the next method and process them.
        // Spawn a new task to process each request.
        while let Ok(Some(request)) = requests.next().await {
            let self_clone = Arc::clone(&self);
            tokio::spawn(self_clone.process_one_request(request));
        }
    }
}
