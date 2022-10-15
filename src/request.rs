use getset::Getters;

use crate::{connection::FastCGIConnectionID, utils::GenericAsyncWriter};

#[derive(Clone, Copy, Debug)]
pub struct FastCGIRequestID(pub u16);

pub type ParamKeyValue<'a> = (&'a str, &'a str);

#[derive(Debug, Getters)]
#[getset(get = "pub")]
pub struct FastCGIRequest<'a> {
    role: &'a str,
    connection_id: FastCGIConnectionID,
    request_id: FastCGIRequestID,
    request_uri: Option<&'a str>,
    params: Vec<ParamKeyValue<'a>>,
}

impl<'a> FastCGIRequest<'a> {
    pub fn new(
        connection_id: FastCGIConnectionID,
        request: &'a tokio_fastcgi::Request<impl GenericAsyncWriter>,
    ) -> FastCGIRequest<'a> {
        let request_id = FastCGIRequestID(request.get_request_id());

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
            connection_id,
            request_id,
            request_uri,
            params,
        }
    }
}
