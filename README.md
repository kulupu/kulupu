# Kulupu

[![Build Status](https://dev.azure.com/kulupu/kulupu/_apis/build/status/kulupu.kulupu?branchName=master)](https://dev.azure.com/kulupu/kulupu/_build/latest?definitionId=1&branchName=master)
[![Discord](https://img.shields.io/discord/586902457053872148.svg)](https://discord.gg/DZbg4rZ)

Kulupu is a pure (no pre-mine, no gadget) proof-of-work blockchain built on the
[Substrate](https://github.com/paritytech/substrate) framework, with support of
on-chain governance and online upgrades. It uses ASIC-resistant mining algorithm
of RandomX.

## Status

The network was launched in September 2019. The first hard fork, code-named
**Slag Ravine** happened in December 2019, at era 0 block 100,000. The second
hard fork, code-named **Swamp Bottom** happened at era 0 block 320,000 on 6th
May 2019.

The current Kulupu blockchain enabled Substrate's balances and governance pallet
modules. Smart contract is a planned but not yet enabled feature, due to
stability concerns.

## Run

You can use the binary build at Kulupu's
[releases](https://github.com/kulupu/kulupu/releases) page.

## Build

### Prerequisites

Clone this repo and update the submodules:

```bash
git clone https://github.com/kulupu/kulupu
cd kulupu
git submodule update --init --recursive
```

Install Rust and required tools:

```bash
curl https://sh.rustup.rs -sSf | sh
./scripts/init.sh
```

Install necessary dependencies. On Ubuntu, run the following:

```bash
sudo apt install -y cmake pkg-config libssl-dev git gcc build-essential clang libclang-dev
```

### Full Node

```bash
cargo run --release
```

### Transition from Era 0

If you previously run Era 0 full node, please purge the current block storage
before continue.

```bash
cargo run --release -- purge-chain
```

### Mining

#### Import or generate a mining key

Kulupu implements signed mining. To mine Kulupu blocks, you have to have the
coinbase private key stored in the mining software, as a new signature is
produced for every new nonce. We refer to the private key for signed mining as
the **mining key**.

The eaiest way to get a mining key is to generate a new one using the
`generate-mining-key` command:

```bash
cargo run --release -- generate-mining-key
```

Keep your secret seed in a secure place.

Alternatively, you can also import an existing private key as the mining key,
using the `import-mining-key` command:

```bash
cargo run --release -- import-mining key "<secret seed>"
```

#### Pass author argument to node for mining

Remember either the public key or the address, and pass it to node for
mining. For example:

```
cargo run --release -- --validator --author 0x7e946b7dd192307b4538d664ead95474062ac3738e04b5f3084998b76bc5122d
```

## Proof of Work Parameters

* **Algorithm**: RandomX
* **Block time**: 60 seconds
* **Total issurance**: Governed on-chain, expected to be no more than 210
  million KLP.
* No premine

## Disclaimer

This project is a side project by Wei Tang, and is not endorsed by Parity
Technologies.
