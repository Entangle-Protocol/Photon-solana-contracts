use solana_sdk::signer::Signer;
use std::sync::Arc;
use tokio::{select, sync::mpsc::unbounded_channel};

use super::{
    config::ExecutorConfig, error::ExecutorError, operation_manager::OperationStateMng,
    rabbitmq_consumer::RabbitmqConsumer, solana_transactor::SolanaTransactor,
    tx_builder_exec::ExecOpTxBuilder, tx_builder_load::LoadOpTxBuilder,
    tx_builder_sign::SignOpTxBuilder,
};

pub(crate) struct ExecutorApp {
    rabbitmq_consumer: RabbitmqConsumer,
    operation_mng: OperationStateMng,
    load_tx_builder: LoadOpTxBuilder,
    sign_tx_builder: SignOpTxBuilder,
    exec_tx_builder: ExecOpTxBuilder,
    solana_transactor: SolanaTransactor,
}

impl ExecutorApp {
    pub(crate) async fn execute(config_path: &str) {
        let Ok(config) = ExecutorConfig::try_from_path(config_path) else {
            return;
        };
        let Ok(app) = ExecutorApp::try_new(config) else {
            return;
        };
        app.execute_impl().await;
    }

    pub async fn execute_impl(self) {
        select! {
            _ = self.rabbitmq_consumer.execute() => {},
            _ = self.operation_mng.execute() => {},
            _ = self.load_tx_builder.execute() => {},
            _ = self.sign_tx_builder.execute() => {},
            _ = self.exec_tx_builder.execute() => {},
            _ = self.solana_transactor.execute() => {}
        };
    }

    fn try_new(config: ExecutorConfig) -> Result<ExecutorApp, ExecutorError> {
        let (op_data_sender, op_data_receiver) = unbounded_channel();
        let (transaction_sender, transaction_receiver) = unbounded_channel();
        let (load_op_builder_sender, load_op_builder_receiver) = unbounded_channel();
        let (sign_op_builder_sender, sign_op_builder_receiver) = unbounded_channel();
        let (exec_op_builder_sender, exec_op_builder_receiver) = unbounded_channel();
        let (tx_status_sender, tx_status_receiver) = unbounded_channel();

        let payer = Arc::new(config.solana.payer);

        Ok(ExecutorApp {
            rabbitmq_consumer: RabbitmqConsumer::new(config.rabbitmq, op_data_sender),
            operation_mng: OperationStateMng::new(
                op_data_receiver,
                tx_status_receiver,
                load_op_builder_sender,
                sign_op_builder_sender,
                exec_op_builder_sender,
            ),
            load_tx_builder: LoadOpTxBuilder::new(
                payer.pubkey(),
                load_op_builder_receiver,
                transaction_sender.clone(),
                tx_status_sender.clone(),
            ),
            sign_tx_builder: SignOpTxBuilder::new(
                payer.pubkey(),
                sign_op_builder_receiver,
                transaction_sender.clone(),
                tx_status_sender.clone(),
            ),
            exec_tx_builder: ExecOpTxBuilder::try_new(
                config.extensions,
                payer.pubkey(),
                config.solana.client.clone(),
                exec_op_builder_receiver,
                transaction_sender.clone(),
                tx_status_sender.clone(),
            )?,
            solana_transactor: SolanaTransactor::new(
                config.solana.client,
                payer.clone(),
                transaction_receiver,
                tx_status_sender,
            ),
        })
    }
}
