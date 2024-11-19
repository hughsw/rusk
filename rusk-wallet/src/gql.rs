// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

//! The graphql endpoint can be queried with this helper struct.
//! The <node-url>/on/gaphql/query if queried with empty bytes returns the
//! graphql schema

use execution_core::transfer::Transaction;
use tokio::time::{sleep, Duration};

use crate::{Error, RuesHttpClient};
use serde::Deserialize;

/// GraphQL is a helper struct that aggregates all queries done
/// to the Dusk GraphQL database.
/// This helps avoid having helper structs and boilerplate code
/// mixed with the wallet logic.
#[derive(Clone)]
pub struct GraphQL {
    client: RuesHttpClient,
    status: fn(&str),
}

/// The tx_for_block returns a Vec<BlockTransaction> which contains
/// the execution-core transaction, its id hash and gas spent
pub struct BlockTransaction {
    /// The execution-core transaction struct obtained from GraphQL endpoint
    pub tx: Transaction,
    /// The hash of the transaction or the id of the transaction in string utf8
    pub id: String,
    /// Gas amount spent for the transaction
    pub gas_spent: u64,
}

#[derive(Deserialize)]
struct SpentTx {
    pub id: String,
    #[serde(default)]
    pub raw: String,
    pub err: Option<String>,
    #[serde(alias = "gasSpent", default)]
    pub gas_spent: f64,
}

#[derive(Deserialize)]
struct Block {
    pub transactions: Vec<SpentTx>,
}

#[derive(Deserialize)]
struct BlockResponse {
    pub block: Option<Block>,
}

#[derive(Deserialize)]
struct SpentTxResponse {
    pub tx: Option<SpentTx>,
}

/// Transaction status
#[derive(Debug)]
pub enum TxStatus {
    Ok,
    NotFound,
    Error(String),
}

impl GraphQL {
    /// Create a new GraphQL wallet client
    pub fn new<S: Into<String>>(url: S, status: fn(&str)) -> Self {
        Self {
            client: RuesHttpClient::new(url),
            status,
        }
    }

    /// Wait for a transaction to be confirmed (included in a block)
    pub async fn wait_for(&self, tx_id: &str) -> anyhow::Result<()> {
        loop {
            let status = self.tx_status(tx_id).await?;

            match status {
                TxStatus::Ok => break,
                TxStatus::Error(err) => return Err(Error::Transaction(err))?,
                TxStatus::NotFound => {
                    (self.status)(
                        "Waiting for tx to be included into a block...",
                    );
                    sleep(Duration::from_millis(1000)).await;
                }
            }
        }
        Ok(())
    }

    /// Obtain transaction status
    async fn tx_status(
        &self,
        tx_id: &str,
    ) -> anyhow::Result<TxStatus, GraphQLError> {
        let query =
            "query { tx(hash: \"####\") { id, err }}".replace("####", tx_id);
        let response = self.query(&query).await?;
        let response = serde_json::from_slice::<SpentTxResponse>(&response)?.tx;

        match response {
            Some(SpentTx { err: Some(err), .. }) => Ok(TxStatus::Error(err)),
            Some(_) => Ok(TxStatus::Ok),
            None => Ok(TxStatus::NotFound),
        }
    }

    /// Obtain transactions inside a block
    pub async fn txs_for_block(
        &self,
        block_height: u64,
    ) -> anyhow::Result<Vec<BlockTransaction>, GraphQLError> {
        let query = "query { block(height: ####) { transactions {id, raw, gasSpent, err}}}"
            .replace("####", block_height.to_string().as_str());

        let response = self.query(&query).await?;
        let response =
            serde_json::from_slice::<BlockResponse>(&response)?.block;
        let block = response.ok_or(GraphQLError::BlockInfo)?;
        let mut ret = vec![];

        for spent_tx in block.transactions {
            let tx_raw = hex::decode(&spent_tx.raw)
                .map_err(|_| GraphQLError::TxStatus)?;
            let ph_tx = Transaction::from_slice(&tx_raw).unwrap();

            ret.push(BlockTransaction {
                tx: ph_tx,
                id: spent_tx.id,
                gas_spent: spent_tx.gas_spent as u64,
            });
        }

        Ok(ret)
    }

    /// Sends an empty body to url to check if its available
    pub async fn check_connection(&self) -> Result<(), Error> {
        self.query("").await.map(|_| ())
    }
}

/// Errors generated from GraphQL
#[derive(Debug, thiserror::Error)]
pub enum GraphQLError {
    /// Generic errors
    #[error("Error fetching data from the node: {0}")]
    Generic(Error),
    /// Failed to fetch transaction status
    #[error("Failed to obtain transaction status")]
    TxStatus,
    #[error("Failed to obtain block info")]
    /// Failed to obtain block info
    BlockInfo,
}

impl From<Error> for GraphQLError {
    fn from(e: Error) -> Self {
        Self::Generic(e)
    }
}

impl From<serde_json::Error> for GraphQLError {
    fn from(e: serde_json::Error) -> Self {
        Self::Generic(e.into())
    }
}

impl GraphQL {
    /// Call the graphql endpoint of a node
    pub async fn query(&self, query: &str) -> Result<Vec<u8>, Error> {
        self.client
            .call("graphql", None, "query", query.as_bytes())
            .await
    }
}

#[ignore = "Leave it here just for manual tests"]
#[tokio::test]
async fn test() -> Result<(), Box<dyn std::error::Error>> {
    let gql = GraphQL {
        status: |s| {
            println!("{s}");
        },
        client: RuesHttpClient::new(
            "http://testnet.nodes.dusk.network:9500/graphql",
        ),
    };
    let _ = gql
        .tx_status(
            "dbc5a2c949516ecfb418406909d195c3cc267b46bd966a3ca9d66d2e13c47003",
        )
        .await?;
    let block_txs = gql.txs_for_block(90).await?;
    block_txs.into_iter().for_each(|tx_block| {
        let tx = tx_block.tx;
        let chain_txid = tx_block.id;
        let hash = tx.hash();
        let tx_id = hex::encode(hash.to_bytes());
        assert_eq!(chain_txid, tx_id);
        println!("txid: {tx_id}");
    });
    Ok(())
}

#[tokio::test]
async fn deser() -> Result<(), Box<dyn std::error::Error>> {
    let block_not_found = r#"{"block":null}"#;
    serde_json::from_str::<BlockResponse>(block_not_found).unwrap();

    let block_without_tx = r#"{"block":{"transactions":[]}}"#;
    serde_json::from_str::<BlockResponse>(block_without_tx).unwrap();

    let block_with_tx = r#"{"block":{"transactions":[{"id":"88e6804989cc2f3fd5bf94dcd39a4e7b7da9a1114d9b8bf4e0515264bc81c50f"}]}}"#;
    serde_json::from_str::<BlockResponse>(block_with_tx).unwrap();

    Ok(())
}