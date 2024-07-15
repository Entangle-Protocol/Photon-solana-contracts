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
use photon::protocol_data::OpStatus;

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

#[derive(Copy, Clone, Debug, PartialEq)]
enum ExecutorOpStatus {
    New,
    Loaded,
    Signed,
    Executed,
    Failed,
}

impl From<OpStatus> for ExecutorOpStatus {
    fn from(value: OpStatus) -> Self {
        match value {
            OpStatus::None => ExecutorOpStatus::New,
            OpStatus::Init => ExecutorOpStatus::Loaded,
            OpStatus::Signed => ExecutorOpStatus::Signed,
            OpStatus::Executed => ExecutorOpStatus::Executed,
        }
    }
}

const OP_DATA_SENDER_CAPACITY: usize = 64;
