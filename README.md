# Kulupu

[![Build Status](https://dev.azure.com/kulupu/kulupu/_apis/build/status/kulupu.kulupu?branchName=master)](https://dev.azure.com/kulupu/kulupu/_build/latest?definitionId=1&branchName=master)
[![Discord](https://img.shields.io/discord/586902457053872148.svg)](https://discord.gg/DZbg4rZ)

Kulupu is a pure (no pre-mine, no gadget) proof-of-work blockchain built on the
[Substrate](https://github.com/paritytech/substrate) framework, with support of
on-chain governance and online upgrades. It uses ASIC-resistant mining algorithm
of RandomX.

## Status

The network was launched in September 2019. The first hard fork, code-named
**Slag Ravine** happened in December 2019, at block 100,000. The second hard
fork, code-named **Swamp Bottom** is planned at block 320,000 on 6th May 2019.

The current code is for the **Swamp Bottom** hard fork. This is a sqaush hard
fork, meaning we export current block state and re-generate a new genesis block.
As a result, the release ready for the coming hard fork can only be published
when we reach the hard fork block. Right now if you plan to run mainnet, please
use one of the v0.2 releases.

The current Kulupu blockchain enabled Substrate's balances and governance pallet
modules. Smart contract is a planned but not yet enabled feature, due to
stability concerns.

## Run

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

Install `subkey`:

```bash
cargo install --force --git https://github.com/paritytech/substrate subkey
```

Generate an account to use as the target for mining:

```bash
subkey --sr25519 --network=kulupu generate
```

Remember the public key, and pass it to node for mining. For example:

```
cargo run --release -- --validator --author 0x7e946b7dd192307b4538d664ead95474062ac3738e04b5f3084998b76bc5122d
```

## Proof of Work Parameters

* **Algorithm**: RandomX
* **Block time**: 60 seconds
* **Issurance**: 1 KULU per second (60 KULU per block)
* No premine

## Disclaimer

This project is a side project by Wei Tang, and is not endorsed by Parity
Technologies.
