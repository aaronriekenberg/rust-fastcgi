use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use async_trait::async_trait;

use serde::Serialize;

use crate::handlers::{
    route::URIAndHandler,
    utils::build_json_response,
    {FastCGIRequest, HttpResponse, RequestHandler},
};

#[derive(Debug, Default, Serialize)]
struct RequestInfoResponse<'a> {
    role: &'a str,
    request_id: u16,
    request_uri: &'a str,
    http_headers: BTreeMap<&'a str, &'a str>,
    other_params: BTreeMap<&'a str, &'a str>,
}

struct RequestInfoHandler {}

impl RequestInfoHandler {
    fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl RequestHandler for RequestInfoHandler {
    async fn handle(&self, request: FastCGIRequest<'_>) -> HttpResponse {
        let mut response = RequestInfoResponse {
            role: request.role(),
            request_id: *request.request_id(),
            request_uri: request.request_uri().unwrap_or("[Unknown URI]"),
            ..Default::default()
        };

        for (key, value) in request.params().iter() {
            if key.starts_with("http_") {
                let http_header_key = &key[5..];
                response.http_headers.insert(http_header_key, value);
            } else {
                response.other_params.insert(key, value);
            }
        }

        build_json_response(response)
    }
}

#[derive(Debug, Default, Serialize)]
struct JemallocStatsResponse {
    allocated_bytes: usize,
    resident_bytes: usize,
    epoch: u64,
}

struct JemallocStatsHandler {
    allocated: tikv_jemalloc_ctl::stats::allocated_mib,
    resident: tikv_jemalloc_ctl::stats::resident_mib,
    epoch: Arc<AtomicU64>,
}

impl JemallocStatsHandler {
    fn new() -> Self {
        let epoch = Arc::new(AtomicU64::new(0));

        let epoch_mib = tikv_jemalloc_ctl::epoch::mib().unwrap();
        epoch.store(epoch_mib.advance().unwrap(), Ordering::Relaxed);

        let epoch_clone = Arc::clone(&epoch);

        tokio::spawn(async move {
            let duration = tokio::time::Duration::from_secs(10);
            loop {
                tokio::time::sleep(duration).await;
                epoch_clone.store(epoch_mib.advance().unwrap(), Ordering::Relaxed);
            }
        });

        let allocated = tikv_jemalloc_ctl::stats::allocated::mib().unwrap();
        let resident = tikv_jemalloc_ctl::stats::resident::mib().unwrap();

        Self {
            allocated,
            resident,
            epoch,
        }
    }
}

#[async_trait]
impl RequestHandler for JemallocStatsHandler {
    async fn handle(&self, _request: FastCGIRequest<'_>) -> HttpResponse {
        let allocated_bytes = self.allocated.read().unwrap();
        let resident_bytes = self.resident.read().unwrap();

        let response = JemallocStatsResponse {
            allocated_bytes,
            resident_bytes,
            epoch: self.epoch.load(Ordering::Relaxed),
        };

        build_json_response(response)
    }
}

pub fn create_routes() -> Vec<URIAndHandler> {
    let mut routes: Vec<URIAndHandler> = Vec::new();

    routes.push((
        "/cgi-bin/debug/request_info".to_string(),
        Box::new(RequestInfoHandler::new()),
    ));

    routes.push((
        "/cgi-bin/debug/jemalloc_stats".to_string(),
        Box::new(JemallocStatsHandler::new()),
    ));

    routes
}
