use std::{fmt::Write, sync::Arc};

use log::{debug, warn};

use tokio_fastcgi::{Request, RequestResult};

use crate::utils::GenericAsyncWriter;

#[derive(Debug)]
pub enum HttpResponseBody {
    ArcString(Arc<String>),

    String(String),
}

impl From<Arc<String>> for HttpResponseBody {
    fn from(s: Arc<String>) -> HttpResponseBody {
        HttpResponseBody::ArcString(s)
    }
}

impl From<String> for HttpResponseBody {
    fn from(s: String) -> HttpResponseBody {
        HttpResponseBody::String(s)
    }
}

pub type HttpResponse = http::Response<Option<HttpResponseBody>>;

#[derive(thiserror::Error, Debug)]
enum SendResponseError {
    #[error("build header string error: {0}")]
    BuildHeaderStringError(#[from] std::fmt::Error),

    #[error("tokio_fastcgi write error: {0}")]
    TokioFastCGIWriteError(#[from] tokio_fastcgi::Error),
}

pub struct Responder<W: GenericAsyncWriter> {
    request: Arc<Request<W>>,
    response: HttpResponse,
}

impl<W: GenericAsyncWriter> Responder<W> {
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

        let mut write_buffer = header_string.into_bytes();

        if let Some(http_response_body) = self.response.into_body() {
            match http_response_body {
                HttpResponseBody::ArcString(s) => write_buffer.extend_from_slice(s.as_bytes()),
                HttpResponseBody::String(s) => write_buffer.append(&mut s.into_bytes()),
            }
        }

        self.request.get_stdout().write(&write_buffer).await?;

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
