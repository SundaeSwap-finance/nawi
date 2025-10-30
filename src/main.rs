use std::{borrow::Cow, collections::BTreeMap, ops::Deref, path::PathBuf, str::FromStr};

use amaru_kernel::{
    MemoizedDatum, MemoizedTransactionOutput, MintedTx, OriginalHash, PlutusData, Redeemer,
    ScriptPurpose, TransactionInput, cbor, network::NetworkName, normalize_redeemers, to_cbor,
};
use amaru_plutus::{
    ToPlutusData,
    script_context::{ScriptContextV1, TxInfoV1, TxInfoV3, v3},
};
use anyhow::{Context, Result, anyhow, bail};
use clap::{ArgGroup, Parser, ValueEnum};
use figment::{
    Figment,
    providers::{Env, Format, Toml},
};

use crate::{
    blockfrost::{Blockfrost, BlockfrostConfig},
    formatter::ReadableFormatter,
};

mod blockfrost;
mod formatter;

#[derive(ValueEnum, Default, Clone, Copy, Debug)]
#[value(rename_all = "verbatim")]
pub enum PlutusVersion {
    PlutusV1,
    PlutusV2,
    #[default]
    PlutusV3,
}

#[derive(ValueEnum, Default, Clone, Copy, Debug)]
#[value(rename_all = "kebab-case")]
pub enum OutputFormat {
    Pretty,
    Cbor,
    #[default]
    Both,
}

#[derive(Debug, PartialEq, Clone, Copy)]
struct NetworkNameAdapter(NetworkName);

impl FromStr for NetworkNameAdapter {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mainnet" => Ok(Self(NetworkName::Mainnet)),
            "preprod" => Ok(Self(NetworkName::Preprod)),
            "preview" => Ok(Self(NetworkName::Preview)),
            _ if s.starts_with("testnet:") => {
                let magic = s
                    .strip_prefix("testnet:")
                    .and_then(|n| n.parse::<u32>().ok())
                    .ok_or_else(|| anyhow!("Invalid testnet format, expected testnet:<magic>"))?;
                Ok(Self(NetworkName::Testnet(magic)))
            }
            _ => Err(anyhow!(
                "Unknown network: {s}. Valid options: mainnet, preprod, preview, testnet:<magic>"
            )),
        }
    }
}

impl Deref for NetworkNameAdapter {
    type Target = NetworkName;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<NetworkNameAdapter> for NetworkName {
    fn from(value: NetworkNameAdapter) -> Self {
        value.0
    }
}

/// 👁️  Nawi: The eye of Amaru.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(group(
    ArgGroup::new("input")
        .required(true)
        .args(&["tx_file", "bytes"])
))]
struct Args {
    /// Path to the transaction file (e.g. path/to/tx.cbor)
    #[arg(short, long, value_name = "FILE")]
    tx_file: Option<PathBuf>,

    /// Hex-encoded bytes of the transaction
    #[arg(short, long, value_name = "HEX")]
    bytes: Option<String>,

    /// The index of the redeemer for which you want to construct the ScriptContext
    #[arg(short, long, value_name = "INDEX")]
    redeemer: u8,

    /// Network to use for resolving UTxOs
    #[arg(short, long, default_value = "mainnet", value_name = "NETWORK")]
    network: NetworkNameAdapter,

    /// Plutus language version
    #[arg(short, long, default_value = "PlutusV3", value_name = "VERSION")]
    plutus_version: PlutusVersion,

    /// Slot number of the transaction
    #[arg(short, long, value_name = "SLOT")]
    slot: u64,

    /// Output format of the ScriptContext
    #[arg(short, long, default_value = "both", value_name = "FORMAT")]
    output: OutputFormat,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let config = load_config()?;
    let blockfrost = Blockfrost::new(&config);

    let tx_bytes = load_transaction_bytes(&args)?;
    let transaction = decode_transaction(&tx_bytes)?;

    let all_inputs = collect_all_inputs(&transaction);
    let utxos = blockfrost.get_utxos(&all_inputs).await?;

    let redeemers = get_redeemers(&transaction)?;
    let redeemer = redeemers.get(args.redeemer as usize).ok_or_else(|| {
        anyhow!(
            "Invalid redeemer index {}. Transaction has {} redeemer(s)",
            args.redeemer,
            redeemers.len()
        )
    })?;

    let (pretty_context, plutus_data) = build_script_context(
        args.plutus_version,
        &transaction,
        &utxos,
        redeemer,
        args.network,
        args.slot,
    )?;

    match args.output {
        OutputFormat::Pretty => println!("{}", pretty_context),
        OutputFormat::Cbor => print_script_context(&plutus_data),
        OutputFormat::Both => {
            println!("{}", pretty_context);
            print_script_context(&plutus_data);
        }
    };

    Ok(())
}

fn load_config() -> Result<BlockfrostConfig> {
    Figment::new()
        .merge(Toml::file("nawi.toml"))
        .merge(Env::prefixed("BLOCKFROST_"))
        .extract()
        .context("Failed to load configuration. Ensure BLOCKFROST_KEY is set or nawi.toml exists")
}

fn load_transaction_bytes(args: &Args) -> Result<Vec<u8>> {
    match (&args.tx_file, &args.bytes) {
        (Some(path), _) => std::fs::read(path)
            .with_context(|| format!("Failed to read transaction file: {}", path.display())),
        (None, Some(hex_str)) => hex::decode(hex_str.trim()).context(
            "Failed to decode hex string. Ensure it contains valid hexadecimal characters",
        ),
        (None, None) => Err(anyhow!(
            "No input provided. Use either --tx-file or --bytes"
        )),
    }
}

fn decode_transaction(tx_bytes: &[u8]) -> Result<MintedTx<'_>> {
    cbor::decode(tx_bytes).context(
        "Failed to decode transaction. Ensure the CBOR data is a valid Cardano transaction",
    )
}

fn collect_all_inputs(transaction: &MintedTx) -> Vec<TransactionInput> {
    let regular_inputs = transaction.transaction_body.inputs.deref().as_slice();
    let ref_inputs = transaction
        .transaction_body
        .reference_inputs
        .as_deref()
        .map(|set| set.as_slice())
        .unwrap_or_default();

    [regular_inputs, ref_inputs].concat()
}

fn get_redeemers<'a>(transaction: &'a MintedTx<'_>) -> Result<Vec<Cow<'a, Redeemer>>> {
    let redeemers = transaction
        .transaction_witness_set
        .redeemer
        .as_ref()
        .ok_or_else(|| anyhow!("Transaction contains no redeemers"))?;

    Ok(normalize_redeemers(redeemers.deref()))
}

fn extract_datum(
    transaction: &MintedTx,
    utxos: &BTreeMap<TransactionInput, MemoizedTransactionOutput>,
    redeemer: &Redeemer,
) -> Result<Option<PlutusData>> {
    if !matches!(redeemer.tag, ScriptPurpose::Spend) {
        return Ok(None);
    }

    let input = transaction
        .transaction_body
        .inputs
        .get(redeemer.index as usize)
        .context("Invalid redeemer index for spending input")?;

    let utxo = utxos
        .get(input)
        .context("Missing UTxO for spending input")?;

    let datum = match &utxo.datum {
        MemoizedDatum::None => None,
        MemoizedDatum::Hash(hash) => Some(PlutusData::BoundedBytes(hash.to_vec().into())),
        amaru_kernel::MemoizedDatum::Inline(plutus_data) => Some(plutus_data.as_ref().clone()),
    };

    Ok(datum)
}

fn build_script_context(
    version: PlutusVersion,
    transaction: &MintedTx,
    utxos: &BTreeMap<TransactionInput, MemoizedTransactionOutput>,
    redeemer: &Redeemer,
    network: NetworkNameAdapter,
    slot: u64,
) -> Result<(String, PlutusData)> {
    let tx_hash = transaction.transaction_body.original_hash();
    let network_name = NetworkName::from(network);

    match version {
        PlutusVersion::PlutusV1 => {
            let tx_info = TxInfoV1::new(
                &transaction.transaction_body,
                &tx_hash,
                &transaction.transaction_witness_set,
                utxos,
                network_name.into(),
                &slot.into(),
                network_name.into(),
            )?;

            let script_context: ScriptContextV1<'_> = ScriptContextV1::new(tx_info, redeemer)
                .context("Failed to construct PlutusV1 script context")?;

            Ok((
                script_context.format_readable(),
                <ScriptContextV1 as ToPlutusData<1>>::to_plutus_data(&script_context),
            ))
        }
        PlutusVersion::PlutusV2 => {
            bail!("PlutusV2 is not yet implemented")
        }
        PlutusVersion::PlutusV3 => {
            let datum = extract_datum(transaction, utxos, redeemer)?;

            let tx_info = TxInfoV3::new(
                &transaction.transaction_body,
                &tx_hash,
                &transaction.transaction_witness_set,
                utxos,
                network_name.into(),
                &slot.into(),
                network_name.into(),
            )?;

            v3::ScriptContext::new(tx_info, redeemer, datum)
                .map(|context| (context.format_readable(), context.to_plutus_data()))
                .context("Failed to construct PlutusV3 script context")
        }
    }
}

fn print_script_context(script_context: &PlutusData) {
    let cbor_bytes = to_cbor(script_context);
    let hex_string = hex::encode(&cbor_bytes);

    println!("CBOR-encoded script context:");
    println!("{}", hex_string);
    println!("\nLength: {} bytes", cbor_bytes.len());
}
