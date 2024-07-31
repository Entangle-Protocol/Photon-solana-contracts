use log::{debug, error, info};
use photon::ProposeEvent;
use solana_sdk::signature::Signature;
use std::str::FromStr;
use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender},
    Mutex,
};

use crate::common::solana_logs::{
    event_processor::EventProcessor, solana_event_listener::LogsBunch,
};
use transmitter_common::{
    data::{default_meta, OperationData, Propose, ProtocolId},
    SOLANA_CHAIN_ID,
};

pub(super) struct ProposalEventProcessor {
    logs_receiver: Mutex<UnboundedReceiver<LogsBunch>>,
    propose_sender: UnboundedSender<Propose>,
    allowed_protocols: Vec<Vec<u8>>,
}

impl ProposalEventProcessor {
    pub(super) fn new(
        logs_receiver: UnboundedReceiver<LogsBunch>,
        propose_sender: UnboundedSender<Propose>,
        allowed_protocols: Vec<String>,
    ) -> ProposalEventProcessor {
        info!("Allowed protocols to listen events: {}", allowed_protocols.join(", "));

        let allowed_protocols: Vec<Vec<u8>> = allowed_protocols
            .iter()
            .map(|p| {
                let mut allowed_protocol = vec![0_u8; 32];
                let bytes_to_write = p.as_bytes();
                let length_to_copy = bytes_to_write.len().min(allowed_protocol.len());
                allowed_protocol[..length_to_copy]
                    .copy_from_slice(&bytes_to_write[..length_to_copy]);
                allowed_protocol
            })
            .collect();

        ProposalEventProcessor {
            logs_receiver: Mutex::new(logs_receiver),
            propose_sender,
            allowed_protocols,
        }
    }

    pub(super) async fn execute(&self) {
        while let Some(logs_bunch) = self.logs_receiver.lock().await.recv().await {
            self.on_logs(logs_bunch);
        }
    }
}

impl EventProcessor for ProposalEventProcessor {
    type Event = ProposeEvent;

    fn on_event(&self, event: Self::Event, signature: &str, slot: u64, need_check: bool) {
        if !self.allowed_protocols.contains(&event.protocol_id) {
            return;
        }

        let Ok(protocol_id) = <[u8; 32]>::try_from(event.protocol_id.clone()).map_err(|_| {
            error!("Failed to get 32 bytes protocol_id chunk from event data, skip event");
        }) else {
            return;
        };

        debug!("Solana event intercepted: {:?}", event);
        let Ok(signature) = Signature::from_str(signature) else {
            error!("Failed to parse tx_signature from: {}", signature);
            return;
        };
        if let Err(err) = self.propose_sender.send(Propose {
            need_check,
            latest_block_id: signature.to_string(),
            operation_data: OperationData {
                src_chain_id: SOLANA_CHAIN_ID,
                meta: default_meta(),
                src_block_number: slot,
                src_op_tx_id: signature.as_ref().to_vec(),
                protocol_id: ProtocolId(protocol_id),
                nonce: event.nonce,
                dest_chain_id: event.dst_chain_id,
                protocol_addr: event.protocol_address,
                function_selector: event.function_selector,
                params: event.params,
                reserved: <Vec<u8>>::default(),
            },
        }) {
            error!("Failed to send proposal through the channel: {}", err);
        }
    }
}
