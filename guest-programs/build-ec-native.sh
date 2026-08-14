#!/bin/sh

# Builds each elliptic-curve benchmark's host library in two variants (same
# scheme as build-crypto-native.sh):
#   libbench_<name>.so        - host portable: runs on any x86-64
#   libbench_<name>_native.so - host native: -C target-cpu=native, all ISA
#                               extensions of the BUILD machine enabled
#                               unconditionally
#
# Both land in target/x86_64-unknown-linux-gnu/release/, where benchtool
# auto-discovers them as "<name>" and "<name>-native".
#
# Note: target-cpu=native is expected to do little here. ark-ff's hand-written
# x86-64 assembly lives behind its `asm` feature, which is off by default and is
# NOT enabled by production sp-crypto-ec-utils either, so both variants run the
# same portable Montgomery backend.
#
# These are single-threaded numbers, which is the apples-to-apples per-core
# comparison against a single-threaded PVM guest. The production host functions
# are not: sp-crypto-ec-utils's `std` feature turns on `ark-ec/parallel` (rayon),
# so a real validator can additionally parallelize MSM across cores. Both facts
# matter for the host-call decision; see the report.
#
# WARNING: never copy *_native.so between machines - they are compiled for the
# exact CPU they were built on and will crash (SIGILL) on CPUs lacking any of
# their instruction-set extensions. Always rebuild locally.
#
# Host baselines are toolchain-sensitive - see the note in build-benchmarks.sh.
# Override: NATIVE_TOOLCHAIN="+1.86.0" ./build-ec-native.sh

set -ex
cd "${0%/*}"

native_toolchain="${NATIVE_TOOLCHAIN:-+nightly-2026-08-01}"

out=target/x86_64-unknown-linux-gnu/release

crates="bench-bls381-pairing bench-bls381-msm-g1 bench-bls381-msm-g2 \
        bench-bls381-mul-g1 bench-bander-msm bench-bander-mul \
        bench-bls377-pairing bench-bw6761-pairing"

for crate in $crates; do
    lib=$(echo "$crate" | tr - _)
    RUSTFLAGS="-C target-cpu=native" cargo $native_toolchain build --target=x86_64-unknown-linux-gnu --release --lib -p "$crate"
    cp "$out/lib$lib.so" "$out/lib${lib}_native.so"
done

# Rebuild without the CPU features; overwrites the portable variants.
for crate in $crates; do
    cargo $native_toolchain build --target=x86_64-unknown-linux-gnu --release --lib -p "$crate"
done
