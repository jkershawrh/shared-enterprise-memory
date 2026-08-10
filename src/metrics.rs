use lazy_static::lazy_static;
use prometheus::{
    Counter, CounterVec, Encoder, GaugeVec, Histogram, HistogramOpts, Opts, Registry, TextEncoder,
};

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
    pub static ref PROJECTION_ENTRIES_PROCESSED: Counter = Counter::with_opts(Opts::new(
        "projection_entries_processed_total",
        "Total ledger entries successfully projected"
    ))
    .unwrap();
    pub static ref PROJECTION_ERRORS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(
            "projection_errors_total",
            "Total projection errors by entry type"
        ),
        &["entry_type"]
    )
    .unwrap();
    pub static ref PROJECTION_CYCLE_DURATION: Histogram = Histogram::with_opts(HistogramOpts::new(
        "projection_cycle_duration_seconds",
        "Duration of a projection processing cycle"
    ))
    .unwrap();
    pub static ref PROJECTION_LAG_ENTRIES: GaugeVec = GaugeVec::new(
        Opts::new(
            "projection_lag_entries",
            "Number of unprocessed entries per chain"
        ),
        &["entry_type"]
    )
    .unwrap();
    pub static ref CONTEXT_API_REQUESTS: CounterVec = CounterVec::new(
        Opts::new(
            "context_api_requests_total",
            "Total context API requests by endpoint and status"
        ),
        &["endpoint", "status"]
    )
    .unwrap();
    pub static ref CONTEXT_API_DURATION: Histogram = Histogram::with_opts(HistogramOpts::new(
        "context_api_duration_seconds",
        "Duration of context API requests"
    ))
    .unwrap();
}

pub fn register_metrics() {
    REGISTRY
        .register(Box::new(PROJECTION_ENTRIES_PROCESSED.clone()))
        .ok();
    REGISTRY
        .register(Box::new(PROJECTION_ERRORS_TOTAL.clone()))
        .ok();
    REGISTRY
        .register(Box::new(PROJECTION_CYCLE_DURATION.clone()))
        .ok();
    REGISTRY
        .register(Box::new(PROJECTION_LAG_ENTRIES.clone()))
        .ok();
    REGISTRY
        .register(Box::new(CONTEXT_API_REQUESTS.clone()))
        .ok();
    REGISTRY
        .register(Box::new(CONTEXT_API_DURATION.clone()))
        .ok();
}

pub fn encode_prometheus() -> String {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
