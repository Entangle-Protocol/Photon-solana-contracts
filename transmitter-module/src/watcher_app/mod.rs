mod app;
mod config;
mod data;
mod error;
mod rabbitmq_publisher;
mod solana_logs_processor;

pub(super) use app::WatcherApp;
