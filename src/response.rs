use std::{fmt::Write, sync::Arc};

use log::{debug, warn};

use tokio::io::AsyncWrite;

use tokio_fastcgi::{Request, RequestResult};

pub type HttpResponse = http::Response<Option<String>>;

fn build_header_string(response: &HttpResponse) -> Result<String, std::fmt::Error> {
    let mut header_string = String::new();

    write!(
        header_string,
        "Status: {} {}\n",
        response.status().as_u16(),
        response.status().canonical_reason().unwrap_or("[Unknown]")
    )?;

    for (key, value) in response.headers() {
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

#[derive(thiserror::Error, Debug)]
enum SendResponseError {
    #[error("build header string error: {0}")]
    BuildHeaderStringError(#[from] std::fmt::Error),

    #[error("tokio_fastcgi write error: {0}")]
    TokioFastCGIWriteError(#[from] tokio_fastcgi::Error),
}

async fn internal_send_response<W: AsyncWrite + Unpin>(
    request: Arc<Request<W>>,
    response: HttpResponse,
) -> Result<(), SendResponseError> {
    let mut stdout = request.get_stdout();

    let header_string = build_header_string(&response)?;

    stdout.write(&header_string.into_bytes()).await?;

    if let Some(body_string) = response.into_body() {
        stdout.write(&body_string.into_bytes()).await?;
    }

    Ok(())
}

// Encodes the HTTP status code and the response string and sends it back to the webserver.
pub async fn send_response<W: AsyncWrite + Unpin>(
    request: Arc<Request<W>>,
    response: HttpResponse,
) -> RequestResult {
    debug!("send_response response = {:?}", response);

    match internal_send_response(request, response).await {
        Ok(_) => RequestResult::Complete(0),
        Err(err) => {
            warn!("Send response failed: {}", err);
            RequestResult::Complete(1)
        }
    }
}
