use std::sync::Arc;

#[derive(Debug)]
pub enum HttpResponseBody {
    ArcString(Arc<String>),

    String(String),
}

impl HttpResponseBody {
    pub(super) fn as_bytes(&self) -> &[u8] {
        match self {
            Self::ArcString(a) => a.as_bytes(),
            Self::String(s) => s.as_bytes(),
        }
    }
}

impl From<Arc<String>> for HttpResponseBody {
    fn from(a: Arc<String>) -> Self {
        Self::ArcString(a)
    }
}

impl From<String> for HttpResponseBody {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}
