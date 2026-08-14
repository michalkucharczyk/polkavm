#!/usr/bin/env bash

# Builds the sp_crypto_ec_utils (arkworks elliptic curve) benchmarks: PVM blobs
# for 64- and 32-bit plus the x86-64 host library.
#
# Deliberately separate from build-benchmarks.sh, which does NOT build these:
# arkworks pulls in enough code that forcing it on every rebuild of the rest of
# the suite is not worth it. The host portable/native pair the report needs
# comes from build-ec-native.sh.
#
# Host baselines are toolchain-sensitive - see the note in build-benchmarks.sh.
# Override: NATIVE_TOOLCHAIN="+1.86.0" ./build-ec-benchmarks.sh

set -euo pipefail

cd "${0%/*}/"

source build-common.sh
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${PWD}/target}"

native_toolchain="${NATIVE_TOOLCHAIN:-+nightly-2026-08-01}"

crates="bench-bls381-pairing bench-bls381-msm-g1 bench-bls381-msm-g2 \
        bench-bls381-mul-g1 bench-bander-msm bench-bander-mul"

build_polkavm() {
    for bits in 64 32; do
        target="$PWD/../crates/polkavm-linker/targets/legacy/riscv${bits}emac-unknown-none-polkavm.json"
        blobs="$CARGO_TARGET_DIR/riscv${bits}emac-unknown-none-polkavm/release"

        echo "> Building: '$1' (polkavm, ${bits}-bit)"
        cargo build -Z build-std=core,alloc --target "$target" -q --release --bin "$1" -p "$1"

        pushd ..
        cargo run -q -p polkatool link --run-only-if-newer "$blobs/$1" -o "$blobs/$1.polkavm"
        popd
    done
}

for crate in $crates; do
    build_polkavm "$crate"

    echo "> Building: '$crate' (native, x86_64)"
    cargo $native_toolchain build -q --target=x86_64-unknown-linux-gnu --release --lib -p "$crate"
done
