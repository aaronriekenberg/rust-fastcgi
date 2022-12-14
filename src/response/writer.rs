use std::{fmt::Write, sync::Arc};

use log::{debug, warn};

use tokio_fastcgi::{Request, RequestResult};

use crate::{response::HttpResponse, utils::GenericAsyncWriter};

#[derive(thiserror::Error, Debug)]
enum SendResponseError {
    #[error("build header string error: {0}")]
    BuildHeaderStringError(#[from] std::fmt::Error),

    #[error("tokio_fastcgi write error: {0}")]
    TokioFastCGIWriteError(#[from] tokio_fastcgi::Error),
}

pub struct ResponseWriter<W: GenericAsyncWriter> {
    request: Arc<Request<W>>,
    response: HttpResponse,
}

impl<W: GenericAsyncWriter> ResponseWriter<W> {
    pub fn new(request: Arc<Request<W>>, response: HttpResponse) -> Self {
        Self { request, response }
    }

    fn build_header_string(&self) -> Result<String, std::fmt::Error> {
        let mut header_string = String::new();

        write!(
            header_string,
            "Status: {} {}\n",
            self.response.status().as_u16(),
            self.response
                .status()
                .canonical_reason()
                .unwrap_or("[Unknown]")
        )?;

        for (key, value) in self.response.headers() {
            write!(
                header_string,
                "{}: {}\n",
                key.as_str(),
                value.to_str().unwrap_or("[Unknown]")
            )?;
        }

        header_string.push('\n');

        Ok(header_string)
    }

    async fn send_response(self) -> Result<(), SendResponseError> {
        let header_string = self.build_header_string()?;

        let mut stdout = self.request.get_stdout();

        stdout.write(&header_string.into_bytes()).await?;

        if let Some(http_response_body) = self.response.into_body() {
            stdout.write(http_response_body.as_bytes()).await?;
        }

        Ok(())
    }

    pub async fn respond(self) -> RequestResult {
        debug!("respond response = {:?}", self.response);

        match self.send_response().await {
            Ok(_) => RequestResult::Complete(0),
            Err(err) => {
                warn!("send response failed: {}", err);
                RequestResult::Complete(1)
            }
        }
    }
}
