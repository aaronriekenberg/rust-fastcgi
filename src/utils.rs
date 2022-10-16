use tokio::io::{AsyncRead, AsyncWrite};

// idea from https://github.com/rust-lang/rust/issues/41517#issuecomment-1140505957
pub trait GenericAsyncWriter: AsyncWrite + Unpin {}

impl<T> GenericAsyncWriter for T where T: AsyncWrite + Unpin {}

pub trait GenericAsyncReader: AsyncRead + Unpin {}

impl<T> GenericAsyncReader for T where T: AsyncRead + Unpin {}
