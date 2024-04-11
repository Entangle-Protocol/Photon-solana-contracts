mod app;
mod config;
mod error;
mod rabbitmq_publisher;
mod solana_event_listener;
mod solana_events_reader;
mod solana_logs_processor;

pub(super) use app::ListenerApp;

struct LogsBunch {
    tx_signature: String,
    logs: Vec<String>,
    slot: u64,
}
