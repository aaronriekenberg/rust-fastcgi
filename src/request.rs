use getset::Getters;

use tokio::io::AsyncWrite;

pub type ParamKeyValue<'a> = (&'a str, &'a str);

#[derive(Debug, Getters)]
#[getset(get = "pub")]
pub struct FastCGIRequest<'a> {
    role: &'a str,
    request_id: u16,
    request_uri: Option<&'a str>,
    params: Vec<ParamKeyValue<'a>>,
}

impl<'a> FastCGIRequest<'a> {
    fn new(
        role: &'a str,
        request_id: u16,
        request_uri: Option<&'a str>,
        params: Vec<ParamKeyValue<'a>>,
    ) -> Self {
        Self {
            role,
            request_id,
            request_uri,
            params,
        }
    }
}

impl<'a, W: AsyncWrite + Unpin> From<&'a tokio_fastcgi::Request<W>> for FastCGIRequest<'a> {
    fn from(request: &'a tokio_fastcgi::Request<W>) -> FastCGIRequest<'a> {
        let role = match request.role {
            tokio_fastcgi::Role::Authorizer => "Authorizer",
            tokio_fastcgi::Role::Filter => "Filter",
            tokio_fastcgi::Role::Responder => "Responder",
        };

        let request_uri = request.get_str_param("request_uri");

        let params: Vec<(&str, &str)> = match request.str_params_iter() {
            Some(iter) => iter
                .filter(|v| v.0 != "request_uri")
                .map(|v| (v.0, v.1.unwrap_or("[Invalid UTF8]")))
                .collect(),
            None => Vec::new(),
        };

        FastCGIRequest::new(role, request.get_request_id(), request_uri, params)
    }
}
