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
    sync::{mpsc::UnboundedReceiver, Mutex, Notify},
};

use transmitter_common::{
    config::ReconnectConfig,
    data::{Propose, TransmitterMsg, TransmitterMsgImpl},
    rabbitmq_client::RabbitmqClient,
};

use super::error::ListenError;
use crate::common::rabbitmq::{ChannelControl, ConnectionControl, RabbitmqPublishConfig};

pub(super) struct RabbitmqPublisher {
    config: RabbitmqPublishConfig,
    propose_receiver: UnboundedReceiver<Propose>,
    buffered_propose: Option<Propose>,
    close_notify: Arc<Notify>,
    connection: Mutex<Option<(Connection, Channel)>>,
}

impl RabbitmqPublisher {
    pub(super) fn new(
        config: RabbitmqPublishConfig,
        propose_receiver: UnboundedReceiver<Propose>,
    ) -> RabbitmqPublisher {
        RabbitmqPublisher {
            config,
            propose_receiver,
            buffered_propose: None,
            close_notify: Arc::new(Notify::new()),
            connection: Mutex::new(None),
        }
    }

    pub(super) async fn publish_to_rabbitmq(&mut self) -> Result<(), ListenError> {
        info!(
            "Rabbitmq messaging arguments are: exchange: {}, routing_key: {}",
            self.config.binding.exchange, self.config.binding.routing_key
        );
        self.init_connection().await?;
        let notify = self.close_notify.clone();
        loop {
            let propose = select! {
                _ = notify.notified() => {
                    self.init_connection().await?;
                    continue
                },
                proposal = self.propose_to_progress() => proposal
            };
            let Some(propose) = propose else {
                return Ok(());
            };
            self.publish_propose(propose).await;
        }
    }

    async fn publish_propose(&mut self, propose: Propose) {
        let transmitter_msg =
            TransmitterMsg::new(TransmitterMsgImpl::Propose(propose.clone()), propose.need_check);
        debug!("operation_data to be sent: {:?}", transmitter_msg);
        let Ok(json_data) = serde_json::to_vec(&transmitter_msg).map_err(|err| {
            error!(
                "Failed to encode operation_data message: {:?}, error: {}",
                transmitter_msg, err
            );
        }) else {
            return;
        };
        let args = BasicPublishArguments::from(&self.config.binding);
        let guard = self.connection.lock().await;
        let (_, channel) = guard.as_ref().expect("Expected rabbitmq channel to be set");
        let res = channel.basic_publish(BasicProperties::default(), json_data, args.clone()).await;
        let _ = res.map_err(|err| {
            self.buffered_propose = Some(propose);
            error!("Failed to publish operation_data message, error: {}", err);
        });
    }

    async fn propose_to_progress(&mut self) -> Option<Propose> {
        if self.buffered_propose.is_some() {
            self.buffered_propose.take()
        } else {
            self.propose_receiver.recv().await
        }
    }
}

#[async_trait]
impl RabbitmqClient for RabbitmqPublisher {
    type Error = ListenError;
    async fn reconnect(&self) -> Result<(), ListenError> {
        let conn_control = ConnectionControl::new(self.close_notify.clone());
        let conn = self.connect(&self.config.connect, conn_control).await?;
        let chann_control = ChannelControl::new(self.close_notify.clone());
        let channel = self.open_channel(&conn, chann_control).await?;
        self.connection.lock().await.replace((conn, channel));
        Ok(())
    }

    fn reconnect_config(&self) -> &ReconnectConfig {
        &self.config.reconnect
    }
}
