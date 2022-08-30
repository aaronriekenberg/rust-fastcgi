use std::{error::Error, fmt::Write, sync::Arc};

use log::{debug, warn};

use tokio::io::AsyncWrite;

use tokio_fastcgi::{OutStream, Request, RequestResult};

pub type HttpResponse = http::Response<Option<String>>;

fn build_header_string(response: &HttpResponse) -> Result<String, Box<dyn Error>> {
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

async fn write_to_stdout<W: AsyncWrite + Unpin>(
    stdout: &mut OutStream<W>,
    data: String,
) -> Result<(), Box<dyn Error>> {
    stdout.write(data.as_bytes()).await?;

    Ok(())
}

async fn internal_send_response<W: AsyncWrite + Unpin>(
    request: Arc<Request<W>>,
    response: HttpResponse,
) -> Result<(), Box<dyn Error>> {
    debug!("send_response response = {:?}", response);

    let mut stdout = request.get_stdout();

    let header_string = build_header_string(&response)?;

    write_to_stdout(&mut stdout, header_string).await?;

    if let Some(body_string) = response.into_body() {
        write_to_stdout(&mut stdout, body_string).await?;
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
