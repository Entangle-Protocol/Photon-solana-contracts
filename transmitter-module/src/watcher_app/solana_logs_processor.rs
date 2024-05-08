use log::{debug, error};
use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender},
    Mutex,
};

use crate::common::solana_logs::{
    event_processor::EventProcessor, solana_event_listener::LogsBunch,
};

use transmitter_common::data::{OpHash, ProposalExecuted};

pub(super) struct OperationExecutedEventProcessor {
    logs_receiver: Mutex<UnboundedReceiver<LogsBunch>>,
    op_status_sender: UnboundedSender<ProposalExecuted>,
}

impl OperationExecutedEventProcessor {
    pub(super) fn new(
        logs_receiver: UnboundedReceiver<LogsBunch>,
        op_status_sender: UnboundedSender<ProposalExecuted>,
    ) -> OperationExecutedEventProcessor {
        OperationExecutedEventProcessor {
            logs_receiver: Mutex::new(logs_receiver),
            op_status_sender,
        }
    }

    pub(super) async fn execute(&self) {
        while let Some(logs_bunch) = self.logs_receiver.lock().await.recv().await {
            self.on_logs(logs_bunch);
        }
    }
}

impl EventProcessor for OperationExecutedEventProcessor {
    type Event = photon::ProposalExecuted;

    fn on_event(&self, event: Self::Event, signature: &str, _slot: u64) {
        debug!("OperationExecuted status event intercepted: {:?}", event);

        if let Err(err) = self.op_status_sender.send(ProposalExecuted {
            last_watched_block: signature.to_string(),
            op_hash: OpHash::try_from(event.op_hash)
                .expect("op_hash expected to be gotten from proposal_executed event"),
            executor: event.executor,
        }) {
            error!("Failed to send proposal through the channel: {}", err);
        }
    }
}
