use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

use crate::config::ServerType;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct FastCGIConnectionID {
    id: u64,
    server_type: ServerType,
}

pub struct FastCGIConnectionIDFactory {
    next_connection_id: AtomicU64,
    server_type: ServerType,
}

impl FastCGIConnectionIDFactory {
    pub fn new(server_type: ServerType) -> Self {
        Self {
            next_connection_id: AtomicU64::new(1),
            server_type,
        }
    }

    pub fn new_connection_id(&self) -> FastCGIConnectionID {
        let id = self.next_connection_id.fetch_add(1, Ordering::Relaxed);

        FastCGIConnectionID {
            id,
            server_type: self.server_type,
        }
    }
}
