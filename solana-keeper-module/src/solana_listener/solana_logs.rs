use anchor_lang;
use regex::Regex;
use solana_client::rpc_response::{Response, RpcLogsResponse};

pub(super) fn parse_logs_response<T: anchor_lang::Event + anchor_lang::AnchorDeserialize>(
    logs: Response<RpcLogsResponse>,
    program_id_str: &str,
) -> anyhow::Result<Vec<T>> {
    let logs = &logs.value.logs[..];
    let logs: Vec<&str> = logs.iter().by_ref().map(String::as_str).collect();
    parse_logs_impl(logs.as_slice(), program_id_str)
}

fn parse_logs_impl<T: anchor_lang::Event + anchor_lang::AnchorDeserialize>(
    logs: &[&str],
    program_id_str: &str,
) -> anyhow::Result<Vec<T>> {
    let mut events: Vec<T> = Vec::new();
    let mut do_pop = false;
    if !logs.is_empty() {
        log::debug!("Logs: {:?}", logs);
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
                    handle_program_log(program_id_str, log).unwrap_or_else(|e| {
                        println!("Unable to parse log: {e}");
                        std::process::exit(1);
                    })
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
) -> anyhow::Result<(Option<T>, bool), String> {
    // Log emitted from the current program.
    if let Some(log) = l
        .strip_prefix(PROGRAM_LOG)
        .or_else(|| l.strip_prefix(PROGRAM_DATA))
    {
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
            let e: T = anchor_lang::AnchorDeserialize::deserialize(&mut slice)
                .map_err(|e| e.to_string())?;
            event = Some(e);
        }
        Ok((event, false))
    }
    // System log.
    else {
        let (_program, did_pop) = handle_irrelevant_log(self_program_str, l);
        Ok((None, did_pop))
    }
}

fn handle_irrelevant_log(this_program_str: &str, log: &str) -> (Option<String>, bool) {
    let re =
        Regex::new(r"^Program (.*) invoke$").expect("Program invoke re should be constructed well");
    if log.starts_with(&format!("Program {this_program_str} log:")) {
        (Some(this_program_str.to_string()), false)
    } else if let Some(c) = re.captures(log) {
        (c.get(1).unwrap().as_str().to_string().into(), false) // Any string will do.
    } else {
        let re = Regex::new(r"^Program (.*) success$").expect("Expected re be constructed well");
        (None, re.is_match(log))
    }
}

struct Execution {
    stack: Vec<String>,
}

impl Execution {
    fn program(&self) -> anyhow::Result<String> {
        if self.stack.is_empty() {
            return Err(anyhow::Error::msg("Stack is empty".to_string()));
        }
        Ok(self.stack[self.stack.len() - 1].clone())
    }

    fn push(&mut self, new_program: String) {
        self.stack.push(new_program);
    }

    fn pop(&mut self) -> anyhow::Result<()> {
        if self.stack.is_empty() {
            return Err(anyhow::Error::msg("Stack is empty".to_string()));
        }
        self.stack.pop().expect("Stack should not be empty");
        Ok(())
    }

    fn update(&mut self, log: &str) -> anyhow::Result<String> {
        let re = Regex::new(r"^Program (.*) invoke.*$").unwrap();
        let Some(c) = re.captures(log) else {
            return self.program();
        };

        let program = c
            .get(1)
            .ok_or_else(|| anyhow::anyhow!("Failed to get program"))?
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

        let events: Vec<ProposeEvent> = parse_logs_impl(SAMPLE, &PROGRAM_ID.to_string())
            .expect("Processing logs should not result in errors");
        assert_eq!(events.len(), 1);
        let propose_event = events.first().expect("No events caught");
        assert_eq!(propose_event.dst_chain_id, 33133);
        assert_eq!(propose_event.params, vec![1, 2, 3]);
        assert_eq!(
            propose_event.protocol_id.as_slice(),
            b"onefunc_________________________"
        );
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
        let events: Vec<ProposeEvent> = parse_logs_impl(SAMPLE, &PROGRAM_ID.to_string())
            .expect("Processing logs should not result in errors");
        assert!(events.is_empty(), "Expected no events have been met")
    }
}
