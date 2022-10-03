use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug)]
pub struct FastCGIConnectionID(pub u64);

pub struct FastCGIConnectionIDFactory {
    next_connection_id: AtomicU64,
}

impl FastCGIConnectionIDFactory {
    pub fn new() -> Self {
        Self {
            next_connection_id: AtomicU64::new(1),
        }
    }

    pub fn new_connection_id(&self) -> FastCGIConnectionID {
        let connection_id = self.next_connection_id.fetch_add(1, Ordering::Relaxed);

        FastCGIConnectionID(connection_id)
    }
}
