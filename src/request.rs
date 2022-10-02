use getset::Getters;

use tokio::io::AsyncWrite;

#[derive(Debug, Getters)]
#[getset(get = "pub")]
pub struct RequestID {
    fastcgi_connection_id: u64,
    fastcgi_request_id: u16,
}

impl RequestID {
    pub fn new(fastcgi_connection_id: u64, fastcgi_request_id: u16) -> Self {
        Self {
            fastcgi_connection_id,
            fastcgi_request_id,
        }
    }
}

pub type ParamKeyValue<'a> = (&'a str, &'a str);

#[derive(Debug, Getters)]
#[getset(get = "pub")]
pub struct FastCGIRequest<'a> {
    role: &'a str,
    request_id: RequestID,
    request_uri: Option<&'a str>,
    params: Vec<ParamKeyValue<'a>>,
}

impl<'a, W: AsyncWrite + Unpin> From<(RequestID, &'a tokio_fastcgi::Request<W>)>
    for FastCGIRequest<'a>
{
    fn from(id_and_request: (RequestID, &'a tokio_fastcgi::Request<W>)) -> FastCGIRequest<'a> {
        let (request_id, request) = id_and_request;

        let role = match request.role {
            tokio_fastcgi::Role::Authorizer => "Authorizer",
            tokio_fastcgi::Role::Filter => "Filter",
            tokio_fastcgi::Role::Responder => "Responder",
        };

        let request_uri = request.get_str_param("request_uri");

        let params: Vec<ParamKeyValue<'a>> = match request.str_params_iter() {
            Some(iter) => iter
                .filter(|v| v.0 != "request_uri")
                .map(|v| (v.0, v.1.unwrap_or("[Invalid UTF8]")))
                .collect(),
            None => Vec::new(),
        };

        FastCGIRequest {
            role,
            request_id,
            request_uri,
            params,
        }
    }
}
