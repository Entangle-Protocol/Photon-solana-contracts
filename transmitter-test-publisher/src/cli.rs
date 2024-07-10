use crate::publish;
use clap::{Parser, Subcommand};

#[derive(Clone)]
pub(crate) enum Operation {
    InitOwnedCounter,
    Increment(u64),
    IncrementOwned(u64),
    CodeBased(Vec<u8>),
    AddProtocol,
}

#[derive(Clone)]
pub(crate) struct Workaround(Vec<u8>);

#[derive(Subcommand)]
pub(crate) enum Command {
    #[command(about = "Publish the increment operation to be called")]
    Increment {
        #[arg(long, short, help = "Config path")]
        config: String,
        #[arg(long, short, help = "Component")]
        value: u64,
    },
    #[command(about = "Publish the increment derived counter operation")]
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
    #[command(about = "Publish the init owned counter operation to be called")]
    InitOwnedCounter {
        #[arg(long, short, help = "Config path")]
        config: String,
    },
    #[command(about = "Publish the hexademical code based operation to be called in a proper way")]
    CodeBased {
        #[arg(value_parser=parse_hex, help = "Hexademical code")]
        code: Workaround,
        #[arg(long, short, help = "Config path")]
        config: String,
    },
    AddProtocol {
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
            Command::Increment { config, value } => {
                publish(config, &Operation::Increment(*value), 1).await
            }
            Command::IncrementOwnedCounter {
                config,
                value,
                times,
            } => publish(config, &Operation::IncrementOwned(*value), *times).await,
            Command::InitOwnedCounter { config } => {
                publish(config, &Operation::InitOwnedCounter, 1).await
            }
            Command::CodeBased { config, code } => {
                publish(config, &Operation::CodeBased(code.0.clone()), 1).await
            }
            Command::AddProtocol { config } => publish(config, &Operation::AddProtocol, 1).await,
        }
    }
}

pub fn parse_hex(value: &str) -> Result<Workaround, String> {
    let bytes = hex::decode(value).map_err(|err| err.to_string())?;
    Ok(Workaround(bytes))
}
