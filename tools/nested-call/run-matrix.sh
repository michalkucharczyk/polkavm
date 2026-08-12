#!/usr/bin/env bash

# Runs the nested host-call overhead matrix.
#
# Numbers of record: bare metal, `--linux`, a quiet machine, and at least three
# free cores — the host thread plus both VMs' worker threads all spin. Rounds
# are interleaved and the configuration order is reversed on even rounds, which
# is how warm-up drift was caught and killed in the wide-arith work; take
# medians across rounds.
#
# Usage: run-matrix.sh [--generic] [rounds]
#   NESTED_CALL_CPUS   taskset core list (default: 2-4)

set -euo pipefail

cd "${0%/*}/../.."

SANDBOX="--linux"
if [ "${1:-}" == "--generic" ]; then
    SANDBOX="--generic"
    shift
fi
ROUNDS="${1:-3}"
CPUS="${NESTED_CALL_CPUS:-2-4}"

TOOL="./target/release/nested-call"
BLOB_DIR="guest-programs/target/riscv64emac-unknown-none-polkavm/release"
CALLER="$BLOB_DIR/bench-nested-caller.polkavm"
MEDIATOR="$BLOB_DIR/bench-nested-mediator.polkavm"

for artifact in "$TOOL" "$CALLER" "$MEDIATOR"; do
    if [ ! -f "$artifact" ]; then
        echo "missing: $artifact" >&2
        echo "build with: cargo build -p nested-call --release && guest-programs/build-nested-call.sh" >&2
        exit 1
    fi
done

# The checksums must pass before anything is timed: this is also the
# stale-blob tell.
echo "== self-test =="
$TOOL --selftest $SANDBOX "$CALLER" "$MEDIATOR" | tail -1
echo

LABELS=(
    "one-jump payload sweep"
    "two-jump payload sweep"
    "two-jump calls sweep"
    "two-jump 96B packed"
    "two-jump 96B per-arg"
    "two-jump 128B packed"
    "two-jump 128B per-arg"
    "two-jump 128B plain instantiate"
    "two-jump 128B inner dynamic paging"
    "nesting tax"
)

CONFIGS=(
    "--mode one-jump --sweep payload --calls-per-run 32"
    "--mode two-jump --sweep payload --calls-per-run 32"
    "--mode two-jump --sweep calls --payload 128"
    "--mode two-jump --payload 96 --peeks packed --calls-per-run 32"
    "--mode two-jump --payload 96 --peeks per-arg --calls-per-run 32"
    "--mode two-jump --payload 128 --peeks packed --calls-per-run 32"
    "--mode two-jump --payload 128 --peeks per-arg --calls-per-run 32"
    "--mode two-jump --payload 128 --calls-per-run 32 --plain-instantiate"
    "--mode two-jump --payload 128 --calls-per-run 32 --inner-dynamic-paging"
    "--mode nesting-tax --compute-iterations 200000"
)

for round in $(seq 1 "$ROUNDS"); do
    echo "======== round $round ========"
    order=$(seq 0 $((${#CONFIGS[@]} - 1)))
    if [ $((round % 2)) -eq 0 ]; then
        order=$(seq $((${#CONFIGS[@]} - 1)) -1 0)
    fi

    for i in $order; do
        echo "---- ${LABELS[$i]} ----"
        # shellcheck disable=SC2086
        taskset -c "$CPUS" $TOOL ${CONFIGS[$i]} $SANDBOX "$CALLER" "$MEDIATOR"
        echo
    done
done

# The per-phase breakdown is a separate pass: instrumenting the mediation path
# perturbs it, so it never shares a run with a headline wall number.
echo "======== phase decomposition (perturbed; decomposition only) ========"
# shellcheck disable=SC2086
taskset -c "$CPUS" $TOOL --mode two-jump --payload 128 --calls-per-run 32 --phases $SANDBOX "$CALLER" "$MEDIATOR"
