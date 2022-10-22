pub mod body;
pub mod responder;

pub type HttpResponse = http::Response<Option<body::HttpResponseBody>>;