mod alt_operation_manager;
mod app;
mod config;
mod error;
mod extension_manager;
mod last_block_updater;
mod rabbitmq_consumer;

pub(super) use app::ExecutorApp;
use transmitter_common::data::OpHash;

pub(crate) enum ServiceCmd {
    UpdateExtensions(Vec<String>),
}

#[derive(Debug)]
struct OpAcknowledge {
    block_number: u64,
    op_hash: OpHash,
    status: ExecutorOpStatus,
}

impl OpAcknowledge {
    fn new(block_number: u64, op_hash: OpHash, status: ExecutorOpStatus) -> Self {
        Self {
            block_number,
            op_hash,
            status,
        }
    }
}

#[derive(Debug)]
enum ExecutorOpStatus {
    New,
    Loaded,
    Signed,
    Executed,
}
