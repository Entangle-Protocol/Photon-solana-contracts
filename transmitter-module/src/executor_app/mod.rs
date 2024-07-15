mod app;
mod config;
mod error;
mod extension_manager;
mod last_block_updater;
mod operation_manager;
mod rabbitmq_consumer;

use std::fmt::{Display, Formatter};

use transmitter_common::data::OpHash;

pub(super) use app::ExecutorApp;

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

impl Display for OpAcknowledge {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "block_number: {}, op_hash: {}, status: {:?}",
            self.block_number,
            hex::encode(self.op_hash),
            self.status
        )
    }
}

#[derive(Debug)]
enum ExecutorOpStatus {
    New,
    Loaded,
    Signed,
    Executed,
}

const OP_DATA_SENDER_CAPACITY: usize = 64;
