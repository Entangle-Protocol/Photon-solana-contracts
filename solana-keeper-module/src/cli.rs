use clap::{Parser, Subcommand};

use super::listener_app::ListenerApp;

#[derive(Subcommand)]
enum Command {
    #[command(
        about = "Starts listening to solana for new events to register them for further processing"
    )]
    Listen {
        #[arg(long, help = "Keeper module config path")]
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
            Command::Listen { config } => ListenerApp::listen(config).await,
        }
    }
}
