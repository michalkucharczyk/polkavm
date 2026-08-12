#!/usr/bin/env bash

# Builds the two guest programs of the nested host-call overhead harness.
#
# 64-bit only: JAM is a 64-bit PVM, and the in-guest crypto route this
# experiment is measured against (wide-arith, 38.58 us/verify) is 64-bit too.
#
# Blobs land next to the benchmark blobs:
#   target/riscv64emac-unknown-none-polkavm/release/bench-nested-{caller,mediator}.polkavm

set -euo pipefail

cd "${0%/*}/"

source build-common.sh
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${PWD}/target}"

build_guest() {
    echo "> Building: '$1' (polkavm, 64-bit)"

    cargo build \
        -Z build-std=core,alloc \
        --target "$PWD/../crates/polkavm-linker/targets/legacy/riscv64emac-unknown-none-polkavm.json" \
        -q --release --bin "$1" -p "$1"

    pushd ..

    cargo run -q -p polkatool link \
        "$CARGO_TARGET_DIR/riscv64emac-unknown-none-polkavm/release/$1" \
        -o "$CARGO_TARGET_DIR/riscv64emac-unknown-none-polkavm/release/$1.polkavm"

    popd
}

build_guest "bench-nested-caller"
build_guest "bench-nested-mediator"
