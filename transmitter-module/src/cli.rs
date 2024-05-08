use clap::{Parser, Subcommand};

use super::{executor_app::ExecutorApp, listener_app::ListenerApp, watcher_app::WatcherApp};

#[derive(Subcommand)]
enum Command {
    #[command(
        about = "Starts listening to solana for new events to register them for further processing"
    )]
    Listener {
        #[arg(long, help = "Listener module config path")]
        config: String,
    },
    #[command(about = "Starts executing operation data to the solana photon messaging circuit")]
    Executor {
        #[arg(long, help = "Executor module config path")]
        config: String,
    },
    #[command(about = "Starts conducting operation data to the solana photon messaging circuit")]
    Watcher {
        #[arg(long, help = "Watcher config path")]
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
            Command::Listener { config } => ListenerApp::execute(config).await,
            Command::Executor { config } => ExecutorApp::execute(config).await,
            Command::Watcher { config } => WatcherApp::execute(config).await,
        }
    }
}
