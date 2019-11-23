#!/usr/bin/env bash

set -e

__dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Install rustup and the specified rust toolchain.
curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain=stable -y

# Load cargo environment. Specifically, put cargo into PATH.
source ~/.cargo/env

rustc --version
rustup --version
cargo --version

source ${__dir}/init.sh

cargo test --release --all
