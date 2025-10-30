use amaru_kernel::{
    Address, AssetName, BigInt, Certificate, ComputeHash, DRep, Network, PlutusData, ScriptPurpose,
    ShelleyDelegationPart, ShelleyPaymentPart, StakeAddress, StakeCredential, StakePayload,
    TransactionInput,
};
use amaru_plutus::script_context::{
    CurrencySymbol, DatumOption, Mint, Redeemers, Script, ScriptContextV3, TimeRange,
    TransactionOutput, TxInfoV3, Value, Withdrawals, v3,
};
use chrono::DateTime;
use std::borrow::Cow;

pub trait ReadableFormatter {
    fn format_readable(&self) -> String;
}

impl ReadableFormatter for ScriptContextV3<'_> {
    fn format_readable(&self) -> String {
        let separator = "=".repeat(80);
        format!(
            "\n{}\nScript Context (Plutus V3)\n{}\n\nTransaction Info:\n{}\nRedeemer:\n  Purpose: {:?}\n  Index: {}\n\nScript Info:\n{}\n{}\n",
            separator,
            separator,
            self.tx_info.format_readable(),
            self.redeemer.tag,
            self.redeemer.index,
            format_script_info(self),
            separator
        )
    }
}

impl ReadableFormatter for TxInfoV3<'_> {
    fn format_readable(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("  Transaction ID: {}\n", hex::encode(&self.id)));

        output.push_str(&format!("\n  Inputs: {} input(s)\n", self.inputs.len()));
        for (i, output_ref) in self.inputs.iter().enumerate() {
            output.push_str(&format!(
                "    [{}] {}\n",
                i,
                output_ref.input.format_readable()
            ));
            for line in output_ref.output.format_readable().lines() {
                output.push_str(&format!("        {}\n", line));
            }
        }

        if !self.reference_inputs.is_empty() {
            output.push_str(&format!(
                "\n  Reference Inputs: {} input(s)\n",
                self.reference_inputs.len()
            ));
            for (i, output_ref) in self.reference_inputs.iter().enumerate() {
                output.push_str(&format!(
                    "    [{}] {}\n",
                    i,
                    output_ref.input.format_readable()
                ));
                for line in output_ref.output.format_readable().lines() {
                    output.push_str(&format!("        {}\n", line));
                }
            }
        }

        output.push_str(&format!("\n  Outputs: {} output(s)\n", self.outputs.len()));
        for (i, tx_output) in self.outputs.iter().enumerate() {
            output.push_str(&format!("    [{}]\n", i));
            for line in tx_output.format_readable().lines() {
                output.push_str(&format!("        {}\n", line));
            }
        }

        output.push_str(&format!("\n  Fee: {} lovelace\n", self.fee));

        output.push_str("\n  Minted Assets:\n");
        for line in self.mint.format_readable().lines() {
            output.push_str(&format!("    {}\n", line));
        }

        output.push_str(&format!(
            "\n  Certificates: {} certificate(s)\n",
            self.certificates.len()
        ));
        for (i, cert) in self.certificates.iter().enumerate() {
            output.push_str(&format!("    [{}] ", i));
            for (j, line) in cert.format_readable().lines().enumerate() {
                if j == 0 {
                    output.push_str(&format!("{}\n", line));
                } else {
                    output.push_str(&format!("        {}\n", line));
                }
            }
        }

        output.push_str(&format!(
            "\n  Withdrawals: {} withdrawal(s)\n",
            self.withdrawals.0.len()
        ));
        for line in self.withdrawals.format_readable().lines() {
            output.push_str(&format!("    {}\n", line));
        }

        output.push_str("\n  Validity Range:\n");
        for line in self.valid_range.format_readable().lines() {
            output.push_str(&format!("    {}\n", line));
        }

        output.push_str(&format!(
            "\n  Required Signers: {} signer(s)\n",
            self.signatories.0.len()
        ));
        for (i, sig) in self.signatories.0.iter().enumerate() {
            output.push_str(&format!("    [{}] {}\n", i, hex::encode(sig)));
        }

        output.push_str(&format!(
            "\n  Redeemers: {} redeemer(s)\n",
            self.redeemers.0.len()
        ));
        if !self.redeemers.0.is_empty() {
            for line in self.redeemers.format_readable().lines() {
                output.push_str(&format!("    {}\n", line));
            }
        }

        output
    }
}

impl ReadableFormatter for TransactionInput {
    fn format_readable(&self) -> String {
        format!(
            "{}#{}",
            hex::encode(self.transaction_id.as_ref()),
            self.index
        )
    }
}

impl ReadableFormatter for TransactionOutput<'_> {
    fn format_readable(&self) -> String {
        format!(
            "Address: {}\nValue:\n{}\nDatum: {}\nScript: {}",
            self.address.as_ref().format_readable(),
            indent_lines(&self.value.format_readable(), 2),
            self.datum.format_readable(),
            self.script.format_readable()
        )
    }
}

impl ReadableFormatter for Value<'_> {
    fn format_readable(&self) -> String {
        let mut result = String::new();

        if let Some(ada) = self.ada() {
            result.push_str(&format!("ADA: {} lovelace\n", ada));
        }

        let native_assets: Vec<_> = self
            .0
            .iter()
            .filter(|(cs, _)| !matches!(cs, CurrencySymbol::Ada))
            .collect();

        if !native_assets.is_empty() {
            result.push_str(&format!("Assets: {} policies\n", native_assets.len()));
            for (policy, asset_map) in native_assets {
                if let CurrencySymbol::Native(hash) = policy {
                    result.push_str(&format!("  Policy: {}\n", hex::encode(hash)));
                    for (asset_name, amount) in asset_map.iter() {
                        result.push_str(&format!(
                            "    {}: {}\n",
                            asset_name.format_readable(),
                            amount
                        ));
                    }
                }
            }
        }

        result.trim_end().to_string()
    }
}

impl ReadableFormatter for Mint<'_> {
    fn format_readable(&self) -> String {
        if self.0.is_empty() {
            return "(none)".to_string();
        }

        let mut result = String::new();
        result.push_str(&format!("Policies: {}\n", self.0.len()));

        for (policy_hash, asset_map) in &self.0 {
            result.push_str(&format!("  Policy: {}\n", hex::encode(policy_hash)));

            let minting: Vec<_> = asset_map.iter().filter(|(_, amt)| **amt > 0).collect();
            let burning: Vec<_> = asset_map.iter().filter(|(_, amt)| **amt < 0).collect();

            if !minting.is_empty() {
                result.push_str("    Minting:\n");
                for (asset_name, amount) in minting {
                    result.push_str(&format!(
                        "      {}: +{}\n",
                        asset_name.format_readable(),
                        amount
                    ));
                }
            }

            if !burning.is_empty() {
                result.push_str("    Burning:\n");
                for (asset_name, amount) in burning {
                    result.push_str(&format!(
                        "      {}: {}\n",
                        asset_name.format_readable(),
                        amount
                    ));
                }
            }
        }

        result.trim_end().to_string()
    }
}

impl ReadableFormatter for Address {
    fn format_readable(&self) -> String {
        match self {
            Address::Byron(_) => "Byron(...)".to_string(),
            Address::Shelley(addr) => {
                let payment = match addr.payment() {
                    ShelleyPaymentPart::Key(hash) => format!("Key({})", hex::encode(hash)),
                    ShelleyPaymentPart::Script(hash) => format!("Script({})", hex::encode(hash)),
                };
                let stake = match addr.delegation() {
                    ShelleyDelegationPart::Key(hash) => format!("Key({})", hex::encode(hash)),
                    ShelleyDelegationPart::Script(hash) => format!("Script({})", hex::encode(hash)),
                    ShelleyDelegationPart::Pointer(pointer) => format!("Pointer({:?})", pointer),
                    ShelleyDelegationPart::Null => "Null".to_string(),
                };
                format!("Shelley {{ payment: {}, stake: {} }}", payment, stake)
            }
            Address::Stake(stake_addr) => {
                let payload = match stake_addr.payload() {
                    StakePayload::Stake(hash) => format!("Key({})", hex::encode(hash)),
                    StakePayload::Script(hash) => format!("Script({})", hex::encode(hash)),
                };
                format!("Stake {{ {} }}", payload)
            }
        }
    }
}

impl ReadableFormatter for TimeRange {
    fn format_readable(&self) -> String {
        let lower = match &self.lower_bound {
            None => "∞".to_string(),
            Some(ms) => format_time_ms_local(ms.clone().into()),
        };

        let upper = match &self.upper_bound {
            None => "∞".to_string(),
            Some(ms) => format_time_ms_local(ms.clone().into()),
        };

        format!("Lower: {}\nUpper: {}", lower, upper)
    }
}

impl ReadableFormatter for DatumOption<'_> {
    fn format_readable(&self) -> String {
        match self {
            DatumOption::None => "None".to_string(),
            DatumOption::Hash(hash) => format!("Hash({})", hex::encode(hash)),
            DatumOption::Inline(data) => format!("Inline({})", data.format_readable()),
        }
    }
}

impl ReadableFormatter for Option<Script<'_>> {
    fn format_readable(&self) -> String {
        match self {
            None => "None".to_string(),
            Some(Script::Native(script)) => format!("Native({})", script.compute_hash()),
            Some(Script::PlutusV1(script)) => format!("PlutusV1({})", script.compute_hash()),
            Some(Script::PlutusV2(script)) => format!("PlutusV2({})", script.compute_hash()),
            Some(Script::PlutusV3(script)) => format!("PlutusV3({})", script.compute_hash()),
        }
    }
}

impl<'a> ReadableFormatter for Redeemers<'a, v3::ScriptPurpose<'a>> {
    fn format_readable(&self) -> String {
        if self.0.is_empty() {
            return "(none)".to_string();
        }

        let mut result = String::new();

        for (i, (purpose, redeemer)) in self.0.iter().enumerate() {
            result.push_str(&format!("[{}] {}\n", i, purpose.format_readable()));
            result.push_str(&format!("    Index: {}\n", redeemer.index));
            result.push_str(&format!("    Data: {}\n", redeemer.data.format_readable()));
            result.push_str(&format!(
                "    Ex Units: {} steps, {} mem\n",
                redeemer.ex_units.steps, redeemer.ex_units.mem
            ));

            if i < self.0.len() - 1 {
                result.push('\n');
            }
        }

        result
    }
}

impl<'a> ReadableFormatter for v3::ScriptPurpose<'a> {
    fn format_readable(&self) -> String {
        match self {
            v3::ScriptPurpose::Spending(_, _) => "Spend".to_string(),
            v3::ScriptPurpose::Minting(_) => "Mint".to_string(),
            v3::ScriptPurpose::Certifying(_, _) => "Certificate".to_string(),
            v3::ScriptPurpose::Rewarding(_) => "Reward".to_string(),
            v3::ScriptPurpose::Voting(_) => "Voting".to_string(),
            v3::ScriptPurpose::Proposing(_, _) => "Proposing".to_string(),
        }
    }
}

impl ReadableFormatter for AssetName {
    fn format_readable(&self) -> String {
        if self.is_empty() {
            return "<empty>".to_string();
        }

        String::from_utf8(self.to_vec())
            .ok()
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| hex::encode(self.to_vec()))
    }
}

impl<'a> ReadableFormatter for Cow<'a, AssetName> {
    fn format_readable(&self) -> String {
        self.as_ref().format_readable()
    }
}

fn format_script_info(ctx: &v3::ScriptContext) -> String {
    match ctx.redeemer.tag {
        ScriptPurpose::Spend => {
            if let Some(input_idx) = ctx.tx_info.inputs.get(ctx.redeemer.index as usize) {
                format!(
                    "  Type: Spending\n  Input: {}\n",
                    input_idx.input.format_readable()
                )
            } else {
                format!(
                    "  Type: Spending\n  Input: Invalid index {}\n",
                    ctx.redeemer.index
                )
            }
        }
        ScriptPurpose::Mint => format!("  Type: Minting\n  Policy Index: {}\n", ctx.redeemer.index),
        ScriptPurpose::Cert => format!(
            "  Type: Certificate\n  Certificate Index: {}\n",
            ctx.redeemer.index
        ),
        ScriptPurpose::Reward => format!(
            "  Type: Withdrawal\n  Withdrawal Index: {}\n",
            ctx.redeemer.index
        ),
        _ => "  Type: Unknown\n".to_string(),
    }
}

impl ReadableFormatter for amaru_kernel::Certificate {
    fn format_readable(&self) -> String {
        match self {
            Certificate::StakeRegistration(cred) => {
                format!("StakeRegistration({})", cred.format_readable())
            }
            Certificate::StakeDeregistration(cred) => {
                format!("StakeDeregistration({})", cred.format_readable())
            }
            Certificate::StakeDelegation(cred, pool) => {
                format!(
                    "StakeDelegation\n  Credential: {}\n  Pool: {}",
                    cred.format_readable(),
                    hex::encode(pool)
                )
            }
            Certificate::PoolRegistration {
                operator,
                vrf_keyhash,
                pledge: _,
                cost: _,
                margin: _,
                reward_account: _,
                pool_owners: _,
                relays: _,
                pool_metadata: _,
            } => {
                let mut result = String::from("PoolRegistration\n");
                result.push_str(&format!("  Operator: {}\n", hex::encode(operator)));
                result.push_str(&format!("  VRF Keyhash: {}", hex::encode(vrf_keyhash)));
                result.to_string()
            }
            Certificate::PoolRetirement(pool, epoch) => {
                format!(
                    "PoolRetirement\n  Pool: {}\n  Epoch: {}",
                    hex::encode(pool),
                    epoch
                )
            }
            Certificate::Reg(cred, coin) => {
                format!(
                    "Reg\n  Credential: {}\n  Deposit: {} lovelace",
                    cred.format_readable(),
                    coin
                )
            }
            Certificate::UnReg(cred, coin) => {
                format!(
                    "UnReg\n  Credential: {}\n  Refund: {} lovelace",
                    cred.format_readable(),
                    coin
                )
            }
            Certificate::VoteDeleg(cred, drep) => {
                format!(
                    "VoteDeleg\n  Credential: {}\n  DRep: {}",
                    cred.format_readable(),
                    drep.format_readable()
                )
            }
            Certificate::StakeVoteDeleg(cred, pool, drep) => {
                format!(
                    "StakeVoteDeleg\n  Credential: {}\n  Pool: {}\n  DRep: {}",
                    cred.format_readable(),
                    hex::encode(pool),
                    drep.format_readable()
                )
            }
            Certificate::StakeRegDeleg(cred, pool, coin) => {
                format!(
                    "StakeRegDeleg\n  Credential: {}\n  Pool: {}\n  Deposit: {} lovelace",
                    cred.format_readable(),
                    hex::encode(pool),
                    coin
                )
            }
            Certificate::VoteRegDeleg(cred, drep, coin) => {
                format!(
                    "VoteRegDeleg\n  Credential: {}\n  DRep: {}\n  Deposit: {} lovelace",
                    cred.format_readable(),
                    drep.format_readable(),
                    coin
                )
            }
            Certificate::StakeVoteRegDeleg(cred, pool, drep, coin) => {
                format!(
                    "StakeVoteRegDeleg\n  Credential: {}\n  Pool: {}\n  DRep: {}\n  Deposit: {} lovelace",
                    cred.format_readable(),
                    hex::encode(pool),
                    drep.format_readable(),
                    coin
                )
            }
            Certificate::AuthCommitteeHot(cold, hot) => {
                format!(
                    "AuthCommitteeHot\n  Cold: {}\n  Hot: {}",
                    cold.format_readable(),
                    hot.format_readable()
                )
            }
            Certificate::ResignCommitteeCold(cold, _) => {
                format!("ResignCommitteeCold\n  Cold: {}", cold.format_readable(),)
            }
            Certificate::RegDRepCert(cred, coin, _) => {
                format!(
                    "RegDRepCert\n  Credential: {}\n  Deposit: {} lovelace",
                    cred.format_readable(),
                    coin,
                )
            }
            Certificate::UnRegDRepCert(cred, coin) => {
                format!(
                    "UnRegDRepCert\n  Credential: {}\n  Refund: {} lovelace",
                    cred.format_readable(),
                    coin
                )
            }
            Certificate::UpdateDRepCert(cred, _) => {
                format!("UpdateDRepCert\n  Credential: {}", cred.format_readable())
            }
        }
    }
}

impl ReadableFormatter for StakeCredential {
    fn format_readable(&self) -> String {
        match self {
            StakeCredential::AddrKeyhash(hash) => {
                format!("Key({})", hex::encode(hash))
            }
            StakeCredential::ScriptHash(hash) => {
                format!("Script({})", hex::encode(hash))
            }
        }
    }
}

impl ReadableFormatter for DRep {
    fn format_readable(&self) -> String {
        match self {
            DRep::Key(hash) => format!("Key({})", hex::encode(hash)),
            DRep::Script(hash) => format!("Script({})", hex::encode(hash)),
            DRep::Abstain => "Abstain".to_string(),
            DRep::NoConfidence => "NoConfidence".to_string(),
        }
    }
}

impl ReadableFormatter for Withdrawals {
    fn format_readable(&self) -> String {
        if self.0.is_empty() {
            return "(none)".to_string();
        }

        self.0
            .iter()
            .enumerate()
            .map(|(i, (stake_addr, amount))| {
                format!(
                    "[{}] {}: {} lovelace",
                    i,
                    StakeAddress::from(stake_addr.clone()).format_readable(),
                    amount
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl ReadableFormatter for StakeAddress {
    fn format_readable(&self) -> String {
        let network = match self.network() {
            Network::Testnet => "Testnet",
            Network::Mainnet => "Mainnet",
            Network::Other(tag) => return format!("Network({})", tag),
        };

        let payload = match self.payload() {
            StakePayload::Stake(hash) => format!("Key({})", hex::encode(hash)),
            StakePayload::Script(hash) => format!("Script({})", hex::encode(hash)),
        };

        format!("{} {{ {} }}", network, payload)
    }
}

impl ReadableFormatter for PlutusData {
    fn format_readable(&self) -> String {
        format_plutus_data(self, 0)
    }
}

fn format_plutus_data(data: &PlutusData, indent: usize) -> String {
    let indent_str = "  ".repeat(indent);
    let next_indent_str = "  ".repeat(indent + 1);

    match data {
        PlutusData::Constr(constr) => {
            if constr.fields.is_empty() {
                format!("Constr({}, [])", constr.tag)
            } else if constr.fields.len() == 1 && is_simple(&constr.fields[0]) {
                format!(
                    "Constr({}, [{}])",
                    constr.tag,
                    format_plutus_data(&constr.fields[0], 0)
                )
            } else {
                let fields = constr
                    .fields
                    .iter()
                    .map(|f| format!("{}{}", next_indent_str, format_plutus_data(f, indent + 1)))
                    .collect::<Vec<_>>()
                    .join(",\n");
                format!("Constr({}, [\n{}\n{}])", constr.tag, fields, indent_str)
            }
        }
        PlutusData::Map(pairs) => {
            if pairs.is_empty() {
                "Map({})".to_string()
            } else if pairs.len() == 1 && is_simple(&pairs[0].0) && is_simple(&pairs[0].1) {
                format!(
                    "Map({{ {} => {} }})",
                    format_plutus_data(&pairs[0].0, 0),
                    format_plutus_data(&pairs[0].1, 0)
                )
            } else {
                let formatted_pairs = pairs
                    .iter()
                    .map(|(k, v)| {
                        format!(
                            "{}{} =>\n{}{}",
                            next_indent_str,
                            format_plutus_data(k, indent + 1),
                            next_indent_str,
                            format_plutus_data(v, indent + 1)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",\n");
                format!("Map({{\n{}\n{}}})", formatted_pairs, indent_str)
            }
        }
        PlutusData::Array(array) => {
            if array.is_empty() {
                "[]".to_string()
            } else if array.len() <= 3 && array.iter().all(is_simple) {
                let elements = array
                    .iter()
                    .map(|e| format_plutus_data(e, 0))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", elements)
            } else {
                let elements = array
                    .iter()
                    .map(|e| format!("{}{}", next_indent_str, format_plutus_data(e, indent + 1)))
                    .collect::<Vec<_>>()
                    .join(",\n");
                format!("[\n{}\n{}]", elements, indent_str)
            }
        }
        PlutusData::BigInt(int) => match int {
            BigInt::Int(i) => format!("Int({})", i.0),
            BigInt::BigUInt(bytes) => {
                format!("BigInt(+0x{})", hex::encode(bytes.to_vec()))
            }
            BigInt::BigNInt(bytes) => {
                format!("BigUInt(-0x{})", hex::encode(bytes.to_vec()))
            }
        },
        PlutusData::BoundedBytes(bytes) => {
            format!("Bytes(0x{})", hex::encode(bytes.to_vec()))
        }
    }
}

fn is_simple(data: &PlutusData) -> bool {
    match data {
        PlutusData::BigInt(_) | PlutusData::BoundedBytes(_) => true,
        PlutusData::Constr(constr) => constr.fields.is_empty(),
        PlutusData::Map(pairs) => pairs.is_empty(),
        PlutusData::Array(array) => array.is_empty(),
    }
}

fn format_time_ms_local(time_ms: u64) -> String {
    match DateTime::from_timestamp_millis(time_ms as i64) {
        Some(dt) => dt.format("%Y-%m-%d %H:%M:%S %Z").to_string(),
        None => format!("Invalid timestamp: {} ms", time_ms),
    }
}

fn indent_lines(text: &str, spaces: usize) -> String {
    let indent = " ".repeat(spaces);
    text.lines()
        .map(|line| format!("{}{}", indent, line))
        .collect::<Vec<_>>()
        .join("\n")
}
