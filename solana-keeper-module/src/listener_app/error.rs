use derive_more::Display;

#[derive(Debug, Display)]
pub(super) enum ListenError {
    #[display(fmt = "Config error")]
    Config,
    #[display(fmt = "Channel error")]
    ProposeEventChannel,
    #[display(fmt = "Solana client error")]
    SolanaClient,
    #[display(fmt = "Solana parse logs error")]
    SolanaParseLogs,
    #[display(fmt = "Rabbitmq client error")]
    Rabbitmq,
}
