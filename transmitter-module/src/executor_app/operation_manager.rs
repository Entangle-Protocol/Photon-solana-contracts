use log::{debug, error, info, warn};
use std::{
    cell::RefCell,
    collections::hash_map::{Entry, HashMap, OccupiedEntry},
};
use tokio::{
    select,
    sync::{
        mpsc::{UnboundedReceiver, UnboundedSender},
        Mutex,
    },
};

use transmitter_common::data::{
    OpHash, OperationData, ProtocolId, SignedOperation, TransmitterSignature,
};

use super::{error::ExecutorError, OperationStatus};

pub(super) struct OperationStateMng {
    op_data_in_progress: RefCell<HashMap<OpHash, OpInfo>>,
    op_data_receiver: Mutex<UnboundedReceiver<SignedOperation>>,
    op_status_receiver: Mutex<UnboundedReceiver<OperationStatus>>,
    load_operation_builder_sender: UnboundedSender<(OpHash, OperationData)>,
    sign_operation_builder_sender: UnboundedSender<(OpHash, ProtocolId, Vec<TransmitterSignature>)>,
    exec_operation_builder_sender: UnboundedSender<(OpHash, OperationData)>,
    last_block_sender: UnboundedSender<u64>,
}

enum OpStage {
    Load,
    Sign,
    Execute,
}

struct OpInfo {
    operation: SignedOperation,
    op_stage: OpStage,
}

impl OperationStateMng {
    pub(super) fn new(
        op_data_receiver: UnboundedReceiver<SignedOperation>,
        status_receiver: UnboundedReceiver<OperationStatus>,
        load_operation_builder_sender: UnboundedSender<(OpHash, OperationData)>,
        sign_operation_builder_sender: UnboundedSender<(
            OpHash,
            ProtocolId,
            Vec<TransmitterSignature>,
        )>,
        exec_operation_builder_sender: UnboundedSender<(OpHash, OperationData)>,
        last_block_sender: UnboundedSender<u64>,
    ) -> OperationStateMng {
        OperationStateMng {
            op_data_in_progress: RefCell::default(),
            op_data_receiver: Mutex::new(op_data_receiver),
            op_status_receiver: Mutex::new(status_receiver),
            load_operation_builder_sender,
            sign_operation_builder_sender,
            exec_operation_builder_sender,
            last_block_sender,
        }
    }

    pub(super) async fn execute(self) -> Result<(), ExecutorError> {
        info!("Start managing operation data pipeline",);
        select! {
            _ = self.execute_op_data() => {},
            _ = self.process_operation_updates() => {}
        }
        Ok(())
    }

    async fn execute_op_data(&self) {
        info!("Start listen for incoming operation_data");
        while let Some(op_data) = self.op_data_receiver.lock().await.recv().await {
            let op_hash = op_data.operation_data.op_hash_with_message();
            self.process_operation(op_hash, op_data);
        }
    }

    fn process_operation(&self, op_hash: OpHash, signed_operation: SignedOperation) {
        debug!("Process operation received: {}", hex::encode(op_hash));

        self.last_block_sender
            .send(signed_operation.eob_block_number)
            .expect("Expected last_block_number to be sent");

        let mut op_data = self.op_data_in_progress.borrow_mut();
        let Entry::Vacant(entry) = op_data.entry(op_hash) else {
            warn!("Operation data is already in progress: {}", hex::encode(op_hash));
            return;
        };
        entry.insert(OpInfo {
            operation: signed_operation.clone(),
            op_stage: OpStage::Load,
        });
        self.load_operation(op_hash, signed_operation);
    }

    fn load_operation(&self, op_hash: OpHash, signed_operation: SignedOperation) {
        debug!("Load operation: {}", hex::encode(op_hash));
        self.load_operation_builder_sender
            .send((op_hash, signed_operation.operation_data))
            .expect("Expected op_data to be sent");
    }

    async fn process_operation_updates(&self) {
        info!("Start listen for operation updates");
        while let Some(operation_status) = self.op_status_receiver.lock().await.recv().await {
            self.on_operation_update(operation_status);
        }
    }

    fn on_operation_update(&self, operation_status: OperationStatus) {
        match operation_status {
            OperationStatus::Complete(op_hash) => self.on_operation_complete(op_hash),
            OperationStatus::Error(op_hash) => self.on_operation_error(op_hash),
            OperationStatus::Reschedule(op_hash) => self.on_operation_reschedule(op_hash),
        }
    }

    fn on_operation_complete(&self, op_hash: OpHash) {
        let mut op_data = self.op_data_in_progress.borrow_mut();
        let Entry::Occupied(mut entry) = op_data.entry(op_hash) else {
            error!("Unknown op_hash: {}", hex::encode(op_hash));
            return;
        };
        let op_info: &mut OpInfo = entry.get_mut();
        match op_info.op_stage {
            OpStage::Load => self.on_operation_loaded(op_hash, op_info),
            OpStage::Sign => self.on_operation_signed(op_hash, op_info),
            OpStage::Execute => self.on_operation_executed(op_hash, entry),
        }
    }

    fn on_operation_loaded(&self, op_hash: OpHash, op_info: &mut OpInfo) {
        debug!("Operation loaded: {}", hex::encode(op_hash));
        op_info.op_stage = OpStage::Sign;
        self.sign_operation(op_hash, op_info.operation.clone());
    }

    fn on_operation_signed(&self, op_hash: OpHash, op_info: &mut OpInfo) {
        debug!("Operation signed: {}", hex::encode(op_hash));
        op_info.op_stage = OpStage::Execute;
        self.execute_operation(op_hash, op_info.operation.clone());
    }

    fn on_operation_executed(&self, op_hash: OpHash, entry: OccupiedEntry<'_, OpHash, OpInfo>) {
        debug!("Operation executed, remove: {}", hex::encode(op_hash));
        entry.remove();
    }

    fn sign_operation(&self, op_hash: OpHash, signed_operation: SignedOperation) {
        debug!("Sign operation: {}", hex::encode(op_hash));

        self.sign_operation_builder_sender
            .send((
                op_hash,
                signed_operation.operation_data.protocol_id,
                signed_operation.signatures,
            ))
            .expect("Expected signatures to be sent");
    }

    fn execute_operation(&self, op_hash: OpHash, signed_operation: SignedOperation) {
        debug!("Execute operation: {}", hex::encode(op_hash));
        self.exec_operation_builder_sender
            .send((op_hash, signed_operation.operation_data))
            .expect("Expected signed operation to be sent");
    }

    fn on_operation_error(&self, op_hash: OpHash) {
        error!("Operation error: {}", hex::encode(op_hash));
        let mut binding = self.op_data_in_progress.borrow_mut();
        let Entry::Occupied(entry) = binding.entry(op_hash) else {
            error!("Failed to get entry by op_hash: {}", hex::encode(op_hash));
            return;
        };
        entry.remove();
    }

    fn on_operation_reschedule(&self, op_hash: OpHash) {
        let mut binding = self.op_data_in_progress.borrow_mut();
        let Entry::Occupied(entry) = binding.entry(op_hash) else {
            error!("Failed to get entry by op_hash: {}", hex::encode(op_hash));
            return;
        };
        error!("Rescheduling is not implemented so operation is removed: {}", hex::encode(op_hash));
        entry.remove();
    }
}
