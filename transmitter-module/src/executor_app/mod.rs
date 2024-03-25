mod app;
mod config;
mod error;
mod extension_manager;
mod operation_manager;
mod rabbitmq_consumer;
mod solana_transactor;
mod tx_builder_exec;
mod tx_builder_load;
mod tx_builder_sign;

use solana_sdk::{hash::Hash, transaction::Transaction};

use transmitter_common::data::OpHash;

pub(super) use app::ExecutorApp;

pub(crate) enum OperationStatus {
    Complete(OpHash),
    Error(OpHash),
    Reschedule(OpHash),
}

pub(crate) struct TransactionSet {
    blockhash: Option<Hash>,
    op_hash: OpHash,
    txs: Vec<Transaction>,
}
