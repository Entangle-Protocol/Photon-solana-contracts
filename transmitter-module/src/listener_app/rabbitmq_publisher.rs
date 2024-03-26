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
    data::{KeeperMsg, KeeperMsgImpl, OperationData},
    rabbitmq_client::{RabbitmqClient, RabbitmqReconnectConfig},
};

use super::{config::RabbitmqConfig, error::ListenError};
use crate::common::rabbitmq::{ChannelControl, ConnectionControl};

pub(super) struct RabbitmqPublisher {
    config: RabbitmqConfig,
    op_data_receiver: UnboundedReceiver<OperationData>,
    buffered_op_data: Option<OperationData>,
    close_notify: Arc<Notify>,
    connection: Option<Connection>,
    channel: Option<Channel>,
}

impl RabbitmqPublisher {
    pub(super) fn new(
        config: RabbitmqConfig,
        op_data_receiver: UnboundedReceiver<OperationData>,
    ) -> RabbitmqPublisher {
        RabbitmqPublisher {
            config,
            op_data_receiver,
            buffered_op_data: None,
            close_notify: Arc::new(Notify::new()),
            connection: None,
            channel: None,
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
            let op_data = select! {
                _ = notify.notified() => {
                    self.init_connection().await?;
                    continue
                },
                op_data = self.op_data_to_progress() => op_data
            };
            let Some(op_data) = op_data else {
                return Ok(());
            };
            self.publish_op_data(op_data).await;
        }
    }

    async fn publish_op_data(&mut self, operation_data: OperationData) {
        let keeper_msg = KeeperMsg::V1(KeeperMsgImpl::OperationData(operation_data.clone()));
        debug!("operation_data to be sent: {:?}", keeper_msg);
        let Ok(json_data) = serde_json::to_vec(&keeper_msg).map_err(|err| {
            error!("Failed to encode operation_data message: {:?}, error: {}", keeper_msg, err);
        }) else {
            return;
        };
        let args = BasicPublishArguments::from(&self.config.binding);
        let channel = self.channel.as_ref().expect("Expected rabbitmq channel to be set");
        let res = channel.basic_publish(BasicProperties::default(), json_data, args.clone()).await;
        let _ = res.map_err(|err| {
            self.buffered_op_data = Some(operation_data);
            error!("Failed to publish operation_data message, error: {}", err);
        });
    }

    async fn op_data_to_progress(&mut self) -> Option<OperationData> {
        if self.buffered_op_data.is_some() {
            self.buffered_op_data.take()
        } else {
            self.op_data_receiver.recv().await
        }
    }
}

#[async_trait]
impl RabbitmqClient for RabbitmqPublisher {
    type Error = ListenError;
    async fn reconnect(&mut self) -> Result<(), ListenError> {
        let conn_control = ConnectionControl::new(self.close_notify.clone());
        let conn = self.connect(&self.config.connect, conn_control).await?;
        let chann_control = ChannelControl::new(self.close_notify.clone());
        self.channel = Some(self.open_channel(&conn, chann_control).await?);
        self.connection = conn.into();
        Ok(())
    }

    fn reconnect_config(&self) -> &RabbitmqReconnectConfig {
        &self.config.reconnect
    }
}
