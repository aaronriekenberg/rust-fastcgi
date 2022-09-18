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

    fn epoch_number(&self) -> u64 {
        self.epoch_number.load(Ordering::Relaxed)
    }
}

struct JemallocStatsService {
    epoch_controller: Arc<JemallocEpochController>,
    active: tikv_jemalloc_ctl::stats::active_mib,
    allocated: tikv_jemalloc_ctl::stats::allocated_mib,
    resident: tikv_jemalloc_ctl::stats::resident_mib,
    retained: tikv_jemalloc_ctl::stats::retained_mib,
    narenas: tikv_jemalloc_ctl::arenas::narenas_mib,
    thread_allocatedp: tikv_jemalloc_ctl::thread::allocatedp_mib,
    thread_deallocatedp: tikv_jemalloc_ctl::thread::deallocatedp_mib,
    jemalloc_version: &'static str,
}

impl JemallocStatsService {
    fn new() -> anyhow::Result<Arc<Self>> {
        let epoch_controller =
            JemallocEpochController::new().context("JemallocEpochController::new")?;
        epoch_controller.start_epoch_updates();

        let active = tikv_jemalloc_ctl::stats::active::mib()
            .context("tikv_jemalloc_ctl::stats::active::mib")?;

        let allocated = tikv_jemalloc_ctl::stats::allocated::mib()
            .context("tikv_jemalloc_ctl::stats::allocated::mib")?;

        let resident = tikv_jemalloc_ctl::stats::resident::mib()
            .context("tikv_jemalloc_ctl::stats::resident::mib")?;

        let retained = tikv_jemalloc_ctl::stats::retained::mib()
            .context("tikv_jemalloc_ctl::stats::retained::mib")?;

        let narenas = tikv_jemalloc_ctl::arenas::narenas::mib()
            .context("tikv_jemalloc_ctl::arenas::narenas::mib")?;

        let thread_allocatedp = tikv_jemalloc_ctl::thread::allocatedp::mib()
            .context("tikv_jemalloc_ctl::thread::allocatedp::mib")?;

        let thread_deallocatedp = tikv_jemalloc_ctl::thread::deallocatedp::mib()
            .context("tikv_jemalloc_ctl::thread::deallocatedp::mib")?;

        let jemalloc_version =
            tikv_jemalloc_ctl::version::read().context("tikv_jemalloc_ctl::version::read")?;

        Ok(Arc::new(Self {
            epoch_controller,
            active,
            allocated,
            resident,
            retained,
            narenas,
            thread_allocatedp,
            thread_deallocatedp,
            jemalloc_version,
        }))
    }

    fn epoch_number(&self) -> u64 {
        self.epoch_controller.epoch_number()
    }

    fn active_bytes(&self) -> usize {
        self.active.read().unwrap_or(0)
    }

    fn allocated_bytes(&self) -> usize {
        self.allocated.read().unwrap_or(0)
    }

    fn resident_bytes(&self) -> usize {
        self.resident.read().unwrap_or(0)
    }

    fn retained_bytes(&self) -> usize {
        self.retained.read().unwrap_or(0)
    }

    fn num_arenas(&self) -> u32 {
        self.narenas.read().unwrap_or(0)
    }

    fn current_thread_allocated_bytes(&self) -> u64 {
        match self.thread_allocatedp.read() {
            Ok(thread_local_data) => thread_local_data.get(),
            Err(_) => 0,
        }
    }

    fn current_thread_deallocated_bytes(&self) -> u64 {
        match self.thread_deallocatedp.read() {
            Ok(thread_local_data) => thread_local_data.get(),
            Err(_) => 0,
        }
    }

    fn jemalloc_version(&self) -> &str {
        self.jemalloc_version
    }
}

#[derive(Debug, Default, Serialize)]
struct JemallocStatsResponse<'a> {
    epoch_interval_seconds: u64,
    epoch_number: u64,
    active_bytes: usize,
    allocated_bytes: usize,
    resident_bytes: usize,
    retained_bytes: usize,
    num_arenas: u32,
    current_thread_name: &'a str,
    current_thread_allocated_bytes: u64,
    current_thread_deallocated_bytes: u64,
    jemalloc_version: &'a str,
}

struct JemallocStatsHandler {
    stats_service: Arc<JemallocStatsService>,
}

impl JemallocStatsHandler {
    fn new(stats_service: Arc<JemallocStatsService>) -> anyhow::Result<Self> {
        Ok(Self { stats_service })
    }
}

#[async_trait]
impl RequestHandler for JemallocStatsHandler {
    async fn handle(&self, _request: FastCGIRequest<'_>) -> HttpResponse {
        let epoch_number = self.stats_service.epoch_number();
        let active_bytes = self.stats_service.active_bytes();
        let allocated_bytes = self.stats_service.allocated_bytes();
        let resident_bytes = self.stats_service.resident_bytes();
        let retained_bytes = self.stats_service.retained_bytes();
        let num_arenas = self.stats_service.num_arenas();

        let current_thread = std::thread::current();
        let current_thread_name = current_thread.name().unwrap_or("UNKNOWN");

        let current_thread_allocated_bytes = self.stats_service.current_thread_allocated_bytes();
        let current_thread_deallocated_bytes =
            self.stats_service.current_thread_deallocated_bytes();

        let jemalloc_version = self.stats_service.jemalloc_version();

        let response = JemallocStatsResponse {
            epoch_interval_seconds: EPOCH_INTERVAL_SECONDS,
            epoch_number,
            active_bytes,
            allocated_bytes,
            resident_bytes,
            retained_bytes,
            num_arenas,
            current_thread_name,
            current_thread_allocated_bytes,
            current_thread_deallocated_bytes,
            jemalloc_version,
        };

        build_json_response(response)
    }
}

pub fn create_routes() -> anyhow::Result<Vec<URIAndHandler>> {
    let stats_service = JemallocStatsService::new()?;

    Ok(vec![(
        "/cgi-bin/jemalloc_stats".to_string(),
        Box::new(JemallocStatsHandler::new(stats_service)?),
    )])
}
