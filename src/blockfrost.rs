use std::collections::BTreeMap;

use amaru_kernel::{MemoizedTransactionOutput, TransactionInput, cbor};
use anyhow::{Context, Result, anyhow};
use blockfrost::BlockfrostAPI;
use futures::future::try_join_all;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockfrostConfig {
    pub key: String,
}

pub struct Blockfrost {
    api: BlockfrostAPI,
}

impl Blockfrost {
    pub fn new(config: &BlockfrostConfig) -> Self {
        Self {
            api: BlockfrostAPI::new(&config.key, Default::default()),
        }
    }

    pub async fn get_utxos(
        &self,
        inputs: &[TransactionInput],
    ) -> Result<BTreeMap<TransactionInput, MemoizedTransactionOutput>> {
        let futures = inputs.iter().map(|input| self.fetch_utxo(input));

        let results = try_join_all(futures)
            .await
            .context("Failed to fetch UTxOs from Blockfrost")?;

        Ok(results.into_iter().collect())
    }

    async fn fetch_utxo(
        &self,
        input: &TransactionInput,
    ) -> Result<(TransactionInput, MemoizedTransactionOutput)> {
        let tx_hash = hex::encode(input.transaction_id);

        let response = self
            .api
            .transactions_cbor(&tx_hash)
            .await
            .with_context(|| format!("Failed to fetch transaction {}", tx_hash))?;

        let cbor_bytes = hex::decode(&response.cbor).with_context(|| {
            format!(
                "Invalid CBOR hex from Blockfrost for tranasction {}",
                tx_hash
            )
        })?;

        let transaction: amaru_kernel::MintedTx<'_> = cbor::decode(&cbor_bytes)
            .with_context(|| format!("Failed to decode transaction CBOR for {}", tx_hash))?;

        let output = transaction
            .transaction_body
            .outputs
            .get(input.index as usize)
            .with_context(|| {
                format!(
                    "Invalid output index {} for transaction {}. Transaction has {} output(s)",
                    input.index,
                    tx_hash,
                    transaction.transaction_body.outputs.len()
                )
            })?
            .clone();

        let memoized_output = MemoizedTransactionOutput::try_from(output)
            .map_err(|e| anyhow!("Failed to convert output to memoized format: {}", e))?;

        Ok((input.clone(), memoized_output))
    }
}
