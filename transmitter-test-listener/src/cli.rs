use super::execute;
use clap::{Parser, Subcommand};

#[derive(Subcommand)]
pub(crate) enum Command {
    #[command(about = "Listen for proposal and mark it as processed for the test purposes")]
    Listen {
        #[arg(long, short, help = "Config path")]
        config: String,
    },
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub(super) struct Cli {
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    pub(super) async fn execute(args: impl Iterator<Item = String>) {
        let mut parsed_cli = Self::parse_from(args);
        match &mut parsed_cli.command {
            Command::Listen { config } => execute(config).await,
        }
    }
}
