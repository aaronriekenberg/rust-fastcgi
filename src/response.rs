use std::error::Error;
use std::fmt::Write;
use std::sync::Arc;

use log::{debug, warn};

use tokio::io::AsyncWrite;

use tokio_fastcgi::{Request, RequestResult};

pub type HttpResponse = http::Response<Option<String>>;

async fn internal_send_response<W: AsyncWrite + Unpin>(
    request: Arc<Request<W>>,
    response: HttpResponse,
) -> Result<(), Box<dyn Error>> {
    debug!("send_response response = {:?}", response);

    let mut stdout = request.get_stdout();

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

    stdout.write(header_string.as_bytes()).await?;

    drop(header_string);

    if let Some(body_string) = response.body() {
        stdout.write(body_string.as_bytes()).await?;
    }

    Ok(())
}

// Encodes the HTTP status code and the response string and sends it back to the webserver.
pub async fn send_response<W: AsyncWrite + Unpin>(
    request: Arc<Request<W>>,
    response: HttpResponse,
) -> RequestResult {
    match internal_send_response(request, response).await {
        Ok(_) => RequestResult::Complete(0),
        Err(err) => {
            warn!("Send response failed: {}", err);
            RequestResult::Complete(1)
        }
    }
}
