# Nawi

**Ñawi [nya-wi]** (Quechua: _eye_) is the "eye of Amaru".

Built on top of [Amaru](https://github.com/pragma-org/amaru), Nawi is a CLI tool which helps provide useful and human readable insight about the Cardano blockchain.

Nawi is currently under development, but already provides a useful feature to produce a ScriptContext for a given transaction and Plutus langauge version.

## Quick Start

```bash
# Install from source
cargo install --path .

# Set your Blockfrost API key
export BLOCKFROST_KEY="your_api_key"

# Analyze a transaction
nawi --tx-file tx.cbor --redeemer 0
```

## Installation

```bash
cargo install --path .
```

**Requirements:**
- Rust 1.90+
- Blockfrost API key

## Configuration

Set your Blockfrost API key via environment variable:

```bash
export BLOCKFROST_KEY="your_api_key"
```

Or create a `nawi.toml` file:

```toml
key = "your_api_key"
```

## Usage

```bash
nawi [OPTIONS] --redeemer <INDEX>
```

**Options:**

```
  -t, --tx-file <FILE>              Path to transaction CBOR file
  -b, --bytes <HEX>                 Hex-encoded transaction bytes
  -r, --redeemer <INDEX>            Redeemer index to construct context for
  -n, --network <NETWORK>           Network [default: mainnet]
  -p, --plutus-version <VERSION>    Plutus version [default: PlutusV3]
  -s, --slot <SLOT>                 Slot number (defaults to chain tip)
  -o, --output <FORMAT>             Output format [default: both]
```

**Networks:** `mainnet`, `preprod`, `preview`, `testnet:<magic>`

**Plutus versions:** `PlutusV1`, `PlutusV3` (PlutusV2 coming soon)

**Output formats:** `pretty`, `cbor`, `both`

## Examples

Construct a script context from a transaction file:

```bash
nawi --tx-file spending-tx.cbor --redeemer 0
```

Analyze a specific redeemer in a multi-script transaction:

```bash
nawi --tx-file tx.cbor --redeemer 2 --network preprod
```

Export CBOR-encoded context for testing:

```bash
nawi --bytes "84a400..." --redeemer 0 --output cbor > context.hex
```

Generate PlutusV1 context:

```bash
nawi --tx-file tx.cbor --redeemer 0 --plutus-version PlutusV1
```

## Output

Nawi produces human-readable output showing the complete script context:

```
================================================================================
Script Context (Plutus V3)
================================================================================

Transaction Info:
  Transaction ID: a1b2c3d4e5f6...

  Inputs: 2 input(s)
    [0] a1b2c3d4e5f6...#0
        Address: Shelley { payment: Script(...), stake: Key(...) }
        Value:
          ADA: 5000000 lovelace
        Datum: Inline(Constr(0, [Int(42)]))
        Script: PlutusV3(...)

  Outputs: 1 output(s)
  Fee: 500000 lovelace
  Minted Assets: (none)
  Certificates: 0 certificate(s)
  ...

Redeemer:
  Purpose: Spend
  Index: 0

Script Info:
  Type: Spending
  Input: a1b2c3d4e5f6...#0
```

When `--output` is `cbor` or `both`, it also outputs the CBOR-encoded hex:

```
CBOR-encoded script context:
d8799fd8799f9fd8799fd8799fd8799f582...

Length: 1247 bytes
```

## Roadmap

- [ ] PlutusV2 support
- [ ] Direct node integration (bypass Blockfrost)
- [ ] Batch processing
- [ ] Expanded feature set!


## Contributing

Contributions welcome. For significant changes (use your best judgement), please open an issue to discuss it before opening your PR.

```bash
git clone https://github.com/yourusername/nawi.git
cd nawi
cargo build
cargo test
```


## Credits

Built with [Amaru](https://github.com/pramga-org/amaru) and [Blockfrost](https://blockfrost.io/).
