#!/bin/sh

# Builds each elliptic-curve benchmark's host library in two variants (same
# scheme as build-crypto-native.sh):
#   libbench_<name>.so        - host portable: runs on any x86-64, and what
#                               production sp-crypto-ec-utils compiles to today
#                               (no target-cpu, no asm)
#   libbench_<name>_native.so - host native: -C target-cpu=native plus
#                               ark-ff/asm, i.e. the fastest host implementation
#                               reachable without touching arkworks - the ceiling
#                               a PVM guest has to beat
#
# Both land in target/x86_64-unknown-linux-gnu/release/, where benchtool
# auto-discovers them as "<name>" and "<name>-native".
#
# ark-ff's `asm` feature does two separable things, which is worth knowing before
# reading the columns:
#   1. Whole-field Montgomery multiply/square in hand-written asm. Gated on
#      feature="asm" && target_feature="bmi2" && target_feature="adx" && N in
#      2..=6 (montgomery_backend.rs). bmi2/adx are not in the x86-64 baseline, so
#      this half needs target-cpu=native (or +bmi2,+adx) to exist at all.
#   2. The carry-chain primitives `adc_for_add_with_carry` / `sbb_for_sub_with_borrow`
#      become `_addcarry_u64` / `_subborrow_u64` (biginteger/arithmetic.rs). Gated
#      only on feature="asm" && x86_64 - no CPU features, no limb-count limit - so
#      this half works on a fully portable build and at any field size.
#
# Measured on Zen 3 (Ryzen 9 5950X), stable 1.86.0, back-to-back:
#                        portable  +asm    native  native+asm
#   bls381-mul-g1 (6)     144.1us  135.5   140.9    133.7   -> asm -6%, cpu -2%
#   bw6761-pairing (12)   3806us   3448    3582     3330    -> asm -9%, cpu -6%
# So most of the win is (2) and is portable-safe, and bw6-761 benefits despite
# being 12 limbs. target-cpu=native on its own is the smaller half.
#
# Production sp-crypto-ec-utils does NOT enable `asm` (checked: no `asm` feature
# appears anywhere in polkadot-sdk), so the portable column is the shipped host
# and the native column is the headroom a tuned host call could claim. Worth
# noting upstream: ~6-9% of that headroom needs no CPU-specific build at all.
#
# These are single-threaded numbers. Production additionally enables
# `ark-ec/parallel` via `std`, which helps MSM (parallel over Pippenger windows)
# and multi-pair Miller loops, but cannot help `mul_*` or `final_exponentiation`
# at all - those host functions take single values. See the report.
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
    RUSTFLAGS="-C target-cpu=native" cargo $native_toolchain build --target=x86_64-unknown-linux-gnu --release --lib --features host-asm -p "$crate"
    cp "$out/lib$lib.so" "$out/lib${lib}_native.so"
done

# Rebuild without the CPU features; overwrites the portable variants.
for crate in $crates; do
    cargo $native_toolchain build --target=x86_64-unknown-linux-gnu --release --lib -p "$crate"
done
