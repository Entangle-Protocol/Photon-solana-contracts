use log::{debug, error, warn};
use mongodb::{
    bson::{doc, Bson, Document},
    options::{ClientOptions, Credential, FindOneOptions, ServerApi, ServerApiVersion},
    Client,
};
use solana_client::{
    nonblocking::rpc_client::RpcClient, rpc_client::GetConfirmedSignaturesForAddress2Config,
    rpc_response::RpcConfirmedTransactionStatusWithSignature,
};
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use solana_transaction_status::UiTransactionEncoding;
use std::{collections::VecDeque, str::FromStr};
use tokio::sync::mpsc::UnboundedSender;

use transmitter_common::mongodb::{mdb_solana_chain_id, MongodbConfig, MDB_LAST_BLOCK_COLLECTION};

use crate::common::{
    config::SolanaClientConfig,
    solana_logs::{solana_event_listener::LogsBunch, EventListenerError},
};

pub(super) struct SolanaRetroReader {
    mongodb_config: MongodbConfig,
    logs_sender: UnboundedSender<LogsBunch>,
}

impl SolanaRetroReader {
    pub(super) fn new(
        mongodb_config: MongodbConfig,
        logs_sender: UnboundedSender<LogsBunch>,
    ) -> SolanaRetroReader {
        SolanaRetroReader {
            mongodb_config,
            logs_sender,
        }
    }

    pub(super) async fn read_events_backward(
        &self,
        solana_config: &SolanaClientConfig,
        mongodb_config: &MongodbConfig,
    ) -> Result<(), EventListenerError> {
        let Ok(Some(tx_start_from)) = self.get_last_processed_block(mongodb_config).await else {
            debug!("No latest_processed_block found, skip retrospective reading");
            return Ok(());
        };
        debug!("Found latest_processed_block, start retrospective reading from: {}", tx_start_from);
        let client =
            RpcClient::new_with_commitment(solana_config.rpc_url.clone(), solana_config.commitment);
        let until = Some(Signature::from_str(&tx_start_from).map_err(|err| {
            error!("Failed to decode tx_start_from: {}", err);
            EventListenerError::SolanaClient
        })?);

        let mut before = None;
        let mut log_bunches = VecDeque::new();
        loop {
            let signatures_backward =
                Self::get_signatures_chunk(&photon::ID, solana_config, &client, until, before)
                    .await?;

            if signatures_backward.is_empty() {
                break;
            }

            Self::process_signatures(&client, &mut before, &mut log_bunches, signatures_backward)
                .await;
        }
        for logs_bunch in log_bunches {
            self.logs_sender.send(logs_bunch).expect("Expected logs_bunch to be sent");
        }
        Ok(())
    }

    async fn process_signatures(
        client: &RpcClient,
        before: &mut Option<Signature>,
        log_bunches: &mut VecDeque<LogsBunch>,
        signatures_with_meta: Vec<RpcConfirmedTransactionStatusWithSignature>,
    ) {
        for signature_with_meta in signatures_with_meta {
            _ = Self::process_signature(client, before, log_bunches, signature_with_meta).await;
        }
    }

    async fn process_signature(
        client: &RpcClient,
        before: &mut Option<Signature>,
        log_bunches: &mut VecDeque<LogsBunch>,
        signature_with_meta: RpcConfirmedTransactionStatusWithSignature,
    ) -> Result<(), ()> {
        let signature = &Signature::from_str(&signature_with_meta.signature)
            .map_err(|err| error!("Failed to parse signature: {}", err))?;
        before.replace(*signature);
        let transaction = client
            .get_transaction(signature, UiTransactionEncoding::Json)
            .await
            .map_err(|err| error!("Failed to get transaction by signature: {}", err))?;

        let logs = transaction
            .transaction
            .meta
            .map(|meta| <Option<Vec<String>>>::from(meta.log_messages))
            .ok_or(())?
            .ok_or(())?;

        if logs.is_empty() {
            return Ok(());
        }

        log_bunches.push_front(LogsBunch {
            tx_signature: signature_with_meta.signature,
            slot: transaction.slot,
            logs,
        });
        Ok(())
    }

    async fn get_signatures_chunk(
        program_id: &Pubkey,
        solana_config: &SolanaClientConfig,
        client: &RpcClient,
        until: Option<Signature>,
        before: Option<Signature>,
    ) -> Result<Vec<RpcConfirmedTransactionStatusWithSignature>, EventListenerError> {
        let args = GetConfirmedSignaturesForAddress2Config {
            before,
            until,
            limit: None,
            commitment: Some(solana_config.commitment),
        };

        let signatures_backward = client
            .get_signatures_for_address_with_config(program_id, args)
            .await
            .map_err(|err| {
                error!("Failed to get signatures for address: {}", err);
                EventListenerError::SolanaClient
            })?;
        Ok(signatures_backward)
    }

    async fn get_last_processed_block(
        &self,
        mongodb_config: &MongodbConfig,
    ) -> Result<Option<String>, EventListenerError> {
        let mut client_options =
            ClientOptions::parse_async(&mongodb_config.uri).await.map_err(|err| {
                error!("Failed to parse mongodb uri: {}", err);
                EventListenerError::from(err)
            })?;
        let server_api = ServerApi::builder().version(ServerApiVersion::V1).build();
        client_options.server_api = Some(server_api);
        client_options.credential = Some(
            Credential::builder()
                .username(mongodb_config.user.clone())
                .password(mongodb_config.password.clone())
                .build(),
        );
        let client = Client::with_options(client_options).map_err(|err| {
            error!("Failed to build mondodb client: {}", err);
            EventListenerError::from(err)
        })?;
        let db = client.database(&mongodb_config.db);
        let collection = db.collection::<Document>(MDB_LAST_BLOCK_COLLECTION);

        let last_block: &str = &self.mongodb_config.key;
        let chain_id = mdb_solana_chain_id();
        let doc = collection
            .find_one(doc! { "direction": "from", "chain": chain_id }, FindOneOptions::default())
            .await
            .map_err(|err| {
                error!("Failed to request {}: {}", last_block, err);
                EventListenerError::from(err)
            })?;
        let Some(doc) = doc else {
            warn!("{}: not found", last_block);
            return Ok(None);
        };
        let Some(Bson::String(tx_signature)) = doc.get(last_block).cloned() else {
            warn!("Failed to get {} from document", last_block);
            return Ok(None);
        };
        debug!("doc: {}", tx_signature);
        Ok(Some(tx_signature))
    }
}
