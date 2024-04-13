use log::{debug, error};
use photon::ProposeEvent;
use regex::Regex;
use solana_sdk::signature::Signature;
use std::str::FromStr;
use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender},
    Mutex,
};

use transmitter_common::{
    data::{Meta, OperationData, Propose, ProtocolId},
    SOLANA_CHAIN_ID,
};

use crate::listener_app::{error::ListenError, LogsBunch};

pub(super) struct SolanaLogsProcessor {
    logs_receiver: Mutex<UnboundedReceiver<LogsBunch>>,
    propose_sender: UnboundedSender<Propose>,
}

impl SolanaLogsProcessor {
    pub(super) fn new(
        logs_receiver: UnboundedReceiver<LogsBunch>,
        propose_sender: UnboundedSender<Propose>,
    ) -> SolanaLogsProcessor {
        SolanaLogsProcessor {
            logs_receiver: Mutex::new(logs_receiver),
            propose_sender,
        }
    }

    pub(super) async fn execute(&self) {
        while let Some(logs_bunch) = self.logs_receiver.lock().await.recv().await {
            self.on_logs(logs_bunch);
        }
    }

    fn on_logs(&self, logs_bunch: LogsBunch) {
        let logs = &logs_bunch.logs[..];
        let logs: Vec<&str> = logs.iter().by_ref().map(String::as_str).collect();
        let Ok(events) =
            parse_logs::<ProposeEvent>(logs.as_slice(), photon::ID.to_string().as_str())
        else {
            log::error!("Failed to parse logs: {:?}", logs);
            return;
        };
        debug!(
            "Logs intercepted, tx_signature: {}, events: {}",
            logs_bunch.tx_signature.to_string(),
            events.len()
        );

        for event in events {
            let Ok(protocol_id) = event.protocol_id.first_chunk().copied().ok_or_else(|| {
                error!("Failed to get 32 bytes protocol_id chunk from event data, skip event")
            }) else {
                continue;
            };
            self.on_event(
                event,
                ProtocolId(protocol_id),
                &logs_bunch.tx_signature,
                logs_bunch.slot,
            );
        }
    }

    fn on_event(&self, event: ProposeEvent, protocol_id: ProtocolId, signature: &str, slot: u64) {
        debug!("Solana event intercepted: {:?}", event);
        let Ok(signature) = Signature::from_str(signature) else {
            error!("Failed to parse tx_signature from: {}", signature);
            return;
        };
        if let Err(err) = self.propose_sender.send(Propose {
            latest_block_id: signature.to_string(),
            operation_data: OperationData {
                src_chain_id: SOLANA_CHAIN_ID,
                meta: Meta::default(),
                src_block_number: slot,
                src_op_tx_id: signature.as_ref().to_vec(),
                protocol_id,
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

fn parse_logs<T: anchor_lang::Event + anchor_lang::AnchorDeserialize>(
    logs: &[&str],
    program_id_str: &str,
) -> Result<Vec<T>, ListenError> {
    let mut events: Vec<T> = Vec::new();
    let mut do_pop = false;
    if !logs.is_empty() {
        let mut execution = Execution {
            stack: <Vec<String>>::default(),
        };
        for log in logs {
            let (event, pop) = {
                if do_pop {
                    execution.pop()?;
                }
                execution.update(log)?;
                if program_id_str == execution.program()? {
                    handle_program_log(program_id_str, log).map_err(|e| {
                        error!("Failed to parse log: {}", e);
                        ListenError::SolanaParseLogs
                    })?
                } else {
                    let (_, pop) = handle_irrelevant_log(program_id_str, log);
                    (None, pop)
                }
            };
            do_pop = pop;
            if let Some(e) = event {
                events.push(e);
            }
        }
    }
    Ok(events)
}

const PROGRAM_LOG: &str = "Program log: ";
const PROGRAM_DATA: &str = "Program data: ";

fn handle_program_log<T: anchor_lang::Event + anchor_lang::AnchorDeserialize>(
    self_program_str: &str,
    l: &str,
) -> Result<(Option<T>, bool), ListenError> {
    if let Some(log) = l.strip_prefix(PROGRAM_LOG).or_else(|| l.strip_prefix(PROGRAM_DATA)) {
        let borsh_bytes = match anchor_lang::__private::base64::decode(log) {
            Ok(borsh_bytes) => borsh_bytes,
            _ => {
                #[cfg(feature = "debug")]
                println!("Could not base64 decode log: {}", log);
                return Ok((None, false));
            }
        };

        let mut slice: &[u8] = &borsh_bytes[..];
        let disc: [u8; 8] = {
            let mut disc = [0; 8];
            disc.copy_from_slice(&borsh_bytes[..8]);
            slice = &slice[8..];
            disc
        };
        let mut event = None;
        if disc == T::discriminator() {
            let e: T = anchor_lang::AnchorDeserialize::deserialize(&mut slice).map_err(|err| {
                error!("Failed to deserialize event: {}", err);
                ListenError::SolanaParseLogs
            })?;
            event = Some(e);
        }
        Ok((event, false))
    } else {
        let (_program, did_pop) = handle_irrelevant_log(self_program_str, l);
        Ok((None, did_pop))
    }
}

fn handle_irrelevant_log(this_program_str: &str, log: &str) -> (Option<String>, bool) {
    let re = Regex::new(r"^Program (.*) invoke$")
        .expect("Expected invoke regexp to be constructed well");
    if log.starts_with(&format!("Program {this_program_str} log:")) {
        (Some(this_program_str.to_string()), false)
    } else if let Some(c) = re.captures(log) {
        (
            c.get(1)
                .expect("Expected the captured program to be available")
                .as_str()
                .to_string()
                .into(),
            false,
        )
    } else {
        let re =
            Regex::new(r"^Program (.*) success$").expect("Expected regexp to be constructed well");
        (None, re.is_match(log))
    }
}

struct Execution {
    stack: Vec<String>,
}

impl Execution {
    fn program(&self) -> Result<String, ListenError> {
        if self.stack.is_empty() {
            error!("Failed to get program from the empty stack");
            return Err(ListenError::SolanaParseLogs);
        }
        Ok(self.stack[self.stack.len() - 1].clone())
    }

    fn push(&mut self, new_program: String) {
        self.stack.push(new_program);
    }

    fn pop(&mut self) -> Result<(), ListenError> {
        if self.stack.is_empty() {
            error!("Failed to get program from the empty stack");
            return Err(ListenError::SolanaParseLogs);
        }
        self.stack.pop().expect("Stack should not be empty");
        Ok(())
    }

    fn update(&mut self, log: &str) -> Result<String, ListenError> {
        let re =
            Regex::new(r"^Program (.*) invoke.*$").expect("Expected regexp to be constructed well");
        let Some(c) = re.captures(log) else {
            return self.program();
        };
        let program = c
            .get(1)
            .expect("Expected captured program address to be available")
            .as_str()
            .to_string();
        self.push(program);
        self.program()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use photon::{ProposeEvent, ID as PROGRAM_ID};

    #[test]
    fn test_logs_parsing() {
        static SAMPLE: &[&str] = &[
            "Program EjpcUpcuJV2Mq9vjELMZHhgpvJ4ggoWtUYCTFqw6D9CZ invoke [1]",
            "Program log: Instruction: ShareMessage",
            "Program log: Share message invoked",
            "Program 3cAFEXstVzff2dXH8PFMgm81h8sQgpdskFGZqqoDgQkJ invoke [2]",
            "Program log: Instruction: Propose",
            "Program data: 8vb9LnW1kqUgAAAAb25lZnVuY19fX19fX19fX19fX19fX19fX19fX19fX18IAAAAAAAAAG2BAAAAAAAAAAAAAAAAAAADAAAAAQIDAwAAAAECAwMAAAABAgM=",
            "Program 3cAFEXstVzff2dXH8PFMgm81h8sQgpdskFGZqqoDgQkJ consumed 16408 of 181429 compute units",
            "Program 3cAFEXstVzff2dXH8PFMgm81h8sQgpdskFGZqqoDgQkJ success",
            "Program EjpcUpcuJV2Mq9vjELMZHhgpvJ4ggoWtUYCTFqw6D9CZ consumed 35308 of 200000 compute units",
            "Program EjpcUpcuJV2Mq9vjELMZHhgpvJ4ggoWtUYCTFqw6D9CZ success",
        ];

        let events: Vec<ProposeEvent> = parse_logs(SAMPLE, &PROGRAM_ID.to_string())
            .expect("Processing logs should not result in errors");
        assert_eq!(events.len(), 1);
        let propose_event = events.first().expect("No events caught");
        assert_eq!(propose_event.dst_chain_id, 33133);
        assert_eq!(propose_event.params, vec![1, 2, 3]);
        assert_eq!(propose_event.protocol_id.as_slice(), b"onefunc_________________________");
    }

    #[test]
    fn test_deploy_programs() {
        static SAMPLE: &[&str] = &[
            "Program 11111111111111111111111111111111 invoke [1]",
            "Program 11111111111111111111111111111111 success",
            "Program BPFLoaderUpgradeab1e11111111111111111111111 invoke [1]",
            "Program 11111111111111111111111111111111 invoke [2]",
            "Program 11111111111111111111111111111111 success",
            "Deployed program 3cAFEXstVzff2dXH8PFMgm81h8sQgpdskFGZqqoDgQkJ",
            "Program BPFLoaderUpgradeab1e11111111111111111111111 success",
        ];
        let events: Vec<ProposeEvent> = parse_logs(SAMPLE, &PROGRAM_ID.to_string())
            .expect("Processing logs should not result in errors");
        assert!(events.is_empty(), "Expected no events have been met")
    }
}
