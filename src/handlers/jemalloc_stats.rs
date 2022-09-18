use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use anyhow::Context;

use async_trait::async_trait;

use serde::Serialize;

use crate::handlers::{
    route::URIAndHandler,
    utils::build_json_response,
    {FastCGIRequest, HttpResponse, RequestHandler},
};

const EPOCH_INTERVAL_SECONDS: u64 = 10;

struct JemallocEpochController {
    epoch_mib: tikv_jemalloc_ctl::epoch_mib,
    epoch_number: AtomicU64,
}

impl JemallocEpochController {
    fn new() -> anyhow::Result<Arc<Self>> {
        let epoch_mib =
            tikv_jemalloc_ctl::epoch::mib().context("tikv_jemalloc_ctl::epoch::mib error")?;

        let epoch_number = epoch_mib.advance().context("epoch_mib.advance error")?;

        Ok(Arc::new(Self {
            epoch_mib,
            epoch_number: AtomicU64::new(epoch_number),
        }))
    }

    fn start_epoch_updates(self: &Arc<Self>) {
        let self_clone = Arc::clone(self);

        tokio::spawn(async move {
            let duration = tokio::time::Duration::from_secs(EPOCH_INTERVAL_SECONDS);
            loop {
                tokio::time::sleep(duration).await;
                self_clone.epoch_number.store(
                    self_clone.epoch_mib.advance().unwrap_or(0),
                    Ordering::Relaxed,
                );
            }
        });
    }

    fn get_epoch_number(&self) -> u64 {
        self.epoch_number.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Default, Serialize)]
struct JemallocStatsResponse {
    allocated_bytes: usize,
    resident_bytes: usize,
    num_arenas: u32,
    epoch_interval_seconds: u64,
    epoch_number: u64,
    jemalloc_version: &'static str,
}

struct JemallocStatsHandler {
    allocated: tikv_jemalloc_ctl::stats::allocated_mib,
    resident: tikv_jemalloc_ctl::stats::resident_mib,
    narenas: tikv_jemalloc_ctl::arenas::narenas_mib,
    epoch_controller: Arc<JemallocEpochController>,
    jemalloc_version: &'static str,
}

impl JemallocStatsHandler {
    fn new(epoch_controller: Arc<JemallocEpochController>) -> anyhow::Result<Self> {
        let allocated = tikv_jemalloc_ctl::stats::allocated::mib()
            .context("tikv_jemalloc_ctl::stats::allocated::mib")?;

        let resident = tikv_jemalloc_ctl::stats::resident::mib()
            .context("tikv_jemalloc_ctl::stats::resident::mib")?;


        let narenas = tikv_jemalloc_ctl::arenas::narenas::mib()
        .context("tikv_jemalloc_ctl::arenas::narenas::mib")?;

        let jemalloc_version =
            tikv_jemalloc_ctl::version::read().context("tikv_jemalloc_ctl::version::read")?;

        Ok(Self {
            allocated,
            resident,
            narenas,
            epoch_controller,
            jemalloc_version,
        })
    }
}

#[async_trait]
impl RequestHandler for JemallocStatsHandler {
    async fn handle(&self, _request: FastCGIRequest<'_>) -> HttpResponse {
        let allocated_bytes = self.allocated.read().unwrap_or(0);
        let resident_bytes = self.resident.read().unwrap_or(0);
        let num_arenas = self.narenas.read().unwrap_or(0);

        let response = JemallocStatsResponse {
            allocated_bytes,
            resident_bytes,
            num_arenas,
            epoch_interval_seconds: EPOCH_INTERVAL_SECONDS,
            epoch_number: self.epoch_controller.get_epoch_number(),
            jemalloc_version: self.jemalloc_version,
        };

        build_json_response(response)
    }
}

pub fn create_routes() -> anyhow::Result<Vec<URIAndHandler>> {
    let epoch_controller = JemallocEpochController::new()?;
    epoch_controller.start_epoch_updates();

    Ok(vec![(
        "/cgi-bin/jemalloc_stats".to_string(),
        Box::new(JemallocStatsHandler::new(epoch_controller)?),
    )])
}
