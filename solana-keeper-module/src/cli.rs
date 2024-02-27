use anyhow::Result;
use clap::{Parser, Subcommand};
use solana_sdk::commitment_config::CommitmentLevel;

use super::solana_listener::SolanaListener;

#[derive(Subcommand)]
enum Command {
    #[command(
        about = "Starts listening to solana for new events to register them for further processing"
    )]
    Listen {
        #[arg(
            long,
            short,
            help = "Solana cluster web socket url to connect to",
            default_value = "ws://127.0.0.1:8900"
        )]
        url: String,
        #[arg(
            long,
            short = 'C',
            help = "Commitment due to be used for event subscription",
            default_value = "finalized"
        )]
        commitment: CommitmentLevel,
    },
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub(super) struct Cli {
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    pub(super) async fn execute(args: impl Iterator<Item = String>) -> Result<()> {
        let mut parsed_cli = Self::parse_from(args);
        match &mut parsed_cli.command {
            Command::Listen { url, commitment } => SolanaListener::listen(url, *commitment).await?,
        }

        Ok(())
    }
}
