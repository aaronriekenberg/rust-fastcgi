use std::sync::Arc;

use log::warn;

use tokio_fastcgi::{Request, Requests};

use crate::utils::{GenericAsyncReader, GenericAsyncWriter};

use crate::{
    connection::FastCGIConnectionID, handlers::RequestHandler, request::FastCGIRequest,
    response::responder::Responder,
};

pub struct ConnectionProcessor {
    handlers: Box<dyn RequestHandler>,
    fastcgi_connection_configuration: crate::config::FastCGIConnectionConfiguration,
}

impl ConnectionProcessor {
    pub fn new(
        handlers: Box<dyn RequestHandler>,
        fastcgi_connection_configuration: &crate::config::FastCGIConnectionConfiguration,
    ) -> Arc<Self> {
        Arc::new(Self {
            handlers,
            fastcgi_connection_configuration: fastcgi_connection_configuration.clone(),
        })
    }

    async fn process_one_request(
        self: Arc<Self>,
        connection_id: FastCGIConnectionID,
        request: Request<impl GenericAsyncWriter>,
    ) {
        if let Err(err) = request
            .process(|request| async move {
                let fastcgi_request = FastCGIRequest::new(connection_id, request.as_ref());

                let http_response = self.handlers.handle(fastcgi_request).await;

                Responder::new(request, http_response).respond().await
            })
            .await
        {
            // This is the error handler that is called if the process call returns an error.
            warn!("request.process failed: err = {}", err,);
        }
    }

    pub fn handle_connection<R, W>(
        self: Arc<Self>,
        connection_id: FastCGIConnectionID,
        split_socket: (R, W),
    ) where
        R: GenericAsyncReader + Send + Sync + 'static,
        W: GenericAsyncWriter + Send + Sync + 'static,
    {
        // If the socket connection was established successfully spawn a new task to handle
        // the requests that the webserver will send us.
        tokio::spawn(async move {
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
                tokio::spawn(Arc::clone(&self).process_one_request(connection_id, request));
            }
        });
    }
}
