use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

use crate::config::ServerType;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct FastCGIConnectionID {
    server_type: ServerType,
    id: u64,
}

pub struct FastCGIConnectionIDFactory {
    server_type: ServerType,
    next_connection_id: AtomicU64,
}

impl FastCGIConnectionIDFactory {
    pub fn new(server_type: ServerType) -> Self {
        Self {
            server_type,
            next_connection_id: AtomicU64::new(1),
        }
    }

    pub fn new_connection_id(&self) -> FastCGIConnectionID {
        let id = self.next_connection_id.fetch_add(1, Ordering::Relaxed);

        FastCGIConnectionID {
            server_type: self.server_type,
            id,
        }
    }
}
