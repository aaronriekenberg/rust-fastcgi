use std::{fmt::Write, sync::Arc};

use log::{debug, warn};

use tokio::io::AsyncWrite;

use tokio_fastcgi::{Request, RequestResult};

pub type HttpResponse = http::Response<Option<String>>;

#[derive(thiserror::Error, Debug)]
enum SendResponseError {
    #[error("build header string error: {0}")]
    BuildHeaderStringError(#[from] std::fmt::Error),

    #[error("tokio_fastcgi write error: {0}")]
    TokioFastCGIWriteError(#[from] tokio_fastcgi::Error),
}

pub struct Responder<W>
where
    W: AsyncWrite + Unpin,
{
    request: Arc<Request<W>>,
    response: HttpResponse,
}

impl<W> Responder<W>
where
    W: AsyncWrite + Unpin,
{
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

    async fn internal_send_response(self) -> Result<(), SendResponseError> {
        let mut stdout = self.request.get_stdout();

        let header_string = self.build_header_string()?;

        stdout.write(&header_string.into_bytes()).await?;

        if let Some(body_string) = self.response.into_body() {
            stdout.write(&body_string.into_bytes()).await?;
        }

        Ok(())
    }

    pub async fn respond(self) -> RequestResult {
        debug!("respond response = {:?}", self.response);

        match self.internal_send_response().await {
            Ok(_) => RequestResult::Complete(0),
            Err(err) => {
                warn!("send response failed: {}", err);
                RequestResult::Complete(1)
            }
        }
    }
}
