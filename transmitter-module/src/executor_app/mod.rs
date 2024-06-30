mod alt_operation_manager;
mod app;
mod config;
mod error;
mod extension_manager;
mod last_block_updater;
mod rabbitmq_consumer;

pub(super) use app::ExecutorApp;

pub(crate) enum ServiceCmd {
    UpdateExtensions(Vec<String>),
}
