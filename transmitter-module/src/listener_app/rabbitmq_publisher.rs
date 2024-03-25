use amqprs::{channel::BasicPublishArguments, BasicProperties};
use async_trait::async_trait;
use log::{debug, error, info};
use tokio::sync::mpsc::UnboundedReceiver;

use transmitter_common::{
    data::{KeeperMsg, KeeperMsgImpl, OperationData},
    rabbitmq_client::RabbitmqClient,
};

use super::{config::RabbitmqConfig, error::ListenError};
use crate::common::rabbitmq::{ChannelControl, ConnectionControl};

pub(super) struct RabbitmqPublisher {
    config: RabbitmqConfig,
    op_data_receiver: UnboundedReceiver<OperationData>,
}

impl RabbitmqPublisher {
    pub(super) fn new(
        config: RabbitmqConfig,
        op_data_receiver: UnboundedReceiver<OperationData>,
    ) -> RabbitmqPublisher {
        RabbitmqPublisher {
            config,
            op_data_receiver,
        }
    }

    pub(super) async fn publish_operation_data(&mut self) -> Result<(), ListenError> {
        let connection = self.connect(&self.config.connect).await?;
        let channel = self.open_channel(&connection).await?;

        info!(
            "Rabbitmq messaging arguments are: exchange: {}, routing_key: {}",
            self.config.binding.exchange, self.config.binding.routing_key
        );
        while let Some(operation_data) = self.op_data_receiver.recv().await {
            let msg = KeeperMsg::V1(KeeperMsgImpl::OperationData(operation_data));
            debug!("operation_data to be sent: {:?}", msg);
            let json_data = match serde_json::to_vec(&msg) {
                Ok(json_data) => json_data,
                Err(err) => {
                    error!("Failed to encode operation_data message: {:?}, error: {}", msg, err);
                    continue;
                }
            };
            let args = BasicPublishArguments::from(&self.config.binding);
            if let Err(err) =
                channel.basic_publish(BasicProperties::default(), json_data, args.clone()).await
            {
                error!("Failed to publish operation_data message, error: {}", err);
            }
        }
        info!("Events handling stopped");
        Ok(())
    }
}

#[async_trait]
impl RabbitmqClient for RabbitmqPublisher {
    type ConnCb = ConnectionControl;
    type ChanCb = ChannelControl;
    type Error = ListenError;
}
