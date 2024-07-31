use amqprs::{
    channel::{BasicPublishArguments, Channel},
    connection::Connection,
    BasicProperties,
};
use async_trait::async_trait;
use log::{debug, error, info};
use std::sync::Arc;
use tokio::{
    select,
    sync::{mpsc::UnboundedReceiver, Notify},
};

use transmitter_common::{
    config::ReconnectConfig, data::TransmitterMsgImpl, rabbitmq_client::RabbitmqClient,
};

use super::error::WatcherError;
use crate::common::rabbitmq::{ChannelControl, ConnectionControl, RabbitmqListenConfig};
use transmitter_common::data::{ProposalExecuted, TransmitterMsg};

pub(super) struct RabbitmqPublisher {
    config: RabbitmqListenConfig,
    op_status_receiver: UnboundedReceiver<ProposalExecuted>,
    buffered_op_status: Option<ProposalExecuted>,
    close_notify: Arc<Notify>,
    connection: Option<Connection>,
    channel: Option<Channel>,
}

impl RabbitmqPublisher {
    pub(super) fn new(
        config: RabbitmqListenConfig,
        propose_receiver: UnboundedReceiver<ProposalExecuted>,
    ) -> RabbitmqPublisher {
        RabbitmqPublisher {
            config,
            op_status_receiver: propose_receiver,
            buffered_op_status: None,
            close_notify: Arc::new(Notify::new()),
            connection: None,
            channel: None,
        }
    }

    pub(super) async fn publish_to_rabbitmq(&mut self) -> Result<(), WatcherError> {
        info!(
            "Rabbitmq messaging arguments are: exchange: {}, routing_key: {}",
            self.config.binding.exchange, self.config.binding.routing_key
        );
        self.init_connection().await?;
        let notify = self.close_notify.clone();
        loop {
            let proposal_executed = select! {
                _ = notify.notified() => {
                    self.init_connection().await?;
                    continue
                },
                proposal = self.propose_to_progress() => proposal
            };
            let Some(operation_status) = proposal_executed else {
                return Ok(());
            };
            self.publish_propose(operation_status).await;
        }
    }

    async fn publish_propose(&mut self, proposal: ProposalExecuted) {
        let transmitter_msg = TransmitterMsg::new(
            TransmitterMsgImpl::ProposalExecuted(proposal.clone()),
            proposal.need_check,
        );
        debug!("operation_status to be sent: {:?}", transmitter_msg);
        let Ok(json_data) = serde_json::to_vec(&transmitter_msg).map_err(|err| {
            error!(
                "Failed to encode operation_data message: {:?}, error: {}",
                transmitter_msg, err
            );
        }) else {
            return;
        };
        let args = BasicPublishArguments::from(&self.config.binding);
        let channel = self.channel.as_ref().expect("Expected rabbitmq channel to be set");
        let res = channel.basic_publish(BasicProperties::default(), json_data, args.clone()).await;
        let _ = res.map_err(|err| {
            self.buffered_op_status = Some(proposal);
            error!("Failed to publish operation_data message, error: {}", err);
        });
    }

    async fn propose_to_progress(&mut self) -> Option<ProposalExecuted> {
        if self.buffered_op_status.is_some() {
            self.buffered_op_status.take()
        } else {
            self.op_status_receiver.recv().await
        }
    }
}

#[async_trait]
impl RabbitmqClient for RabbitmqPublisher {
    type Error = WatcherError;
    async fn reconnect(&mut self) -> Result<(), WatcherError> {
        let conn_control = ConnectionControl::new(self.close_notify.clone());
        let conn = self.connect(&self.config.connect, conn_control).await?;
        let chann_control = ChannelControl::new(self.close_notify.clone());
        self.channel = Some(self.open_channel(&conn, chann_control).await?);
        self.connection = conn.into();
        Ok(())
    }

    fn reconnect_config(&self) -> &ReconnectConfig {
        &self.config.reconnect
    }
}
