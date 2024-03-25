use crate::publish;
use clap::{Parser, Subcommand};

#[derive(Clone)]
pub(crate) enum Operation {
    InitOwnedCounter,
    Increment(u64),
}

#[derive(Subcommand)]
pub(crate) enum Command {
    #[command(about = "Publish an operation to be called")]
    IncrementOwnedCounter {
        #[arg(long, short, help = "Config path")]
        config: String,
        #[arg(long, short, help = "Component")]
        value: u64,
        #[arg(
            long,
            short,
            help = "If it's needed to repeat operation",
            default_value_t = 1
        )]
        times: u64,
    },
    #[command(about = "Publish an operation to be called")]
    InitOwnedCounter {
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
            Command::IncrementOwnedCounter {
                config,
                value,
                times,
            } => publish(config, &Operation::Increment(*value), *times).await,
            Command::InitOwnedCounter { config } => {
                publish(config, &Operation::InitOwnedCounter, 1).await
            }
        }
    }
}
