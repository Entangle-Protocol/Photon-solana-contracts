use async_signal::{Signal, Signals};
use futures_util::StreamExt;
use log::{error, info};
use solana_sdk::{commitment_config::CommitmentConfig, signer::Signer};
use solana_transactor::{RpcPool, SolanaTransactor};
use std::io;
use tokio::{
    select,
    sync::mpsc::{unbounded_channel, UnboundedSender},
};

use super::{
    alt_operation_manager::AltOperationManager, config::ExecutorConfig, error::ExecutorError,
    last_block_updater::LastBlockUpdater, rabbitmq_consumer::RabbitmqConsumer, ServiceCmd,
};

pub(crate) struct ExecutorApp {
    rabbitmq_consumer: RabbitmqConsumer,
    operation_mng: AltOperationManager,
    service_sender: UnboundedSender<ServiceCmd>,
    last_block_updater: LastBlockUpdater,
}

impl ExecutorApp {
    pub(crate) async fn execute(config_path: &str) {
        info!("Application restarted {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
        let Ok(config) = ExecutorConfig::try_from_path(config_path) else {
            return;
        };
        let Ok(app) = ExecutorApp::try_new(config).await else {
            return;
        };
        app.execute_impl(config_path).await;
    }

    pub async fn execute_impl(self, config_path: &str) {
        select! {
            _ = self.rabbitmq_consumer.execute() => {},
            _ = self.operation_mng.execute() => {},
            _ = self.last_block_updater.execute() => {},
            _ = Self::listen_to_signals(config_path, self.service_sender.clone()) => {}
        };
    }

    async fn try_new(config: ExecutorConfig) -> Result<ExecutorApp, ExecutorError> {
        let (op_data_sender, op_data_receiver) = unbounded_channel();
        let (service_sender, service_receiver) = unbounded_channel();
        let (last_block_sender, last_block_receiver) = unbounded_channel();
        let executor = config.solana.payer.pubkey();
        log::info!("Total read RPCs: {}", config.solana.client.read_rpcs.len());
        log::info!("Total write RPCs: {}", config.solana.client.write_rpcs.len());
        let transactor = SolanaTransactor::start(RpcPool::new(
            &config.solana.client.read_rpcs,
            &config.solana.client.write_rpcs,
        )?)
        .await?;
        let balance = transactor
            .rpc_pool()
            .with_read_rpc_loop(
                |rpc| async move { rpc.get_balance(&executor).await },
                CommitmentConfig::confirmed(),
            )
            .await;
        log::info!("Executor: {}, balance: {}", executor, balance);
        Ok(ExecutorApp {
            rabbitmq_consumer: RabbitmqConsumer::new(config.rabbitmq, op_data_sender),
            operation_mng: AltOperationManager::new(
                op_data_receiver,
                last_block_sender,
                transactor,
                config.extensions,
                config.solana.payer,
                service_receiver,
            ),
            last_block_updater: LastBlockUpdater::try_new(config.mongodb, last_block_receiver)
                .await?,
            service_sender,
        })
    }

    async fn listen_to_signals(
        config_path: &str,
        service_sender: UnboundedSender<ServiceCmd>,
    ) -> Result<(), io::Error> {
        let mut signals = Signals::new([Signal::Hup]).map_err(|err| {
            error!("Failed to create signals object: {}", err);
            err
        })?;

        while let Some(Ok(signal @ Signal::Hup)) = signals.next().await {
            info!("Received signal is to be processed: {:?}", signal);
            let Ok(config) = ExecutorConfig::try_from_path(config_path) else {
                continue;
            };
            service_sender
                .send(ServiceCmd::UpdateExtensions(config.extensions))
                .expect("Expected service_cmd to be sent");
        }
        Ok(())
    }
}
