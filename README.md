# Kulupu

[![Discord](https://img.shields.io/discord/586902457053872148.svg)](https://discord.gg/DZbg4rZ)

Proof-of-work blockchain built on
[Substrate](https://github.com/paritytech/substrate).

## Overview

Kulupu is a sister project related to [Solri](https://solri.org). Kulupu's goal
is to create a working proof-of-work blockchain built using unmodified Substrate
blockchain framework. Compared with Solri, Kulupu aims to take a more practical
approach of an on-chain governed self-updating blockchain, while Solri maintains
the ideal minimalist blockchain design.

The consensus engine for Kulupu is the CPU mining algorithm RandomX. For initial
launch, the emission rate is fixed at one coin per second. This, however can be
changed using hard fork or on-chain governance in the future.

## Network Launch

The first launch attempt is on! We currently do not provide any official binary
release, so please compile the node by yourself, using the instructions below.

Launch attempt means that it's an experimental launch. We relaunch the network
when bugs are found. Otherwise, the current network becomes the mainnet.

Substrate contains a variety of features including smart contracts and
democracy. However, for initial launch of Kulupu, we plan to only enable basic
balance and transfer module. This is to keep the network focused, and reduce
risks in terms of stability and safety. Also note that initially the democracy
module is also disabled, meaning we'll be updating runtime via hard fork until
that part is enabled.

## Prerequisites

Clone this repo and update the submodules:

```bash
git clone https://github.com/kulupu/kulupu
cd kulupu
git submodule update --init --recursive
```

Install Rust:

```bash
curl https://sh.rustup.rs -sSf | sh
```

Install required tools:

```bash
./scripts/init.sh
```

## Run

### Full Node

```bash
cargo run --release
```

### Mining

Install `subkey`:

```bash
cargo install --force --git https://github.com/paritytech/substrate subkey
```

Generate an account to use as the target for mining:

```bash
subkey --sr25519 --network=16 generate
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
